use anyhow::{Context, Result};
use cmem_eval::DerivedType;
use cmem_eval_locomo::{ingest::to_memory_inputs, load_path};

fn main() -> Result<()> {
    let path =
        std::env::var_os("LOCOMO_OFFICIAL_DATASET").context("set LOCOMO_OFFICIAL_DATASET")?;
    let samples = load_path(std::path::Path::new(&path))?;
    let sessions = samples
        .iter()
        .flat_map(|sample| &sample.sessions)
        .collect::<Vec<_>>();
    let observations = sessions
        .iter()
        .flat_map(|session| &session.generated_observations)
        .collect::<Vec<_>>();
    let counts = (
        sessions
            .iter()
            .filter(|session| session.summary.is_some())
            .count(),
        observations.len(),
        observations
            .iter()
            .map(|observation| observation.evidence_dialog_ids.len())
            .sum::<usize>(),
        samples
            .iter()
            .map(|sample| sample.unresolved_evidence_references)
            .sum::<usize>(),
        samples
            .iter()
            .map(|sample| sample.dropped_observation_entries)
            .sum::<usize>(),
    );
    println!(
        "summaries={} observations={} evidence_references={} unresolved={} dropped={}",
        counts.0, counts.1, counts.2, counts.3, counts.4
    );
    let inputs = samples
        .iter()
        .map(|sample| to_memory_inputs(sample, false, true, true))
        .collect::<Vec<_>>();
    let memories = inputs
        .iter()
        .flat_map(|input| &input.derived_memories)
        .collect::<Vec<_>>();
    println!(
        "derived_reflections={} derived_claims={} source_observation_references={}",
        memories
            .iter()
            .filter(|memory| memory.derived_type == DerivedType::Reflection)
            .count(),
        memories
            .iter()
            .filter(|memory| memory.derived_type == DerivedType::Claim)
            .count(),
        memories
            .iter()
            .map(|memory| memory.source_observation_external_ids.len())
            .sum::<usize>()
    );
    Ok(())
}
