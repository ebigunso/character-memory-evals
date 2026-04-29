use crate::LongMemEvalInstance;
use cmem_eval_core::{RetrievedItem, insert_retrieval_metrics};
use serde_json::{Map, Value};

pub fn score(instance: &LongMemEvalInstance, items: &[RetrievedItem]) -> Value {
    let session_ids = items
        .iter()
        .filter(|item| item.kind == "episode")
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let turn_ids = items
        .iter()
        .filter(|item| item.kind == "observation")
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let gold_turn_ids = instance.gold_turn_ids();
    let mut out = Map::new();
    for k in [5, 10] {
        insert_retrieval_metrics(
            &mut out,
            "session",
            &session_ids,
            &instance.answer_session_ids,
            k,
        );
    }
    for k in [10, 50] {
        insert_retrieval_metrics(&mut out, "turn", &turn_ids, &gold_turn_ids, k);
    }
    Value::Object(out)
}
