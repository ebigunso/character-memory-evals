use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongMemEvalInstance {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub answer: Option<String>,
    pub question_date: Option<String>,
    pub sessions: Vec<LongMemEvalSession>,
    pub answer_session_ids: Vec<String>,
}

impl LongMemEvalInstance {
    pub fn namespace(&self) -> String {
        format!("lme:{}", self.question_id)
    }

    pub fn gold_turn_ids(&self) -> Vec<String> {
        self.sessions
            .iter()
            .flat_map(|session| {
                session
                    .turns
                    .iter()
                    .filter(|turn| turn.has_answer)
                    .map(|turn| format!("{}:turn:{}", session.session_id, turn.index))
            })
            .collect()
    }

    pub fn observation_splits(&self) -> Vec<cmem_eval::ObservationSplit> {
        if !self
            .sessions
            .iter()
            .flat_map(|session| &session.turns)
            .any(|turn| !turn.observation_parts.is_empty())
        {
            return Vec::new();
        }
        let identities = crate::identity::Identities::new(self);
        self.sessions
            .iter()
            .zip(&identities.episodes)
            .flat_map(|(session, episode)| {
                session
                    .turns
                    .iter()
                    .filter(|turn| !turn.observation_parts.is_empty())
                    .map(|turn| cmem_eval::ObservationSplit {
                        dataset_item_id: self.question_id.clone(),
                        source_observation_id: crate::identity::observation_id(
                            episode, turn.index, 0,
                        ),
                        parts: turn
                            .observation_parts
                            .iter()
                            .enumerate()
                            .map(|(index, part)| cmem_eval::ObservationSplitPart {
                                observation_id: crate::identity::observation_id(
                                    episode, turn.index, index,
                                ),
                                boundary_kind: part.boundary_kind,
                            })
                            .collect(),
                        boundary_policy: "greedy_sentence_paragraph_then_newline_whitespace_v1"
                            .into(),
                        tokenizer: "cl100k_base".into(),
                        max_input_tokens: crate::loader::OBSERVATION_TOKEN_LIMIT,
                    })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongMemEvalSession {
    pub session_id: String,
    pub raw_date: Option<String>,
    pub date: Option<String>,
    pub turns: Vec<LongMemEvalTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongMemEvalTurn {
    pub index: usize,
    pub speaker: Option<String>,
    pub text: String,
    pub observation_parts: Vec<LongMemEvalObservationPart>,
    pub has_answer: bool,
}

impl LongMemEvalTurn {
    pub fn observation_texts(&self) -> impl Iterator<Item = &str> {
        self.observation_parts
            .is_empty()
            .then_some(self.text.as_str())
            .into_iter()
            .chain(self.observation_parts.iter().map(|part| part.text.as_str()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongMemEvalObservationPart {
    pub text: String,
    pub boundary_kind: cmem_eval::ObservationBoundary,
}

#[derive(Debug, Clone)]
pub struct LongMemEvalMemoryInputs {
    pub episodes: Vec<cmem_eval::EpisodeInput>,
    pub observations: Vec<cmem_eval::ObservationInput>,
}
