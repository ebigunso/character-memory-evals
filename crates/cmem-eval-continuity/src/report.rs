use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cmem_eval_core::{
    PerQuestionResult, RetrievalFanoutUtilization, RetrievalRationaleCategory,
    RetrievalSelectivityDecision, RetrievedContextPack, RunAdapterMetadata, RunSummary,
    aggregate_numeric_metrics, metric_support_summary, registry_coverage_summary_for,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    CONTINUITY_FIXTURE_SCHEMA_VERSION, CONTINUITY_TRACE_SCHEMA_VERSION, ContinuityQueryTrace,
    ContinuityScenario, RestartObservation, ScenarioPattern,
};

pub const CONTINUITY_REPORT_SCHEMA_VERSION: &str = "1.0.0";

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
    pub dataset: String,
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
    let fixture_ids = input
        .scenarios
        .iter()
        .map(|scenario| scenario.fixture_id.clone())
        .collect::<Vec<_>>();
    let embedding_seeds = input
        .scenarios
        .iter()
        .map(|scenario| (scenario.fixture_id.clone(), scenario.embedding.seed))
        .collect::<BTreeMap<_, _>>();
    let mut schema_versions = BTreeMap::new();
    schema_versions.insert(
        "continuity_fixture".to_string(),
        CONTINUITY_FIXTURE_SCHEMA_VERSION.to_string(),
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
            .filter_map(|row| row.metrics.as_object().cloned())
            .collect::<Vec<Map<String, Value>>>();
        let rationale_samples = scenario_traces
            .iter()
            .map(|trace| rationale_sample(trace))
            .collect();
        let fanout_decisions = scenario_traces
            .iter()
            .map(|trace| QueryFanoutDecisions {
                query_id: trace.query_id.clone(),
                utilization: trace.retrieval.telemetry.fanout_utilization.clone(),
                selectivity: trace.retrieval.telemetry.selectivity_decisions.clone(),
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

    let restart_count = input.restart_observations.values().map(Vec::len).sum();
    let tuning_observations = tuning_observation(&input.config, &input.adapter, input.traces)
        .into_iter()
        .collect();
    Ok(ContinuityReport {
        schema_version: CONTINUITY_REPORT_SCHEMA_VERSION.to_string(),
        metadata: ContinuityReportMetadata {
            generated_at: input.generated_at,
            run_id: input.summary.run_id.clone(),
            dataset: input.summary.dataset.clone(),
            adapter: input.adapter,
            fixture_schema_version: CONTINUITY_FIXTURE_SCHEMA_VERSION,
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

pub fn write_continuity_report(path: &Path, report: &ContinuityReport) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, report)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn rationale_sample(trace: &ContinuityQueryTrace) -> QueryRationaleSample {
    let categories = trace
        .retrieval
        .telemetry
        .rationale_categories_by_internal_id
        .as_ref();
    QueryRationaleSample {
        query_id: trace.query_id.clone(),
        query: trace.query.clone(),
        context_pack: trace.retrieval.clone(),
        items: trace
            .retrieval
            .items
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
    let decisions = trace.retrieval.telemetry.selectivity_decisions.as_ref();
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
    let decisions = traces
        .iter()
        .filter(|trace| trace.pattern == ScenarioPattern::RecurringHubEntity)
        .filter_map(|trace| trace.retrieval.telemetry.selectivity_decisions.as_ref())
        .flatten()
        .collect::<Vec<_>>();
    if decisions.is_empty() {
        return None;
    }
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
        finding: "Character Memory's default max_graph_roots=12 plus equal-score object-type ordering truncated entity roots in the hub fixture; this evaluation uses the caller-supplied candidate-limit regime recorded here without changing Character Memory defaults.".to_string(),
        measurement_regime: serde_json::json!({
            "max_vector_candidates": config.pointer("/retrieval/max_vector_candidates"),
            "max_graph_roots": config.pointer("/retrieval/max_graph_roots"),
        }),
        observed: serde_json::json!({
            "selectivity_decision_count": decisions.len(),
            "scored_selectivity_count": scored_count,
            "fallback_selectivity_count": fallback_count,
        }),
    })
}
