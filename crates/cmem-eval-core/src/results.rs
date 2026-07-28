use crate::{
    DatasetId, DatasetKind, DegradationSummary, EmbeddingBindingRecord, LifecycleOutcomeRecord,
    MetricFamily, MetricSupportSummary, MetricsRecord, NumericMetricAggregate,
    NumericMetricSummary, RegistryCoverageSummary, RepairMarkerRecord, RetrievalTelemetry,
    RetrievedItem, WriteOutcomeRecord, aggregate_numeric_metrics, metric_support_summary,
    registry_coverage_summary_for,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub const RESULT_SCHEMA_VERSION: &str = "2.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerQuestionResult {
    pub schema_version: String,
    pub run_id: String,
    pub dataset: DatasetId,
    pub dataset_kind: DatasetKind,
    pub embedding_binding: EmbeddingBindingRecord,
    pub adapter: RunAdapterMetadata,
    pub question_id: String,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub schema_version: String,
    pub run_id: String,
    pub dataset: DatasetId,
    pub dataset_kind: DatasetKind,
    pub adapter: RunAdapterMetadata,
    /// Dynamic-by-design snapshot whose shape is owned by the selected runner
    /// and backend configuration rather than the result schema.
    pub config: Value,
    pub embedding_bindings: Vec<EmbeddingBindingRecord>,
    pub num_questions: usize,
    pub metrics: NumericMetricSummary,
    pub metric_support: MetricSupportSummary,
    pub registry_coverage: RegistryCoverageSummary,
    pub latency: LatencySummary,
    pub degradation: DegradationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LatencySummary {
    pub latency_ms: NumericMetricAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryInvariantError {
    ConflictingWriteOutcome {
        operation_id: String,
        attempt_index: u32,
    },
    ConflictingLifecycleOutcome {
        operation_id: String,
        attempt_index: u32,
    },
    NonContiguousWriteAttempts {
        operation_id: String,
        expected_attempt_index: u32,
        actual_attempt_index: u32,
    },
    NonContiguousLifecycleAttempts {
        operation_id: String,
        expected_attempt_index: u32,
        actual_attempt_index: u32,
    },
}

impl std::fmt::Display for SummaryInvariantError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingWriteOutcome {
                operation_id,
                attempt_index,
            } => write!(
                formatter,
                "conflicting write outcomes share identity ({operation_id:?}, {attempt_index})"
            ),
            Self::ConflictingLifecycleOutcome {
                operation_id,
                attempt_index,
            } => write!(
                formatter,
                "conflicting lifecycle outcomes share identity ({operation_id:?}, {attempt_index})"
            ),
            Self::NonContiguousWriteAttempts {
                operation_id,
                expected_attempt_index,
                actual_attempt_index,
            } => write!(
                formatter,
                "write outcome operation {operation_id:?} has non-contiguous attempts: expected {expected_attempt_index}, found {actual_attempt_index}"
            ),
            Self::NonContiguousLifecycleAttempts {
                operation_id,
                expected_attempt_index,
                actual_attempt_index,
            } => write!(
                formatter,
                "lifecycle outcome operation {operation_id:?} has non-contiguous attempts: expected {expected_attempt_index}, found {actual_attempt_index}"
            ),
        }
    }
}

impl std::error::Error for SummaryInvariantError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryIdentityError {
    EmptyRows,
    SchemaVersionMismatch {
        row_index: usize,
        expected: String,
        found: String,
    },
    RunIdMismatch {
        row_index: usize,
        expected: String,
        found: String,
    },
    DatasetMismatch {
        row_index: usize,
        expected: DatasetId,
        found: DatasetId,
    },
    DatasetKindMismatch {
        row_index: usize,
        expected: DatasetKind,
        found: DatasetKind,
    },
    AdapterMismatch {
        row_index: usize,
        expected: RunAdapterMetadata,
        found: RunAdapterMetadata,
    },
}

impl std::fmt::Display for SummaryIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRows => formatter.write_str("cannot summarize empty result rows"),
            Self::SchemaVersionMismatch {
                row_index,
                expected,
                found,
            } => write!(
                formatter,
                "summary row schema_version mismatch at index {row_index}: expected {expected:?}, found {found:?}"
            ),
            Self::RunIdMismatch {
                row_index,
                expected,
                found,
            } => write!(
                formatter,
                "summary row run_id mismatch at index {row_index}: expected {expected:?}, found {found:?}"
            ),
            Self::DatasetMismatch {
                row_index,
                expected,
                found,
            } => write!(
                formatter,
                "summary row dataset mismatch at index {row_index}: expected {expected:?}, found {found:?}"
            ),
            Self::DatasetKindMismatch {
                row_index,
                expected,
                found,
            } => write!(
                formatter,
                "summary row dataset_kind mismatch at index {row_index}: expected {expected:?}, found {found:?}"
            ),
            Self::AdapterMismatch {
                row_index,
                expected,
                found,
            } => write!(
                formatter,
                "summary row adapter mismatch at index {row_index}: expected {expected:?}, found {found:?}"
            ),
        }
    }
}

impl std::error::Error for SummaryIdentityError {}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResultContextMetrics {
    pub retrieved_context_chars: usize,
    pub retrieved_context_words: usize,
    pub retrieved_context_tokens: usize,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub full_history_chars: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub full_history_words: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub full_history_tokens: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub compression_ratio: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub reduction_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResultCompositionMetrics {
    pub total_items: usize,
    pub episodes: usize,
    pub observations: usize,
    pub derived_memories: usize,
    pub memory_threads: usize,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub entities: Option<usize>,
    pub items_with_rationale: usize,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub rationale_coverage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResultIntegrityDetails {
    pub returned_items_without_external_id: usize,
    pub returned_derived_memories_without_provenance: usize,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub suppressed_or_deleted_returned_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub superseded_current_returned_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub provenance_coverage: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub context_validation_pass_rate: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub suppressed_memory_leakage_rate: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub orphan_vector_leakage_rate: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub superseded_current_leakage_rate: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub cross_store_id_validation_pass_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReaderResult {
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub model: Option<String>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub answer: Option<String>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub qa_score: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub qa_metric_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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
    for (index, row) in rows.iter().enumerate() {
        if row.schema_version != RESULT_SCHEMA_VERSION {
            anyhow::bail!(
                "result row at index {index} has schema_version {:?}; expected {:?}",
                row.schema_version,
                RESULT_SCHEMA_VERSION
            );
        }
    }
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    for row in rows {
        serde_json::to_writer(&mut file, &versioned_row_value(row)?)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

fn versioned_row_value(row: &PerQuestionResult) -> serde_json::Result<Value> {
    let mut canonical = row.clone();
    canonical.write_outcomes.sort_by(|left, right| {
        (&left.operation_id, left.attempt_index).cmp(&(&right.operation_id, right.attempt_index))
    });
    canonical.lifecycle_outcomes.sort_by(|left, right| {
        (&left.operation_id, left.attempt_index).cmp(&(&right.operation_id, right.attempt_index))
    });
    serde_json::to_value(canonical)
}

pub fn write_summary(path: &Path, summary: &RunSummary) -> Result<()> {
    if summary.schema_version != RESULT_SCHEMA_VERSION {
        anyhow::bail!(
            "summary has schema_version {:?}; expected {:?}",
            summary.schema_version,
            RESULT_SCHEMA_VERSION
        );
    }
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, summary)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_summary(path: &Path) -> Result<RunSummary> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("deserialize summary {}", path.display()))?;
    let schema_version = crate::serde_contract::schema_version_from_str(&raw)
        .with_context(|| format!("deserialize summary {}", path.display()))?;
    validate_summary_schema(schema_version.as_deref())?;
    crate::serde_contract::reject_duplicate_json_keys(&raw)
        .with_context(|| format!("decode summary {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("decode summary {}", path.display()))
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
    validate_summary_row_identity(&run_id, &dataset, dataset_kind, &adapter, rows)?;
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
        latency: LatencySummary {
            latency_ms: NumericMetricAggregate::from_values(&latency_values),
        },
        degradation,
    })
}

pub fn validate_summary_row_identity(
    run_id: &str,
    dataset: &DatasetId,
    dataset_kind: DatasetKind,
    adapter: &RunAdapterMetadata,
    rows: &[PerQuestionResult],
) -> std::result::Result<(), SummaryIdentityError> {
    if rows.is_empty() {
        return Err(SummaryIdentityError::EmptyRows);
    }
    for (index, row) in rows.iter().enumerate() {
        if row.schema_version != RESULT_SCHEMA_VERSION {
            return Err(SummaryIdentityError::SchemaVersionMismatch {
                row_index: index,
                expected: RESULT_SCHEMA_VERSION.to_string(),
                found: row.schema_version.clone(),
            });
        }
        if row.run_id != run_id {
            return Err(SummaryIdentityError::RunIdMismatch {
                row_index: index,
                expected: run_id.to_string(),
                found: row.run_id.clone(),
            });
        }
        if row.dataset != *dataset {
            return Err(SummaryIdentityError::DatasetMismatch {
                row_index: index,
                expected: dataset.clone(),
                found: row.dataset.clone(),
            });
        }
        if row.dataset_kind != dataset_kind {
            return Err(SummaryIdentityError::DatasetKindMismatch {
                row_index: index,
                expected: dataset_kind,
                found: row.dataset_kind,
            });
        }
        if row.adapter != *adapter {
            return Err(SummaryIdentityError::AdapterMismatch {
                row_index: index,
                expected: adapter.clone(),
                found: row.adapter.clone(),
            });
        }
    }
    Ok(())
}

fn summarize_degradation(
    rows: &[PerQuestionResult],
) -> std::result::Result<DegradationSummary, SummaryInvariantError> {
    let mut seen_writes = BTreeMap::new();
    let mut seen_lifecycle = BTreeMap::new();
    let mut write_attempts = BTreeMap::<&str, BTreeSet<u32>>::new();
    let mut lifecycle_attempts = BTreeMap::<&str, BTreeSet<u32>>::new();
    let mut degraded_writes = BTreeSet::new();
    let mut lifecycle_failures = BTreeSet::new();
    let mut repair_markers_by_kind = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut summary = DegradationSummary::default();
    for outcome in rows.iter().flat_map(|row| &row.write_outcomes) {
        let identity = (outcome.operation_id.as_str(), outcome.attempt_index);
        if let Some(previous) = seen_writes.get(&identity) {
            if *previous != outcome {
                return Err(SummaryInvariantError::ConflictingWriteOutcome {
                    operation_id: outcome.operation_id.clone(),
                    attempt_index: outcome.attempt_index,
                });
            }
            continue;
        }
        seen_writes.insert(identity, outcome);
        write_attempts
            .entry(outcome.operation_id.as_str())
            .or_default()
            .insert(outcome.attempt_index);
        if outcome.attempt_index > 0 {
            summary.repair_attempt_count += 1;
        }
        if outcome.vector_indexing_failure.is_some()
            || outcome.stats_update_status.failure.is_some()
            || !outcome.repair_needed.is_empty()
        {
            degraded_writes.insert(outcome.operation_id.as_str());
        }
        for marker in &outcome.repair_needed {
            let kind = match marker {
                RepairMarkerRecord::VectorIndex { .. } => "vector_index",
                RepairMarkerRecord::StatsUpdate { .. } => "stats_update",
            };
            repair_markers_by_kind
                .entry(kind)
                .or_default()
                .insert(outcome.operation_id.as_str());
        }
    }
    summary.degraded_write_count = degraded_writes.len();
    summary.repair_marker_counts_by_kind = repair_markers_by_kind
        .into_iter()
        .map(|(kind, operations)| (kind.to_string(), operations.len()))
        .collect();
    for (operation_id, attempts) in write_attempts {
        for (expected, actual) in (0_u32..).zip(attempts) {
            if expected != actual {
                return Err(SummaryInvariantError::NonContiguousWriteAttempts {
                    operation_id: operation_id.to_string(),
                    expected_attempt_index: expected,
                    actual_attempt_index: actual,
                });
            }
        }
    }
    for outcome in rows.iter().flat_map(|row| &row.lifecycle_outcomes) {
        let identity = (outcome.operation_id.as_str(), outcome.attempt_index);
        if let Some(previous) = seen_lifecycle.get(&identity) {
            if *previous != outcome {
                return Err(SummaryInvariantError::ConflictingLifecycleOutcome {
                    operation_id: outcome.operation_id.clone(),
                    attempt_index: outcome.attempt_index,
                });
            }
            continue;
        }
        seen_lifecycle.insert(identity, outcome);
        lifecycle_attempts
            .entry(outcome.operation_id.as_str())
            .or_default()
            .insert(outcome.attempt_index);
        if outcome.attempt_index > 0 {
            summary.repair_attempt_count += 1;
        }
        if !outcome.vector_maintenance_failures.is_empty()
            || outcome.stats_update_status.failure.is_some()
        {
            lifecycle_failures.insert(outcome.operation_id.as_str());
        }
    }
    summary.lifecycle_maintenance_failure_count = lifecycle_failures.len();
    for (operation_id, attempts) in lifecycle_attempts {
        for (expected, actual) in (0_u32..).zip(attempts) {
            if expected != actual {
                return Err(SummaryInvariantError::NonContiguousLifecycleAttempts {
                    operation_id: operation_id.to_string(),
                    expected_attempt_index: expected,
                    actual_attempt_index: actual,
                });
            }
        }
    }
    Ok(summary)
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
        let schema_version = crate::serde_contract::schema_version_from_str(&line)?;
        validate_row_schema(schema_version.as_deref())?;
        crate::serde_contract::reject_duplicate_json_keys(&line)?;
        rows.push(serde_json::from_str(&line)?);
    }
    Ok(rows)
}

fn validate_row_schema(schema_version: Option<&str>) -> Result<()> {
    match schema_version {
        Some(RESULT_SCHEMA_VERSION) => Ok(()),
        Some(version) => anyhow::bail!(
            "unsupported result schema_version {version:?}; expected {RESULT_SCHEMA_VERSION:?}"
        ),
        None => {
            anyhow::bail!("missing result schema_version; expected {RESULT_SCHEMA_VERSION:?}")
        }
    }
}

fn validate_summary_schema(schema_version: Option<&str>) -> Result<()> {
    match schema_version {
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

    fn metrics(value: Value) -> MetricsRecord {
        MetricsRecord::try_from(value.as_object().unwrap().clone()).unwrap()
    }

    fn row(metric_values: Value) -> PerQuestionResult {
        PerQuestionResult {
            schema_version: RESULT_SCHEMA_VERSION.to_string(),
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

        assert!(summary.metric_support["suppressed_or_deleted_items_returned"].unsupported);
        assert_eq!(summary.registry_coverage.required_metrics_present, 0);
        assert!(
            !summary
                .registry_coverage
                .missing_required_metrics
                .is_empty()
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
        assert_eq!(summary.latency.latency_ms.p95, Some(7.0));
        assert!(!summary.metrics.contains_key("retrieval_latency_ms"));
        assert_eq!(summary.registry_coverage.required_metrics_present, 1);
    }

    #[test]
    fn summary_rejects_empty_or_mismatched_row_identity() {
        let error = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<SummaryIdentityError>(),
            Some(&SummaryIdentityError::EmptyRows)
        );

        let mut wrong_schema = row(serde_json::json!({}));
        wrong_schema.schema_version = "9.9.9".to_string();
        let mut wrong_run = row(serde_json::json!({}));
        wrong_run.run_id = "other-run".to_string();
        let mut wrong_dataset = row(serde_json::json!({}));
        wrong_dataset.dataset = DatasetId::new("other-dataset").unwrap();
        let mut wrong_kind = row(serde_json::json!({}));
        wrong_kind.dataset_kind = DatasetKind::Continuity;
        let mut wrong_adapter = row(serde_json::json!({}));
        wrong_adapter.adapter = RunAdapterMetadata::live();

        for (candidate, expected) in [
            (
                wrong_schema,
                SummaryIdentityError::SchemaVersionMismatch {
                    row_index: 0,
                    expected: RESULT_SCHEMA_VERSION.to_string(),
                    found: "9.9.9".to_string(),
                },
            ),
            (
                wrong_run,
                SummaryIdentityError::RunIdMismatch {
                    row_index: 0,
                    expected: "r".to_string(),
                    found: "other-run".to_string(),
                },
            ),
            (
                wrong_dataset,
                SummaryIdentityError::DatasetMismatch {
                    row_index: 0,
                    expected: dataset(),
                    found: DatasetId::new("other-dataset").unwrap(),
                },
            ),
            (
                wrong_kind,
                SummaryIdentityError::DatasetKindMismatch {
                    row_index: 0,
                    expected: DatasetKind::Synthetic,
                    found: DatasetKind::Continuity,
                },
            ),
            (
                wrong_adapter,
                SummaryIdentityError::AdapterMismatch {
                    row_index: 0,
                    expected: RunAdapterMetadata::mock_smoke(),
                    found: RunAdapterMetadata::live(),
                },
            ),
        ] {
            let error = summarize_rows(
                "r".into(),
                dataset(),
                DatasetKind::Synthetic,
                RunAdapterMetadata::mock_smoke(),
                serde_json::json!({}),
                &[candidate],
                &[],
            )
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<SummaryIdentityError>(),
                Some(&expected)
            );
        }

        let first = row(serde_json::json!({}));
        let mut second = row(serde_json::json!({}));
        second.question_id = "q2".to_string();
        second.adapter = RunAdapterMetadata::live();
        let error = summarize_rows(
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
            error.downcast_ref::<SummaryIdentityError>(),
            Some(&SummaryIdentityError::AdapterMismatch {
                row_index: 1,
                expected: RunAdapterMetadata::mock_smoke(),
                found: RunAdapterMetadata::live(),
            })
        );
    }

    #[test]
    fn summary_counts_degradation_once_per_operation_across_cumulative_retry_rows() {
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
        lifecycle.stats_update_status = crate::StatsUpdateStatusRecord {
            updated_object_internal_ids: Vec::new(),
            failure: Some(crate::StatsUpdateFailureRecord {
                failed_object_internal_ids: vec!["episode-1".into()],
                causes: vec![crate::StatsUpdateCauseRecord::HealthCheck {
                    error: crate::RetrievalStatsStoreErrorRecord::Sqlite {
                        detail: "unavailable".into(),
                    },
                }],
            }),
        };
        let mut first = row(serde_json::json!({}));
        first.write_outcomes.push(write.clone());
        first.lifecycle_outcomes.push(lifecycle.clone());
        let mut second = row(serde_json::json!({}));
        second.question_id = "q2".into();
        second.write_outcomes.push(write.clone());
        let mut write_retry = write;
        write_retry.attempt_index = 1;
        second.write_outcomes.push(write_retry);
        second.lifecycle_outcomes.push(lifecycle.clone());
        let mut lifecycle_retry = lifecycle;
        lifecycle_retry.attempt_index = 1;
        second.lifecycle_outcomes.push(lifecycle_retry);

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
        assert_eq!(summary.degradation.repair_attempt_count, 2);
    }

    #[test]
    fn summary_admits_converged_correction_retry_as_a_distinct_attempt() {
        let original = crate::ObjectRefRecord {
            object_type: crate::ObjectType::DerivedMemory,
            internal_id: "derived-original".into(),
            external_id: Some("delivery-v1".into()),
        };
        let replacement = crate::ObjectRefRecord {
            object_type: crate::ObjectType::DerivedMemory,
            internal_id: "derived-replacement".into(),
            external_id: Some("delivery-v2".into()),
        };
        let mut retry = LifecycleOutcomeRecord::clean(
            "correction-operation",
            crate::LifecycleOperationKind::Correct,
        );
        retry.requested_targets.push(original.clone());
        retry.vector_maintained_objects = vec![original, replacement];
        retry.stats_update_status.updated_object_internal_ids =
            vec!["derived-original".into(), "derived-replacement".into()];

        assert!(retry.graph_mutated_objects.is_empty());
        assert!(retry.graph_mutated_link_internal_ids.is_empty());
        assert!(retry.superseded.is_empty());
        assert_eq!(retry.vector_maintained_objects.len(), 2);
        assert_eq!(
            retry.stats_update_status.updated_object_internal_ids.len(),
            2
        );
        assert!(retry.stats_update_status.failure.is_none());

        let mut first = row(serde_json::json!({}));
        first.lifecycle_outcomes.push(retry.clone());
        let mut second = row(serde_json::json!({}));
        second.question_id = "q2".into();
        retry.attempt_index = 1;
        second.lifecycle_outcomes.push(retry);

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

        assert_eq!(summary.degradation.degraded_write_count, 0);
        assert_eq!(summary.degradation.lifecycle_maintenance_failure_count, 0);
        assert!(summary.degradation.repair_marker_counts_by_kind.is_empty());
        assert_eq!(summary.degradation.repair_attempt_count, 1);
    }

    #[test]
    fn summary_counts_lifecycle_stats_projection_failure_without_vector_failure() {
        let mut lifecycle = LifecycleOutcomeRecord::clean(
            "lifecycle-stats-operation",
            crate::LifecycleOperationKind::Correct,
        );
        lifecycle.stats_update_status = crate::StatsUpdateStatusRecord {
            updated_object_internal_ids: Vec::new(),
            failure: Some(crate::StatsUpdateFailureRecord {
                failed_object_internal_ids: vec!["episode-1".into()],
                causes: vec![crate::StatsUpdateCauseRecord::HealthCheck {
                    error: crate::RetrievalStatsStoreErrorRecord::LockPoisoned,
                }],
            }),
        };
        let mut result = row(serde_json::json!({}));
        result.lifecycle_outcomes.push(lifecycle);

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[result],
            &[],
        )
        .unwrap();

        assert_eq!(summary.degradation.lifecycle_maintenance_failure_count, 1);
    }

    #[test]
    fn summary_rejects_conflicting_outcomes_with_the_same_attempt_identity() {
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
                attempt_index: 0,
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
                attempt_index: 0,
            })
        );
    }

    #[test]
    fn summary_rejects_non_contiguous_attempt_indexes() {
        let mut write = WriteOutcomeRecord::clean(
            "gapped-write-operation",
            crate::WriteOperationKind::ExplicitCommit,
        );
        write.attempt_index = 1;
        let mut result = row(serde_json::json!({}));
        result.write_outcomes.push(write);
        let error = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[result],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<SummaryInvariantError>(),
            Some(&SummaryInvariantError::NonContiguousWriteAttempts {
                operation_id: "gapped-write-operation".to_string(),
                expected_attempt_index: 0,
                actual_attempt_index: 1,
            })
        );

        let first = LifecycleOutcomeRecord::clean(
            "gapped-lifecycle-operation",
            crate::LifecycleOperationKind::Correct,
        );
        let mut third = first.clone();
        third.attempt_index = 2;
        let mut result = row(serde_json::json!({}));
        result.lifecycle_outcomes.extend([first, third]);
        let error = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[result],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<SummaryInvariantError>(),
            Some(&SummaryInvariantError::NonContiguousLifecycleAttempts {
                operation_id: "gapped-lifecycle-operation".to_string(),
                expected_attempt_index: 1,
                actual_attempt_index: 2,
            })
        );
    }

    #[test]
    fn row_schema_is_exact_and_rejects_missing_or_unsupported_versions() {
        validate_row_schema(Some(RESULT_SCHEMA_VERSION)).unwrap();
        let missing = validate_row_schema(None).unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("missing result schema_version")
        );
        for version in ["1.0.0", "0.9.0"] {
            let unsupported = validate_row_schema(Some(version)).unwrap_err();
            let error = unsupported.to_string();
            assert!(
                error.contains("unsupported result schema_version"),
                "{error}"
            );
            assert!(error.contains(version), "{error}");
            assert!(error.contains(RESULT_SCHEMA_VERSION), "{error}");
        }
    }

    #[test]
    fn read_jsonl_round_trips_v2_and_rejects_v1_at_schema_detection() {
        let path = temp_path("results", "jsonl");
        let result_row = row(serde_json::json!({"recall_any@1": 1.0}));
        let expected_bytes = format!(
            "{}\n",
            serde_json::to_string(&versioned_row_value(&result_row).unwrap()).unwrap()
        );
        write_jsonl(&path, std::slice::from_ref(&result_row)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected_bytes);
        let rows = read_jsonl(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].question_id, result_row.question_id);

        let mut legacy = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        legacy["schema_version"] = Value::String("1.0.0".into());
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&legacy).unwrap()),
        )
        .unwrap();
        let error = read_jsonl(&path).unwrap_err().to_string();
        assert!(
            error.contains("unsupported result schema_version"),
            "{error}"
        );
        assert!(error.contains("1.0.0"), "{error}");
        assert!(error.contains(RESULT_SCHEMA_VERSION), "{error}");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_jsonl_canonicalizes_outcomes_by_operation_and_attempt() {
        let first_path = temp_path("results-canonical-first", "jsonl");
        let second_path = temp_path("results-canonical-second", "jsonl");
        let mut result = row(serde_json::json!({}));
        for (operation_id, attempt_index) in [("b", 0), ("a", 1), ("a", 0)] {
            let mut write =
                WriteOutcomeRecord::clean(operation_id, crate::WriteOperationKind::ExplicitCommit);
            write.attempt_index = attempt_index;
            result.write_outcomes.push(write);

            let mut lifecycle =
                LifecycleOutcomeRecord::clean(operation_id, crate::LifecycleOperationKind::Correct);
            lifecycle.attempt_index = attempt_index;
            result.lifecycle_outcomes.push(lifecycle);
        }

        write_jsonl(&first_path, std::slice::from_ref(&result)).unwrap();
        result.write_outcomes.reverse();
        result.lifecycle_outcomes.reverse();
        write_jsonl(&second_path, &[result]).unwrap();

        let first = std::fs::read_to_string(&first_path).unwrap();
        assert_eq!(first, std::fs::read_to_string(&second_path).unwrap());
        let value: Value = serde_json::from_str(first.trim()).unwrap();
        for family in ["write_outcomes", "lifecycle_outcomes"] {
            let identities = value[family]
                .as_array()
                .unwrap()
                .iter()
                .map(|outcome| {
                    (
                        outcome["operation_id"].as_str().unwrap(),
                        outcome["attempt_index"].as_u64().unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(identities, vec![("a", 0), ("a", 1), ("b", 0)]);
        }

        std::fs::remove_file(first_path).unwrap();
        std::fs::remove_file(second_path).unwrap();
    }

    #[test]
    fn write_jsonl_rejects_non_v2_rows_before_replacing_the_destination() {
        let path = temp_path("results-writer-schema", "jsonl");
        for schema_version in ["1.0.0", "9.9.9"] {
            let mut invalid = row(serde_json::json!({}));
            invalid.schema_version = schema_version.to_string();
            std::fs::write(&path, "preserved\n").unwrap();

            let error = write_jsonl(&path, &[row(serde_json::json!({})), invalid])
                .unwrap_err()
                .to_string();
            assert!(error.contains("index 1"), "{error}");
            assert!(error.contains(schema_version), "{error}");
            assert!(error.contains(RESULT_SCHEMA_VERSION), "{error}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserved\n");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn v2_result_reader_rejects_shape_drift() {
        let path = temp_path("results-shape-drift", "jsonl");

        let current = serde_json::to_string(&row(serde_json::json!({}))).unwrap();
        let duplicate_root = current.replacen(r#""run_id":"r""#, r#""run_id":"r","run_id":"r""#, 1);
        std::fs::write(&path, format!("{duplicate_root}\n")).unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(error.contains("duplicate JSON object key"), "{error}");

        let mut verdict_row = row(serde_json::json!({}));
        let mut outcome = WriteOutcomeRecord::clean(
            "duplicate-verdict",
            crate::WriteOperationKind::ExplicitCommit,
        );
        outcome.vector_indexing_failure = Some(crate::VectorIndexingFailureRecord {
            unindexed_objects: Vec::new(),
            cause: crate::VectorIndexingCauseRecord::VectorDatabase(
                crate::VectorDatabaseErrorRecord {
                    backend: "qdrant".to_string(),
                    kind: crate::VectorDatabaseErrorKind::Response,
                    status: None,
                    message: "rejected".to_string(),
                    retry_after_seconds: None,
                },
            ),
        });
        verdict_row.write_outcomes.push(outcome);
        verdict_row
            .lifecycle_outcomes
            .push(LifecycleOutcomeRecord::clean(
                "required-attempt-index",
                crate::LifecycleOperationKind::Correct,
            ));
        let current = serde_json::to_string(&verdict_row).unwrap();
        let duplicate_nested_verdict = current.replacen(
            r#""kind":"response""#,
            r#""kind":"response","kind":"response""#,
            1,
        );
        assert_ne!(current, duplicate_nested_verdict);
        std::fs::write(&path, format!("{duplicate_nested_verdict}\n")).unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(error.contains("duplicate JSON object key"), "{error}");

        let current = serde_json::to_value(&verdict_row).unwrap();
        for family in ["write_outcomes", "lifecycle_outcomes"] {
            let mut missing_attempt = current.clone();
            missing_attempt[family][0]
                .as_object_mut()
                .unwrap()
                .remove("attempt_index");
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string(&missing_attempt).unwrap()),
            )
            .unwrap();
            let error = format!("{:#}", read_jsonl(&path).unwrap_err());
            assert!(error.contains("missing field `attempt_index`"), "{error}");
        }

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

        let mut current = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        current["context"]["unexpected_v2_field"] = Value::Bool(true);
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&current).unwrap()),
        )
        .unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(error.contains("unknown field"), "{error}");

        let mut current = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        current["context"]
            .as_object_mut()
            .unwrap()
            .remove("retrieved_context_chars");
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&current).unwrap()),
        )
        .unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(
            error.contains("missing field `retrieved_context_chars`"),
            "{error}"
        );

        let mut current = versioned_row_value(&row(serde_json::json!({}))).unwrap();
        current["context"]
            .as_object_mut()
            .unwrap()
            .remove("full_history_chars");
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&current).unwrap()),
        )
        .unwrap();
        let error = format!("{:#}", read_jsonl(&path).unwrap_err());
        assert!(
            error.contains("missing field `full_history_chars`"),
            "{error}"
        );

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
        let mut summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::Synthetic,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[row(serde_json::json!({"fixed_metric": 1.0}))],
            &[],
        )
        .unwrap();
        write_summary(&path, &summary).unwrap();
        assert_eq!(
            read_summary(&path).unwrap().schema_version,
            RESULT_SCHEMA_VERSION
        );

        summary.schema_version = "9.9.9".to_string();
        std::fs::write(&path, "preserved\n").unwrap();
        let error = write_summary(&path, &summary).unwrap_err().to_string();
        assert!(error.contains("9.9.9"), "{error}");
        assert!(error.contains(RESULT_SCHEMA_VERSION), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserved\n");
        summary.schema_version = RESULT_SCHEMA_VERSION.to_string();

        let raw = serde_json::to_string(&summary).unwrap();
        let duplicate_root = raw.replacen(r#""run_id":"r""#, r#""run_id":"r","run_id":"r""#, 1);
        std::fs::write(&path, duplicate_root).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("duplicate JSON object key"), "{error}");

        summary.config = serde_json::json!({"nested": {"mode": "strict"}});
        let raw = serde_json::to_string(&summary).unwrap();
        let duplicate_dynamic_value = raw.replacen(
            r#""mode":"strict""#,
            r#""mode":"strict","mode":"strict""#,
            1,
        );
        assert_ne!(raw, duplicate_dynamic_value);
        std::fs::write(&path, duplicate_dynamic_value).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("duplicate JSON object key"), "{error}");

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

        let mut malformed_latency = serde_json::to_value(&summary).unwrap();
        malformed_latency["latency"] = Value::Null;
        std::fs::write(&path, serde_json::to_vec(&malformed_latency).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("invalid type"), "{error}");

        let mut incomplete_latency = serde_json::to_value(&summary).unwrap();
        incomplete_latency["latency"]["latency_ms"]
            .as_object_mut()
            .unwrap()
            .remove("p95");
        std::fs::write(&path, serde_json::to_vec(&incomplete_latency).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("missing field `p95`"), "{error}");

        let mut malformed_coverage = serde_json::to_value(&summary).unwrap();
        malformed_coverage["registry_coverage"] = Value::String("open".to_string());
        std::fs::write(&path, serde_json::to_vec(&malformed_coverage).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("invalid type"), "{error}");

        let mut drifted_support = serde_json::to_value(&summary).unwrap();
        drifted_support["metric_support"]["fixed_metric"]["unexpected_v2_field"] =
            Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&drifted_support).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("unknown field"), "{error}");

        let mut drifted_metric = serde_json::to_value(&summary).unwrap();
        drifted_metric["metrics"]["fixed_metric"]["unexpected_v2_field"] = Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&drifted_metric).unwrap()).unwrap();
        let error = format!("{:#}", read_summary(&path).unwrap_err());
        assert!(error.contains("unknown field"), "{error}");

        std::fs::remove_file(path).unwrap();
    }
}
