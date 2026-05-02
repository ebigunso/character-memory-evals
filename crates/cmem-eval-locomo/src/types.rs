use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoCoMoSample {
    pub sample_id: String,
    pub speaker_a: Option<String>,
    pub speaker_b: Option<String>,
    pub sessions: Vec<LoCoMoSession>,
    pub qa: Vec<LoCoMoQa>,
    pub raw: serde_json::Value,
}

impl LoCoMoSample {
    pub fn namespace(&self) -> String {
        format!("locomo:{}", self.sample_id)
    }

    pub fn evidence_sessions(&self, qa: &LoCoMoQa) -> Vec<String> {
        let mut sessions = self
            .sessions
            .iter()
            .filter(|session| {
                session
                    .turns
                    .iter()
                    .any(|turn| qa.evidence_dialog_ids.contains(&turn.dialog_id))
            })
            .map(|session| session.session_id.clone())
            .collect::<Vec<_>>();
        sessions.sort();
        sessions.dedup();
        sessions
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoCoMoSession {
    pub session_id: String,
    pub timestamp: Option<String>,
    pub summary: Option<String>,
    pub turns: Vec<LoCoMoTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoCoMoTurn {
    pub dialog_id: String,
    pub speaker: Option<String>,
    pub text: String,
    pub image_urls: Vec<String>,
    pub blip_caption: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoCoMoQa {
    pub question_id: String,
    pub qa_index: usize,
    pub question_type: Option<String>,
    pub question: String,
    pub answer: Option<String>,
    pub evidence_dialog_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoCoMoMemoryInputs {
    pub episodes: Vec<cmem_eval_core::EpisodeInput>,
    pub observations: Vec<cmem_eval_core::ObservationInput>,
}
