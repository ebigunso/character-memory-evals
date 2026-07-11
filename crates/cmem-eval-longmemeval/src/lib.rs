pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use loader::{load_path, load_value};
pub use types::*;

use anyhow::{Result, bail};
use cmem_eval_core::BenchmarkRunConfig;

pub fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
    if config.dataset != "longmemeval_s" {
        bail!(
            "config dataset {:?} does not match selected longmemeval_s pipeline",
            config.dataset
        );
    }
    Ok(())
}

pub fn full_history_text(instance: &LongMemEvalInstance) -> String {
    instance
        .sessions
        .iter()
        .flat_map(|session| {
            session.turns.iter().map(|turn| {
                format!(
                    "{}: {}",
                    turn.speaker.as_deref().unwrap_or("unknown"),
                    turn.text
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod dataset_spec_tests {
    use super::*;

    #[test]
    fn config_validation_is_owned_by_the_dataset_crate() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "longmemeval_s"
        }))
        .unwrap();
        validate_config(&config).unwrap();
        config.dataset = "locomo".to_string();
        assert!(validate_config(&config).is_err());
    }
}
