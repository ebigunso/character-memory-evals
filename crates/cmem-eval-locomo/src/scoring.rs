use crate::{LoCoMoQa, LoCoMoSample};
use cmem_eval_core::{RetrievedItem, insert_retrieval_metrics};
use serde_json::{Map, Value};

pub fn score(sample: &LoCoMoSample, qa: &LoCoMoQa, items: &[RetrievedItem]) -> Value {
    let dialog_ids = items
        .iter()
        .filter(|item| item.kind == "observation")
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let session_ids = items
        .iter()
        .filter(|item| item.kind == "episode")
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let gold_sessions = sample.evidence_sessions(qa);
    let mut out = Map::new();
    for k in [5, 10] {
        insert_retrieval_metrics(&mut out, "dialog", &dialog_ids, &qa.evidence_dialog_ids, k);
        insert_retrieval_metrics(&mut out, "session", &session_ids, &gold_sessions, k);
    }
    Value::Object(out)
}
