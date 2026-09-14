use crate::LongMemEvalInstance;
use std::collections::{HashMap, HashSet};

/// Assign before filtering by date so visibility cannot change an identity.
pub(crate) struct Identities {
    pub episodes: Vec<String>,
    pub session_ids: HashMap<String, String>,
    pub turn_ids: HashMap<String, String>,
}

impl Identities {
    pub fn new(instance: &LongMemEvalInstance) -> Self {
        let mut used = instance
            .sessions
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<HashSet<_>>();
        let mut ordinals = HashMap::new();
        let mut identities = Self {
            episodes: Vec::new(),
            session_ids: HashMap::new(),
            turn_ids: HashMap::new(),
        };
        for session in &instance.sessions {
            let raw = &session.session_id;
            let ordinal = ordinals.entry(raw).or_insert(1);
            let assigned = if *ordinal == 1 {
                raw.clone()
            } else {
                loop {
                    let candidate = format!("{raw}#{ordinal}");
                    if used.insert(candidate.clone()) {
                        break candidate;
                    }
                    *ordinal += 1;
                }
            };
            *ordinal += 1;
            identities.session_ids.insert(assigned.clone(), raw.clone());
            for turn in &session.turns {
                identities.turn_ids.insert(
                    format!("{assigned}:turn:{}", turn.index),
                    format!("{raw}:turn:{}", turn.index),
                );
            }
            identities.episodes.push(assigned);
        }
        identities
    }
}
