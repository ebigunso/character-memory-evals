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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioStatus {
    Passed,
    Failed,
    NotRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    pub status: ScenarioStatus,
    pub reason: String,
}

impl CheckResult {
    pub fn checked(passed: bool, reason: &str) -> Self {
        Self {
            status: if passed {
                ScenarioStatus::Passed
            } else {
                ScenarioStatus::Failed
            },
            reason: reason.into(),
        }
    }

    pub fn not_run(reason: &str) -> Self {
        Self {
            status: ScenarioStatus::NotRun,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssertionResult {
    pub identity: crate::AssertionIdentity,
    #[serde(flatten)]
    pub check: CheckResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioOutcome {
    pub status: ScenarioStatus,
    pub missing_features: Vec<crate::ScenarioFeature>,
    pub assertions: Vec<AssertionResult>,
    pub probes: BTreeMap<String, crate::SituatedProbeMeasures>,
    pub omission_reason_invariant: CheckResult,
}

impl ScenarioOutcome {
    pub fn executed() -> Self {
        Self {
            status: ScenarioStatus::Passed,
            missing_features: Vec::new(),
            assertions: Vec::new(),
            probes: BTreeMap::new(),
            omission_reason_invariant: CheckResult::not_run("no retrieval executed"),
        }
    }

    pub fn not_run(
        scenario: &crate::ContinuityScenario,
        missing_features: Vec<crate::ScenarioFeature>,
    ) -> Self {
        let reason = format!(
            "unsupported features: {}",
            serde_json::to_string(&missing_features).expect("feature enums")
        );
        Self {
            status: ScenarioStatus::NotRun,
            missing_features,
            assertions: scenario
                .events
                .iter()
                .flat_map(|event| event.assertion_identities())
                .map(|identity| AssertionResult {
                    identity,
                    check: CheckResult::not_run(&reason),
                })
                .collect(),
            probes: scenario
                .events
                .iter()
                .filter_map(|event| match event {
                    crate::InteractionEvent::Probe {
                        query_id,
                        assertions,
                        measures,
                        ..
                    } => Some((
                        query_id.clone(),
                        crate::situated_probe_measures(scenario, assertions, measures, None),
                    )),
                    _ => None,
                })
                .collect(),
            omission_reason_invariant: CheckResult::not_run(&reason),
        }
    }

    pub(crate) fn record_probe(
        &mut self,
        scenario: &crate::ContinuityScenario,
        event: &crate::InteractionEvent,
        pack: &cmem_eval::RetrievedContextPack,
    ) {
        let crate::InteractionEvent::Probe {
            query_id,
            assertions,
            measures,
            ..
        } = event
        else {
            unreachable!("probe outcome requires a probe event");
        };
        let checks = crate::check_probe_assertions(scenario, event, pack);
        if checks
            .iter()
            .any(|result| result.check.status == ScenarioStatus::Failed)
        {
            self.status = ScenarioStatus::Failed;
        }
        self.assertions.extend(checks);
        self.probes.insert(
            query_id.clone(),
            crate::situated_probe_measures(scenario, assertions, measures, Some(pack)),
        );
        self.record_retrieval(pack);
    }

    pub fn record_retrieval(&mut self, pack: &cmem_eval::RetrievedContextPack) {
        let passed = self.omission_reason_invariant.status != ScenarioStatus::Failed
            && crate::omissions_have_reasons(pack.outcomes());
        self.omission_reason_invariant = CheckResult::checked(
            passed,
            if passed {
                "native lifecycle and currency omissions are accounted for by reason"
            } else {
                "at least one retrieval omitted a memory without a native reason"
            },
        );
        if !passed {
            self.status = ScenarioStatus::Failed;
        }
    }
}

impl Default for ScenarioOutcome {
    fn default() -> Self {
        Self::executed()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityReport {
    pub aggregate: AggregateContinuityReport,
    pub scenarios: BTreeMap<String, ScenarioContinuityReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateContinuityReport {
    pub carried_recall_by_reason: BTreeMap<String, crate::CarriedRecall>,
    pub omission_reason_invariant: CheckResult,
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
    pub carried_recall_by_reason: BTreeMap<String, crate::CarriedRecall>,
    pub outcome: ScenarioOutcome,
    pub query_count: usize,
    pub metrics: NumericMetricSummary,
    pub metric_support: MetricSupportSummary,
    pub registry_coverage: RegistryCoverageSummary,
}

pub struct ContinuityReportInput<'a> {
    pub traces: &'a [ContinuityQueryTrace],
    pub outcomes: &'a BTreeMap<String, ScenarioOutcome>,
    pub metric_family: &'a cmem_eval::MetricFamily,
}

pub fn assemble_continuity_report(input: ContinuityReportInput<'_>) -> Result<ContinuityReport> {
    let rows = input
        .traces
        .iter()
        .map(|trace| trace.result.clone())
        .collect::<Vec<_>>();
    let summary = summarize_rows(&rows, std::slice::from_ref(input.metric_family))?;
    let mut grouped = input
        .outcomes
        .keys()
        .map(|id| (id.clone(), Vec::<Map<String, Value>>::new()))
        .collect::<BTreeMap<_, _>>();
    for trace in input.traces {
        grouped
            .get_mut(&trace.fixture_id)
            .with_context(|| format!("trace has no scenario outcome: {}", trace.fixture_id))?
            .push(trace.result.metrics.to_json_map());
    }
    let scenarios = grouped
        .into_iter()
        .map(|(fixture_id, metrics)| {
            (
                fixture_id.clone(),
                ScenarioContinuityReport {
                    carried_recall_by_reason: crate::metrics::pool_carried_recall(
                        input.outcomes[&fixture_id].probes.values(),
                    ),
                    outcome: input.outcomes[&fixture_id].clone(),
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
            carried_recall_by_reason: crate::metrics::pool_carried_recall(
                input
                    .outcomes
                    .values()
                    .flat_map(|outcome| outcome.probes.values()),
            ),
            omission_reason_invariant: combined_invariant(input.outcomes.values()),
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
    })
}

fn combined_invariant<'a>(outcomes: impl Iterator<Item = &'a ScenarioOutcome>) -> CheckResult {
    let checks = outcomes
        .map(|outcome| outcome.omission_reason_invariant.status)
        .collect::<Vec<_>>();
    if checks.contains(&ScenarioStatus::Failed) {
        CheckResult::checked(
            false,
            "at least one retrieval omitted a memory without a native reason",
        )
    } else if checks.contains(&ScenarioStatus::Passed) {
        CheckResult::checked(
            true,
            "all executed retrievals account for lifecycle and currency omissions by reason",
        )
    } else {
        CheckResult::not_run("no retrieval executed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityRepeatDifference {
    pub scenario_id: Option<String>,
    pub assertion: Option<crate::AssertionIdentity>,
    pub field: String,
    pub before: Value,
    pub after: Value,
}

pub fn compare_continuity_reports(
    before: &ContinuityReport,
    after: &ContinuityReport,
) -> Vec<ContinuityRepeatDifference> {
    let mut differences = Vec::new();
    let mut compare =
        |scenario_id: Option<String>, assertion, field: &str, before: Value, after: Value| {
            if before != after {
                differences.push(ContinuityRepeatDifference {
                    scenario_id,
                    assertion,
                    field: field.into(),
                    before,
                    after,
                });
            }
        };
    compare(
        None,
        None,
        "carried_recall_by_reason",
        serde_json::json!(before.aggregate.carried_recall_by_reason),
        serde_json::json!(after.aggregate.carried_recall_by_reason),
    );
    compare(
        None,
        None,
        "omission_reason_invariant",
        serde_json::json!(before.aggregate.omission_reason_invariant),
        serde_json::json!(after.aggregate.omission_reason_invariant),
    );
    for id in before
        .scenarios
        .keys()
        .chain(after.scenarios.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        compare(
            Some(id.clone()),
            None,
            "carried_recall_by_reason",
            serde_json::json!(
                before
                    .scenarios
                    .get(id)
                    .map(|scenario| &scenario.carried_recall_by_reason)
            ),
            serde_json::json!(
                after
                    .scenarios
                    .get(id)
                    .map(|scenario| &scenario.carried_recall_by_reason)
            ),
        );
        let left = before.scenarios.get(id).map(|scenario| &scenario.outcome);
        let right = after.scenarios.get(id).map(|scenario| &scenario.outcome);
        let probe_recalls = |outcome: Option<&ScenarioOutcome>| {
            outcome
                .into_iter()
                .flat_map(|outcome| &outcome.probes)
                .map(|(id, probe)| (id.clone(), probe.carried_recall_by_reason.clone()))
                .collect::<BTreeMap<_, _>>()
        };
        compare(
            Some(id.clone()),
            None,
            "probe_carried_recall_by_reason",
            serde_json::json!(probe_recalls(left)),
            serde_json::json!(probe_recalls(right)),
        );
        compare(
            Some(id.clone()),
            None,
            "outcome",
            serde_json::json!(left.map(|outcome| (&outcome.status, &outcome.missing_features))),
            serde_json::json!(right.map(|outcome| (&outcome.status, &outcome.missing_features))),
        );
        compare(
            Some(id.clone()),
            None,
            "omission_reason_invariant",
            serde_json::json!(left.map(|outcome| &outcome.omission_reason_invariant)),
            serde_json::json!(right.map(|outcome| &outcome.omission_reason_invariant)),
        );
        let assertions = |outcome: Option<&ScenarioOutcome>| {
            outcome
                .into_iter()
                .flat_map(|outcome| &outcome.assertions)
                .map(|result| (result.identity.clone(), result.check.clone()))
                .collect::<BTreeMap<_, _>>()
        };
        let left = assertions(left);
        let right = assertions(right);
        for identity in left
            .keys()
            .chain(right.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            compare(
                Some(id.clone()),
                Some(identity.clone()),
                "assertion",
                serde_json::json!(left.get(identity)),
                serde_json::json!(right.get(identity)),
            );
        }
    }
    differences
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use cmem_eval::PerQuestionResult;

    use super::*;

    fn hub_trace() -> ContinuityQueryTrace {
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
                context: Default::default(),
                retrieval_outcomes: Vec::new(),
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
    fn report_writer_preserves_existing_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let report = assemble_continuity_report(ContinuityReportInput {
            traces: &[hub_trace()],
            outcomes: &BTreeMap::from([(
                "recurring-hub-entity".into(),
                ScenarioOutcome::executed(),
            )]),
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
    }

    #[test]
    fn executed_probe_records_checks_measures_and_failure_status() {
        use cmem_eval::character_memory::{
            ContinuityContextPack, EpisodeDraft, RetrievalRationale, RetrieveOutcome,
        };
        let scenario = crate::driver::tests::situated_scenario();
        let event: crate::InteractionEvent = serde_json::from_value(serde_json::json!({
            "kind":"probe", "event_id":"probe", "query_id":"probe", "timestamp":"2024-01-04T09:00:00Z",
            "scene":{"kind":"named","name":"pair"}, "topic":"Garden",
            "assertions":{
                "carried":[{"memory":"visit","reason":"pair","section":"episodes"}],
                "in_order":[["visit","noise"]]
            },
            "measures":{"bystanders":["noise"]}
        })).unwrap();
        let visit = EpisodeDraft {
            scene: Some(cmem_eval::character_memory::Scene::at(
                chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
            )),
            ..EpisodeDraft::new("visit")
        }
        .into_domain()
        .unwrap();
        let noise = EpisodeDraft {
            scene: Some(cmem_eval::character_memory::Scene::at(
                chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
            )),
            ..EpisodeDraft::new("noise")
        }
        .into_domain()
        .unwrap();
        let refs = [(visit.id, "visit"), (noise.id, "noise")]
            .map(|(id, external_id)| {
                (
                    id.to_string(),
                    cmem_eval::MemoryEndpointInput {
                        object_type: cmem_eval::ObjectType::Episode,
                        external_id: external_id.into(),
                    },
                )
            })
            .into_iter()
            .collect();
        let native = RetrieveOutcome {
            time_range: None,
            activity: None,
            scene: cmem_eval::character_memory::Scene::at(
                chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
            ),
            scene_references: Vec::new(),
            memory_scenes: Vec::new(),
            pack: ContinuityContextPack {
                relevant_episodes: vec![visit, noise],
                ..ContinuityContextPack::empty()
            },
            rationale: RetrievalRationale::new("native probe"),
            trace: None,
        };
        let pack = cmem_eval::RetrievedContextPack::from_ranked_items(
            vec![],
            vec![native.clone()],
            cmem_eval::ContextRenderer::PlainText,
        )
        .with_object_refs(refs);
        let mut outcome = ScenarioOutcome::executed();
        outcome.record_probe(&scenario, &event, &pack);
        assert_eq!(outcome.assertions.len(), 2);
        assert!(
            outcome
                .assertions
                .iter()
                .all(|result| result.check.status == ScenarioStatus::Passed)
        );
        assert_eq!(outcome.status, ScenarioStatus::Passed);
        assert_eq!(
            outcome.omission_reason_invariant.status,
            ScenarioStatus::Passed
        );
        assert_eq!(
            outcome.probes["probe"].carried_recall_by_reason["pair"].recall,
            Some(1.0)
        );
        assert_eq!(outcome.probes["probe"].bystander_context_share, Some(0.5));
        assert_eq!(outcome.probes["probe"].context_tokens, Some(0));

        let mut wrong_section = event.clone();
        let crate::InteractionEvent::Probe { assertions, .. } = &mut wrong_section else {
            unreachable!()
        };
        assertions.carried[0].section = Some(crate::MemorySection::Commitments);
        let mut failed = ScenarioOutcome::executed();
        failed.record_probe(&scenario, &wrong_section, &pack);
        assert_eq!(
            failed
                .assertions
                .iter()
                .filter(|result| result.check.status == ScenarioStatus::Failed)
                .count(),
            1
        );
        assert_eq!(failed.status, ScenarioStatus::Failed);
        failed.record_probe(&scenario, &event, &pack);
        assert_eq!(failed.status, ScenarioStatus::Failed);

        let mut unexplained = native;
        unexplained.rationale.lifecycle_omission_count = 1;
        let unexplained = cmem_eval::RetrievedContextPack::from_ranked_items(
            vec![],
            vec![unexplained],
            cmem_eval::ContextRenderer::PlainText,
        )
        .with_object_refs(pack.object_refs().clone());
        let mut failed = ScenarioOutcome::executed();
        failed.record_probe(&scenario, &event, &unexplained);
        assert!(
            failed
                .assertions
                .iter()
                .all(|result| result.check.status == ScenarioStatus::Passed)
        );
        let failed_invariant = CheckResult::checked(
            false,
            "at least one retrieval omitted a memory without a native reason",
        );
        assert_eq!(failed.omission_reason_invariant, failed_invariant);
        assert_eq!(failed.status, ScenarioStatus::Failed);
        failed.record_probe(&scenario, &event, &pack);
        assert_eq!(failed.omission_reason_invariant, failed_invariant);
        assert_eq!(failed.status, ScenarioStatus::Failed);
        assert_eq!(
            combined_invariant([&outcome, &failed].into_iter()).status,
            ScenarioStatus::Failed
        );
    }

    #[test]
    fn repeat_comparison_detects_cue_and_warning_identity_changes() {
        use crate::{AssertionIdentity, AssertionSubject, CueKind, ExpectedWriteWarning};

        let mut outcome = ScenarioOutcome::executed();
        outcome.assertions = [
            (
                "probe",
                AssertionSubject::NotCued {
                    memory: "noise".into(),
                    cue: CueKind::Date,
                },
            ),
            (
                "derive",
                AssertionSubject::WriteWarning(ExpectedWriteWarning::NearVerbatimRestatement),
            ),
        ]
        .map(|(event_id, assertion)| AssertionResult {
            identity: AssertionIdentity {
                event_id: event_id.into(),
                assertion,
            },
            check: CheckResult::checked(true, "observed"),
        })
        .to_vec();
        let report = assemble_continuity_report(ContinuityReportInput {
            traces: &[],
            outcomes: &BTreeMap::from([("scenario".into(), outcome)]),
            metric_family: &crate::continuity_metric_family(&Default::default(), &[]),
        })
        .unwrap();
        let mut reordered = report.clone();
        reordered
            .scenarios
            .get_mut("scenario")
            .unwrap()
            .outcome
            .assertions
            .reverse();
        assert!(compare_continuity_reports(&report, &reordered).is_empty());

        for (index, replacement) in [
            (
                0,
                AssertionSubject::NotCued {
                    memory: "noise".into(),
                    cue: CueKind::Topic,
                },
            ),
            (
                1,
                AssertionSubject::WriteWarning(ExpectedWriteWarning::ChurningChain),
            ),
        ] {
            let mut changed = report.clone();
            let assertions = &mut changed
                .scenarios
                .get_mut("scenario")
                .unwrap()
                .outcome
                .assertions;
            let removed = assertions[index].identity.clone();
            assertions[index].identity.assertion = replacement;
            let added = assertions[index].identity.clone();
            let check = serde_json::to_value(&assertions[index].check).unwrap();
            assertions.reverse();
            let differences = compare_continuity_reports(&report, &changed);
            assert_eq!(differences.len(), 2);
            for (identity, before, after) in [
                (removed, check.clone(), Value::Null),
                (added, Value::Null, check),
            ] {
                assert!(differences.contains(&ContinuityRepeatDifference {
                    scenario_id: Some("scenario".into()),
                    assertion: Some(identity),
                    field: "assertion".into(),
                    before,
                    after,
                }));
            }
        }
    }

    #[test]
    fn carried_recall_pools_counts_across_probes_and_scenarios_and_compares_nulls() {
        use crate::{CarriedAssertion, RecallReason, situated_probe_measures};
        use cmem_eval::character_memory::{
            ContinuityContextPack, EpisodeDraft, RetrievalRationale, RetrieveOutcome,
        };
        use cmem_eval::{ContextRenderer, RetrievedContextPack};
        let scenario = crate::driver::tests::situated_scenario();
        let episodes = (0..9)
            .map(|i| {
                EpisodeDraft {
                    scene: Some(cmem_eval::character_memory::Scene::at(
                        chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
                    )),
                    ..EpisodeDraft::new(format!("memory {i}"))
                }
                .into_domain()
                .unwrap()
            })
            .collect::<Vec<_>>();
        let refs = episodes
            .iter()
            .enumerate()
            .map(|(i, episode)| {
                (
                    episode.id.to_string(),
                    cmem_eval::MemoryEndpointInput {
                        object_type: cmem_eval::ObjectType::Episode,
                        external_id: format!("memory-{i}"),
                    },
                )
            })
            .collect();
        let pack = RetrievedContextPack::from_ranked_items(
            vec![],
            vec![RetrieveOutcome {
                time_range: None,
                activity: None,
                scene: cmem_eval::character_memory::Scene::at(
                    chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
                ),
                scene_references: Vec::new(),
                memory_scenes: Vec::new(),
                pack: ContinuityContextPack {
                    relevant_episodes: episodes,
                    ..ContinuityContextPack::empty()
                },
                rationale: RetrievalRationale::new("probe"),
                trace: None,
            }],
            ContextRenderer::PlainText,
        )
        .with_object_refs(refs);
        let measures = |memories: Vec<String>, ran: bool| {
            situated_probe_measures(
                &scenario,
                &crate::ProbeAssertions {
                    carried: memories
                        .into_iter()
                        .map(|memory| CarriedAssertion {
                            memory,
                            reason: RecallReason::Pair,
                            section: None,
                        })
                        .collect(),
                    ..Default::default()
                },
                &Default::default(),
                ran.then_some(&pack),
            )
        };
        let mut first = ScenarioOutcome::executed();
        first.probes = BTreeMap::from([
            ("miss".into(), measures(vec!["absent".into()], true)),
            (
                "nine".into(),
                measures((0..9).map(|i| format!("memory-{i}")).collect(), true),
            ),
        ]);
        let mut second = ScenarioOutcome::executed();
        second
            .probes
            .insert("another-miss".into(), measures(vec!["absent".into()], true));
        let mut skipped = ScenarioOutcome::executed();
        skipped.status = ScenarioStatus::NotRun;
        skipped
            .probes
            .insert("skipped".into(), measures(vec!["memory-0".into()], false));
        let report = assemble_continuity_report(ContinuityReportInput {
            traces: &[],
            outcomes: &BTreeMap::from([
                ("first".into(), first),
                ("second".into(), second),
                ("skipped".into(), skipped),
            ]),
            metric_family: &crate::continuity_metric_family(&Default::default(), &[]),
        })
        .unwrap();
        assert_eq!(
            report.scenarios["first"].carried_recall_by_reason["pair"],
            crate::CarriedRecall {
                expected: 10,
                admitted: Some(9),
                recall: Some(0.9)
            }
        );
        assert_eq!(
            report.aggregate.carried_recall_by_reason["pair"],
            crate::CarriedRecall {
                expected: 11,
                admitted: Some(9),
                recall: Some(9.0 / 11.0)
            }
        );
        assert_eq!(
            report.scenarios["second"].carried_recall_by_reason["pair"].recall,
            Some(0.0)
        );
        assert_eq!(
            report.scenarios["skipped"].carried_recall_by_reason["pair"],
            crate::CarriedRecall {
                expected: 0,
                admitted: None,
                recall: None
            }
        );
        assert_eq!(
            report.scenarios["skipped"].outcome.probes["skipped"].carried_recall_by_reason["pair"]
                .expected,
            1
        );
        assert_eq!(
            report.aggregate.carried_recall_by_reason["date"].recall,
            None
        );
        assert_eq!(
            report.scenarios["first"].carried_recall_by_reason["date"].recall,
            None
        );
        assert!(compare_continuity_reports(&report, &report).is_empty());
        let mut changed = report.clone();
        changed
            .aggregate
            .carried_recall_by_reason
            .get_mut("pair")
            .unwrap()
            .recall = Some(0.5);
        let changed_scenario = changed.scenarios.get_mut("first").unwrap();
        changed_scenario
            .carried_recall_by_reason
            .get_mut("pair")
            .unwrap()
            .recall = Some(0.5);
        changed_scenario
            .outcome
            .probes
            .get_mut("miss")
            .unwrap()
            .carried_recall_by_reason
            .get_mut("pair")
            .unwrap()
            .recall = None;
        let differences = compare_continuity_reports(&report, &changed);
        assert_eq!(differences.len(), 3);
        assert!(
            differences
                .iter()
                .any(|d| d.scenario_id.is_none() && d.field == "carried_recall_by_reason")
        );
        assert!(
            differences
                .iter()
                .any(|d| d.scenario_id.as_deref() == Some("first")
                    && d.field == "carried_recall_by_reason")
        );
        assert!(
            differences
                .iter()
                .any(|d| d.scenario_id.as_deref() == Some("first")
                    && d.field == "probe_carried_recall_by_reason")
        );
    }

    #[test]
    fn repeat_comparison_uses_assertion_identity_result_reason_and_invariant() {
        let mut outcome = ScenarioOutcome::executed();
        outcome.assertions = ["one", "two"]
            .map(|memory| AssertionResult {
                identity: crate::AssertionIdentity {
                    event_id: "probe".into(),
                    assertion: crate::AssertionSubject::Carried(memory.into()),
                },
                check: CheckResult::checked(true, "admitted"),
            })
            .to_vec();
        let report = assemble_continuity_report(ContinuityReportInput {
            traces: &[],
            outcomes: &BTreeMap::from([("scenario".into(), outcome)]),
            metric_family: &crate::continuity_metric_family(&Default::default(), &[]),
        })
        .unwrap();
        let mut repeat = report.clone();
        repeat
            .scenarios
            .get_mut("scenario")
            .unwrap()
            .outcome
            .assertions
            .reverse();
        assert!(compare_continuity_reports(&report, &repeat).is_empty());
        repeat
            .scenarios
            .get_mut("scenario")
            .unwrap()
            .outcome
            .assertions[0]
            .check = CheckResult::checked(false, "missing");
        let changed = compare_continuity_reports(&report, &repeat);
        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed[0].assertion.as_ref().unwrap().assertion,
            crate::AssertionSubject::Carried("two".into())
        );
        repeat = report.clone();
        repeat
            .scenarios
            .get_mut("scenario")
            .unwrap()
            .outcome
            .assertions[0]
            .check
            .reason = "different reason".into();
        assert_eq!(compare_continuity_reports(&report, &repeat).len(), 1);
        repeat = report.clone();
        repeat.aggregate.omission_reason_invariant =
            CheckResult::checked(false, "missing omission reason");
        assert_eq!(
            compare_continuity_reports(&report, &repeat)[0].field,
            "omission_reason_invariant"
        );
    }
}
