pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use loader::{load_path, load_value};
pub use types::*;

use cmem_eval_core::{MetricFamily, MetricsConfig, retrieval_metric_family};

pub fn metric_family(config: &MetricsConfig) -> MetricFamily {
    retrieval_metric_family(
        "longmemeval_s_retrieval",
        [
            ("session", config.ks_session.as_slice()),
            ("turn", config.ks_turn.as_slice()),
        ],
    )
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
    fn declares_configured_retrieval_metric_family() {
        let config = MetricsConfig {
            ks_session: vec![3],
            ks_turn: vec![7],
            ..MetricsConfig::default()
        };
        let family = metric_family(&config);
        assert!(family.required_metrics.contains("session_ndcg@3"));
        assert!(family.required_metrics.contains("turn_recall_fraction@7"));
        assert!(!family.required_metrics.contains("dialog_ndcg@3"));
    }
}
