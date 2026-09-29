//! LongMemEval-S loading, ingestion and scoring.
//!
//! [`load_value`] admits a non-empty root array. [`load_path`] distinguishes I/O
//! and JSON syntax errors from structural and identity defects through [`LoadError`].
//! Admission errors name the file root or item index/available ID and field.
//!
//! Each item needs a non-blank string `question_id`, unique in the file, a
//! non-blank string `question`, and a non-empty `haystack_sessions` array of
//! non-empty turn arrays. Every turn must be an object with string `content`;
//! empty text is admitted. Turn IDs use their one-based position in the session.
//! The required `haystack_session_ids` array must match the session count and
//! contain a non-blank string in every slot. Repeated IDs require identical raw
//! turn arrays, including `has_answer` labels. Admitted repeats retain every
//! copy and its annotations; dates do not enter the comparison.
//!
//! `haystack_dates` is optional: absent or null is admitted; otherwise it must be
//! an array matching the session count. Each null or non-string slot means no
//! date at that position, without shifting later dates. Raw dates are retained
//! alongside normalized timestamps. `question_type`, `answer`, `question_date`,
//! `role` and `answer_session_ids` remain optional. `has_answer` defaults to false
//! when absent or not boolean; references to absent sessions do not reject the
//! item. Optional annotations do not determine abstention status.
//!
//! Ingest assigns each occurrence its own episode identity: the first keeps the
//! raw benchmark session ID, later copies receive `#2`, `#3`, ... while skipping
//! all raw IDs and previously assigned IDs in the item. Observation identities
//! follow the assigned episode; dates remain per occurrence and text uses the
//! raw ID. Scoring recomputes the same identity tables from the item to map both
//! session and turn rankings back to benchmark IDs, crediting each ID only at
//! its first retrieved rank. Retrieved row IDs are assigned; gold IDs are raw.

mod error;
mod identity;
pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use error::{AdmissionLocation, LoadError};
pub use loader::{load_path, load_value};
pub use types::*;

use anyhow::Result;
use cmem_eval::{BenchmarkRunConfig, MetricFamily, MetricsConfig, retrieval_metric_family};

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    DatasetMismatch {
        expected: &'static str,
        found: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DatasetMismatch { expected, found } => {
                write!(
                    f,
                    "config dataset {found:?} does not match selected {expected} pipeline"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
    if config.dataset.as_str() != "longmemeval_s" {
        return Err(ConfigError::DatasetMismatch {
            expected: "longmemeval_s",
            found: config.dataset.to_string(),
        }
        .into());
    }
    Ok(())
}

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

    #[test]
    fn validates_its_own_dataset_name() {
        let valid: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "longmemeval_s"
        }))
        .unwrap();
        validate_config(&valid).unwrap();

        let invalid: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "locomo"
        }))
        .unwrap();
        let error = validate_config(&invalid)
            .unwrap_err()
            .downcast::<ConfigError>()
            .unwrap();
        assert_eq!(
            error,
            ConfigError::DatasetMismatch {
                expected: "longmemeval_s",
                found: "locomo".to_string(),
            }
        );
    }
}
