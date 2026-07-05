use anyhow::{Context, Result, anyhow, bail};
use cmem_eval_core::{
    DerivedMemoryInput, EntityInput, GraphEnrichmentInput, GraphSnapshotInput, MemoryLinkInput,
    MemoryThreadInput,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

const FORBIDDEN_KEYS: &[&str] = &[
    "answer",
    "answers",
    "answer_session_ids",
    "evidence",
    "evidence_dialog_ids",
    "has_answer",
    "gold",
    "gold_label",
    "gold_labels",
    "label",
    "labels",
];

pub fn load_enrichment_path(path: &Path) -> Result<HashMap<String, GraphEnrichmentInput>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut by_namespace: HashMap<String, GraphEnrichmentInput> = HashMap::new();
    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse enrichment JSONL line {}", line_idx + 1))?;
        reject_forbidden_keys(&value)
            .with_context(|| format!("validate enrichment JSONL line {}", line_idx + 1))?;
        let input: GraphEnrichmentInput = serde_json::from_value(value)
            .with_context(|| format!("decode enrichment JSONL line {}", line_idx + 1))?;
        validate_enrichment(&input)
            .with_context(|| format!("validate enrichment JSONL line {}", line_idx + 1))?;
        let entry = by_namespace
            .entry(input.namespace.clone())
            .or_insert_with(|| GraphEnrichmentInput {
                namespace: input.namespace.clone(),
                ..GraphEnrichmentInput::default()
            });
        entry.entities.extend(input.entities);
        entry.threads.extend(input.threads);
        entry.derived_memories.extend(input.derived_memories);
        entry.links.extend(input.links);
    }
    for input in by_namespace.values() {
        validate_enrichment(input)
            .with_context(|| format!("validate merged enrichment namespace {}", input.namespace))?;
    }
    Ok(by_namespace)
}

pub fn load_snapshot_path(path: &Path) -> Result<HashMap<String, GraphSnapshotInput>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut by_dataset_item: HashMap<String, GraphSnapshotInput> = HashMap::new();
    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse snapshot JSONL line {}", line_idx + 1))?;
        reject_forbidden_keys(&value)
            .with_context(|| format!("validate snapshot JSONL line {}", line_idx + 1))?;
        let snapshot: GraphSnapshotInput = serde_json::from_value(value)
            .with_context(|| format!("decode snapshot JSONL line {}", line_idx + 1))?;
        validate_snapshot(&snapshot)
            .with_context(|| format!("validate snapshot JSONL line {}", line_idx + 1))?;
        if by_dataset_item
            .insert(snapshot.dataset_item_id.clone(), snapshot)
            .is_some()
        {
            bail!(
                "duplicate snapshot dataset_item_id in {} line {}",
                path.display(),
                line_idx + 1
            );
        }
    }
    Ok(by_dataset_item)
}

pub fn validate_snapshot(snapshot: &GraphSnapshotInput) -> Result<()> {
    require_non_empty("snapshot_id", &snapshot.snapshot_id)?;
    require_non_empty("snapshot.namespace", &snapshot.namespace)?;
    require_non_empty("snapshot.dataset_item_id", &snapshot.dataset_item_id)?;
    require_non_empty("snapshot.cutoff.type", &snapshot.cutoff.cutoff_type)?;
    require_non_empty("snapshot.cutoff.value", &snapshot.cutoff.value)?;
    if snapshot.graph.namespace != snapshot.namespace {
        bail!(
            "snapshot {} graph namespace {} does not match snapshot namespace {}",
            snapshot.snapshot_id,
            snapshot.graph.namespace,
            snapshot.namespace
        );
    }
    validate_enrichment(&snapshot.graph)
}

pub fn validate_enrichment(input: &GraphEnrichmentInput) -> Result<()> {
    if input.namespace.trim().is_empty() {
        bail!("enrichment namespace must not be empty");
    }
    let mut ids = HashSet::new();
    for entity in &input.entities {
        insert_id(&mut ids, "entity", &entity.external_id)?;
        require_non_empty("entity.name", &entity.name)?;
    }
    for thread in &input.threads {
        insert_id(&mut ids, "memory_thread", &thread.external_id)?;
        require_non_empty("thread.title", &thread.title)?;
        require_non_empty("thread.summary", &thread.summary)?;
        validate_score("thread.salience_score", thread.salience_score)?;
    }
    for memory in &input.derived_memories {
        insert_id(&mut ids, "derived_memory", &memory.external_id)?;
        require_non_empty("derived_memory.text", &memory.text)?;
        validate_score("derived_memory.confidence", memory.confidence)?;
        validate_score("derived_memory.salience_score", memory.salience_score)?;
        if memory.source_episode_external_ids.is_empty()
            && memory.source_observation_external_ids.is_empty()
        {
            bail!(
                "derived memory {} must include source episode or observation external IDs",
                memory.external_id
            );
        }
        reject_forbidden_keys(&memory.metadata)
            .with_context(|| format!("validate metadata for {}", memory.external_id))?;
    }
    for link in &input.links {
        insert_id(&mut ids, "memory_link", &link.external_id)?;
        validate_score("link.confidence", link.confidence)?;
        require_non_empty("link.from.external_id", &link.from.external_id)?;
        require_non_empty("link.to.external_id", &link.to.external_id)?;
    }
    Ok(())
}

pub fn merge_enrichment(
    base: &mut GraphEnrichmentInput,
    addition: GraphEnrichmentInput,
) -> Result<()> {
    if base.namespace != addition.namespace {
        bail!(
            "cannot merge enrichment namespace {} into {}",
            addition.namespace,
            base.namespace
        );
    }
    base.entities.extend(addition.entities);
    base.threads.extend(addition.threads);
    base.derived_memories.extend(addition.derived_memories);
    base.links.extend(addition.links);
    validate_enrichment(base)
}

fn insert_id(ids: &mut HashSet<String>, kind: &str, external_id: &str) -> Result<()> {
    require_non_empty("external_id", external_id)?;
    let key = format!("{kind}\0{external_id}");
    if !ids.insert(key) {
        bail!("duplicate enrichment external_id {external_id} for {kind}");
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(())
}

fn validate_score(field: &str, value: f32) -> Result<()> {
    if !(0.0..=1.0).contains(&value) || !value.is_finite() {
        bail!("{field} must be finite and in 0.0..=1.0");
    }
    Ok(())
}

fn reject_forbidden_keys(value: &Value) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let normalized = key.to_ascii_lowercase();
                if FORBIDDEN_KEYS
                    .iter()
                    .any(|forbidden| normalized == *forbidden || normalized.starts_with("gold_"))
                {
                    return Err(anyhow!(
                        "enrichment contains forbidden gold-label key {key}"
                    ));
                }
                reject_forbidden_keys(value)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_forbidden_keys(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn empty_namespace(namespace: String) -> GraphEnrichmentInput {
    GraphEnrichmentInput {
        namespace,
        entities: Vec::<EntityInput>::new(),
        threads: Vec::<MemoryThreadInput>::new(),
        derived_memories: Vec::<DerivedMemoryInput>::new(),
        links: Vec::<MemoryLinkInput>::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_derived_memory_without_provenance() {
        let input = GraphEnrichmentInput {
            namespace: "n".into(),
            derived_memories: vec![DerivedMemoryInput {
                external_id: "dm1".into(),
                derived_type: "reflection".into(),
                text: "A memory".into(),
                source_episode_external_ids: vec![],
                source_observation_external_ids: vec![],
                thread_external_ids: vec![],
                entity_external_ids: vec![],
                confidence: 1.0,
                salience_score: 0.5,
                stability: "medium".into(),
                is_current: true,
                supersedes_external_ids: vec![],
                metadata: serde_json::json!({}),
            }],
            ..empty_namespace("n".into())
        };

        assert!(
            validate_enrichment(&input)
                .unwrap_err()
                .to_string()
                .contains("source episode or observation")
        );
    }

    #[test]
    fn rejects_forbidden_gold_keys_recursively() {
        let err = reject_forbidden_keys(&serde_json::json!({
            "namespace": "n",
            "derived_memories": [{
                "external_id": "dm1",
                "derived_type": "reflection",
                "text": "leaked",
                "source_episode_external_ids": ["s1"],
                "metadata": { "answer": "secret" }
            }]
        }))
        .unwrap_err()
        .to_string();
        assert!(err.contains("forbidden"));
    }

    #[test]
    fn loads_and_groups_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enrichment.jsonl");
        std::fs::write(
            &path,
            r#"{"namespace":"n","derived_memories":[{"external_id":"dm1","derived_type":"reflection","text":"User prefers concise answers.","source_episode_external_ids":["s1"]}]}"#,
        )
        .unwrap();

        let loaded = load_enrichment_path(&path).unwrap();
        assert_eq!(loaded["n"].derived_memories[0].external_id, "dm1");
    }
}
