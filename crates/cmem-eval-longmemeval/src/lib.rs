//! LongMemEval-S loading, ingestion and scoring.
//!
//! [`load_value`] admits a non-empty array, either at the root or under `data`,
//! `instances` or `questions`. [`load_path`] also distinguishes I/O and JSON
//! syntax errors from structural and identity defects through [`LoadError`].
//! Admission errors name the file root or the item index/available ID and the
//! offending field.
//!
//! Each item needs a non-blank string `question_id` (alias `id`), unique in the
//! file, a non-blank string `question`, and a non-empty `haystack_sessions` array.
//! A session is either a non-empty turn array or an object with a non-empty array
//! under `turns`, `messages` or `conversation`. Every turn must be an object with
//! string `content` (alias `text`); empty text is admitted. Turn IDs use their
//! one-based position within the session.
//!
//! Session identity comes from a non-blank string `session_id` (alias `id`) in
//! the record, falling back to the same position in `haystack_session_ids`.
//! Whenever present, that parallel ID array must match the session count and
//! contain a non-blank string in every slot, even for records with their own IDs.
//! Within an item, repeated IDs are rejected unless every occurrence gets its ID
//! from the parallel array and the raw turn arrays are identical, including
//! `has_answer` labels. Admitted repeats retain every copy and its annotations;
//! dates do not enter the comparison. Record/record and record/parallel collisions
//! are rejected even when their turns are identical.
//!
//! `haystack_dates` is optional: absent or null is admitted; otherwise it must be
//! an array matching the session count. Each null or non-string slot means no
//! date at that position, without shifting later dates. Record `date` (alias
//! `timestamp`) takes precedence over a parallel date. Raw dates are retained
//! alongside normalized timestamps. Question type (`question_type`/`type`),
//! `answer`, `question_date`, speakers (`role`/`speaker`) and `answer_session_ids`
//! remain optional. `has_answer` defaults to false when absent or not boolean;
//! answer-session references to absent sessions do not cause rejection. Optional
//! annotations do not determine abstention status.

mod error;
pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use error::{AdmissionLocation, LoadError};
pub use loader::{load_path, load_value};
pub use types::*;

use anyhow::{Result, bail};
use cmem_eval::{BenchmarkRunConfig, MetricFamily, MetricsConfig, retrieval_metric_family};

pub fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
    if config.dataset.as_str() != "longmemeval_s" {
        bail!(
            "config dataset {:?} does not match selected longmemeval_s pipeline",
            config.dataset
        );
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
        let error = validate_config(&invalid).unwrap_err().to_string();
        assert!(error.contains("longmemeval_s pipeline"), "{error}");
        assert!(error.contains("locomo"), "{error}");
    }
}
