use crate::LongMemEvalInstance;
use cmem_eval::{ObjectType, RetrievedItem, insert_retrieval_metrics};
use serde_json::{Map, Value};
use std::collections::HashMap;

fn benchmark_ranking(
    items: &[RetrievedItem],
    kind: ObjectType,
    ids: &HashMap<String, String>,
) -> Vec<String> {
    items
        .iter()
        .filter(|item| item.kind == kind)
        .filter_map(|item| ids.get(item.external_id.as_ref()?))
        .cloned()
        .collect()
}

pub fn score(
    instance: &LongMemEvalInstance,
    items: &[RetrievedItem],
    ks_session: &[usize],
    ks_turn: &[usize],
) -> Value {
    let identities = crate::identity::Identities::new(instance);
    let gold_turn_ids = instance.gold_turn_ids();
    let session_ids = benchmark_ranking(items, ObjectType::Episode, &identities.session_ids);
    let turn_ids = benchmark_ranking(items, ObjectType::Observation, &identities.turn_ids);
    let mut out = Map::new();
    for k in ks_session {
        insert_retrieval_metrics(
            &mut out,
            "session",
            &session_ids,
            &instance.answer_session_ids,
            *k,
        );
    }
    for k in ks_turn {
        insert_retrieval_metrics(&mut out, "turn", &turn_ids, &gold_turn_ids, *k);
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
            "question_id": "q1",
            "question": "q",
            "haystack_session_ids": ["s1"],
            "haystack_sessions": [[{"role": "user", "content": "answer", "has_answer": true}]],
            "answer_session_ids": ["s1"]
        }]))
        .unwrap();
        let metrics = score(
            &rows[0],
            &[RetrievedItem {
                kind: ObjectType::Observation,
                internal_id: "i".to_string(),
                external_id: Some("s1:turn:1".to_string()),
                episode_external_id: Some("s1".to_string()),
                score: None,
                rank: 1,
                rationale: vec![],
                text: None,
            }],
            &[3],
            &[7],
        );

        assert!(metrics.get("turn_recall_any@7").is_some());
        assert!(metrics.get("turn_recall_any@10").is_none());
        assert!(metrics.get("session_recall_any@3").is_some());
        assert!(metrics.get("session_recall_any@5").is_none());
    }
}
