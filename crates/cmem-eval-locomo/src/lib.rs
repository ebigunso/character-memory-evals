pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use loader::{load_path, load_value};
pub use types::*;

use anyhow::{Result, bail};
use cmem_eval_core::{BenchmarkRunConfig, MetricFamily, MetricsConfig, retrieval_metric_family};

pub fn metric_family(config: &MetricsConfig) -> MetricFamily {
    retrieval_metric_family(
        "locomo_retrieval",
        [
            ("dialog", config.ks_dialog.as_slice()),
            ("session", config.ks_session.as_slice()),
        ],
    )
}

pub fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
    if config.dataset != "locomo" {
        bail!(
            "config dataset {:?} does not match selected locomo pipeline",
            config.dataset
        );
    }
    Ok(())
}

pub fn full_history_text(sample: &LoCoMoSample) -> String {
    sample
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
            "dataset": "locomo"
        }))
        .unwrap();
        validate_config(&config).unwrap();
        config.dataset = "longmemeval_s".to_string();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn declares_configured_retrieval_metric_family() {
        let config = MetricsConfig {
            ks_session: vec![3],
            ks_dialog: vec![7],
            ..MetricsConfig::default()
        };
        let family = metric_family(&config);
        assert!(family.required_metrics.contains("session_ndcg@3"));
        assert!(family.required_metrics.contains("dialog_recall_fraction@7"));
        assert!(!family.required_metrics.contains("turn_ndcg@3"));
    }
}
