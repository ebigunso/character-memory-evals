use crate::{
    DatasetId, DatasetKind, DegradationSummary, EmbeddingBindingRecord, LifecycleOutcomeRecord,
    MetricFamily, MetricsRecord, RepairMarkerRecord, RetrievalTelemetry, RetrievedItem,
    WriteOutcomeRecord, aggregate_numeric_metrics, metric_support_summary,
    registry_coverage_summary_for,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub const RESULT_SCHEMA_VERSION: &str = "2.0.0";
const LEGACY_RESULT_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerQuestionResult {
    pub run_id: String,
    pub dataset: DatasetId,
    pub dataset_kind: DatasetKind,
    pub embedding_binding: EmbeddingBindingRecord,
    pub adapter: RunAdapterMetadata,
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub gold_episode_ids: Vec<String>,
    pub gold_observation_ids: Vec<String>,
    pub retrieved: Vec<RetrievedItem>,
    pub context_text: String,
    pub write_outcomes: Vec<WriteOutcomeRecord>,
    pub lifecycle_outcomes: Vec<LifecycleOutcomeRecord>,
    pub metrics: MetricsRecord,
    pub latency_ms: u128,
    pub context_char_count: usize,
    pub context_word_count: usize,
    pub context: ResultContextMetrics,
    pub telemetry: RetrievalTelemetry,
    pub composition: ResultCompositionMetrics,
    pub integrity: ResultIntegrityDetails,
    pub reader: ReaderResult,
}

/// Read-only shape for the Compatibility Policy's sealed-artifact exemption.
/// It is intentionally not upgraded into the 2.0 DTO: historical string
/// vocabularies and missing verdicts remain exactly as recorded.
#[derive(Debug, Serialize, Deserialize)]
pub struct LegacyPerQuestionResultV1 {
    pub run_id: String,
    pub dataset: String,
    #[serde(default)]
    pub adapter: RunAdapterMetadata,
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub gold_episode_ids: Vec<String>,
    pub gold_observation_ids: Vec<String>,
    pub retrieved: Vec<LegacyRetrievedItemV1>,
    pub metrics: Value,
    pub latency_ms: u128,
    pub context_char_count: usize,
    pub context_word_count: usize,
    #[serde(default)]
    pub context: ResultContextMetrics,
    #[serde(default)]
    pub telemetry: Value,
    #[serde(default)]
    pub composition: ResultCompositionMetrics,
    #[serde(default)]
    pub integrity: ResultIntegrityDetails,
    #[serde(default)]
    pub reader: ReaderResult,
}

/// The sealed 1.0.0 row vocabulary is deliberately kept stringly typed. It
/// must not inherit the closed 2.0.0 `ObjectType` vocabulary or its evolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LegacyRetrievedItemV1 {
    pub kind: String,
    pub internal_id: String,
    pub external_id: Option<String>,
    pub episode_external_id: Option<String>,
    pub score: Option<f64>,
    pub rank: usize,
    #[serde(default)]
    pub rationale: Vec<String>,
    pub text: Option<String>,
}

impl LegacyPerQuestionResultV1 {
    /// Reconstructs the as-built 1.0.0 context for read-only exports. New
    /// evidence persists the authoritative 2.0.0 `context_text` instead.
    /// As-built producer: 49984a5:crates/cmem-eval-runner/src/official_exports.rs:181.
    pub fn rendered_context_text(&self) -> String {
        let mut sorted = self.retrieved.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|item| item.rank);
        sorted
            .iter()
            .filter_map(|item| {
                item.text.as_ref().map(|text| {
                    format!(
                        "[{}:{} rank={}] {}",
                        item.kind,
                        item.external_id.as_deref().unwrap_or("unknown"),
                        item.rank,
                        text
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug)]
pub enum VersionedPerQuestionResult {
    V1(Box<LegacyPerQuestionResultV1>),
    V2(Box<PerQuestionResult>),
}

impl VersionedPerQuestionResult {
    pub fn as_v2(&self) -> Option<&PerQuestionResult> {
        match self {
            Self::V1(_) => None,
            Self::V2(row) => Some(row),
        }
    }

    pub fn into_v2(self) -> Result<PerQuestionResult> {
        match self {
            Self::V1(_) => anyhow::bail!(
                "operation requires result schema 2.0.0; sealed schema 1.0.0 rows are read-only"
            ),
            Self::V2(row) => Ok(*row),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub schema_version: String,
    pub run_id: String,
    pub dataset: DatasetId,
    pub dataset_kind: DatasetKind,
    pub adapter: RunAdapterMetadata,
    pub config: Value,
    pub embedding_bindings: Vec<EmbeddingBindingRecord>,
    pub num_questions: usize,
    pub metrics: Value,
    pub metric_support: Value,
    pub registry_coverage: Value,
    pub latency: Value,
    pub degradation: DegradationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryInvariantError {
    ConflictingWriteOutcome { operation_id: String },
    ConflictingLifecycleOutcome { operation_id: String },
}

impl std::fmt::Display for SummaryInvariantError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingWriteOutcome { operation_id } => write!(
                formatter,
                "conflicting write outcomes share operation_id {operation_id:?}"
            ),
            Self::ConflictingLifecycleOutcome { operation_id } => write!(
                formatter,
                "conflicting lifecycle outcomes share operation_id {operation_id:?}"
            ),
        }
    }
}

impl std::error::Error for SummaryInvariantError {}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultContextMetrics {
    #[serde(default)]
    pub retrieved_context_chars: usize,
    #[serde(default)]
    pub retrieved_context_words: usize,
    #[serde(default)]
    pub retrieved_context_tokens: usize,
    #[serde(default)]
    pub full_history_chars: Option<usize>,
    #[serde(default)]
    pub full_history_words: Option<usize>,
    #[serde(default)]
    pub full_history_tokens: Option<usize>,
    #[serde(default)]
    pub compression_ratio: Option<f64>,
    #[serde(default)]
    pub reduction_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultCompositionMetrics {
    #[serde(default)]
    pub total_items: usize,
    #[serde(default)]
    pub episodes: usize,
    #[serde(default)]
    pub observations: usize,
    #[serde(default)]
    pub derived_memories: usize,
    #[serde(default)]
    pub memory_threads: usize,
    #[serde(default)]
    pub entities: Option<usize>,
    #[serde(default)]
    pub items_with_rationale: usize,
    #[serde(default)]
    pub rationale_coverage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultIntegrityDetails {
    #[serde(default)]
    pub returned_items_without_external_id: usize,
    #[serde(default)]
    pub returned_derived_memories_without_provenance: usize,
    #[serde(default)]
    pub suppressed_or_deleted_returned_count: Option<usize>,
    #[serde(default)]
    pub superseded_current_returned_count: Option<usize>,
    #[serde(default)]
    pub provenance_coverage: Option<f64>,
    #[serde(default)]
    pub context_validation_pass_rate: Option<f64>,
    #[serde(default)]
    pub suppressed_memory_leakage_rate: Option<f64>,
    #[serde(default)]
    pub orphan_vector_leakage_rate: Option<f64>,
    #[serde(default)]
    pub superseded_current_leakage_rate: Option<f64>,
    #[serde(default)]
    pub cross_store_id_validation_pass_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ReaderResult {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub qa_score: Option<f64>,
    #[serde(default)]
    pub qa_metric_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunAdapterMetadata {
    pub adapter: String,
    pub mode: String,
    pub is_mock: bool,
}

impl RunAdapterMetadata {
    pub fn live() -> Self {
        Self {
            adapter: "real".to_string(),
            mode: "live".to_string(),
            is_mock: false,
        }
    }

    pub fn mock_smoke() -> Self {
        Self {
            adapter: "mock".to_string(),
            mode: "mock_smoke".to_string(),
            is_mock: true,
        }
    }
}

impl Default for RunAdapterMetadata {
    fn default() -> Self {
        Self {
            adapter: "unknown".to_string(),
            mode: "unknown".to_string(),
            is_mock: false,
        }
    }
}

pub fn write_jsonl(path: &Path, rows: &[PerQuestionResult]) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    for row in rows {
        serde_json::to_writer(&mut file, &versioned_row_value(row)?)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

fn versioned_row_value(row: &PerQuestionResult) -> Result<Value> {
    let mut value = serde_json::to_value(row)?;
    value
        .as_object_mut()
        .expect("PerQuestionResult always serializes as an object")
        .insert(
            "schema_version".to_string(),
            Value::String(RESULT_SCHEMA_VERSION.to_string()),
        );
    Ok(value)
}

pub fn write_summary(path: &Path, summary: &RunSummary) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, summary)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_summary(path: &Path) -> Result<RunSummary> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let value: Value = serde_json::from_reader(file)
        .with_context(|| format!("deserialize summary {}", path.display()))?;
    validate_summary_schema(&value)?;
    serde_json::from_value(value).with_context(|| format!("decode summary {}", path.display()))
}

pub fn summarize_rows(
    run_id: String,
    dataset: DatasetId,
    dataset_kind: DatasetKind,
    adapter: RunAdapterMetadata,
    config: Value,
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
    let embedding_bindings = rows
        .iter()
        .map(|row| row.embedding_binding.clone())
        .fold(BTreeMap::new(), |mut bindings, binding| {
            let key =
                serde_json::to_string(&binding).expect("EmbeddingBindingRecord always serializes");
            bindings.entry(key).or_insert(binding);
            bindings
        })
        .into_values()
        .collect();
    let degradation = summarize_degradation(rows)?;
    Ok(RunSummary {
        schema_version: RESULT_SCHEMA_VERSION.to_string(),
        run_id,
        dataset,
        dataset_kind,
        adapter,
        config,
        embedding_bindings,
        num_questions: rows.len(),
        metrics: aggregate_numeric_metrics(&metric_rows),
        metric_support: metric_support_summary(&metric_rows),
        registry_coverage: registry_coverage_summary_for(&metric_rows, metric_families),
        latency: serde_json::json!({
            "latency_ms": {
                "mean": crate::mean(&latency_values),
                "median": crate::median(&latency_values),
                "p50": crate::percentile(&latency_values, 50.0),
                "p95": crate::percentile(&latency_values, 95.0),
            }
        }),
        degradation,
    })
}

fn summarize_degradation(
    rows: &[PerQuestionResult],
) -> std::result::Result<DegradationSummary, SummaryInvariantError> {
    let mut seen_writes = BTreeMap::new();
    let mut seen_lifecycle = BTreeMap::new();
    let mut summary = DegradationSummary::default();
    for outcome in rows.iter().flat_map(|row| &row.write_outcomes) {
        if let Some(previous) = seen_writes.get(outcome.operation_id.as_str()) {
            if *previous != outcome {
                return Err(SummaryInvariantError::ConflictingWriteOutcome {
                    operation_id: outcome.operation_id.clone(),
                });
            }
            continue;
        }
        seen_writes.insert(outcome.operation_id.as_str(), outcome);
        if outcome.vector_indexing_failure.is_some()
            || outcome.stats_update_status.failure.is_some()
            || !outcome.repair_needed.is_empty()
        {
            summary.degraded_write_count += 1;
        }
        for marker in &outcome.repair_needed {
            let kind = match marker {
                RepairMarkerRecord::VectorIndex { .. } => "vector_index",
                RepairMarkerRecord::StatsUpdate { .. } => "stats_update",
            };
            *summary
                .repair_marker_counts_by_kind
                .entry(kind.to_string())
                .or_default() += 1;
        }
    }
    for outcome in rows.iter().flat_map(|row| &row.lifecycle_outcomes) {
        if let Some(previous) = seen_lifecycle.get(outcome.operation_id.as_str()) {
            if *previous != outcome {
                return Err(SummaryInvariantError::ConflictingLifecycleOutcome {
                    operation_id: outcome.operation_id.clone(),
                });
            }
            continue;
        }
        seen_lifecycle.insert(outcome.operation_id.as_str(), outcome);
        if !outcome.vector_maintenance_failures.is_empty() {
            summary.lifecycle_maintenance_failure_count += 1;
        }
    }
    Ok(summary)
}

pub fn read_jsonl(path: &Path) -> Result<Vec<VersionedPerQuestionResult>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(&line)?;
        let schema = validate_row_schema(&value)?;
        let row = match schema {
            RowSchema::LegacyV1 => {
                VersionedPerQuestionResult::V1(Box::new(serde_json::from_value(value)?))
            }
            RowSchema::CurrentV2 => {
                let mut row_value = value;
                // The writer adds schema_version as envelope metadata; the
                // strict DTO models only the result payload after dispatch.
                row_value
                    .as_object_mut()
                    .expect("validated result rows are JSON objects")
                    .remove("schema_version");
                VersionedPerQuestionResult::V2(Box::new(serde_json::from_value(row_value)?))
            }
        };
        if rows.first().is_some_and(|first| {
            !matches!(
                (first, &row),
                (
                    VersionedPerQuestionResult::V1(_),
                    VersionedPerQuestionResult::V1(_)
                ) | (
                    VersionedPerQuestionResult::V2(_),
                    VersionedPerQuestionResult::V2(_)
                )
            )
        }) {
            anyhow::bail!("mixed result schema versions are not allowed in one JSONL artifact");
        }
        rows.push(row);
    }
    Ok(rows)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowSchema {
    LegacyV1,
    CurrentV2,
}

fn validate_row_schema(value: &Value) -> Result<RowSchema> {
    match value.get("schema_version").and_then(Value::as_str) {
        // Compatibility Policy sealed-artifact exemption: only result rows and
        // continuity traces dispatch exact 1.0.0. Summary/report readers remain
        // strict 2.0.0 so the compatibility surface cannot grow accidentally.
        // Evidence: reports/v0-1-5-findings-register.md:393 and
        // runs/continuity/v0-1-5-baseline/shipped-a/results.jsonl:1.
        Some(LEGACY_RESULT_SCHEMA_VERSION) => Ok(RowSchema::LegacyV1),
        Some(RESULT_SCHEMA_VERSION) => Ok(RowSchema::CurrentV2),
        Some(version) => anyhow::bail!(
            "unsupported result schema_version {version:?}; expected {LEGACY_RESULT_SCHEMA_VERSION:?} or {RESULT_SCHEMA_VERSION:?}"
        ),
        None => {
            anyhow::bail!("missing result schema_version; expected {RESULT_SCHEMA_VERSION:?}")
        }
    }
}

fn validate_summary_schema(value: &Value) -> Result<()> {
    match value.get("schema_version").and_then(Value::as_str) {
        Some(RESULT_SCHEMA_VERSION) => Ok(()),
        Some(version) => anyhow::bail!(
            "unsupported summary schema_version {version:?}; expected {RESULT_SCHEMA_VERSION:?}"
        ),
        None => anyhow::bail!("missing summary schema_version; expected {RESULT_SCHEMA_VERSION:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dataset() -> DatasetId {
        DatasetId::new("synthetic").unwrap()
    }

    fn embedding_binding() -> EmbeddingBindingRecord {
        EmbeddingBindingRecord::Live {
            provider: crate::LiveEmbeddingProvider::Deterministic,
            model: "text-embedding-3-small".into(),
            vector_size: 1536,
        }
    }

    #[test]
    fn legacy_context_export_matches_the_as_built_identity_renderer() {
        let row = LegacyPerQuestionResultV1 {
            run_id: "legacy-run".into(),
            dataset: "locomo".into(),
            adapter: RunAdapterMetadata::default(),
            question_id: "q1".into(),
            question_type: None,
            question: "question".into(),
            gold_episode_ids: Vec::new(),
            gold_observation_ids: Vec::new(),
            retrieved: vec![
                LegacyRetrievedItemV1 {
                    kind: "observation".into(),
                    internal_id: "observation-internal".into(),
                    external_id: None,
                    episode_external_id: Some("episode-external".into()),
                    score: Some(0.7),
                    rank: 2,
                    rationale: Vec::new(),
                    text: Some("second".into()),
                },
                LegacyRetrievedItemV1 {
                    kind: "episode".into(),
                    internal_id: "episode-internal".into(),
                    external_id: Some("episode-external".into()),
                    episode_external_id: None,
                    score: Some(0.9),
                    rank: 1,
                    rationale: Vec::new(),
                    text: Some("first".into()),
                },
            ],
            metrics: serde_json::json!({}),
            latency_ms: 0,
            context_char_count: 0,
            context_word_count: 0,
            context: ResultContextMetrics::default(),
            telemetry: serde_json::json!({}),
            composition: ResultCompositionMetrics::default(),
            integrity: ResultIntegrityDetails::default(),
            reader: ReaderResult::default(),
        };

        assert_eq!(
            row.rendered_context_text(),
            "[episode:episode-external rank=1] first\n[observation:unknown rank=2] second"
        );
    }

    fn metrics(value: Value) -> MetricsRecord {
        MetricsRecord::try_from(value.as_object().unwrap().clone()).unwrap()
    }

    fn row(metric_values: Value) -> PerQuestionResult {
        PerQuestionResult {
            run_id: "r".into(),
            dataset: dataset(),
            dataset_kind: DatasetKind::Synthetic,
            embedding_binding: embedding_binding(),
            adapter: RunAdapterMetadata::mock_smoke(),
            question_id: "q".into(),
            question_type: None,
            question: "question".into(),
            gold_episode_ids: vec!["s1".into()],
            gold_observation_ids: vec!["s1:turn:1".into()],
            retrieved: Vec::new(),
            context_text: String::new(),
            write_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
            metrics: metrics(metric_values),
            latency_ms: 1,
            context_char_count: 0,
            context_word_count: 0,
            context: ResultContextMetrics::default(),
            telemetry: RetrievalTelemetry::default(),
            composition: ResultCompositionMetrics::default(),
            integrity: ResultIntegrityDetails::default(),
            reader: ReaderResult::default(),
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
    fn serializes_per_question_result() {
        let row = row(serde_json::json!({"recall_any@1": 1.0}));
        let value = versioned_row_value(&row).unwrap();
        assert_eq!(value["question_id"], "q");
        assert_eq!(value["schema_version"], RESULT_SCHEMA_VERSION);
        assert_eq!(value["adapter"]["mode"], "mock_smoke");
        assert_eq!(value["dataset_kind"], "synthetic");
        assert_eq!(value["embedding_binding"]["kind"], "live");
    }

    #[test]
    fn summary_preserves_null_metric_support() {
        let row = row(serde_json::json!({
            "suppressed_or_deleted_items_returned": null
        }));

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[row],
            &[],
        )
        .unwrap();

        assert_eq!(
            summary.metric_support["suppressed_or_deleted_items_returned"]["unsupported"],
            true
        );
        assert_eq!(summary.registry_coverage["required_metrics_present"], 0);
        assert!(
            summary.registry_coverage["missing_required_metrics"]
                .as_array()
                .is_some_and(|missing| !missing.is_empty())
        );
    }

    #[test]
    fn summary_records_schema_binding_and_separate_latency() {
        let mut row = row(serde_json::json!({"session_recall_any@5": 1.0}));
        row.latency_ms = 7;
        let family = crate::retrieval_metric_family("synthetic", [("session", [5].as_slice())]);

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({"backend": {"embedding": {"provider": "openai"}}}),
            &[row],
            &[family],
        )
        .unwrap();

        assert_eq!(summary.schema_version, RESULT_SCHEMA_VERSION);
        assert_eq!(summary.embedding_bindings, vec![embedding_binding()]);
        assert_eq!(summary.latency["latency_ms"]["p95"], 7.0);
        assert!(summary.metrics.get("retrieval_latency_ms").is_none());
        assert_eq!(summary.registry_coverage["required_metrics_present"], 1);
    }

    #[test]
    fn summary_deduplicates_identical_cumulative_outcomes_across_rows() {
        let object = crate::ObjectRefRecord {
            object_type: crate::ObjectType::Episode,
            internal_id: "episode-1".into(),
            external_id: Some("source-1".into()),
        };
        let mut write = WriteOutcomeRecord::clean(
            "write-operation-1",
            crate::WriteOperationKind::ExplicitCommit,
        );
        write.repair_needed.push(RepairMarkerRecord::StatsUpdate {
            object_internal_ids: vec![object.internal_id.clone()],
            causes: vec![crate::StatsUpdateCauseRecord::HealthCheck {
                error: crate::RetrievalStatsStoreErrorRecord::Sqlite {
                    detail: "unavailable".into(),
                },
            }],
        });
        let mut lifecycle = LifecycleOutcomeRecord::clean(
            "lifecycle-operation-1",
            crate::LifecycleOperationKind::Forget,
        );
        lifecycle
            .vector_maintenance_failures
            .push(crate::VectorMaintenanceFailureItemRecord {
                operation: crate::VectorMaintenanceOperation::Delete,
                objects: vec![object],
                cause: crate::VectorIndexingCauseRecord::CardinalityMismatch {
                    expected: 1,
                    actual: 0,
                },
            });
        let mut first = row(serde_json::json!({}));
        first.write_outcomes.push(write.clone());
        first.lifecycle_outcomes.push(lifecycle.clone());
        let mut second = row(serde_json::json!({}));
        second.question_id = "q2".into();
        second.write_outcomes.push(write);
        second.lifecycle_outcomes.push(lifecycle);

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[first, second],
            &[],
        )
        .unwrap();

        assert_eq!(summary.degradation.degraded_write_count, 1);
        assert_eq!(summary.degradation.lifecycle_maintenance_failure_count, 1);
        assert_eq!(
            summary.degradation.repair_marker_counts_by_kind["stats_update"],
            1
        );
    }

    #[test]
    fn summary_rejects_conflicting_outcomes_with_the_same_operation_id() {
        let write = WriteOutcomeRecord::clean(
            "shared-write-operation",
            crate::WriteOperationKind::ExplicitCommit,
        );
        let mut conflicting_write = write.clone();
        conflicting_write
            .persisted_link_internal_ids
            .push("unexpected-link".into());
        let mut first = row(serde_json::json!({}));
        first.write_outcomes.push(write);
        let mut second = row(serde_json::json!({}));
        second.question_id = "q2".into();
        second.write_outcomes.push(conflicting_write);
        let write_error = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[first, second],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            write_error.downcast_ref::<SummaryInvariantError>(),
            Some(&SummaryInvariantError::ConflictingWriteOutcome {
                operation_id: "shared-write-operation".to_string(),
            })
        );

        let lifecycle = LifecycleOutcomeRecord::clean(
            "shared-lifecycle-operation",
            crate::LifecycleOperationKind::Correct,
        );
        let mut conflicting_lifecycle = lifecycle.clone();
        conflicting_lifecycle
            .graph_mutated_link_internal_ids
            .push("unexpected-link".into());
        let mut first = row(serde_json::json!({}));
        first.lifecycle_outcomes.push(lifecycle);
        let mut second = row(serde_json::json!({}));
        second.question_id = "q2".into();
        second.lifecycle_outcomes.push(conflicting_lifecycle);
        let lifecycle_error = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[first, second],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            lifecycle_error.downcast_ref::<SummaryInvariantError>(),
            Some(&SummaryInvariantError::ConflictingLifecycleOutcome {
                operation_id: "shared-lifecycle-operation".to_string(),
            })
        );
    }

    #[test]
    fn row_schema_dispatch_is_exact_and_rejects_missing_or_unsupported_versions() {
        assert_eq!(
            validate_row_schema(&serde_json::json!({
                "schema_version": RESULT_SCHEMA_VERSION
            }))
            .unwrap(),
            RowSchema::CurrentV2
        );
        assert_eq!(
            validate_row_schema(&serde_json::json!({
                "schema_version": LEGACY_RESULT_SCHEMA_VERSION
            }))
            .unwrap(),
            RowSchema::LegacyV1
        );
        let missing = validate_row_schema(&serde_json::json!({})).unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("missing result schema_version")
        );
        let unsupported =
            validate_row_schema(&serde_json::json!({"schema_version": "0.9.0"})).unwrap_err();
        assert!(
            unsupported
                .to_string()
                .contains("unsupported result schema_version")
        );
    }

    #[test]
    fn read_jsonl_round_trips_v2_and_rejects_mixed_versions() {
        let path = temp_path("results", "jsonl");
        let result_row = row(serde_json::json!({"recall_any@1": 1.0}));
        write_jsonl(&path, &[result_row]).unwrap();
        let rows = read_jsonl(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], VersionedPerQuestionResult::V2(_)));

        let mut current = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        let mut legacy = current.clone();
        legacy["schema_version"] = Value::String(LEGACY_RESULT_SCHEMA_VERSION.into());
        let mixed = format!(
            "{}\n{}\n",
            serde_json::to_string(&current).unwrap(),
            serde_json::to_string(&legacy).unwrap()
        );
        std::fs::write(&path, mixed).unwrap();
        assert!(
            read_jsonl(&path)
                .unwrap_err()
                .to_string()
                .contains("mixed result schema versions")
        );
        current["schema_version"] = Value::String(RESULT_SCHEMA_VERSION.into());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn v2_result_reader_rejects_shape_drift_while_sealed_v1_stays_tolerant() {
        let path = temp_path("results-shape-drift", "jsonl");

        let mut current = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        current["unexpected_v2_field"] = Value::Bool(true);
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&current).unwrap()),
        )
        .unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(error.contains("unknown field"), "{error}");

        let mut current = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        current.as_object_mut().unwrap().remove("context");
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&current).unwrap()),
        )
        .unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(error.contains("missing field `context`"), "{error}");

        let mut legacy = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        legacy["schema_version"] = Value::String(LEGACY_RESULT_SCHEMA_VERSION.into());
        legacy["sealed_legacy_extra"] = Value::Bool(true);
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&legacy).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            read_jsonl(&path).unwrap().as_slice(),
            [VersionedPerQuestionResult::V1(_)]
        ));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn read_jsonl_rejects_invalid_encoding_partial_input_and_bad_schema() {
        let path = temp_path("results-invalid", "jsonl");
        for (bytes, expected) in [
            (vec![0xff], "stream did not contain valid UTF-8"),
            (b"{".to_vec(), "EOF while parsing"),
            (
                br#"{"schema_version":"9.0.0"}"#.to_vec(),
                "unsupported result schema_version",
            ),
            (
                br#"{"run_id":"r"}"#.to_vec(),
                "missing result schema_version",
            ),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let error = read_jsonl(&path).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn read_summary_round_trips_v2_and_rejects_invalid_inputs() {
        let path = temp_path("summary-schema", "json");
        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[row(serde_json::json!({}))],
            &[],
        )
        .unwrap();
        write_summary(&path, &summary).unwrap();
        assert_eq!(
            read_summary(&path).unwrap().schema_version,
            RESULT_SCHEMA_VERSION
        );

        for (bytes, expected) in [
            (vec![0xff], "deserialize summary"),
            (b"{".to_vec(), "deserialize summary"),
            (
                serde_json::to_vec(&serde_json::json!({})).unwrap(),
                "missing summary schema_version",
            ),
            (
                serde_json::to_vec(&serde_json::json!({"schema_version": "0.9.0"})).unwrap(),
                "unsupported summary schema_version",
            ),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let error = read_summary(&path).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }

        let mut drifted = serde_json::to_value(&summary).unwrap();
        drifted["unexpected_v2_field"] = Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&drifted).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("unknown field"), "{error}");

        let mut incomplete = serde_json::to_value(&summary).unwrap();
        incomplete
            .as_object_mut()
            .unwrap()
            .remove("embedding_bindings");
        std::fs::write(&path, serde_json::to_vec(&incomplete).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(
            error.contains("missing field `embedding_bindings`"),
            "{error}"
        );

        std::fs::remove_file(path).unwrap();
    }
}
