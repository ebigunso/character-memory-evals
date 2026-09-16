use crate::{LoCoMoQa, LoCoMoSample};
use cmem_eval::{ObjectType, RetrievedItem, insert_retrieval_metrics};
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
        .filter(|item| item.kind == ObjectType::Observation)
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let session_ids = items
        .iter()
        .filter(|item| item.kind == ObjectType::Episode)
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
    fn rank_one_hits_and_misses_have_hand_computed_dialog_and_session_scores() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": [
                {"session_id": "session-alpha", "turns": [
                    {"dia_id": "dialog-x", "text": "first evidence"},
                    {"dia_id": "dialog-y", "text": "second evidence"}
                ]},
                {"session_id": "session-beta", "turns": [
                    {"dia_id": "dialog-z", "text": "unrelated"}
                ]}
            ],
            "qa": [{"question": "Which?", "evidence": ["dialog-x", "dialog-y"]}]
        }]))
        .unwrap();
        let sample = &rows[0];
        let qa = &sample.qa[0];
        let item = |kind, id: &str, rank| RetrievedItem {
            kind,
            internal_id: id.to_string(),
            external_id: Some(id.to_string()),
            episode_external_id: None,
            score: None,
            rank,
            rationale: vec![],
            text: None,
        };
        let hit = score(
            sample,
            qa,
            &[
                item(ObjectType::Observation, "dialog-x", 1),
                item(ObjectType::Episode, "session-alpha", 1),
                item(ObjectType::Observation, "dialog-z", 2),
                item(ObjectType::Episode, "session-beta", 2),
            ],
            &[2],
            &[2],
        );
        let miss = score(
            sample,
            qa,
            &[
                item(ObjectType::Observation, "dialog-z", 1),
                item(ObjectType::Episode, "session-beta", 1),
            ],
            &[2],
            &[2],
        );

        // One of two gold dialogs is at rank 1: DCG = 1, ideal DCG = 1 + 1/log2(3).
        // Both gold dialogs project to one session, whose rank-1 hit gives recall and nDCG = 1.
        let ideal_dialog_dcg = 1.0 + 1.0 / 3.0_f64.log2();
        for (key, expected) in [
            ("dialog_recall_any@2", 1.0),
            ("dialog_recall_fraction@2", 1.0 / 2.0),
            ("dialog_ndcg@2", 1.0 / ideal_dialog_dcg),
            ("session_recall_any@2", 1.0),
            ("session_recall_fraction@2", 1.0),
            ("session_ndcg@2", 1.0),
        ] {
            assert!(
                (hit[key].as_f64().unwrap() - expected).abs() < 1e-12,
                "{key}"
            );
            assert_eq!(miss[key], 0.0, "{key}");
        }
    }

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
                kind: ObjectType::Observation,
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
