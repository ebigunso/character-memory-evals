use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cmem_eval_core::{
    DatasetId, DatasetKind, DegradationSummary, EmbeddingBindingRecord, PerQuestionResult,
    RetrievalFanoutUtilization, RetrievalRationaleCategory, RetrievalSelectivityDecision,
    RetrievedContextPack, RunAdapterMetadata, RunSummary, aggregate_numeric_metrics,
    metric_support_summary, registry_coverage_summary_for, summarize_rows,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    CONTINUITY_TRACE_SCHEMA_VERSION, ContinuityQueryTrace, ContinuityScenario, InteractionEvent,
    RestartObservation, ScenarioPattern,
};

pub const CONTINUITY_REPORT_SCHEMA_VERSION: &str = "2.0.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityReport {
    pub schema_version: String,
    pub metadata: ContinuityReportMetadata,
    pub content: ContinuityReportContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityReportMetadata {
    pub generated_at: DateTime<Utc>,
    pub run_id: String,
    pub dataset: DatasetId,
    pub dataset_kind: DatasetKind,
    pub embedding_bindings: Vec<EmbeddingBindingRecord>,
    pub degradation: DegradationSummary,
    pub adapter: RunAdapterMetadata,
    pub fixture_schema_version: u32,
    pub fixture_seed: u64,
    pub embedding_seeds: BTreeMap<String, u64>,
    pub fixture_ids: Vec<String>,
    pub config: Value,
    pub schema_versions: BTreeMap<String, String>,
    pub normalization: ReportNormalization,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportNormalization {
    pub nondeterministic_paths: Vec<String>,
    pub excluded_nondeterministic_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityReportContent {
    pub aggregate: AggregateContinuityReport,
    pub scenarios: BTreeMap<String, ScenarioContinuityReport>,
    pub tuning_observations: Vec<TuningObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateContinuityReport {
    pub query_count: usize,
    pub restart_count: usize,
    pub metrics: Value,
    pub metric_support: Value,
    pub registry_coverage: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioContinuityReport {
    pub pattern: String,
    pub query_count: usize,
    pub metrics: Value,
    pub metric_support: Value,
    pub registry_coverage: Value,
    pub rationale_samples: Vec<QueryRationaleSample>,
    pub fanout_decisions: Vec<QueryFanoutDecisions>,
    pub stats_health_events: Vec<StatsHealthEvent>,
    pub restart_observations: Vec<RestartObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryRationaleSample {
    pub query_id: String,
    pub query: String,
    pub context_pack: RetrievedContextPack,
    pub items: Vec<RationaleSampleItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RationaleSampleItem {
    pub rank: usize,
    pub object_id: String,
    pub rationale: Vec<String>,
    pub categories: Vec<RetrievalRationaleCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryFanoutDecisions {
    pub query_id: String,
    pub utilization: Option<Vec<RetrievalFanoutUtilization>>,
    pub selectivity: Option<Vec<RetrievalSelectivityDecision>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatsHealthEvent {
    pub query_id: String,
    pub status: String,
    pub decision_count: Option<usize>,
    pub scored_count: Option<usize>,
    pub fallback_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TuningObservation {
    pub id: String,
    pub finding: String,
    pub measurement_regime: Value,
    pub observed: Value,
}

pub struct ContinuityReportInput<'a> {
    pub generated_at: DateTime<Utc>,
    pub fixture_schema_version: u32,
    pub fixture_seed: u64,
    pub config: Value,
    pub adapter: RunAdapterMetadata,
    pub scenarios: &'a [ContinuityScenario],
    pub traces: &'a [ContinuityQueryTrace],
    pub rows: &'a [PerQuestionResult],
    pub summary: &'a RunSummary,
    pub metric_family: &'a cmem_eval_core::MetricFamily,
    pub restart_observations: &'a BTreeMap<String, Vec<RestartObservation>>,
}

pub fn assemble_continuity_report(input: ContinuityReportInput<'_>) -> Result<ContinuityReport> {
    let restart_count = validate_restart_observations(input.scenarios, input.restart_observations)?;
    let fixture_ids = input
        .scenarios
        .iter()
        .map(|scenario| scenario.fixture_id.clone())
        .collect::<Vec<_>>();
    let embedding_seeds = input
        .scenarios
        .iter()
        .filter_map(|scenario| {
            scenario
                .embedding
                .controllable_similarity()
                .map(|embedding| (scenario.fixture_id.clone(), embedding.seed))
        })
        .collect::<BTreeMap<_, _>>();
    let mut schema_versions = BTreeMap::new();
    schema_versions.insert(
        "continuity_fixture".to_string(),
        input.fixture_schema_version.to_string(),
    );
    schema_versions.insert(
        "continuity_report".to_string(),
        CONTINUITY_REPORT_SCHEMA_VERSION.to_string(),
    );
    schema_versions.insert(
        "continuity_trace".to_string(),
        CONTINUITY_TRACE_SCHEMA_VERSION.to_string(),
    );
    schema_versions.insert("result".to_string(), input.summary.schema_version.clone());

    let expected_queries = input
        .scenarios
        .iter()
        .flat_map(|scenario| {
            scenario.events.iter().filter_map(move |event| match event {
                InteractionEvent::Query {
                    event_id,
                    query_id,
                    text,
                    ..
                } => Some((
                    scenario,
                    event_id.as_str(),
                    query_id.as_str(),
                    text.as_str(),
                )),
                _ => None,
            })
        })
        .collect::<Vec<_>>();
    if input.traces.len() != expected_queries.len() {
        bail!(
            "continuity report received {} traces but selected scenarios script {} queries",
            input.traces.len(),
            expected_queries.len()
        );
    }
    for (index, (trace, (scenario, event_id, query_id, query))) in
        input.traces.iter().zip(expected_queries).enumerate()
    {
        if trace.fixture_id != scenario.fixture_id
            || trace.namespace != scenario.namespace
            || trace.pattern != scenario.pattern
            || trace.event_id != event_id
            || trace.query_id != query_id
            || trace.query != query
        {
            bail!(
                "continuity report scripted query mismatch at index {index}: trace ({:?}, {:?}, {:?}), fixture ({:?}, {:?}, {:?})",
                trace.fixture_id,
                trace.event_id,
                trace.query_id,
                scenario.fixture_id,
                event_id,
                query_id
            );
        }
    }

    if input.traces.len() != input.rows.len() {
        bail!(
            "continuity report received {} traces but {} result rows",
            input.traces.len(),
            input.rows.len()
        );
    }
    for (index, (trace, row)) in input.traces.iter().zip(input.rows).enumerate() {
        let trace_question_type = serde_json::to_value(trace.pattern)?
            .as_str()
            .expect("ScenarioPattern serializes as a string")
            .to_string();
        if trace.query_id != row.question_id
            || trace.query != row.question
            || row.question_type.as_deref() != Some(trace_question_type.as_str())
        {
            bail!(
                "continuity report trace/result mismatch at index {index}: trace ({:?}, {:?}, {:?}), result ({:?}, {:?}, {:?})",
                trace.query_id,
                trace_question_type,
                trace.query,
                row.question_id,
                row.question_type,
                row.question
            );
        }
    }

    if input.summary.adapter != input.adapter {
        bail!("continuity report summary adapter does not match report adapter metadata");
    }
    if input.summary.config != input.config {
        bail!("continuity report summary config does not match report config snapshot");
    }
    for (index, row) in input.rows.iter().enumerate() {
        if row.run_id != input.summary.run_id
            || row.dataset != input.summary.dataset
            || row.adapter != input.summary.adapter
        {
            bail!(
                "continuity report summary/result identity mismatch at index {index}: row ({:?}, {:?}, {:?}), summary ({:?}, {:?}, {:?})",
                row.run_id,
                row.dataset,
                row.adapter,
                input.summary.run_id,
                input.summary.dataset,
                input.summary.adapter
            );
        }
    }
    let recomputed_summary = summarize_rows(
        input.summary.run_id.clone(),
        input.summary.dataset.clone(),
        input.summary.dataset_kind,
        input.summary.adapter.clone(),
        input.summary.config.clone(),
        input.rows,
        std::slice::from_ref(input.metric_family),
    );
    if input.summary.schema_version != recomputed_summary.schema_version
        || input.summary.dataset_kind != recomputed_summary.dataset_kind
        || input.summary.embedding_bindings != recomputed_summary.embedding_bindings
        || input.summary.degradation != recomputed_summary.degradation
        || input.summary.num_questions != recomputed_summary.num_questions
    {
        bail!(
            "continuity report summary identity/count does not match result rows: summary schema/bindings/count ({:?}, {:?}, {}), recomputed ({:?}, {:?}, {})",
            input.summary.schema_version,
            input.summary.embedding_bindings,
            input.summary.num_questions,
            recomputed_summary.schema_version,
            recomputed_summary.embedding_bindings,
            recomputed_summary.num_questions
        );
    }
    for (field, actual, expected) in [
        (
            "metrics",
            &input.summary.metrics,
            &recomputed_summary.metrics,
        ),
        (
            "metric_support",
            &input.summary.metric_support,
            &recomputed_summary.metric_support,
        ),
        (
            "registry_coverage",
            &input.summary.registry_coverage,
            &recomputed_summary.registry_coverage,
        ),
        (
            "latency",
            &input.summary.latency,
            &recomputed_summary.latency,
        ),
    ] {
        if actual != expected {
            bail!("continuity report summary {field} does not match result-row aggregates");
        }
    }

    let mut scenario_reports = BTreeMap::new();
    let mut assigned_trace_count = 0;
    let mut assigned_row_count = 0;
    for scenario in input.scenarios {
        let scenario_traces = input
            .traces
            .iter()
            .filter(|trace| trace.fixture_id == scenario.fixture_id)
            .collect::<Vec<_>>();
        assigned_trace_count += scenario_traces.len();
        let scenario_rows = input
            .traces
            .iter()
            .zip(input.rows)
            .filter_map(|(trace, row)| (trace.fixture_id == scenario.fixture_id).then_some(row))
            .collect::<Vec<_>>();
        assigned_row_count += scenario_rows.len();
        let metric_rows = scenario_rows
            .iter()
            .map(|row| row.metrics.to_json_map())
            .collect::<Vec<Map<String, Value>>>();
        let rationale_samples = scenario_traces
            .iter()
            .map(|trace| rationale_sample(trace))
            .collect();
        let fanout_decisions = scenario_traces
            .iter()
            .map(|trace| QueryFanoutDecisions {
                query_id: trace.query_id.clone(),
                utilization: trace.retrieval.telemetry().fanout_utilization.clone(),
                selectivity: trace.retrieval.telemetry().selectivity_decisions.clone(),
            })
            .collect();
        let stats_health_events = scenario_traces
            .iter()
            .map(|trace| stats_health_event(trace))
            .collect();
        scenario_reports.insert(
            scenario.fixture_id.clone(),
            ScenarioContinuityReport {
                pattern: serde_json::to_value(scenario.pattern)?
                    .as_str()
                    .expect("ScenarioPattern serializes as a string")
                    .to_string(),
                query_count: scenario_traces.len(),
                metrics: aggregate_numeric_metrics(&metric_rows),
                metric_support: metric_support_summary(&metric_rows),
                registry_coverage: registry_coverage_summary_for(
                    &metric_rows,
                    std::slice::from_ref(input.metric_family),
                ),
                rationale_samples,
                fanout_decisions,
                stats_health_events,
                restart_observations: input
                    .restart_observations
                    .get(&scenario.fixture_id)
                    .cloned()
                    .unwrap_or_default(),
            },
        );
    }
    if assigned_trace_count != input.traces.len() {
        bail!(
            "continuity report assigned {assigned_trace_count} of {} traces to selected scenarios",
            input.traces.len()
        );
    }
    if assigned_row_count != input.rows.len() {
        bail!(
            "continuity report assigned {assigned_row_count} of {} result rows to traces",
            input.rows.len()
        );
    }

    let tuning_observations = tuning_observation(&input.config, &input.adapter, input.traces)
        .into_iter()
        .collect();
    Ok(ContinuityReport {
        schema_version: CONTINUITY_REPORT_SCHEMA_VERSION.to_string(),
        metadata: ContinuityReportMetadata {
            generated_at: input.generated_at,
            run_id: input.summary.run_id.clone(),
            dataset: input.summary.dataset.clone(),
            dataset_kind: input.summary.dataset_kind,
            embedding_bindings: input.summary.embedding_bindings.clone(),
            degradation: input.summary.degradation.clone(),
            adapter: input.adapter,
            fixture_schema_version: input.fixture_schema_version,
            fixture_seed: input.fixture_seed,
            embedding_seeds,
            fixture_ids,
            config: input.config.clone(),
            schema_versions,
            normalization: ReportNormalization {
                nondeterministic_paths: vec!["metadata.generated_at".to_string()],
                excluded_nondeterministic_sources: vec![
                    "correction and forget library mutation timestamps".to_string(),
                    "measured query retrieval latency in results and summaries".to_string(),
                ],
            },
        },
        content: ContinuityReportContent {
            aggregate: AggregateContinuityReport {
                query_count: input.summary.num_questions,
                restart_count,
                metrics: input.summary.metrics.clone(),
                metric_support: input.summary.metric_support.clone(),
                registry_coverage: input.summary.registry_coverage.clone(),
            },
            scenarios: scenario_reports,
            tuning_observations,
        },
    })
}

fn validate_restart_observations(
    scenarios: &[ContinuityScenario],
    observations: &BTreeMap<String, Vec<RestartObservation>>,
) -> Result<usize> {
    let selected_fixture_ids = scenarios
        .iter()
        .map(|scenario| scenario.fixture_id.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(unknown_fixture_id) = observations
        .keys()
        .find(|fixture_id| !selected_fixture_ids.contains(fixture_id.as_str()))
    {
        bail!(
            "continuity report restart observations contain unknown or unselected fixture ID {unknown_fixture_id:?}"
        );
    }

    let mut restart_count = 0;
    for scenario in scenarios {
        let expected = scenario
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Restart {
                    event_id,
                    timestamp,
                    reopen_graph,
                    reopen_stats,
                } => Some((event_id, timestamp, reopen_graph, reopen_stats)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let actual = observations
            .get(&scenario.fixture_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if actual.len() != expected.len() {
            bail!(
                "continuity report fixture {:?} has {} restart observations but scripts {} restart events",
                scenario.fixture_id,
                actual.len(),
                expected.len()
            );
        }
        for (index, (observation, (event_id, timestamp, reopen_graph, reopen_stats))) in
            actual.iter().zip(expected.iter()).enumerate()
        {
            if observation.event_id != **event_id
                || observation.timestamp != **timestamp
                || observation.reopen_graph != **reopen_graph
                || observation.reopen_stats != **reopen_stats
            {
                bail!(
                    "continuity report fixture {:?} restart observation mismatch at index {index}: observation ({:?}, {}, {}, {}), scripted event ({:?}, {}, {}, {})",
                    scenario.fixture_id,
                    observation.event_id,
                    observation.timestamp,
                    observation.reopen_graph,
                    observation.reopen_stats,
                    event_id,
                    timestamp,
                    reopen_graph,
                    reopen_stats
                );
            }
        }
        restart_count += expected.len();
    }
    Ok(restart_count)
}

pub fn write_continuity_report(path: &Path, report: &ContinuityReport) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, report)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_continuity_report(path: &Path) -> Result<ContinuityReport> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let value: Value = serde_json::from_reader(file)
        .with_context(|| format!("deserialize continuity report {}", path.display()))?;
    match value.get("schema_version").and_then(Value::as_str) {
        Some(CONTINUITY_REPORT_SCHEMA_VERSION) => {}
        Some(version) => bail!(
            "unsupported continuity report schema_version {version:?}; expected {CONTINUITY_REPORT_SCHEMA_VERSION:?}"
        ),
        None => bail!(
            "missing continuity report schema_version; expected {CONTINUITY_REPORT_SCHEMA_VERSION:?}"
        ),
    }
    serde_json::from_value(value)
        .with_context(|| format!("decode continuity report {}", path.display()))
}

fn rationale_sample(trace: &ContinuityQueryTrace) -> QueryRationaleSample {
    let categories = trace
        .retrieval
        .telemetry()
        .rationale_categories_by_internal_id
        .as_ref();
    QueryRationaleSample {
        query_id: trace.query_id.clone(),
        query: trace.query.clone(),
        context_pack: trace.retrieval.clone(),
        items: trace
            .retrieval
            .items()
            .iter()
            .map(|item| RationaleSampleItem {
                rank: item.rank,
                object_id: item
                    .external_id
                    .clone()
                    .unwrap_or_else(|| format!("{}:{}", item.kind, item.internal_id)),
                rationale: item.rationale.clone(),
                categories: categories
                    .and_then(|by_id| by_id.get(&item.internal_id))
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

fn stats_health_event(trace: &ContinuityQueryTrace) -> StatsHealthEvent {
    let decisions = trace.retrieval.telemetry().selectivity_decisions.as_ref();
    let decision_count = decisions.map(Vec::len);
    let scored_count = decisions.map(|values| {
        values
            .iter()
            .filter(|decision| decision.score.is_some())
            .count()
    });
    let fallback_count =
        decisions.map(|values| values.iter().filter(|decision| decision.fallback).count());
    let status = match (decision_count, scored_count, fallback_count) {
        (None, _, _) => "unsupported",
        (Some(0), _, _) => "no_decisions",
        (Some(_), Some(0), Some(_)) => "fallback_only",
        (Some(_), Some(_), Some(fallback)) if fallback > 0 => "scored_with_fallback",
        _ => "scored",
    };
    StatsHealthEvent {
        query_id: trace.query_id.clone(),
        status: status.to_string(),
        decision_count,
        scored_count,
        fallback_count,
    }
}

fn tuning_observation(
    config: &Value,
    adapter: &RunAdapterMetadata,
    traces: &[ContinuityQueryTrace],
) -> Option<TuningObservation> {
    if adapter.is_mock {
        return None;
    }
    let hub_traces = traces
        .iter()
        .filter(|trace| trace.pattern == ScenarioPattern::RecurringHubEntity)
        .collect::<Vec<_>>();
    let root_counter_samples = hub_traces
        .iter()
        .filter_map(|trace| {
            let telemetry = trace.retrieval.telemetry();
            Some((
                telemetry.unique_graph_root_candidate_count?,
                telemetry.selected_graph_root_count?,
                telemetry.graph_root_omission_count?,
            ))
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
        .filter_map(|trace| trace.retrieval.telemetry().selectivity_decisions.as_ref())
        .flatten()
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
    use cmem_eval_core::{ContextRenderer, RetrievalTelemetry, RetrievedContextPack};
    use uuid::Uuid;

    use super::*;

    fn hub_trace(counters: Option<(usize, usize, usize)>) -> ContinuityQueryTrace {
        let mut telemetry = RetrievalTelemetry {
            trace_available: true,
            selectivity_decisions: Some(Vec::new()),
            ..RetrievalTelemetry::default()
        };
        if let Some((unique, selected, omitted)) = counters {
            telemetry.unique_graph_root_candidate_count = Some(unique);
            telemetry.selected_graph_root_count = Some(selected);
            telemetry.graph_root_omission_count = Some(omitted);
        }
        ContinuityQueryTrace {
            schema_version: CONTINUITY_TRACE_SCHEMA_VERSION.to_string(),
            fixture_id: "recurring-hub-entity".to_string(),
            namespace: "continuity:hub".to_string(),
            pattern: ScenarioPattern::RecurringHubEntity,
            event_id: "query-hub".to_string(),
            query_id: "query-hub".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            query: "What context is connected to the hub?".to_string(),
            expected: crate::ExpectedRelevance {
                relevant_external_ids: vec!["relevant".to_string()],
                irrelevant_external_ids: vec!["negative".to_string()],
            },
            history_text: String::new(),
            retrieval: RetrievedContextPack::from_ranked_items(
                Vec::new(),
                telemetry,
                ContextRenderer::PlainText,
            ),
            write_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
        }
    }

    #[test]
    fn tuning_observation_uses_measured_root_counters() {
        let observation = tuning_observation(
            &serde_json::json!({
                "retrieval": {
                    "max_vector_candidates": 48,
                    "max_graph_roots": 12
                }
            }),
            &RunAdapterMetadata::live(),
            &[hub_trace(Some((21, 12, 9)))],
        )
        .unwrap();

        assert_eq!(observation.id, "entity_root_candidate_limit");
        assert_eq!(
            observation.observed,
            serde_json::json!({
                "root_counter_query_count": 1,
                "unique_graph_root_candidate_count": 21,
                "selected_graph_root_count": 12,
                "graph_root_omission_count": 9,
                "selectivity_decision_count": 0,
                "scored_selectivity_count": 0,
                "fallback_selectivity_count": 0,
            })
        );
    }

    #[test]
    fn tuning_observation_requires_measured_root_counters() {
        assert!(
            tuning_observation(
                &serde_json::json!({ "retrieval": {} }),
                &RunAdapterMetadata::live(),
                &[hub_trace(None)],
            )
            .is_none()
        );
    }

    #[test]
    fn report_reader_rejects_missing_and_legacy_schema_versions() {
        let path = std::env::temp_dir().join(format!(
            "cmem-continuity-report-schema-{}.json",
            Uuid::new_v4()
        ));

        std::fs::write(&path, br#"{}"#).unwrap();
        let missing = read_continuity_report(&path).unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("missing continuity report schema_version")
        );

        std::fs::write(&path, br#"{"schema_version":"1.0.0"}"#).unwrap();
        let legacy = read_continuity_report(&path).unwrap_err();
        assert!(
            legacy
                .to_string()
                .contains("unsupported continuity report schema_version")
        );

        let _ = std::fs::remove_file(path);
    }
}
