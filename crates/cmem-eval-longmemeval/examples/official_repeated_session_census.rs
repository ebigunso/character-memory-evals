use anyhow::{Context, Result};
use cmem_eval_longmemeval::{ingest::to_memory_inputs, load_path};
use serde_json::json;
use std::collections::HashSet;

fn main() -> Result<()> {
    let path = std::env::var_os("LONGMEMEVAL_DATASET").context("set LONGMEMEVAL_DATASET")?;
    let output =
        std::env::var_os("LONGMEMEVAL_IDENTITY_DUMP").context("set LONGMEMEVAL_IDENTITY_DUMP")?;
    let rows = load_path(std::path::Path::new(&path))?;
    let mut dump = Vec::new();
    let mut changed = Vec::new();
    let mut global_identities = HashSet::new();
    for item in &rows {
        let mapped = to_memory_inputs(item);
        let mut raw_seen = std::collections::HashMap::new();
        let mut assigned_seen = HashSet::new();
        for (session, episode) in item.sessions.iter().zip(&mapped.episodes) {
            assert!(!session.session_id.contains('#'));
            assert!(assigned_seen.insert(&episode.external_id));
            assert!(global_identities.insert((
                episode.namespace.clone(),
                "episode",
                episode.external_id.clone()
            )));
            assert_eq!(episode.started_at, session.date);
            if let Some(previous) = raw_seen.insert(&session.session_id, session) {
                assert_eq!(
                    serde_json::to_value(&previous.turns).unwrap(),
                    serde_json::to_value(&session.turns).unwrap()
                );
                assert_ne!(previous.date, session.date);
                assert!(!item.answer_session_ids.contains(&session.session_id));
                assert_eq!(episode.external_id, format!("{}#2", session.session_id));
                changed.push(item.question_id.clone());
            } else {
                assert_eq!(episode.external_id, session.session_id);
            }
            dump.push(
                json!({"item": item.question_id, "before": session.session_id,
                "after": episode.external_id, "date": episode.started_at}),
            );
        }
        for observation in &mapped.observations {
            assert!(global_identities.insert((
                observation.namespace.clone(),
                "observation",
                observation.external_id.clone()
            )));
        }
        assert_eq!(
            mapped
                .observations
                .iter()
                .map(|o| &o.external_id)
                .collect::<HashSet<_>>()
                .len(),
            mapped.observations.len()
        );
    }
    let output = std::path::PathBuf::from(output);
    let file = std::fs::File::create_new(&output)
        .with_context(|| format!("create {}", output.display()))?;
    serde_json::to_writer(file, &dump)?;
    println!(
        "items={} episodes={} changed_copies={} changed_item_count={} changed_items={}",
        rows.len(),
        dump.len(),
        changed.len(),
        changed.iter().collect::<HashSet<_>>().len(),
        serde_json::to_string(&changed)?
    );
    Ok(())
}
