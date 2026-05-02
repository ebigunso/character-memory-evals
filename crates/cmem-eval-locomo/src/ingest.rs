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
            values.extend(sample.speaker_a.iter().cloned());
            values.extend(sample.speaker_b.iter().cloned());
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
                text: observation_text(turn, include_image_captions),
                observed_at: session.timestamp.clone(),
                metadata: serde_json::json!({
                    "source": "locomo",
                    "img_url": turn.image_urls,
                    "query": turn.query
                }),
            });
        }
    }
    LoCoMoMemoryInputs {
        episodes,
        observations,
    }
}

fn observation_text(turn: &crate::LoCoMoTurn, include_image_captions: bool) -> String {
    let mut text = turn.text.clone();
    if include_image_captions {
        if let Some(caption) = turn
            .blip_caption
            .as_deref()
            .map(str::trim)
            .filter(|caption| !caption.is_empty())
        {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("Image caption: ");
            text.push_str(caption);
        }
    }
    text
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
        assert!(!mapped.observations[0].text.contains("d1"));
    }

    #[test]
    fn image_caption_flag_controls_observation_text() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "speaker_a": "A",
                "speaker_b": "B",
                "session_1_date_time": "1:56 pm on 8 May, 2023",
                "session_1": [{
                    "dia_id": "D1:1",
                    "speaker": "A",
                    "text": "look at this",
                    "blip_caption": "a dog by a mural",
                    "query": "mural",
                    "img_url": ["https://example.test/a.jpg"]
                }]
            },
            "qa": [{"question": "q", "answer": "a", "evidence": ["D1:1"]}]
        }]))
        .unwrap();

        let without_caption = to_memory_inputs(&rows[0], false);
        assert_eq!(without_caption.observations[0].text, "look at this");

        let with_caption = to_memory_inputs(&rows[0], true);
        assert!(
            with_caption.observations[0]
                .text
                .contains("Image caption: a dog by a mural")
        );
        assert!(!with_caption.observations[0].text.contains("D1:1"));
        assert!(!with_caption.observations[0].text.contains("evidence"));

        let metadata = serde_json::to_string(&with_caption.observations[0].metadata).unwrap();
        assert!(metadata.contains("https://example.test/a.jpg"));
        assert!(metadata.contains("mural"));
        assert!(!metadata.contains("evidence"));
    }
}
