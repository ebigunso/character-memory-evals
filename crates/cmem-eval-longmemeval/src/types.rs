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
    pub raw: serde_json::Value,
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
    pub has_answer: bool,
}

#[derive(Debug, Clone)]
pub struct LongMemEvalMemoryInputs {
    pub episodes: Vec<cmem_eval_core::EpisodeInput>,
    pub observations: Vec<cmem_eval_core::ObservationInput>,
}
