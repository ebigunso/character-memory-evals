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
use std::path::Path;

pub const RESULT_SCHEMA_VERSION: &str = "3.0.0";

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
    pub write_outcomes: Vec<crate::RecordedOutcome<crate::RememberOutcome>>,
    pub link_outcomes: Vec<crate::RecordedOutcome<crate::LinkOutcome>>,
    pub lifecycle_outcomes: Vec<crate::RecordedOutcome<crate::LifecycleMutationOutcome>>,
    pub metrics: MetricsRecord,
    pub latency_ms: u128,
    pub context_char_count: usize,
    pub context_word_count: usize,
    pub context: ResultContextMetrics,
    pub retrieval_outcomes: Vec<crate::RetrieveOutcome>,
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
        .link_outcomes
        .sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
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

pub fn summarize_degradation(rows: &[PerQuestionResult]) -> DegradationSummary {
    DegradationSummary {
        any_degradation: rows.iter().any(|row| {
            row.write_outcomes.iter().any(|record| {
                let outcome = &record.outcome;
                outcome.vector_indexing_failure.is_some()
                    || outcome.stats_update_status.failure.is_some()
                    || !outcome.repair_needed.is_empty()
            }) || row
                .link_outcomes
                .iter()
                .any(|record| record.outcome.stats_update_status.failure.is_some())
                || row.lifecycle_outcomes.iter().any(|record| {
                    let outcome = &record.outcome;
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
            adapter: RunAdapterMetadata::live(),
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
    fn serializes_per_question_result() {
        let row = row(serde_json::json!({"recall_any@1": 1.0}));
        let value = versioned_row_value(&row).unwrap();
        assert_eq!(value["question_id"], "q");
        assert_eq!(value["schema_version"], RESULT_SCHEMA_VERSION);
        assert_eq!(value["adapter"]["mode"], "live");
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
            RunAdapterMetadata::live(),
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
            RunAdapterMetadata::live(),
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

    fn write_outcome(operation_id: &str) -> crate::RecordedOutcome<crate::RememberOutcome> {
        crate::RecordedOutcome {
            operation_id: operation_id.into(),
            outcome: crate::RememberOutcome {
                persisted_object_ids: Vec::new(),
                persisted_link_ids: Vec::new(),
                vector_indexed_object_ids: Vec::new(),
                vector_indexing_failure: None,
                stats_update_status: Default::default(),
                repair_needed: Vec::new(),
                diagnostics: Default::default(),
            },
        }
    }

    fn lifecycle_outcome(
        operation_id: &str,
    ) -> crate::RecordedOutcome<crate::LifecycleMutationOutcome> {
        crate::RecordedOutcome {
            operation_id: operation_id.into(),
            outcome: crate::LifecycleMutationOutcome {
                graph_mutated_object_ids: Vec::new(),
                graph_mutated_link_ids: Vec::new(),
                vector_maintained_object_ids: Vec::new(),
                vector_maintenance_failure: None,
                stats_update_status: Default::default(),
                trace: None,
                diagnostics: Default::default(),
            },
        }
    }

    #[test]
    fn native_outcomes_preserve_failures_and_drive_degradation() {
        let mut result = row(serde_json::json!({}));
        result.write_outcomes.push(write_outcome("write"));
        result
            .lifecycle_outcomes
            .push(lifecycle_outcome("lifecycle"));
        assert!(!summarize_degradation(&[result.clone()]).any_degradation);
        result.write_outcomes[0].outcome.vector_indexing_failure =
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
        result.lifecycle_outcomes[0].outcome.stats_update_status =
            character_memory::StatsUpdateStatus::failed([], [], Vec::new());
        assert!(summarize_degradation(&[result.clone()]).any_degradation);
        result.lifecycle_outcomes.clear();
        let mut link = link_outcome("link");
        link.outcome.stats_update_status =
            character_memory::StatsUpdateStatus::failed([], [], Vec::new());
        result.link_outcomes.push(link);
        assert!(summarize_degradation(&[result.clone()]).any_degradation);
        write_jsonl(&path, &[result.clone()]).unwrap();
        assert_eq!(
            read_jsonl(&path).unwrap()[0].link_outcomes,
            result.link_outcomes
        );
        std::fs::remove_file(path).unwrap();
    }

    fn link_outcome(operation_id: &str) -> crate::RecordedOutcome<crate::LinkOutcome> {
        crate::RecordedOutcome {
            operation_id: operation_id.into(),
            outcome: crate::LinkOutcome {
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
            },
        }
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
    fn read_jsonl_round_trips_current_and_rejects_superseded_schemas() {
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
        for version in ["1.0.0", "2.0.0"] {
            legacy["schema_version"] = Value::String(version.into());
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
            assert!(error.contains(version), "{error}");
            assert!(error.contains(RESULT_SCHEMA_VERSION), "{error}");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_jsonl_canonicalizes_outcomes_by_operation() {
        let first_path = temp_path("results-canonical-first", "jsonl");
        let second_path = temp_path("results-canonical-second", "jsonl");
        let mut result = row(serde_json::json!({}));
        for operation_id in ["b", "c", "a"] {
            result.write_outcomes.push(write_outcome(operation_id));
            result.link_outcomes.push(link_outcome(operation_id));
            result
                .lifecycle_outcomes
                .push(lifecycle_outcome(operation_id));
        }

        write_jsonl(&first_path, std::slice::from_ref(&result)).unwrap();
        result.write_outcomes.reverse();
        result.link_outcomes.reverse();
        result.lifecycle_outcomes.reverse();
        write_jsonl(&second_path, &[result]).unwrap();

        let first = std::fs::read_to_string(&first_path).unwrap();
        assert_eq!(first, std::fs::read_to_string(&second_path).unwrap());
        let value: Value = serde_json::from_str(first.trim()).unwrap();
        for family in ["write_outcomes", "link_outcomes", "lifecycle_outcomes"] {
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
    fn result_reader_rejects_owned_shape_drift() {
        let path = temp_path("result-shape", "jsonl");
        let value = serde_json::to_value(row(serde_json::json!({}))).unwrap();
        for field in [
            "retrieval_outcomes",
            "write_outcomes",
            "link_outcomes",
            "lifecycle_outcomes",
            "metrics",
            "context",
        ] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            std::fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
            assert!(read_jsonl(&path).is_err(), "missing {field}");
        }
        let mut unknown = value.clone();
        unknown["unexpected_field"] = Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert!(read_jsonl(&path).is_err());
        let mut wrong_metric = value;
        wrong_metric["metrics"]["test"] = Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&wrong_metric).unwrap()).unwrap();
        assert!(read_jsonl(&path).is_err());
        let duplicate = r#"{"schema_version":"3.0.0","schema_version":"3.0.0"}"#;
        std::fs::write(&path, duplicate).unwrap();
        assert!(format!("{:#}", read_jsonl(&path).unwrap_err()).contains("duplicate"));
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
            RunAdapterMetadata::live(),
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
            (
                serde_json::to_vec(&serde_json::json!({"schema_version": "2.0.0"})).unwrap(),
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
