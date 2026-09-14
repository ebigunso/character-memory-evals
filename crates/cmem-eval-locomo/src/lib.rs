//! LoCoMo loading, ingestion and scoring.
//!
//! [`load_value`] admits a non-empty array, either at the root or under `data`,
//! `samples` or `items`. [`load_path`] also distinguishes I/O and JSON syntax errors
//! from structural and identity defects through [`LoadError`]. Admission errors
//! name the file root or the item index/available ID and the offending field.
//!
//! Each sample needs a non-blank string `sample_id` (alias `id`), unique in the
//! file, and at least one session under `conversation` (alias `conversations`):
//! - An array contains session objects with a non-blank string `session_id`
//!   (aliases `session`, `id`), unique within the sample, and a non-empty turn
//!   array under `turns`, `dialog` or `conversation`. Record IDs are opaque strings;
//!   `session_number` is not an identity source.
//! - A keyed object contains non-empty turn arrays named `session_<N>`, where
//!   `<N>` contains only decimal digits, with no leading zero except `0`. Sessions
//!   are ordered numerically. Other `session_` keys are rejected unless they are
//!   canonical `session_<N>_date_time` annotations; orphan date annotations are
//!   ignored. This canonical-key restriction does not apply to array record IDs.
//!
//! Every turn must be an object with a non-blank string `dia_id` (aliases
//! `dialog_id`, `id`), unique within the sample, and string `text` (aliases
//! `content`, `utterance`). Empty text is admitted. Each sample also needs a
//! non-empty `qa` array of objects with non-blank string `question` (alias `q`).
//! A non-blank string `question_id` (aliases `qid`, `id`) is retained; otherwise
//! the loader derives `<sample_id>:qa:<one-based position>`. Effective QA IDs
//! must be unique across the file.
//!
//! Answers (`answer`/`a`), question types (`question_type`/`category`/`type`),
//! speakers (`speaker`/`role`), dates, images, queries, summaries and observations
//! remain optional annotations. Record dates use `timestamp`/`date`/
//! `session_timestamp`; their raw text is retained alongside normalized timestamps.
//! Record summaries use `session_summary`/`summary`, and observations use
//! `observation`/`observations`/`generated_observations`. Top-level
//! `session_summary` and `observation` maps first look up the exact session ID,
//! then its `session_` suffix parsed as `usize` and rendered as decimal (so record
//! IDs `session_01` and `session_+1` can look up key `1`). Record summaries take
//! precedence; top-level observations append to record observations.
//! QA `evidence` (alias `evidence_dialog_ids`) admits scalar references and arrays
//! of scalars or objects using `dia_id`/`dialog_id`/`id`. References to absent
//! turns remain annotations and do not cause rejection. Missing annotations do
//! not determine abstention status.

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
