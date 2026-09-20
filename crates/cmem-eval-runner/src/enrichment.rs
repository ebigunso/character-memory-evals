use anyhow::{Context, Result, anyhow, bail};
use cmem_eval::{
    DerivedMemoryInput, EntityInput, GraphEnrichmentInput, GraphSnapshotInput, MemoryLinkInput,
    MemoryThreadInput,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum EnrichmentError {
    MissingManifest {
        path: PathBuf,
    },
    WrongWorkflow {
        expected: &'static str,
        actual: Option<String>,
    },
    WrongDataset {
        expected: &'static str,
        actual: Option<String>,
    },
    ArtifactHashMismatch {
        expected: Option<String>,
        actual: String,
    },
    MissingDatasetHash,
    DatasetHashMismatch {
        expected: String,
        actual: String,
    },
    DuplicateExternalId {
        kind: &'static str,
        external_id: String,
    },
}

impl std::fmt::Display for EnrichmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingManifest { path } => {
                write!(f, "missing snapshot manifest {}", path.display())
            }
            Self::WrongWorkflow { expected, actual } => {
                write!(f, "snapshot workflow must be {expected}, got {actual:?}")
            }
            Self::WrongDataset { expected, actual } => {
                write!(f, "snapshot dataset must be {expected}, got {actual:?}")
            }
            Self::ArtifactHashMismatch { expected, actual } => write!(
                f,
                "snapshot artifact hash mismatch: expected {expected:?}, got {actual}"
            ),
            Self::MissingDatasetHash => write!(f, "v2 snapshot manifest requires dataset.sha256"),
            Self::DatasetHashMismatch { expected, actual } => write!(
                f,
                "snapshot dataset hash mismatch: expected {expected}, got {actual}"
            ),
            Self::DuplicateExternalId { kind, external_id } => write!(
                f,
                "duplicate enrichment external_id {external_id} for {kind}"
            ),
        }
    }
}

impl std::error::Error for EnrichmentError {}

fn admitted_snapshot_file(path: &Path, dataset: &str, input_sha256: &str) -> Result<File> {
    let (expected_workflow, expected_dataset) = match dataset {
        "locomo" => ("deterministic-exact-source-replay-v1", "locomo"),
        "longmemeval_s" => ("deterministic-exact-source-replay-v2", "longmemeval-s"),
        _ => bail!("unsupported snapshot dataset {dataset:?}"),
    };
    let mut name = path
        .file_stem()
        .context("snapshot path needs a file stem")?
        .to_os_string();
    name.push("_manifest.json");
    let manifest_path = path.with_file_name(name);
    let manifest_file = File::open(&manifest_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow::Error::from(EnrichmentError::MissingManifest {
                path: manifest_path.clone(),
            })
        } else {
            anyhow::Error::from(error).context(format!("open {}", manifest_path.display()))
        }
    })?;
    let manifest: Value = serde_json::from_reader(BufReader::new(manifest_file))
        .with_context(|| format!("parse {}", manifest_path.display()))?;
    let workflow = manifest["workflow_id"].as_str();
    if workflow != Some(expected_workflow) {
        return Err(EnrichmentError::WrongWorkflow {
            expected: expected_workflow,
            actual: workflow.map(ToOwned::to_owned),
        }
        .into());
    }
    let manifest_dataset = manifest["dataset"]
        .as_str()
        .or_else(|| manifest["dataset"]["name"].as_str());
    if manifest_dataset != Some(expected_dataset) {
        return Err(EnrichmentError::WrongDataset {
            expected: expected_dataset,
            actual: manifest_dataset.map(ToOwned::to_owned),
        }
        .into());
    }
    match manifest["dataset"].get("sha256") {
        Some(expected)
            if expected
                .as_str()
                .is_none_or(|hash| !hash.eq_ignore_ascii_case(input_sha256)) =>
        {
            return Err(EnrichmentError::DatasetHashMismatch {
                expected: expected
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| expected.to_string()),
                actual: input_sha256.to_string(),
            }
            .into());
        }
        None if dataset == "longmemeval_s" => {
            return Err(EnrichmentError::MissingDatasetHash.into());
        }
        _ => {}
    }
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hash = Sha256::new();
    std::io::copy(&mut BufReader::new(&mut file), &mut hash)?;
    let actual = format!("{:x}", hash.finalize());
    let expected = manifest["artifact"]["sha256"].as_str();
    if expected.is_none_or(|expected| !expected.eq_ignore_ascii_case(&actual)) {
        return Err(EnrichmentError::ArtifactHashMismatch {
            expected: expected.map(ToOwned::to_owned),
            actual,
        }
        .into());
    }
    file.rewind()?;
    Ok(file)
}

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

pub fn load_snapshot_path(
    path: &Path,
    dataset: &str,
    input_sha256: &str,
) -> Result<HashMap<String, GraphSnapshotInput>> {
    let file = admitted_snapshot_file(path, dataset, input_sha256)?;
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

fn insert_id(ids: &mut HashSet<String>, kind: &'static str, external_id: &str) -> Result<()> {
    require_non_empty("external_id", external_id)?;
    let key = format!("{kind}\0{external_id}");
    if !ids.insert(key) {
        return Err(EnrichmentError::DuplicateExternalId {
            kind,
            external_id: external_id.to_string(),
        }
        .into());
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
    use cmem_eval::{DerivedType, Stability};

    #[test]
    fn rejects_derived_memory_without_provenance() {
        let input = GraphEnrichmentInput {
            namespace: "n".into(),
            derived_memories: vec![DerivedMemoryInput {
                created_at: None,
                external_id: "dm1".into(),
                derived_type: DerivedType::Reflection,
                text: "A memory".into(),
                source_episode_external_ids: vec![],
                source_observation_external_ids: vec![],
                thread_external_ids: vec![],
                entity_external_ids: vec![],
                confidence: 1.0,
                salience_score: 0.5,
                stability: Stability::Medium,
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
        let rows =
            [("n", "dm1"), ("other", "dm2"), ("n", "dm3")].map(|(namespace, external_id)| {
                serde_json::json!({"namespace": namespace, "derived_memories": [{
                    "external_id": external_id, "derived_type": "reflection",
                    "text": "User prefers concise answers.", "source_episode_external_ids": ["s1"]
                }]})
                .to_string()
            });
        std::fs::write(&path, rows.join("\n")).unwrap();
        let loaded = load_enrichment_path(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        for (namespace, expected) in [("n", vec!["dm1", "dm3"]), ("other", vec!["dm2"])] {
            assert_eq!(loaded[namespace].namespace, namespace);
            assert_eq!(
                loaded[namespace]
                    .derived_memories
                    .iter()
                    .map(|memory| memory.external_id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    const SNAPSHOT_DATASETS: [(&str, &str, &str); 2] = [
        ("locomo", "locomo", "deterministic-exact-source-replay-v1"),
        (
            "longmemeval_s",
            "longmemeval-s",
            "deterministic-exact-source-replay-v2",
        ),
    ];
    const SNAPSHOT_SOURCE: &str = "source dataset bytes";

    fn snapshot_fixture(name: &str, workflow: &str) -> (tempfile::TempDir, PathBuf, Value) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("snapshot.jsonl");
        let artifact = serde_json::json!({
            "snapshot_id": "snapshot", "namespace": "test", "dataset_item_id": "item",
            "cutoff": {"type": "session", "value": "s1"}, "graph": {"namespace": "test"}
        })
        .to_string();
        std::fs::write(&path, &artifact).unwrap();
        let manifest = serde_json::json!({
            "workflow_id": workflow, "artifact": {"sha256": cmem_eval::text_sha256(&artifact)},
            "dataset": {"name": name, "sha256": cmem_eval::text_sha256(SNAPSHOT_SOURCE)}
        });
        (directory, path, manifest)
    }

    fn load_with_manifest(
        path: &Path,
        dataset: &str,
        manifest: &Value,
    ) -> Result<HashMap<String, GraphSnapshotInput>> {
        std::fs::write(
            path.with_file_name("snapshot_manifest.json"),
            serde_json::to_vec(manifest).unwrap(),
        )
        .unwrap();
        load_snapshot_path(path, dataset, &cmem_eval::text_sha256(SNAPSHOT_SOURCE))
    }

    #[test]
    fn snapshot_loader_rejects_missing_manifest() {
        for (dataset, name, workflow) in SNAPSHOT_DATASETS {
            let (_directory, path, _) = snapshot_fixture(name, workflow);
            let error =
                load_snapshot_path(&path, dataset, &cmem_eval::text_sha256(SNAPSHOT_SOURCE))
                    .unwrap_err();
            assert_eq!(
                error.downcast_ref::<EnrichmentError>(),
                Some(&EnrichmentError::MissingManifest {
                    path: path.with_file_name("snapshot_manifest.json"),
                })
            );
        }
    }

    #[test]
    fn snapshot_loader_rejects_wrong_workflow_before_dataset_identity() {
        for (dataset, name, workflow) in SNAPSHOT_DATASETS {
            let (_directory, path, mut manifest) = snapshot_fixture(name, workflow);
            manifest["workflow_id"] = serde_json::json!("wrong");
            manifest["dataset"]["name"] = serde_json::json!("wrong");
            let error = load_with_manifest(&path, dataset, &manifest).unwrap_err();
            assert_eq!(
                error.downcast_ref::<EnrichmentError>(),
                Some(&EnrichmentError::WrongWorkflow {
                    expected: workflow,
                    actual: Some("wrong".into()),
                })
            );
        }
    }

    #[test]
    fn snapshot_loader_rejects_wrong_dataset_identity_before_hashes() {
        for (dataset, name, workflow) in SNAPSHOT_DATASETS {
            let other = if dataset == "locomo" {
                "longmemeval-s"
            } else {
                "locomo"
            };
            for (value, actual) in [
                (Some(serde_json::json!(other)), Some(other)),
                (
                    Some(
                        serde_json::json!({"name": other, "sha256": cmem_eval::text_sha256(SNAPSHOT_SOURCE)}),
                    ),
                    Some(other),
                ),
                (None, None),
                (Some(serde_json::json!({"sha256": "stale"})), None),
                (
                    Some(
                        serde_json::json!({"name": null, "sha256": cmem_eval::text_sha256(SNAPSHOT_SOURCE)}),
                    ),
                    None,
                ),
                (
                    Some(
                        serde_json::json!({"name": 42, "sha256": cmem_eval::text_sha256(SNAPSHOT_SOURCE)}),
                    ),
                    None,
                ),
                (
                    Some(
                        serde_json::json!({"name": "", "sha256": cmem_eval::text_sha256(SNAPSHOT_SOURCE)}),
                    ),
                    Some(""),
                ),
            ] {
                let (_directory, path, mut manifest) = snapshot_fixture(name, workflow);
                if let Some(value) = value {
                    manifest["dataset"] = value;
                } else {
                    manifest.as_object_mut().unwrap().remove("dataset");
                }
                let error = load_with_manifest(&path, dataset, &manifest).unwrap_err();
                assert_eq!(
                    error.downcast_ref::<EnrichmentError>(),
                    Some(&EnrichmentError::WrongDataset {
                        expected: name,
                        actual: actual.map(str::to_string),
                    })
                );
            }
        }
    }

    #[test]
    fn snapshot_loader_rejects_artifact_hash_mismatch() {
        for (dataset, name, workflow) in SNAPSHOT_DATASETS {
            let (_directory, path, mut manifest) = snapshot_fixture(name, workflow);
            let actual = manifest["artifact"]["sha256"].as_str().unwrap().to_string();
            manifest["artifact"]["sha256"] = serde_json::json!("stale");
            let error = load_with_manifest(&path, dataset, &manifest).unwrap_err();
            assert_eq!(
                error.downcast_ref::<EnrichmentError>(),
                Some(&EnrichmentError::ArtifactHashMismatch {
                    expected: Some("stale".into()),
                    actual,
                })
            );
        }
    }

    #[test]
    fn snapshot_loader_requires_dataset_hash_for_v2() {
        let (_directory, path, mut manifest) =
            snapshot_fixture("longmemeval-s", "deterministic-exact-source-replay-v2");
        manifest["dataset"] = serde_json::json!("longmemeval-s");
        let error = load_with_manifest(&path, "longmemeval_s", &manifest).unwrap_err();
        assert_eq!(
            error.downcast_ref::<EnrichmentError>(),
            Some(&EnrichmentError::MissingDatasetHash)
        );
    }

    #[test]
    fn snapshot_loader_rejects_dataset_hash_mismatch() {
        for (dataset, name, workflow) in SNAPSHOT_DATASETS {
            for (value, expected) in [
                (serde_json::json!("different dataset"), "different dataset"),
                (Value::Null, "null"),
                (serde_json::json!(42), "42"),
            ] {
                let (_directory, path, mut manifest) = snapshot_fixture(name, workflow);
                manifest["dataset"]["sha256"] = value;
                let error = load_with_manifest(&path, dataset, &manifest).unwrap_err();
                assert_eq!(
                    error.downcast_ref::<EnrichmentError>(),
                    Some(&EnrichmentError::DatasetHashMismatch {
                        expected: expected.into(),
                        actual: cmem_eval::text_sha256(SNAPSHOT_SOURCE),
                    })
                );
            }
        }
    }

    #[test]
    fn snapshot_loader_accepts_valid_manifests_and_legacy_dataset_name() {
        for (dataset, name, workflow) in SNAPSHOT_DATASETS {
            let (_directory, path, mut manifest) = snapshot_fixture(name, workflow);
            let loaded = load_with_manifest(&path, dataset, &manifest).unwrap();
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded["item"].snapshot_id, "snapshot");
            if dataset == "locomo" {
                manifest["dataset"] = serde_json::json!(name);
                let loaded = load_with_manifest(&path, dataset, &manifest).unwrap();
                assert_eq!(loaded.len(), 1);
                assert_eq!(loaded["item"].snapshot_id, "snapshot");
            }
        }
    }
}
