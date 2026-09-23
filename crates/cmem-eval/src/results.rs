use crate::{
    DatasetId, DatasetKind, DegradationSummary, EmbeddingBindingRecord, MetricFamily,
    MetricSupportSummary, MetricsRecord, NumericMetricAggregate, NumericMetricSummary,
    RegistryCoverageSummary, RetrievedItem, aggregate_numeric_metrics, metric_support_summary,
    registry_coverage_summary_for,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerQuestionResult {
    pub run_id: String,
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub gold_episode_ids: Vec<String>,
    pub gold_observation_ids: Vec<String>,
    pub retrieved: Vec<RetrievedItem>,
    pub context_text: String,
    pub write_outcomes: Vec<crate::RememberOutcome>,
    pub link_outcomes: Vec<crate::LinkOutcome>,
    pub lifecycle_outcomes: Vec<crate::LifecycleMutationOutcome>,
    pub metrics: MetricsRecord,
    pub latency_ms: u64,
    pub context_char_count: usize,
    pub context_word_count: usize,
    pub context: ResultContextMetrics,
    pub retrieval_outcomes: Vec<crate::RetrieveOutcome>,
    pub composition: ResultCompositionMetrics,
    pub integrity: ResultIntegrityDetails,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunSummary {
    pub num_questions: usize,
    pub metrics: NumericMetricSummary,
    pub metric_support: MetricSupportSummary,
    pub registry_coverage: RegistryCoverageSummary,
    pub latency: LatencySummary,
    pub degradation: DegradationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunHeader {
    pub run_id: String,
    pub dataset: DatasetId,
    pub dataset_kind: DatasetKind,
    pub input_sha256: String,
    /// Scenario id (continuity) or dataset id maps to the embedding used at runtime.
    pub embedding_bindings: BTreeMap<String, EmbeddingBindingRecord>,
    pub harness_commit: String,
    pub library_commit: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// Exact TOML supplied to the runner, before defaults are applied.
    pub config: String,
    pub config_sha256: String,
    pub adapter: RunAdapterMetadata,
    pub storage_root: PathBuf,
    pub storage_root_sha256: String,
    /// Stores are retained if and only if a reason is present.
    pub retain_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencySummary {
    pub latency_ms: NumericMetricAggregate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultContextMetrics {
    pub retrieved_context_chars: usize,
    pub retrieved_context_words: usize,
    pub retrieved_context_tokens: usize,
    pub full_history_chars: Option<usize>,
    pub full_history_words: Option<usize>,
    pub full_history_tokens: Option<usize>,
    pub compression_ratio: Option<f64>,
    pub reduction_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultCompositionMetrics {
    pub total_items: usize,
    pub episodes: usize,
    pub observations: usize,
    pub derived_memories: usize,
    pub memory_threads: usize,
    pub entities: Option<usize>,
    pub items_with_rationale: usize,
    pub rationale_coverage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultIntegrityDetails {
    pub returned_items_without_external_id: usize,
    pub returned_derived_memories_without_provenance: usize,
    pub suppressed_returned_count: Option<usize>,
    pub superseded_current_returned_count: Option<usize>,
    pub provenance_coverage: Option<f64>,
    pub context_validation_pass_rate: Option<f64>,
    pub suppressed_memory_leakage_rate: Option<f64>,
    pub orphan_vector_leakage_rate: Option<f64>,
    pub superseded_current_leakage_rate: Option<f64>,
    pub cross_store_id_validation_pass_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunAdapterMetadata {
    pub adapter: String,
    pub mode: String,
}

impl RunAdapterMetadata {
    pub fn live() -> Self {
        Self {
            adapter: "real".to_string(),
            mode: "live".to_string(),
        }
    }

    pub fn bm25() -> Self {
        Self {
            adapter: "bm25".to_string(),
            mode: "lexical".to_string(),
        }
    }
}

impl Default for RunAdapterMetadata {
    fn default() -> Self {
        Self {
            adapter: "unknown".to_string(),
            mode: "unknown".to_string(),
        }
    }
}

pub fn write_jsonl(path: &Path, rows: &[PerQuestionResult]) -> Result<()> {
    let mut file = File::create_new(path).with_context(|| format!("create {}", path.display()))?;
    for row in rows {
        serde_json::to_writer(&mut file, row)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

pub fn write_summary(path: &Path, summary: &RunSummary) -> Result<()> {
    let mut file = File::create_new(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, summary)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_summary(path: &Path) -> Result<RunSummary> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("deserialize summary {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("decode summary {}", path.display()))
}

/// A run that produced no rows is a failed or missing evaluation, not an
/// empty success: `diff` refuses such artifacts, so `run` must never emit one.
pub fn reject_empty_run(rows: &[PerQuestionResult]) -> Result<()> {
    if rows.is_empty() {
        anyhow::bail!(
            "the run produced no result rows; an empty run is a failed or missing evaluation and is not written as a summary"
        );
    }
    Ok(())
}

pub fn summarize_rows(
    rows: &[PerQuestionResult],
    metric_families: &[MetricFamily],
) -> Result<RunSummary> {
    let metric_rows = rows
        .iter()
        .map(|row| row.metrics.to_json_map())
        .collect::<Vec<Map<String, Value>>>();
    let latency_values = rows
        .iter()
        .map(|row| row.latency_ms as f64)
        .collect::<Vec<_>>();
    let degradation = summarize_degradation(rows);
    Ok(RunSummary {
        num_questions: rows.len(),
        metrics: aggregate_numeric_metrics(&metric_rows),
        metric_support: metric_support_summary(&metric_rows),
        registry_coverage: registry_coverage_summary_for(&metric_rows, metric_families),
        latency: LatencySummary {
            latency_ms: NumericMetricAggregate::from_values(&latency_values),
        },
        degradation,
    })
}

pub fn summarize_degradation(rows: &[PerQuestionResult]) -> DegradationSummary {
    DegradationSummary {
        any_degradation: rows.iter().any(|row| {
            row.write_outcomes.iter().any(|record| {
                let outcome = record;
                outcome.vector_indexing_failure.is_some()
                    || outcome.stats_update_status.failure.is_some()
                    || !outcome.repair_needed.is_empty()
            }) || row
                .link_outcomes
                .iter()
                .any(|record| record.stats_update_status.failure.is_some())
                || row.lifecycle_outcomes.iter().any(|record| {
                    let outcome = record;
                    outcome.vector_maintenance_failure.is_some()
                        || outcome.stats_update_status.failure.is_some()
                })
        }),
    }
}

pub fn read_jsonl(path: &Path) -> Result<Vec<PerQuestionResult>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line)?);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_accepts_additive_controllable_policy_fields() {
        let mut header = RunHeader {
            run_id: "r".into(),
            dataset: DatasetId::new("locomo").unwrap(),
            dataset_kind: DatasetKind::LoCoMo,
            input_sha256: crate::text_sha256("input"),
            embedding_bindings: BTreeMap::new(),
            harness_commit: "test".into(),
            library_commit: "test".into(),
            generated_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
            config: String::new(),
            config_sha256: crate::text_sha256(""),
            adapter: RunAdapterMetadata::live(),
            storage_root: "stores".into(),
            storage_root_sha256: "test".into(),
            retain_reason: None,
        };
        header.embedding_bindings.insert(
            "scenario".into(),
            EmbeddingBindingRecord::Controllable {
                fixture_sha256: "fixture".into(),
                vector_size: 3,
                dimension_policy: crate::ControllableDimensionPolicy::Exact { vector_size: 1536 },
            },
        );
        let mut value = serde_json::to_value(&header).unwrap();
        value["embedding_bindings"]["scenario"]["dimension_policy"]["exact"]["future_annotation"] =
            serde_json::json!(true);
        assert_eq!(serde_json::from_value::<RunHeader>(value).unwrap(), header);
    }

    #[test]
    fn empty_run_is_rejected_before_summary() {
        assert!(reject_empty_run(&[]).is_err());
    }

    fn metrics(value: Value) -> MetricsRecord {
        MetricsRecord::try_from(value.as_object().unwrap().clone()).unwrap()
    }

    fn row(metric_values: Value) -> PerQuestionResult {
        PerQuestionResult {
            run_id: "r".into(),
            question_id: "q".into(),
            question_type: None,
            question: "question".into(),
            gold_episode_ids: vec!["s1".into()],
            gold_observation_ids: vec!["s1:turn:1".into()],
            retrieved: Vec::new(),
            context_text: String::new(),
            write_outcomes: Vec::new(),
            link_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
            metrics: metrics(metric_values),
            latency_ms: 1,
            context_char_count: 0,
            context_word_count: 0,
            context: ResultContextMetrics::default(),
            retrieval_outcomes: Vec::new(),
            composition: ResultCompositionMetrics::default(),
            integrity: ResultIntegrityDetails::default(),
        }
    }

    fn temp_path(stem: &str, extension: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cmem-{stem}-{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ))
    }

    #[test]
    fn summary_preserves_null_metric_support() {
        let row = row(serde_json::json!({
            "suppressed_items_returned": null
        }));

        let summary = summarize_rows(&[row], &[]).unwrap();

        assert!(summary.metric_support["suppressed_items_returned"].unsupported);
        assert_eq!(summary.registry_coverage.required_metrics_present, 0);
        assert!(
            !summary
                .registry_coverage
                .missing_required_metrics
                .is_empty()
        );
    }

    #[test]
    fn summary_measures_latency() {
        let mut row = row(serde_json::json!({"session_recall_any@5": 1.0}));
        row.latency_ms = 7;
        let family = crate::retrieval_metric_family("locomo", [("session", [5].as_slice())]);

        let summary = summarize_rows(&[row], &[family]).unwrap();
        assert_eq!(summary.latency.latency_ms.p95, Some(7.0));
        assert!(!summary.metrics.contains_key("retrieval_latency_ms"));
        assert_eq!(summary.registry_coverage.required_metrics_present, 1);
    }

    fn write_outcome(id: u128) -> crate::RememberOutcome {
        crate::RememberOutcome {
            persisted_object_ids: vec![uuid::Uuid::from_u128(id)],
            persisted_link_ids: Vec::new(),
            vector_indexed_object_ids: Vec::new(),
            vector_indexing_failure: None,
            stats_update_status: Default::default(),
            repair_needed: Vec::new(),
            diagnostics: Default::default(),
        }
    }

    fn lifecycle_outcome(id: u128) -> crate::LifecycleMutationOutcome {
        crate::LifecycleMutationOutcome {
            graph_mutated_object_ids: vec![character_memory::MemoryObjectRef::new(
                crate::ObjectType::Episode,
                uuid::Uuid::from_u128(id),
            )],
            graph_mutated_link_ids: Vec::new(),
            vector_maintained_object_ids: Vec::new(),
            vector_maintenance_failure: None,
            stats_update_status: Default::default(),
            trace: None,
            diagnostics: Default::default(),
        }
    }

    #[test]
    fn native_outcomes_preserve_failures_and_drive_degradation() {
        let mut result = row(serde_json::json!({}));
        result.write_outcomes.push(write_outcome(1));
        result.lifecycle_outcomes.push(lifecycle_outcome(1));
        assert!(!summarize_degradation(&[result.clone()]).any_degradation);
        result.write_outcomes[0].vector_indexing_failure =
            Some(character_memory::VectorIndexingFailure {
                unindexed_objects: Vec::new(),
                cause: character_memory::VectorIndexingCause::ZeroNormEmbedding {
                    object: character_memory::MemoryObjectRef::new(
                        crate::ObjectType::Episode,
                        uuid::Uuid::nil(),
                    ),
                },
            });
        assert!(summarize_degradation(&[result.clone()]).any_degradation);
        let path = temp_path("native-outcomes", "jsonl");
        write_jsonl(&path, &[result.clone()]).unwrap();
        let decoded = read_jsonl(&path).unwrap();
        assert_eq!(decoded[0].write_outcomes, result.write_outcomes);
        result.write_outcomes.clear();
        result.lifecycle_outcomes[0].stats_update_status =
            character_memory::StatsUpdateStatus::failed([], [], Vec::new());
        assert!(summarize_degradation(&[result.clone()]).any_degradation);
        result.lifecycle_outcomes.clear();
        let mut link = link_outcome();
        link.stats_update_status = character_memory::StatsUpdateStatus::failed([], [], Vec::new());
        result.link_outcomes.push(link);
        assert!(summarize_degradation(&[result.clone()]).any_degradation);
        let existing = std::fs::read(&path).unwrap();
        assert!(write_jsonl(&path, &[result.clone()]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), existing);
        std::fs::remove_file(&path).unwrap();
        write_jsonl(&path, &[result.clone()]).unwrap();
        assert_eq!(
            read_jsonl(&path).unwrap()[0].link_outcomes,
            result.link_outcomes
        );
        std::fs::remove_file(path).unwrap();
    }

    fn link_outcome() -> crate::LinkOutcome {
        crate::LinkOutcome {
            link: character_memory::MemoryLinkDraft::new(
                crate::ObjectType::Episode,
                uuid::Uuid::from_u128(1),
                crate::RelationType::Mentions,
                crate::ObjectType::Entity,
                uuid::Uuid::from_u128(2),
            )
            .into_domain()
            .unwrap(),
            stats_update_status: character_memory::StatsUpdateStatus::default(),
        }
    }

    #[test]
    fn read_jsonl_round_trips_results() {
        let path = temp_path("results", "jsonl");
        let result_row = row(serde_json::json!({"recall_any@1": 1.0}));
        let expected_bytes = format!("{}\n", serde_json::to_string(&result_row).unwrap());
        write_jsonl(&path, std::slice::from_ref(&result_row)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected_bytes);
        let rows = read_jsonl(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].question_id, result_row.question_id);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_jsonl_preserves_native_outcomes_in_event_order() {
        let path = temp_path("results-event-order", "jsonl");
        let mut result = row(serde_json::json!({}));
        for id in [2, 3, 1] {
            result.write_outcomes.push(write_outcome(id));
            result.link_outcomes.push(link_outcome());
            result.lifecycle_outcomes.push(lifecycle_outcome(id));
        }
        write_jsonl(&path, std::slice::from_ref(&result)).unwrap();
        assert_eq!(read_jsonl(&path).unwrap(), vec![result]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn read_jsonl_rejects_invalid_encoding_and_partial_input() {
        let path = temp_path("results-invalid", "jsonl");
        for (label, bytes) in [
            ("invalid UTF-8", vec![0xff]),
            ("truncated JSON", b"{".to_vec()),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(read_jsonl(&path).is_err(), "{label} must be refused");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn read_summary_round_trips_summary() {
        let path = temp_path("summary", "json");
        let summary =
            summarize_rows(&[row(serde_json::json!({"fixed_metric": 1.0}))], &[]).unwrap();
        write_summary(&path, &summary).unwrap();
        let existing = std::fs::read(&path).unwrap();
        assert!(write_summary(&path, &summary).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), existing);
        assert_eq!(
            serde_json::to_value(read_summary(&path).unwrap()).unwrap(),
            serde_json::to_value(summary).unwrap()
        );
        std::fs::remove_file(path).unwrap();
    }
}
