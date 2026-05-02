use crate::{LoCoMoQa, LoCoMoSample};
use cmem_eval_core::{RetrievedItem, insert_retrieval_metrics};
use serde_json::{Map, Value};

pub fn score(
    sample: &LoCoMoSample,
    qa: &LoCoMoQa,
    items: &[RetrievedItem],
    ks_dialog: &[usize],
    ks_session: &[usize],
) -> Value {
    let gold_sessions = sample.evidence_sessions(qa);
    score_with_gold_sessions(qa, &gold_sessions, items, ks_dialog, ks_session)
}

pub fn score_with_gold_sessions(
    qa: &LoCoMoQa,
    gold_sessions: &[String],
    items: &[RetrievedItem],
    ks_dialog: &[usize],
    ks_session: &[usize],
) -> Value {
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
    let mut out = Map::new();
    for k in ks_dialog {
        insert_retrieval_metrics(&mut out, "dialog", &dialog_ids, &qa.evidence_dialog_ids, *k);
    }
    for k in ks_session {
        insert_retrieval_metrics(&mut out, "session", &session_ids, gold_sessions, *k);
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_value;

    #[test]
    fn uses_configured_metric_ks() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": [{"session_id": "s1", "turns": [{"dia_id": "d1", "text": "answer"}]}],
            "qa": [{"question": "q", "evidence": ["d1"]}]
        }]))
        .unwrap();
        let qa = &rows[0].qa[0];
        let metrics = score(
            &rows[0],
            qa,
            &[RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "i".to_string(),
                external_id: Some("d1".to_string()),
                episode_external_id: Some("s1".to_string()),
                score: None,
                rank: 1,
                rationale: vec![],
                text: None,
            }],
            &[4],
            &[6],
        );

        assert!(metrics.get("dialog_recall_any@4").is_some());
        assert!(metrics.get("dialog_recall_any@5").is_none());
        assert!(metrics.get("session_recall_any@6").is_some());
        assert!(metrics.get("session_recall_any@5").is_none());
    }
}
