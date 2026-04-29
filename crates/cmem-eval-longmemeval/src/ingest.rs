use crate::{LongMemEvalInstance, LongMemEvalMemoryInputs};
use cmem_eval_core::{EpisodeInput, ObservationInput};

pub fn to_memory_inputs(instance: &LongMemEvalInstance) -> LongMemEvalMemoryInputs {
    let namespace = instance.namespace();
    let mut episodes = Vec::new();
    let mut observations = Vec::new();

    for session in &instance.sessions {
        let participants = {
            let mut values = session
                .turns
                .iter()
                .filter_map(|turn| turn.speaker.clone())
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            values
        };
        episodes.push(EpisodeInput {
            external_id: session.session_id.clone(),
            namespace: namespace.clone(),
            summary: format!(
                "Conversation session {} containing messages between {}.",
                session.session_id,
                participants.join(", ")
            ),
            started_at: session.date.clone(),
            ended_at: session.date.clone(),
            participants,
            metadata: serde_json::json!({"source": "longmemeval_s"}),
        });
        for turn in &session.turns {
            observations.push(ObservationInput {
                external_id: format!("{}:turn:{}", session.session_id, turn.index),
                episode_external_id: session.session_id.clone(),
                namespace: namespace.clone(),
                speaker: turn.speaker.clone(),
                text: turn.text.clone(),
                observed_at: session.date.clone(),
                metadata: serde_json::json!({"source": "longmemeval_s"}),
            });
        }
    }

    LongMemEvalMemoryInputs {
        episodes,
        observations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_value;

    #[test]
    fn gold_fields_are_not_ingested() {
        let rows = load_value(serde_json::json!([{
            "question_id": "q1",
            "question": "q",
            "haystack_session_ids": ["s1"],
            "haystack_sessions": [[{"role": "user", "content": "answer", "has_answer": true}]],
            "answer_session_ids": ["s1"]
        }]))
        .unwrap();
        let mapped = to_memory_inputs(&rows[0]);
        let metadata = serde_json::to_string(&mapped.observations[0].metadata).unwrap();
        assert!(!metadata.contains("has_answer"));
        assert!(!metadata.contains("answer_session"));
    }
}
