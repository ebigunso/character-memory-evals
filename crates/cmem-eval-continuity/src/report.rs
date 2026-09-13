use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::ContinuityQueryTrace;
use anyhow::{Context, Result};
use cmem_eval::{
    DegradationSummary, LatencySummary, MetricSupportSummary, NumericMetricSummary,
    RegistryCoverageSummary, aggregate_numeric_metrics, metric_support_summary,
    registry_coverage_summary_for, summarize_rows,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityReport {
    pub aggregate: AggregateContinuityReport,
    pub scenarios: BTreeMap<String, ScenarioContinuityReport>,
    pub tuning_observations: Vec<TuningObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateContinuityReport {
    pub degradation: DegradationSummary,
    pub query_count: usize,
    pub restart_count: usize,
    pub metrics: NumericMetricSummary,
    pub metric_support: MetricSupportSummary,
    pub registry_coverage: RegistryCoverageSummary,
    pub latency: LatencySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioContinuityReport {
    pub query_count: usize,
    pub metrics: NumericMetricSummary,
    pub metric_support: MetricSupportSummary,
    pub registry_coverage: RegistryCoverageSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TuningObservation {
    pub id: String,
    pub finding: String,
    /// Experiment-specific controls used by this measurement.
    pub measurement_regime: Value,
    /// Measurement keys depend on the tuning observation.
    pub observed: Value,
}

pub struct ContinuityReportInput<'a> {
    pub config: Value,
    pub traces: &'a [ContinuityQueryTrace],
    pub metric_family: &'a cmem_eval::MetricFamily,
}

pub fn assemble_continuity_report(input: ContinuityReportInput<'_>) -> Result<ContinuityReport> {
    let rows = input
        .traces
        .iter()
        .map(|trace| trace.result.clone())
        .collect::<Vec<_>>();
    let summary = summarize_rows(&rows, std::slice::from_ref(input.metric_family))?;
    let mut grouped = BTreeMap::<String, Vec<Map<String, Value>>>::new();
    for trace in input.traces {
        grouped
            .entry(trace.fixture_id.clone())
            .or_default()
            .push(trace.result.metrics.to_json_map());
    }
    let scenarios = grouped
        .into_iter()
        .map(|(fixture_id, metrics)| {
            (
                fixture_id,
                ScenarioContinuityReport {
                    query_count: metrics.len(),
                    metrics: aggregate_numeric_metrics(&metrics),
                    metric_support: metric_support_summary(&metrics),
                    registry_coverage: registry_coverage_summary_for(
                        &metrics,
                        std::slice::from_ref(input.metric_family),
                    ),
                },
            )
        })
        .collect();
    Ok(ContinuityReport {
        aggregate: AggregateContinuityReport {
            degradation: summary.degradation,
            query_count: summary.num_questions,
            restart_count: input
                .traces
                .iter()
                .map(|trace| trace.restart_observations.len())
                .sum(),
            metrics: summary.metrics,
            metric_support: summary.metric_support,
            registry_coverage: summary.registry_coverage,
            latency: summary.latency,
        },
        scenarios,
        tuning_observations: tuning_observation(&input.config, input.traces)
            .into_iter()
            .collect(),
    })
}

pub fn write_continuity_report(path: &Path, report: &ContinuityReport) -> Result<()> {
    let mut file = File::create_new(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, report)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_continuity_report(path: &Path) -> Result<ContinuityReport> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("deserialize continuity report {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("decode continuity report {}", path.display()))
}

fn tuning_observation(
    config: &Value,
    traces: &[ContinuityQueryTrace],
) -> Option<TuningObservation> {
    let hub_traces = traces
        .iter()
        .filter(|trace| trace.result.question_type.as_deref() == Some("recurring_hub_entity"))
        .collect::<Vec<_>>();
    let root_counter_samples = hub_traces
        .iter()
        .filter_map(|trace| {
            trace
                .result
                .retrieval_outcomes
                .iter()
                .map(|outcome| {
                    let telemetry = &outcome.rationale.telemetry;
                    (
                        telemetry.unique_graph_root_candidate_count,
                        telemetry.selected_graph_root_count,
                        telemetry.graph_root_omission_count,
                    )
                })
                .reduce(|(a, b, c), (unique, selected, omitted)| {
                    (a + unique, b + selected, c + omitted)
                })
        })
        .collect::<Vec<_>>();
    if root_counter_samples.is_empty() {
        return None;
    }
    let unique_graph_root_candidate_count = root_counter_samples
        .iter()
        .map(|(unique, _, _)| unique)
        .sum::<usize>();
    let selected_graph_root_count = root_counter_samples
        .iter()
        .map(|(_, selected, _)| selected)
        .sum::<usize>();
    let graph_root_omission_count = root_counter_samples
        .iter()
        .map(|(_, _, omitted)| omitted)
        .sum::<usize>();
    let decisions = hub_traces
        .iter()
        .flat_map(|trace| &trace.result.retrieval_outcomes)
        .filter_map(|outcome| outcome.trace.as_ref())
        .flat_map(|trace| &trace.selectivity_decisions)
        .collect::<Vec<_>>();
    let scored_count = decisions
        .iter()
        .filter(|decision| decision.score.is_some())
        .count();
    let fallback_count = decisions
        .iter()
        .filter(|decision| decision.fallback)
        .count();
    Some(TuningObservation {
        id: "entity_root_candidate_limit".to_string(),
        finding: format!(
            "Measured graph-root selection across {} recurring-hub query trace(s): {} unique candidates, {} selected roots, and {} omissions under the recorded candidate-limit regime. These root-type-neutral counters do not identify omitted object types.",
            root_counter_samples.len(),
            unique_graph_root_candidate_count,
            selected_graph_root_count,
            graph_root_omission_count,
        ),
        measurement_regime: serde_json::json!({
            "max_vector_candidates": config.pointer("/retrieval/surface_policy/max_vector_candidates"),
            "max_graph_roots": config.pointer("/retrieval/surface_policy/max_graph_roots"),
        }),
        observed: serde_json::json!({
            "root_counter_query_count": root_counter_samples.len(),
            "unique_graph_root_candidate_count": unique_graph_root_candidate_count,
            "selected_graph_root_count": selected_graph_root_count,
            "graph_root_omission_count": graph_root_omission_count,
            "selectivity_decision_count": decisions.len(),
            "scored_selectivity_count": scored_count,
            "fallback_selectivity_count": fallback_count,
        }),
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use cmem_eval::PerQuestionResult;
    use uuid::Uuid;

    use super::*;

    fn hub_trace(counters: Option<(usize, usize, usize)>) -> ContinuityQueryTrace {
        let outcomes = counters
            .map(|(unique, selected, omitted)| {
                let mut rationale = cmem_eval::character_memory::RetrievalRationale::new("test");
                rationale.telemetry.unique_graph_root_candidate_count = unique;
                rationale.telemetry.selected_graph_root_count = selected;
                rationale.telemetry.graph_root_omission_count = omitted;
                cmem_eval::RetrieveOutcome {
                    pack: cmem_eval::character_memory::ContinuityContextPack::empty(),
                    rationale,
                    trace: Some(cmem_eval::RetrievalTrace::empty()),
                }
            })
            .into_iter()
            .collect();
        ContinuityQueryTrace {
            result: PerQuestionResult {
                run_id: "test".into(),
                question_id: "query-hub".into(),
                question_type: Some("recurring_hub_entity".into()),
                question: "hub?".into(),
                gold_episode_ids: Vec::new(),
                gold_observation_ids: Vec::new(),
                retrieved: Vec::new(),
                context_text: String::new(),
                write_outcomes: Vec::new(),
                link_outcomes: Vec::new(),
                lifecycle_outcomes: Vec::new(),
                metrics: cmem_eval::MetricsRecord::default(),
                latency_ms: 0,
                context_char_count: 0,
                context_word_count: 0,
                context: Default::default(),
                retrieval_outcomes: outcomes,
                composition: Default::default(),
                integrity: Default::default(),
            },
            fixture_id: "recurring-hub-entity".into(),
            namespace: "continuity:hub".into(),
            event_id: "query-hub".into(),
            timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            expected: crate::ExpectedRelevanceRecord {
                relevant_external_ids: vec!["relevant".into()],
                irrelevant_external_ids: vec!["negative".into()],
            },
            history_text: String::new(),
            restart_observations: Vec::new(),
        }
    }

    #[test]
    fn tuning_observation_uses_measured_root_counters() {
        use cmem_eval::character_memory::{
            MemoryObjectRef, ObjectType, RelationType, SelectivityCountScope, SelectivityDecision,
            SelectivityTrace,
        };
        let mut trace = hub_trace(Some((21, 12, 9)));
        let outcomes = [false, true].map(|fallback| {
            let [mut outcome] = trace.result.retrieval_outcomes.clone().try_into().unwrap();
            outcome
                .trace
                .as_mut()
                .unwrap()
                .selectivity_decisions
                .push(SelectivityTrace {
                    root: MemoryObjectRef::new(ObjectType::Entity, Uuid::nil()),
                    relation: RelationType::Mentions,
                    object_type: ObjectType::Episode,
                    count_scope: SelectivityCountScope::Current,
                    score: (!fallback).then_some(0.5),
                    entity_count: Some(1),
                    global_count: Some(2),
                    support_factor: 1.0,
                    chosen_fanout: 4,
                    max_fanout: 16,
                    decision: if fallback {
                        SelectivityDecision::ConservativeFallback
                    } else {
                        SelectivityDecision::HighSelectivity
                    },
                    fallback,
                });
            outcome
        });
        trace.result.retrieval_outcomes = outcomes.to_vec();
        let observation = tuning_observation(
            &serde_json::json!({
                "retrieval": {
                    "max_vector_candidates": 48,
                    "max_graph_roots": 12
                }
            }),
            &[trace.clone()],
        )
        .unwrap();

        assert_eq!(observation.id, "entity_root_candidate_limit");
        assert_eq!(
            observation.observed,
            serde_json::json!({
                "root_counter_query_count": 1,
                "unique_graph_root_candidate_count": 42,
                "selected_graph_root_count": 24,
                "graph_root_omission_count": 18,
                "selectivity_decision_count": 2,
                "scored_selectivity_count": 1,
                "fallback_selectivity_count": 1,
            })
        );
    }

    #[test]
    fn tuning_observation_requires_measured_root_counters() {
        assert!(
            tuning_observation(&serde_json::json!({ "retrieval": {} }), &[hub_trace(None)],)
                .is_none()
        );
    }

    #[test]
    fn report_reader_round_trips_report() {
        let path = std::env::temp_dir().join(format!(
            "cmem-continuity-report-shape-drift-{}.json",
            Uuid::new_v4()
        ));
        let report = assemble_continuity_report(ContinuityReportInput {
            config: serde_json::json!({}),
            traces: &[hub_trace(Some((21, 12, 9)))],
            metric_family: &cmem_eval::retrieval_metric_family(
                "continuity",
                [("session", [5].as_slice())],
            ),
        })
        .unwrap();

        write_continuity_report(&path, &report).unwrap();
        let existing = std::fs::read(&path).unwrap();
        assert!(write_continuity_report(&path, &report).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), existing);
        assert_eq!(read_continuity_report(&path).unwrap(), report);
        std::fs::remove_file(path).unwrap();
    }
}
