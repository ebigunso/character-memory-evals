//! LoCoMo loading, ingestion and scoring.
//!
//! [`load_value`] admits a non-empty root array. [`load_path`] distinguishes I/O
//! and JSON syntax errors from structural and identity defects through [`LoadError`].
//! Admission errors name the file root or item index/available ID and field.
//!
//! Each sample needs a non-blank string `sample_id`, unique in the file, and a
//! keyed `conversation` object with non-empty turn arrays named `session_<N>`.
//! `<N>` contains decimal digits, with no leading zero except `0`. Sessions are
//! ordered numerically. Other `session_` keys are rejected unless they are
//! canonical `session_<N>_date_time` annotations; orphan dates are ignored.
//! Every turn must have a non-blank string `dia_id`, unique within the sample,
//! and string `text`; empty text is admitted.
//!
//! A non-empty `qa` array contains objects with non-blank string `question`.
//! Optional non-blank `question_id` is retained; otherwise the loader derives
//! `<sample_id>:qa:<one-based position>`. Effective QA IDs are unique across the file.
//! `answer`, `category`, `speaker`, `img_url`, `blip_caption`, `query` and dates
//! remain optional annotations. Raw dates are retained alongside normalized timestamps.
//! QA `evidence` admits scalar references and arrays of scalars or objects with
//! `dia_id`. References to absent turns do not reject the sample. Missing
//! annotations do not determine abstention status.
//!
//! Top-level `session_summary` and `observation` maps use `session_<N>_summary`
//! and `session_<N>_observation` keys. Observation maps contain speaker-keyed
//! lists of `[statement, evidence]` pairs; evidence is a string or string list.
//! Exact dialog IDs resolve before comma splitting. Bare statements carry no
//! evidence. Malformed observations and unresolved references are counted
//! without rejecting the sample. Non-string summaries are ignored.
//! Ingest emits summaries as Reflections and observations as Claims with episode
//! and resolved observation provenance. Speaker metadata is retained in the
//! input; the adapter does not persist it. Both baseline modes reject
//! derived-content/enrichment configuration and use generic descriptive episode
//! text rather than dataset summaries.

mod error;
pub mod ingest;
pub mod loader;
pub mod scoring;
pub mod types;

pub use error::{AdmissionLocation, LoadError};
pub use loader::{load_path, load_value};
pub use types::*;

use anyhow::{Result, bail};
use cmem_eval::{
    BenchmarkRunConfig, MetricFamily, MetricsConfig, RetrievalMode, retrieval_metric_family,
};

pub fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
    if config.dataset != "locomo" {
        bail!(
            "config dataset {:?} does not match selected locomo pipeline",
            config.dataset
        );
    }
    if matches!(
        config.retrieval.mode,
        RetrievalMode::Bm25Only | RetrievalMode::VectorOnly
    ) {
        for (field, enabled) in [
            (
                "index_session_summaries",
                config.ingest.index_session_summaries,
            ),
            (
                "index_generated_observations",
                config.ingest.index_generated_observations,
            ),
            (
                "enrichment_snapshot_path",
                config.ingest.enrichment_snapshot_path.is_some(),
            ),
        ] {
            if enabled {
                bail!(
                    "LoCoMo baseline {:?} forbids ingest.{field}",
                    config.retrieval.mode
                );
            }
        }
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
        assert_eq!(
            validate_config(&invalid).unwrap_err().to_string(),
            "config dataset \"longmemeval_s\" does not match selected locomo pipeline"
        );
    }
}
