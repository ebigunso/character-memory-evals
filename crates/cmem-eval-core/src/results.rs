use crate::{
    DatasetId, DatasetKind, DegradationSummary, EmbeddingBindingRecord, LifecycleOutcomeRecord,
    MetricFamily, MetricSupportSummary, MetricsRecord, NumericMetricAggregate,
    NumericMetricSummary, RegistryCoverageSummary, RetrievalTelemetry, RetrievedItem,
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
    canonical
        .write_outcomes
        .sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
    canonical
        .lifecycle_outcomes
        .sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
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
    let degradation = summarize_degradation(rows);
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

fn summarize_degradation(rows: &[PerQuestionResult]) -> DegradationSummary {
    let degraded_write = rows
        .iter()
        .flat_map(|row| &row.write_outcomes)
        .any(|outcome| {
            outcome.vector_indexing_failure.is_some()
                || outcome.stats_update_status.failure.is_some()
                || !outcome.repair_needed.is_empty()
        });
    let degraded_lifecycle = rows
        .iter()
        .flat_map(|row| &row.lifecycle_outcomes)
        .any(|outcome| {
            !outcome.vector_maintenance_failures.is_empty()
                || outcome.stats_update_status.failure.is_some()
        });
    DegradationSummary {
        any_degradation: degraded_write || degraded_lifecycle,
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

    #[test]
    fn empty_run_is_rejected_before_summary() {
        let error = reject_empty_run(&[]).unwrap_err().to_string();
        assert!(error.contains("produced no result rows"), "{error}");
    }
    use crate::RepairMarkerRecord;

    fn dataset() -> DatasetId {
        DatasetId::new("locomo").unwrap()
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
            dataset_kind: DatasetKind::LoCoMo,
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
        assert_eq!(value["dataset_kind"], "lo_co_mo");
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
            DatasetKind::LoCoMo,
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
        let family = crate::retrieval_metric_family("locomo", [("session", [5].as_slice())]);

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::LoCoMo,
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
    fn summary_reports_any_degradation() {
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
        second.write_outcomes.push(write);
        second.lifecycle_outcomes.push(lifecycle.clone());
        second.lifecycle_outcomes.push(lifecycle);

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::LoCoMo,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[first, second],
            &[],
        )
        .unwrap();

        assert!(summary.degradation.any_degradation);
    }

    #[test]
    fn summary_keeps_clean_lifecycle_outcomes_non_degraded() {
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
        second.lifecycle_outcomes.push(retry);

        let summary = summarize_rows(
            "r".into(),
            dataset(),
            DatasetKind::LoCoMo,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[first, second],
            &[],
        )
        .unwrap();

        assert!(!summary.degradation.any_degradation);
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
            DatasetKind::LoCoMo,
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[result],
            &[],
        )
        .unwrap();

        assert!(summary.degradation.any_degradation);
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
    fn write_jsonl_canonicalizes_outcomes_by_operation() {
        let first_path = temp_path("results-canonical-first", "jsonl");
        let second_path = temp_path("results-canonical-second", "jsonl");
        let mut result = row(serde_json::json!({}));
        for operation_id in ["b", "c", "a"] {
            result.write_outcomes.push(WriteOutcomeRecord::clean(
                operation_id,
                crate::WriteOperationKind::ExplicitCommit,
            ));
            result
                .lifecycle_outcomes
                .push(LifecycleOutcomeRecord::clean(
                    operation_id,
                    crate::LifecycleOperationKind::Correct,
                ));
        }

        write_jsonl(&first_path, std::slice::from_ref(&result)).unwrap();
        result.write_outcomes.reverse();
        result.lifecycle_outcomes.reverse();
        write_jsonl(&second_path, &[result]).unwrap();

        let first = std::fs::read_to_string(&first_path).unwrap();
        assert_eq!(first, std::fs::read_to_string(&second_path).unwrap());
        let value: Value = serde_json::from_str(first.trim()).unwrap();
        for family in ["write_outcomes", "lifecycle_outcomes"] {
            let operation_ids = value[family]
                .as_array()
                .unwrap()
                .iter()
                .map(|outcome| outcome["operation_id"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(operation_ids, vec!["a", "b", "c"]);
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
            DatasetKind::LoCoMo,
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
