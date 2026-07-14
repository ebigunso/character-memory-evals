use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const CORE_BASE_METRICS: &[&str] = &[
    "retrieved_context_tokens",
    "full_history_tokens",
    "context_compression_ratio",
    "context_reduction_rate",
    "num_retrieved_items",
    "num_relevant_episodes",
    "num_relevant_observations",
    "num_derived_memories",
    "num_active_threads",
    "num_entities",
    "retrieval_rationale_coverage",
    "vector_candidate_count",
    "graph_relations_count",
    "graph_verified_count",
    "section_assignment_count",
    "provenance_coverage",
    "context_validation_pass_rate",
    "suppressed_memory_leakage_rate",
    "orphan_vector_leakage_rate",
    "superseded_current_leakage_rate",
    "cross_store_id_validation_pass_rate",
    "qa_accuracy",
    "qa_f1",
    "qa_exact_match",
    "abstention_accuracy",
    "unsupported_answer_rate",
];

const RANKING_METRIC_NAMES: &[&str] =
    &["recall_any", "recall_all", "recall_fraction", "mrr", "ndcg"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricFamily {
    pub name: String,
    pub required_metrics: BTreeSet<String>,
}

impl MetricFamily {
    pub fn new(
        name: impl Into<String>,
        required_metrics: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            name: name.into(),
            required_metrics: required_metrics.into_iter().collect(),
        }
    }
}

pub fn core_base_metric_family() -> MetricFamily {
    MetricFamily::new(
        "core",
        CORE_BASE_METRICS.iter().map(|metric| (*metric).to_string()),
    )
}

pub fn retrieval_metric_family<'a>(
    name: impl Into<String>,
    rankings: impl IntoIterator<Item = (&'a str, &'a [usize])>,
) -> MetricFamily {
    let mut required_metrics = BTreeSet::new();
    for (prefix, ks) in rankings {
        for k in ks {
            for metric in RANKING_METRIC_NAMES {
                required_metrics.insert(format!("{prefix}_{metric}@{k}"));
            }
        }
    }
    MetricFamily {
        name: name.into(),
        required_metrics,
    }
}

pub fn required_metric_set(families: &[MetricFamily]) -> BTreeSet<String> {
    let mut required = core_base_metric_family().required_metrics;
    for family in families {
        required.extend(family.required_metrics.iter().cloned());
    }
    required
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalMetricSummary {
    pub recall_any: f64,
    pub recall_all: f64,
    pub recall_fraction: f64,
    pub mrr: f64,
    pub ndcg: f64,
}

pub fn retrieval_metrics(
    retrieved_ids: &[String],
    gold_ids: &[String],
    k: usize,
) -> Option<RetrievalMetricSummary> {
    let gold = gold_ids.iter().collect::<BTreeSet<_>>();
    if gold.is_empty() {
        return None;
    }
    let top = retrieved_ids.iter().take(k).collect::<Vec<_>>();
    let hits = top
        .iter()
        .filter(|id| gold.contains(**id))
        .copied()
        .collect::<BTreeSet<_>>();
    let first_rank = top
        .iter()
        .position(|id| gold.contains(*id))
        .map(|idx| idx + 1);
    let dcg = top
        .iter()
        .enumerate()
        .filter(|(_, id)| gold.contains(*id))
        .map(|(idx, _)| 1.0 / ((idx + 2) as f64).log2())
        .sum::<f64>();
    let ideal_len = gold.len().min(k);
    let idcg = (0..ideal_len)
        .map(|idx| 1.0 / ((idx + 2) as f64).log2())
        .sum::<f64>();

    Some(RetrievalMetricSummary {
        recall_any: if hits.is_empty() { 0.0 } else { 1.0 },
        recall_all: if hits.len() == gold.len() { 1.0 } else { 0.0 },
        recall_fraction: hits.len() as f64 / gold.len() as f64,
        mrr: first_rank.map_or(0.0, |rank| 1.0 / rank as f64),
        ndcg: if idcg == 0.0 { 0.0 } else { dcg / idcg },
    })
}

pub fn insert_retrieval_metrics(
    out: &mut Map<String, Value>,
    prefix: &str,
    retrieved_ids: &[String],
    gold_ids: &[String],
    k: usize,
) {
    if let Some(metrics) = retrieval_metrics(retrieved_ids, gold_ids, k) {
        out.insert(
            format!("{prefix}_recall_any@{k}"),
            Value::from(metrics.recall_any),
        );
        out.insert(
            format!("{prefix}_recall_all@{k}"),
            Value::from(metrics.recall_all),
        );
        out.insert(
            format!("{prefix}_recall_fraction@{k}"),
            Value::from(metrics.recall_fraction),
        );
        out.insert(format!("{prefix}_mrr@{k}"), Value::from(metrics.mrr));
        out.insert(format!("{prefix}_ndcg@{k}"), Value::from(metrics.ndcg));
    }
}

pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

pub fn median(values: &[f64]) -> Option<f64> {
    percentile(values, 50.0)
}

pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p / 100.0) * (sorted.len().saturating_sub(1)) as f64).round() as usize;
    sorted.get(rank).copied()
}

pub fn aggregate_numeric_metrics(rows: &[Map<String, Value>]) -> Value {
    let mut by_key: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for row in rows {
        for (key, value) in row {
            if let Some(number) = value.as_f64() {
                by_key.entry(key.clone()).or_default().push(number);
            }
        }
    }

    let mut out = Map::new();
    for (key, values) in by_key {
        out.insert(
            key,
            serde_json::json!({
                "mean": mean(&values),
                "median": median(&values),
                "p50": percentile(&values, 50.0),
                "p95": percentile(&values, 95.0),
            }),
        );
    }
    Value::Object(out)
}

pub fn metric_support_summary(rows: &[Map<String, Value>]) -> Value {
    let mut by_key: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for row in rows {
        for (key, value) in row {
            let entry = by_key.entry(key.clone()).or_default();
            entry.0 += 1;
            if value.is_number() {
                entry.1 += 1;
            } else if value.is_null() {
                entry.2 += 1;
            }
        }
    }

    let mut out = Map::new();
    for (key, (rows_present, numeric_rows, null_rows)) in by_key {
        out.insert(
            key,
            serde_json::json!({
                "rows_present": rows_present,
                "numeric_rows": numeric_rows,
                "null_rows": null_rows,
                "unsupported": rows_present > 0 && numeric_rows == 0 && null_rows == rows_present,
            }),
        );
    }
    Value::Object(out)
}

pub fn initialize_registry_metrics(out: &mut Map<String, Value>) {
    initialize_registry_metrics_for(out, &[]);
}

pub fn initialize_registry_metrics_for(out: &mut Map<String, Value>, families: &[MetricFamily]) {
    for key in required_metric_set(families) {
        out.entry(key).or_insert(Value::Null);
    }
}

pub fn registry_coverage_summary(rows: &[Map<String, Value>]) -> Value {
    registry_coverage_summary_for(rows, &[])
}

pub fn registry_coverage_summary_for(
    rows: &[Map<String, Value>],
    families: &[MetricFamily],
) -> Value {
    let required_metrics = required_metric_set(families);
    let mut present = 0usize;
    let mut numeric = 0usize;
    let mut null = 0usize;
    let mut missing = Vec::new();
    for key in &required_metrics {
        let mut key_present = false;
        let mut key_numeric = false;
        let mut key_null = false;
        for row in rows {
            if let Some(value) = row.get(key) {
                key_present = true;
                key_numeric |= value.is_number();
                key_null |= value.is_null();
            }
        }
        if key_present {
            present += 1;
        } else {
            missing.push(key.clone());
        }
        if key_numeric {
            numeric += 1;
        }
        if key_null && !key_numeric {
            null += 1;
        }
    }
    let required_metrics_null_only = null;
    serde_json::json!({
        "required_metrics_total": required_metrics.len(),
        "required_metrics_present": present,
        "required_metrics_numeric": numeric,
        "required_metrics_null_only": required_metrics_null_only,
        "missing_required_metrics": missing,
    })
}

pub fn insert_context_metrics(out: &mut Map<String, Value>, context: &crate::ResultContextMetrics) {
    out.insert(
        "retrieved_context_tokens".to_string(),
        Value::from(context.retrieved_context_tokens),
    );
    out.insert(
        "full_history_tokens".to_string(),
        option_usize(context.full_history_tokens),
    );
    out.insert(
        "context_compression_ratio".to_string(),
        option_f64(context.compression_ratio),
    );
    out.insert(
        "context_reduction_rate".to_string(),
        option_f64(context.reduction_rate),
    );
}

pub fn insert_composition_metrics(
    out: &mut Map<String, Value>,
    composition: &crate::ResultCompositionMetrics,
) {
    out.insert(
        "num_retrieved_items".to_string(),
        Value::from(composition.total_items),
    );
    out.insert(
        "num_relevant_episodes".to_string(),
        Value::from(composition.episodes),
    );
    out.insert(
        "num_relevant_observations".to_string(),
        Value::from(composition.observations),
    );
    out.insert(
        "num_derived_memories".to_string(),
        Value::from(composition.derived_memories),
    );
    out.insert(
        "num_active_threads".to_string(),
        Value::from(composition.memory_threads),
    );
    out.insert(
        "num_entities".to_string(),
        option_usize(composition.entities),
    );
    out.insert(
        "retrieval_rationale_coverage".to_string(),
        option_f64(composition.rationale_coverage),
    );
}

pub fn insert_telemetry_metrics(
    out: &mut Map<String, Value>,
    telemetry: &crate::RetrievalTelemetry,
) {
    out.insert(
        "vector_candidate_count".to_string(),
        option_usize(telemetry.vector_candidate_count),
    );
    out.insert(
        "retrieved_by_vector_count".to_string(),
        option_usize(telemetry.vector_candidate_count),
    );
    out.insert(
        "graph_relations_count".to_string(),
        option_usize(telemetry.graph_relation_count),
    );
    out.insert(
        "graph_verified_count".to_string(),
        option_usize(telemetry.graph_verified_count),
    );
    out.insert(
        "stale_candidate_omission_count".to_string(),
        option_usize(telemetry.stale_candidate_omission_count),
    );
    out.insert(
        "lifecycle_omission_count".to_string(),
        option_usize(telemetry.lifecycle_omission_count),
    );
    out.insert(
        "lifecycle_filter_decision_count".to_string(),
        option_usize(telemetry.lifecycle_filter_decision_count),
    );
    out.insert(
        "suppressed_or_deleted_returned_count".to_string(),
        option_usize(telemetry.suppressed_or_deleted_returned_count),
    );
    out.insert(
        "superseded_current_returned_count".to_string(),
        option_usize(telemetry.superseded_current_returned_count),
    );
    out.insert(
        "unsafe_lifecycle_returned_count".to_string(),
        option_usize(telemetry.unsafe_lifecycle_returned_count),
    );
    out.insert(
        "graph_object_missing_omitted_count".to_string(),
        option_usize(telemetry.graph_object_missing_omitted_count),
    );
    out.insert(
        "graph_object_missing_returned_count".to_string(),
        option_usize(telemetry.graph_object_missing_returned_count),
    );
    out.insert(
        "section_assignment_count".to_string(),
        option_usize(telemetry.section_assignment_count),
    );
    if let Some(total) = telemetry.section_assignment_count {
        for (section, count) in &telemetry.section_assignment_counts {
            out.insert(
                format!("section_assignment_{section}_count"),
                Value::from(*count),
            );
            out.insert(
                format!("section_assignment_{section}_rate"),
                rate(*count, total),
            );
        }
    }
}

pub fn insert_integrity_metrics(out: &mut Map<String, Value>, retrieved: &[crate::RetrievedItem]) {
    let without_external_id = retrieved
        .iter()
        .filter(|item| item.external_id.is_none())
        .count();
    let derived_without_provenance = retrieved
        .iter()
        .filter(|item| item.kind == "derived_memory" && item.episode_external_id.is_none())
        .count();

    out.insert(
        "returned_items_without_external_id".to_string(),
        Value::from(without_external_id),
    );
    out.insert(
        "returned_derived_memories_without_provenance".to_string(),
        Value::from(derived_without_provenance),
    );
    let derived_count = retrieved
        .iter()
        .filter(|item| item.kind == "derived_memory")
        .count();
    let provenance_coverage = if derived_count == 0 {
        Some(1.0)
    } else {
        Some((derived_count - derived_without_provenance) as f64 / derived_count as f64)
    };
    out.insert(
        "provenance_coverage".to_string(),
        option_f64(provenance_coverage),
    );
    out.insert("context_validation_pass_rate".to_string(), Value::Null);
    out.insert("suppressed_memory_leakage_rate".to_string(), Value::Null);
    out.insert("orphan_vector_leakage_rate".to_string(), Value::Null);
    out.insert("superseded_current_leakage_rate".to_string(), Value::Null);
    out.insert(
        "cross_store_id_validation_pass_rate".to_string(),
        Value::Null,
    );
    out.insert(
        "returned_items_with_authoritative_validation".to_string(),
        Value::Null,
    );
    out.insert(
        "suppressed_or_deleted_items_returned".to_string(),
        Value::Null,
    );
    out.insert(
        "superseded_items_returned_as_current".to_string(),
        Value::Null,
    );
}

pub fn insert_integrity_detail_metrics(
    out: &mut Map<String, Value>,
    integrity: &crate::ResultIntegrityDetails,
) {
    out.insert(
        "returned_items_without_external_ids".to_string(),
        Value::from(integrity.returned_items_without_external_id),
    );
    out.insert(
        "returned_items_without_external_id".to_string(),
        Value::from(integrity.returned_items_without_external_id),
    );
    out.insert(
        "returned_derived_memories_without_provenance".to_string(),
        Value::from(integrity.returned_derived_memories_without_provenance),
    );
    out.insert(
        "provenance_coverage".to_string(),
        option_f64(integrity.provenance_coverage),
    );
    out.insert(
        "context_validation_pass_rate".to_string(),
        option_f64(integrity.context_validation_pass_rate),
    );
    out.insert(
        "suppressed_memory_leakage_rate".to_string(),
        option_f64(integrity.suppressed_memory_leakage_rate),
    );
    out.insert(
        "orphan_vector_leakage_rate".to_string(),
        option_f64(integrity.orphan_vector_leakage_rate),
    );
    out.insert(
        "superseded_current_leakage_rate".to_string(),
        option_f64(integrity.superseded_current_leakage_rate),
    );
    out.insert(
        "suppressed_or_deleted_items_returned".to_string(),
        option_usize(integrity.suppressed_or_deleted_returned_count),
    );
    out.insert(
        "superseded_items_returned_as_current".to_string(),
        option_usize(integrity.superseded_current_returned_count),
    );
    out.insert(
        "returned_items_with_authoritative_validation".to_string(),
        Value::Null,
    );
    out.insert(
        "cross_store_id_validation_pass_rate".to_string(),
        option_f64(integrity.cross_store_id_validation_pass_rate),
    );
}

pub fn integrity_details(retrieved: &[crate::RetrievedItem]) -> crate::ResultIntegrityDetails {
    let without_external_id = retrieved
        .iter()
        .filter(|item| item.external_id.is_none())
        .count();
    let derived_count = retrieved
        .iter()
        .filter(|item| item.kind == "derived_memory")
        .count();
    let derived_without_provenance = retrieved
        .iter()
        .filter(|item| item.kind == "derived_memory" && item.episode_external_id.is_none())
        .count();
    crate::ResultIntegrityDetails {
        returned_items_without_external_id: without_external_id,
        returned_derived_memories_without_provenance: derived_without_provenance,
        suppressed_or_deleted_returned_count: None,
        superseded_current_returned_count: None,
        provenance_coverage: if derived_count == 0 {
            Some(1.0)
        } else {
            Some((derived_count - derived_without_provenance) as f64 / derived_count as f64)
        },
        context_validation_pass_rate: None,
        suppressed_memory_leakage_rate: None,
        orphan_vector_leakage_rate: None,
        superseded_current_leakage_rate: None,
        cross_store_id_validation_pass_rate: None,
    }
}

pub fn integrity_details_with_telemetry(
    retrieved: &[crate::RetrievedItem],
    telemetry: &crate::RetrievalTelemetry,
) -> crate::ResultIntegrityDetails {
    let mut details = integrity_details(retrieved);
    let returned_count = retrieved.len();
    if telemetry.trace_available {
        details.suppressed_memory_leakage_rate = telemetry
            .suppressed_or_deleted_returned_count
            .map(|count| leakage_rate(count, returned_count));
        details.suppressed_or_deleted_returned_count =
            telemetry.suppressed_or_deleted_returned_count;
        details.superseded_current_leakage_rate = telemetry
            .superseded_current_returned_count
            .map(|count| leakage_rate(count, returned_count));
        details.superseded_current_returned_count = telemetry.superseded_current_returned_count;
        details.orphan_vector_leakage_rate = match (
            telemetry.graph_object_missing_returned_count,
            telemetry.graph_object_missing_omitted_count,
        ) {
            (Some(returned), Some(omitted)) => Some(leakage_rate(returned, returned + omitted)),
            _ => None,
        };
        details.context_validation_pass_rate = context_validation_pass_rate(
            returned_count,
            telemetry.graph_verified_count,
            telemetry.unsafe_lifecycle_returned_count,
            telemetry.graph_object_missing_returned_count,
        );
    }
    details
}

pub fn composition_metrics(retrieved: &[crate::RetrievedItem]) -> crate::ResultCompositionMetrics {
    let total_items = retrieved.len();
    let items_with_rationale = retrieved
        .iter()
        .filter(|item| {
            item.rationale
                .iter()
                .any(|rationale| !rationale.trim().is_empty())
        })
        .count();
    let entities = count_kind(retrieved, "entity");
    crate::ResultCompositionMetrics {
        total_items,
        episodes: count_kind(retrieved, "episode"),
        observations: count_kind(retrieved, "observation"),
        derived_memories: count_kind(retrieved, "derived_memory"),
        memory_threads: count_kind(retrieved, "memory_thread"),
        entities: (entities > 0).then_some(entities),
        items_with_rationale,
        rationale_coverage: if total_items == 0 {
            Some(1.0)
        } else {
            Some(items_with_rationale as f64 / total_items as f64)
        },
    }
}

fn count_kind(retrieved: &[crate::RetrievedItem], kind: &str) -> usize {
    retrieved.iter().filter(|item| item.kind == kind).count()
}

fn option_f64(value: Option<f64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn option_usize(value: Option<usize>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn leakage_rate(count: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        count as f64 / denominator as f64
    }
}

fn context_validation_pass_rate(
    returned_count: usize,
    graph_verified_count: Option<usize>,
    unsafe_lifecycle_returned_count: Option<usize>,
    graph_object_missing_returned_count: Option<usize>,
) -> Option<f64> {
    let graph_verified_count = graph_verified_count?;
    let unsafe_lifecycle_returned_count = unsafe_lifecycle_returned_count?;
    let graph_object_missing_returned_count = graph_object_missing_returned_count?;
    if returned_count == 0 {
        return Some(1.0);
    }
    let graph_unverified = returned_count.saturating_sub(graph_verified_count.min(returned_count));
    let invalid_count =
        graph_unverified + unsafe_lifecycle_returned_count + graph_object_missing_returned_count;
    Some(1.0 - invalid_count.min(returned_count) as f64 / returned_count as f64)
}

fn rate(count: usize, denominator: usize) -> Value {
    if denominator == 0 {
        Value::Null
    } else {
        Value::from(count as f64 / denominator as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_ranking_metrics() {
        let retrieved = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let gold = vec!["b".to_string(), "c".to_string()];
        let metrics = retrieval_metrics(&retrieved, &gold, 3).unwrap();
        assert_eq!(metrics.recall_any, 1.0);
        assert_eq!(metrics.recall_all, 1.0);
        assert_eq!(metrics.recall_fraction, 1.0);
        assert_eq!(metrics.mrr, 0.5);
        assert!(metrics.ndcg > 0.0);
    }

    #[test]
    fn skips_empty_gold() {
        assert!(retrieval_metrics(&["a".to_string()], &[], 10).is_none());
    }

    #[test]
    fn inserts_integrity_metrics_with_nulls_for_unsupported_states() {
        let mut out = Map::new();
        insert_integrity_metrics(
            &mut out,
            &[crate::RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "i".to_string(),
                external_id: None,
                episode_external_id: Some("e".to_string()),
                score: None,
                rank: 1,
                rationale: vec![],
                text: None,
            }],
        );

        assert_eq!(out["returned_items_without_external_id"], 1);
        assert!(out["suppressed_or_deleted_items_returned"].is_null());
    }

    #[test]
    fn telemetry_backed_integrity_metrics_are_numeric_when_trace_exists() {
        let retrieved = vec![
            crate::RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "o1".to_string(),
                external_id: Some("o1".to_string()),
                episode_external_id: Some("e1".to_string()),
                score: None,
                rank: 1,
                rationale: vec!["matched".to_string()],
                text: None,
            },
            crate::RetrievedItem {
                kind: "derived_memory".to_string(),
                internal_id: "d1".to_string(),
                external_id: Some("d1".to_string()),
                episode_external_id: Some("e1".to_string()),
                score: None,
                rank: 2,
                rationale: vec!["expanded".to_string()],
                text: None,
            },
        ];
        let telemetry = crate::RetrievalTelemetry {
            trace_available: true,
            graph_verified_count: Some(2),
            suppressed_or_deleted_returned_count: Some(0),
            superseded_current_returned_count: Some(0),
            unsafe_lifecycle_returned_count: Some(0),
            graph_object_missing_omitted_count: Some(3),
            graph_object_missing_returned_count: Some(0),
            ..crate::RetrievalTelemetry::default()
        };

        let integrity = integrity_details_with_telemetry(&retrieved, &telemetry);
        let mut out = Map::new();
        insert_integrity_detail_metrics(&mut out, &integrity);

        assert_eq!(out["context_validation_pass_rate"], 1.0);
        assert_eq!(out["suppressed_memory_leakage_rate"], 0.0);
        assert_eq!(out["superseded_current_leakage_rate"], 0.0);
        assert_eq!(out["orphan_vector_leakage_rate"], 0.0);
        assert_eq!(out["suppressed_or_deleted_items_returned"], 0);
        assert_eq!(out["superseded_items_returned_as_current"], 0);
        assert!(out["cross_store_id_validation_pass_rate"].is_null());
    }

    #[test]
    fn context_validation_rate_accounts_for_lifecycle_leakage() {
        let retrieved = vec![
            crate::RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "o1".to_string(),
                external_id: Some("o1".to_string()),
                episode_external_id: Some("e1".to_string()),
                score: None,
                rank: 1,
                rationale: vec!["matched".to_string()],
                text: None,
            },
            crate::RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "o2".to_string(),
                external_id: Some("o2".to_string()),
                episode_external_id: Some("e1".to_string()),
                score: None,
                rank: 2,
                rationale: vec!["matched".to_string()],
                text: None,
            },
        ];
        let telemetry = crate::RetrievalTelemetry {
            trace_available: true,
            graph_verified_count: Some(2),
            suppressed_or_deleted_returned_count: Some(1),
            superseded_current_returned_count: Some(0),
            unsafe_lifecycle_returned_count: Some(1),
            graph_object_missing_omitted_count: Some(0),
            graph_object_missing_returned_count: Some(0),
            ..crate::RetrievalTelemetry::default()
        };

        let integrity = integrity_details_with_telemetry(&retrieved, &telemetry);

        assert_eq!(integrity.context_validation_pass_rate, Some(0.5));
        assert_eq!(integrity.suppressed_memory_leakage_rate, Some(0.5));
    }

    #[test]
    fn metric_support_preserves_all_null_metrics() {
        let rows = vec![
            Map::from_iter([("unsupported_metric".to_string(), Value::Null)]),
            Map::from_iter([("unsupported_metric".to_string(), Value::Null)]),
        ];

        let support = metric_support_summary(&rows);
        assert_eq!(support["unsupported_metric"]["rows_present"], 2);
        assert_eq!(support["unsupported_metric"]["numeric_rows"], 0);
        assert_eq!(support["unsupported_metric"]["null_rows"], 2);
        assert_eq!(support["unsupported_metric"]["unsupported"], true);
    }

    #[test]
    fn runtime_registry_combines_core_and_dataset_families() {
        let family = retrieval_metric_family("example_retrieval", [("dialog", [3].as_slice())]);
        let mut metrics = Map::new();
        metrics.insert("dialog_ndcg@3".to_string(), Value::from(0.75));
        initialize_registry_metrics_for(&mut metrics, std::slice::from_ref(&family));

        assert_eq!(metrics["dialog_ndcg@3"], 0.75);
        assert!(metrics["dialog_mrr@3"].is_null());
        assert!(metrics["retrieved_context_tokens"].is_null());
        assert!(!metrics.contains_key("session_ndcg@5"));
        assert!(!metrics.contains_key("retrieval_latency_ms"));

        let coverage = registry_coverage_summary_for(&[metrics], &[family]);
        assert_eq!(
            coverage["required_metrics_total"],
            CORE_BASE_METRICS.len() + RANKING_METRIC_NAMES.len()
        );
        assert_eq!(coverage["required_metrics_numeric"], 1);
        assert_eq!(
            coverage["required_metrics_null_only"],
            CORE_BASE_METRICS.len() + RANKING_METRIC_NAMES.len() - 1
        );
        assert_eq!(coverage["missing_required_metrics"], serde_json::json!([]));
    }
}
