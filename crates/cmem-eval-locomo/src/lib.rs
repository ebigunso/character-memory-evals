pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use loader::{load_path, load_value};
pub use types::*;

use anyhow::{Result, bail};
use cmem_eval_core::{BenchmarkRunConfig, MetricFamily, MetricsConfig, retrieval_metric_family};

pub fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
    if config.dataset.as_str() != "locomo" {
        bail!(
            "config dataset {:?} does not match selected locomo pipeline",
            config.dataset
        );
    }
    Ok(())
}

pub fn metric_family(config: &MetricsConfig) -> MetricFamily {
    retrieval_metric_family(
        "locomo_retrieval",
        [
            ("dialog", config.ks_dialog.as_slice()),
            ("session", config.ks_session.as_slice()),
        ],
    )
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

    #[test]
    fn validates_its_own_dataset_name() {
        let valid: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "locomo"
        }))
        .unwrap();
        validate_config(&valid).unwrap();

        let invalid: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "longmemeval_s"
        }))
        .unwrap();
        let error = validate_config(&invalid).unwrap_err().to_string();
        assert!(error.contains("locomo pipeline"), "{error}");
        assert!(error.contains("longmemeval_s"), "{error}");
    }
}
