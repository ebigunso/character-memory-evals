use crate::{LoCoMoMemoryInputs, LoCoMoSample};
use cmem_eval_core::{EpisodeInput, ObservationInput};

pub fn to_memory_inputs(sample: &LoCoMoSample, include_image_captions: bool) -> LoCoMoMemoryInputs {
    let namespace = sample.namespace();
    let mut episodes = Vec::new();
    let mut observations = Vec::new();
    for session in &sample.sessions {
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
            summary: session.summary.clone().unwrap_or_else(|| {
                format!(
                    "Conversation session {} containing messages between {}.",
                    session.session_id,
                    participants.join(", ")
                )
            }),
            started_at: session.timestamp.clone(),
            ended_at: session.timestamp.clone(),
            participants,
            metadata: serde_json::json!({
                "source": "locomo",
                "include_image_captions": include_image_captions
            }),
        });
        for turn in &session.turns {
            observations.push(ObservationInput {
                external_id: turn.dialog_id.clone(),
                episode_external_id: session.session_id.clone(),
                namespace: namespace.clone(),
                speaker: turn.speaker.clone(),
                text: turn.text.clone(),
                observed_at: session.timestamp.clone(),
                metadata: serde_json::json!({"source": "locomo"}),
            });
        }
    }
    LoCoMoMemoryInputs {
        episodes,
        observations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_value;

    #[test]
    fn qa_evidence_is_not_ingested() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": [{"session_id": "s1", "turns": [{"dia_id": "d1", "text": "answer"}]}],
            "qa": [{"question_id": "q1", "question": "q", "evidence": ["d1"]}]
        }]))
        .unwrap();
        let mapped = to_memory_inputs(&rows[0], false);
        let metadata = serde_json::to_string(&mapped.observations[0].metadata).unwrap();
        assert!(!metadata.contains("evidence"));
    }
}
