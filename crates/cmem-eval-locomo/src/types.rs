use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        self.evidence_session_index().evidence_sessions(qa)
    }

    pub fn evidence_session_index(&self) -> LoCoMoEvidenceSessionIndex {
        LoCoMoEvidenceSessionIndex::new(self)
    }
}

#[derive(Debug, Clone)]
pub struct LoCoMoEvidenceSessionIndex {
    dialog_to_sessions: HashMap<String, Vec<String>>,
}

impl LoCoMoEvidenceSessionIndex {
    pub fn new(sample: &LoCoMoSample) -> Self {
        let mut dialog_to_sessions: HashMap<String, Vec<String>> = HashMap::new();
        for session in &sample.sessions {
            for turn in &session.turns {
                dialog_to_sessions
                    .entry(turn.dialog_id.clone())
                    .or_default()
                    .push(session.session_id.clone());
            }
        }
        for sessions in dialog_to_sessions.values_mut() {
            sessions.sort();
            sessions.dedup();
        }
        Self { dialog_to_sessions }
    }

    pub fn evidence_sessions(&self, qa: &LoCoMoQa) -> Vec<String> {
        let mut sessions = qa
            .evidence_dialog_ids
            .iter()
            .filter_map(|dialog_id| self.dialog_to_sessions.get(dialog_id))
            .flat_map(|sessions| sessions.iter().cloned())
            .collect::<Vec<_>>();
        sessions.sort();
        sessions.dedup();
        sessions
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoCoMoSession {
    pub session_id: String,
    pub raw_timestamp: Option<String>,
    pub timestamp: Option<String>,
    pub summary: Option<String>,
    pub generated_observations: Vec<String>,
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
    pub derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
}
