use anyhow::{Result, bail};
use clap::Args;
use cmem_eval_core::{PerQuestionResult, RetrievedItem, read_jsonl};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(crate) struct DiffArgs {
    pub(crate) run_a: PathBuf,
    pub(crate) run_b: PathBuf,
}

pub(crate) fn run(args: DiffArgs) -> Result<()> {
    let report = compare(
        normalize(read_jsonl(&args.run_a)?),
        normalize(read_jsonl(&args.run_b)?),
    )?;
    print!("{}", report.render());
    Ok(())
}

fn normalize(mut rows: Vec<PerQuestionResult>) -> Vec<PerQuestionResult> {
    for row in &mut rows {
        row.run_id = "__RUN__".to_string();
        row.latency_ms = 0;
    }
    rows
}

#[derive(Debug, Default)]
struct DiffReport {
    details: Vec<String>,
    queries: usize,
    differing_queries: usize,
    missing_from_a: usize,
    missing_from_b: usize,
    identity_changes: usize,
    rank_changes: usize,
    metric_changes: usize,
    degradation_changes: usize,
}

impl DiffReport {
    fn render(&self) -> String {
        let mut output = self.details.join("\n");
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!(
            "summary: queries={} differing={} missing_from_a={} missing_from_b={} identity_changes={} rank_changes={} metric_changes={} degradation_changes={}\n",
            self.queries,
            self.differing_queries,
            self.missing_from_a,
            self.missing_from_b,
            self.identity_changes,
            self.rank_changes,
            self.metric_changes,
            self.degradation_changes,
        ));
        output
    }
}

fn compare(run_a: Vec<PerQuestionResult>, run_b: Vec<PerQuestionResult>) -> Result<DiffReport> {
    let run_a = index(run_a, "run A")?;
    let run_b = index(run_b, "run B")?;
    let query_ids = run_a
        .keys()
        .chain(run_b.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut report = DiffReport {
        queries: query_ids.len(),
        ..Default::default()
    };

    for query_id in query_ids {
        let Some(a) = run_a.get(&query_id) else {
            report
                .details
                .push(format!("query {query_id}: missing from run A"));
            report.differing_queries += 1;
            report.missing_from_a += 1;
            continue;
        };
        let Some(b) = run_b.get(&query_id) else {
            report
                .details
                .push(format!("query {query_id}: missing from run B"));
            report.differing_queries += 1;
            report.missing_from_b += 1;
            continue;
        };

        let mut changes = Vec::new();
        let identities_a = identities(&a.retrieved);
        let identities_b = identities(&b.retrieved);
        if identities_a != identities_b {
            changes.push(format!("identities: a={identities_a:?} b={identities_b:?}"));
            report.identity_changes += 1;
        }
        let ranks_a = ranks(&a.retrieved);
        let ranks_b = ranks(&b.retrieved);
        if ranks_a != ranks_b {
            changes.push(format!("ranks: a={ranks_a:?} b={ranks_b:?}"));
            report.rank_changes += 1;
        }
        if a.metrics != b.metrics {
            changes.push(format!(
                "metrics: a={} b={}",
                serde_json::to_string(&a.metrics)?,
                serde_json::to_string(&b.metrics)?
            ));
            report.metric_changes += 1;
        }
        let degradation_a = degraded(a);
        let degradation_b = degraded(b);
        if degradation_a != degradation_b {
            changes.push(format!("degradation: a={degradation_a} b={degradation_b}"));
            report.degradation_changes += 1;
        }
        if !changes.is_empty() {
            report
                .details
                .push(format!("query {query_id}: {}", changes.join("; ")));
            report.differing_queries += 1;
        }
    }
    Ok(report)
}

fn index(rows: Vec<PerQuestionResult>, label: &str) -> Result<BTreeMap<String, PerQuestionResult>> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let question_id = row.question_id.clone();
        if indexed.insert(question_id.clone(), row).is_some() {
            bail!("{label} contains duplicate question_id {question_id:?}");
        }
    }
    Ok(indexed)
}

fn identities(items: &[RetrievedItem]) -> Vec<String> {
    let mut identities = items.iter().map(identity).collect::<Vec<_>>();
    identities.sort();
    identities
}

fn ranks(items: &[RetrievedItem]) -> Vec<(String, usize)> {
    let mut ranks = items
        .iter()
        .map(|item| (identity(item), item.rank))
        .collect::<Vec<_>>();
    ranks.sort();
    ranks
}

fn identity(item: &RetrievedItem) -> String {
    serde_json::to_string(&(
        &item.kind,
        &item.internal_id,
        &item.external_id,
        &item.episode_external_id,
    ))
    .expect("retrieved item identity always serializes")
}

fn degraded(row: &PerQuestionResult) -> bool {
    row.write_outcomes.iter().any(|outcome| {
        outcome.vector_indexing_failure.is_some()
            || outcome.stats_update_status.failure.is_some()
            || !outcome.repair_needed.is_empty()
    }) || row.lifecycle_outcomes.iter().any(|outcome| {
        !outcome.vector_maintenance_failures.is_empty()
            || outcome.stats_update_status.failure.is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmem_eval_core::{
        DatasetId, DatasetKind, EmbeddingBindingRecord, LiveEmbeddingProvider, MetricsRecord,
        ObjectType, ResultCompositionMetrics, ResultContextMetrics, ResultIntegrityDetails,
        RetrievalTelemetry, RunAdapterMetadata,
    };
    use serde_json::json;

    fn row(run_id: &str, latency_ms: u128, items: Vec<RetrievedItem>) -> PerQuestionResult {
        PerQuestionResult {
            schema_version: cmem_eval_core::RESULT_SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            dataset: DatasetId::new("locomo").unwrap(),
            dataset_kind: DatasetKind::LoCoMo,
            embedding_binding: EmbeddingBindingRecord::Live {
                provider: LiveEmbeddingProvider::Deterministic,
                model: "test".to_string(),
                vector_size: 2,
            },
            adapter: RunAdapterMetadata::mock_smoke(),
            question_id: "q1".to_string(),
            question_type: None,
            question: "question".to_string(),
            gold_episode_ids: Vec::new(),
            gold_observation_ids: Vec::new(),
            retrieved: items,
            context_text: String::new(),
            write_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
            metrics: MetricsRecord::try_from(
                json!({"recall_any@1": 1.0}).as_object().unwrap().clone(),
            )
            .unwrap(),
            latency_ms,
            context_char_count: 0,
            context_word_count: 0,
            context: ResultContextMetrics::default(),
            telemetry: RetrievalTelemetry::default(),
            composition: ResultCompositionMetrics::default(),
            integrity: ResultIntegrityDetails::default(),
        }
    }

    fn item(id: &str, rank: usize) -> RetrievedItem {
        RetrievedItem {
            kind: ObjectType::Episode,
            internal_id: id.to_string(),
            external_id: Some(id.to_string()),
            episode_external_id: Some(id.to_string()),
            score: Some(1.0),
            rank,
            rationale: Vec::new(),
            text: Some(id.to_string()),
        }
    }

    #[test]
    fn run_identity_and_latency_are_the_only_normalized_fields() {
        let report = compare(
            normalize(vec![row("a", 1, vec![item("one", 1)])]),
            normalize(vec![row("b", 99, vec![item("one", 1)])]),
        )
        .unwrap();
        assert_eq!(report.differing_queries, 0);
        assert_eq!(
            report.render(),
            "summary: queries=1 differing=0 missing_from_a=0 missing_from_b=0 identity_changes=0 rank_changes=0 metric_changes=0 degradation_changes=0\n"
        );
    }

    #[test]
    fn rank_only_change_is_reported_once() {
        let report = compare(
            normalize(vec![row("a", 1, vec![item("one", 1), item("two", 2)])]),
            normalize(vec![row("b", 2, vec![item("one", 2), item("two", 1)])]),
        )
        .unwrap();
        assert_eq!(report.differing_queries, 1);
        assert_eq!(report.identity_changes, 0);
        assert_eq!(report.rank_changes, 1);
        assert_eq!(report.metric_changes, 0);
        assert_eq!(report.degradation_changes, 0);
        assert!(report.render().contains("query q1: ranks:"));
    }
}
