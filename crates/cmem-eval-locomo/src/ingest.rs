use crate::{LoCoMoMemoryInputs, LoCoMoSample};
use cmem_eval_core::{DerivedMemoryInput, EpisodeInput, ObservationInput};

pub fn to_memory_inputs(
    sample: &LoCoMoSample,
    include_image_captions: bool,
    index_session_summaries: bool,
    index_generated_observations: bool,
) -> LoCoMoMemoryInputs {
    let namespace = sample.namespace();
    let mut episodes = Vec::new();
    let mut observations = Vec::new();
    let mut derived_memories = Vec::new();
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
                "include_image_captions": include_image_captions,
                "raw_timestamp": session.raw_timestamp.clone(),
                "normalized_timestamp": session.timestamp.clone()
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
                    "query": turn.query,
                    "raw_timestamp": session.raw_timestamp.clone(),
                    "normalized_timestamp": session.timestamp.clone()
                }),
            });
        }
        if index_session_summaries {
            if let Some(summary) = session
                .summary
                .as_deref()
                .map(str::trim)
                .filter(|summary| !summary.is_empty())
            {
                derived_memories.push(DerivedMemoryInput {
                    external_id: format!("{}:derived:session_summary", session.session_id),
                    derived_type: "reflection".to_string(),
                    text: summary.to_string(),
                    source_episode_external_ids: vec![session.session_id.clone()],
                    source_observation_external_ids: vec![],
                    thread_external_ids: vec![],
                    entity_external_ids: vec![],
                    confidence: 1.0,
                    salience_score: 0.6,
                    stability: "medium".to_string(),
                    is_current: true,
                    supersedes_external_ids: vec![],
                    metadata: serde_json::json!({
                        "source": "locomo",
                        "source_field": "session_summary"
                    }),
                });
            }
        }
        if index_generated_observations {
            for (idx, observation) in session.generated_observations.iter().enumerate() {
                let observation = observation.trim();
                if observation.is_empty() {
                    continue;
                }
                derived_memories.push(DerivedMemoryInput {
                    external_id: format!(
                        "{}:derived:generated_observation:{}",
                        session.session_id,
                        idx + 1
                    ),
                    derived_type: "claim".to_string(),
                    text: observation.to_string(),
                    source_episode_external_ids: vec![session.session_id.clone()],
                    source_observation_external_ids: vec![],
                    thread_external_ids: vec![],
                    entity_external_ids: vec![],
                    confidence: 1.0,
                    salience_score: 0.6,
                    stability: "medium".to_string(),
                    is_current: true,
                    supersedes_external_ids: vec![],
                    metadata: serde_json::json!({
                        "source": "locomo",
                        "source_field": "observation"
                    }),
                });
            }
        }
    }
    LoCoMoMemoryInputs {
        episodes,
        observations,
        derived_memories,
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
        let mapped = to_memory_inputs(&rows[0], false, false, false);
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

        let without_caption = to_memory_inputs(&rows[0], false, false, false);
        assert_eq!(without_caption.observations[0].text, "look at this");

        let with_caption = to_memory_inputs(&rows[0], true, false, false);
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

    #[test]
    fn preserves_raw_and_normalized_timestamps_without_evidence() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "session_1_date_time": "1:56 pm on 8 May, 2023",
                "session_1": [{"dia_id": "D1:1", "speaker": "A", "text": "hello"}]
            },
            "qa": [{"question": "q", "answer": "a", "evidence": ["D1:1"]}]
        }]))
        .unwrap();

        let mapped = to_memory_inputs(&rows[0], false, false, false);
        assert_eq!(
            mapped.episodes[0].started_at.as_deref(),
            Some("2023-05-08T13:56:00Z")
        );
        let metadata = serde_json::to_string(&mapped.observations[0].metadata).unwrap();
        assert!(metadata.contains("1:56 pm on 8 May, 2023"));
        assert!(metadata.contains("2023-05-08T13:56:00Z"));
        assert!(!metadata.contains("evidence"));
        assert!(!metadata.contains("answer"));
    }

    #[test]
    fn session_summaries_and_generated_observations_become_derived_memories() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "session_1": [{"dia_id": "D1:1", "speaker": "A", "text": "hello"}]
            },
            "session_summary": {
                "session_1": "They discussed a trip."
            },
            "observation": {
                "session_1": ["A likes quiet cafes."]
            },
            "qa": [{"question": "q", "answer": "a", "evidence": ["D1:1"]}]
        }]))
        .unwrap();

        let mapped = to_memory_inputs(&rows[0], false, true, true);
        assert_eq!(mapped.derived_memories.len(), 2);
        assert_eq!(
            mapped.derived_memories[0].source_episode_external_ids,
            vec!["session_1"]
        );
        let serialized = serde_json::to_string(&mapped.derived_memories).unwrap();
        assert!(!serialized.contains("evidence"));
        assert!(!serialized.contains("answer"));
    }
}
