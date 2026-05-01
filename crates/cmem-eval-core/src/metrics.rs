use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

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
}
