use std::collections::{BTreeMap, BTreeSet};

use cmem_eval::{MetricFamily, MetricsConfig, RationaleCategory, RetrievedItem, retrieval_metrics};
use serde_json::{Map, Value};

use crate::{ContinuityQueryTrace, ContinuityScenario, InteractionEvent, ScenarioPattern};

const GAP_BUCKETS: [&str; 3] = ["short", "medium", "long"];
const RATIONALE_CATEGORIES: [RationaleCategory; 8] = [
    RationaleCategory::Semantic,
    RationaleCategory::Entity,
    RationaleCategory::Thread,
    RationaleCategory::Temporal,
    RationaleCategory::Salience,
    RationaleCategory::Scope,
    RationaleCategory::Lifecycle,
    RationaleCategory::GraphBound,
];
const CONTINUITY_METRICS: [&str; 8] = [
    "continuity_gap_days",
    "hub_context_share",
    "hub_expansion_relevant_hit_rate",
    "correction_lifecycle_safe_admission_rate",
    "supersession_replacement_recall",
    "typed_rationale_coverage",
    "sampled_context_pollution_rate",
    "sampled_event_pollution_rate",
];

pub fn continuity_metric_family(
    config: &MetricsConfig,
    _scenarios: &[ContinuityScenario],
) -> MetricFamily {
    let mut required_metrics = CONTINUITY_METRICS
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    for k in &config.ks_session {
        for bucket in GAP_BUCKETS {
            required_metrics.insert(format!("continuity_recall_fraction_gap_{bucket}@{k}"));
        }
        required_metrics.insert(format!("temporal_recall_fraction@{k}"));
    }
    for category in RATIONALE_CATEGORIES {
        required_metrics.insert(format!(
            "rationale_category_share_{}",
            rationale_category_name(category)
        ));
        required_metrics.insert(format!(
            "sampled_pollution_rationale_share_{}",
            rationale_category_name(category)
        ));
    }
    MetricFamily::new("continuity", required_metrics)
}

pub fn insert_continuity_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
    config: &MetricsConfig,
) {
    if trace.pattern == ScenarioPattern::Abstention {
        insert_pollution_metrics(out, trace);
        return;
    }

    let retrieved_ids = ranked_ids_for_gold(
        trace.retrieval.items(),
        &trace.expected.relevant_external_ids,
    );
    if let Some(gap_days) = gap_days(scenario, trace) {
        out.insert("continuity_gap_days".to_string(), Value::from(gap_days));
        let bucket = gap_bucket(gap_days);
        for k in &config.ks_session {
            if let Some(summary) =
                retrieval_metrics(&retrieved_ids, &trace.expected.relevant_external_ids, *k)
            {
                out.insert(
                    format!("continuity_recall_fraction_gap_{bucket}@{k}"),
                    Value::from(summary.recall_fraction),
                );
            }
        }
    }

    insert_hub_metrics(out, scenario, trace);
    if matches!(
        trace.pattern,
        ScenarioPattern::TemporalStructure | ScenarioPattern::TemporalPatterns
    ) {
        for k in &config.ks_session {
            if let Some(summary) =
                retrieval_metrics(&retrieved_ids, &trace.expected.relevant_external_ids, *k)
            {
                out.insert(
                    format!("temporal_recall_fraction@{k}"),
                    Value::from(summary.recall_fraction),
                );
            }
        }
    }
    insert_correction_metrics(out, scenario, trace, &retrieved_ids);
    insert_rationale_metrics(out, trace);
    insert_pollution_metrics(out, trace);
}

fn insert_hub_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
) {
    let hub_ids = scenario
        .entities
        .iter()
        .filter(|entity| entity.is_hub)
        .map(|entity| entity.external_id.as_str())
        .collect::<BTreeSet<_>>();
    if hub_ids.is_empty() {
        return;
    }
    let hub_incident_ids = scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Remember {
                external_id,
                entity_external_ids,
                ..
            } if entity_external_ids
                .iter()
                .any(|entity_id| hub_ids.contains(entity_id.as_str())) =>
            {
                Some(external_id.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    out.insert(
        "hub_context_share".to_string(),
        rate(
            trace
                .retrieval
                .items()
                .iter()
                .filter(|item| item_matches_any(item, &hub_incident_ids))
                .count(),
            trace.retrieval.items().len(),
        ),
    );

    if let Some(categories) = &rationale_categories(trace) {
        let relevant = trace
            .expected
            .relevant_external_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let sampled_irrelevant = trace
            .expected
            .irrelevant_external_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let labeled_entity_expansions = trace
            .retrieval
            .items()
            .iter()
            .filter(|item| {
                categories
                    .get(&item.internal_id)
                    .is_some_and(|values| values.contains(&RationaleCategory::Entity))
                    && (item_matches_any(item, &relevant)
                        || item_matches_any(item, &sampled_irrelevant))
            })
            .collect::<Vec<_>>();
        out.insert(
            "hub_expansion_relevant_hit_rate".to_string(),
            rate(
                labeled_entity_expansions
                    .iter()
                    .filter(|item| item_matches_any(item, &relevant))
                    .count(),
                labeled_entity_expansions.len(),
            ),
        );
    }
}

fn insert_correction_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
    retrieved_ids: &[String],
) {
    if !matches!(
        scenario.pattern,
        ScenarioPattern::CorrectionChains | ScenarioPattern::EntrenchedCorrection
    ) {
        return;
    }
    if let Some(rate) = cmem_eval::lifecycle_safe_admission_rate(
        trace.retrieval.items(),
        trace.retrieval.outcomes(),
    ) {
        out.insert(
            "correction_lifecycle_safe_admission_rate".to_string(),
            Value::from(rate),
        );
    }
    let labeled_replacements = scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Correct {
                replacement_external_id,
                timestamp,
                ..
            } if *timestamp <= trace.timestamp
                && trace
                    .expected
                    .relevant_external_ids
                    .contains(replacement_external_id) =>
            {
                Some(replacement_external_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if !labeled_replacements.is_empty() {
        let retrieved = retrieved_ids.iter().collect::<BTreeSet<_>>();
        out.insert(
            "supersession_replacement_recall".to_string(),
            Value::from(
                labeled_replacements
                    .iter()
                    .filter(|external_id| retrieved.contains(external_id))
                    .count() as f64
                    / labeled_replacements.len() as f64,
            ),
        );
    }
}

fn insert_rationale_metrics(out: &mut Map<String, Value>, trace: &ContinuityQueryTrace) {
    let Some(categories) = &rationale_categories(trace) else {
        return;
    };
    let returned_categories = trace
        .retrieval
        .items()
        .iter()
        .map(|item| {
            categories
                .get(&item.internal_id)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        })
        .collect::<Vec<_>>();
    out.insert(
        "typed_rationale_coverage".to_string(),
        Value::from(if returned_categories.is_empty() {
            1.0
        } else {
            returned_categories
                .iter()
                .filter(|values| !values.is_empty())
                .count() as f64
                / returned_categories.len() as f64
        }),
    );
    insert_category_distribution(out, "rationale_category_share", &returned_categories);
}

fn insert_pollution_metrics(out: &mut Map<String, Value>, trace: &ContinuityQueryTrace) {
    let relevant = trace
        .expected
        .relevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let sampled_irrelevant = trace
        .expected
        .irrelevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let labeled = trace
        .retrieval
        .items()
        .iter()
        .filter(|item| {
            item_matches_any(item, &relevant) || item_matches_any(item, &sampled_irrelevant)
        })
        .collect::<Vec<_>>();
    let pollution = labeled
        .iter()
        .filter(|item| {
            !item_matches_any(item, &relevant) && item_matches_any(item, &sampled_irrelevant)
        })
        .collect::<Vec<_>>();
    out.insert(
        "sampled_context_pollution_rate".to_string(),
        rate(pollution.len(), labeled.len()),
    );
    let mut labeled_event_roots = BTreeMap::new();
    for item in &labeled {
        let is_relevant = item_matches_any(item, &relevant);
        let is_pollution = item_matches_any(item, &sampled_irrelevant);
        let Some(root_external_id) = item
            .episode_external_id
            .as_deref()
            .or(item.external_id.as_deref())
        else {
            continue;
        };
        let root_is_pollution = labeled_event_roots
            .entry(root_external_id)
            .or_insert(is_pollution);
        if is_relevant {
            *root_is_pollution = false;
        }
    }
    out.insert(
        "sampled_event_pollution_rate".to_string(),
        rate(
            labeled_event_roots
                .values()
                .filter(|is_pollution| **is_pollution)
                .count(),
            labeled_event_roots.len(),
        ),
    );
    if let Some(categories) = &rationale_categories(trace) {
        let pollution_categories = pollution
            .iter()
            .map(|item| {
                categories
                    .get(&item.internal_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
            })
            .collect::<Vec<_>>();
        insert_category_distribution(
            out,
            "sampled_pollution_rationale_share",
            &pollution_categories,
        );
    }
}

pub(crate) fn rationale_categories(
    trace: &ContinuityQueryTrace,
) -> Option<BTreeMap<String, Vec<RationaleCategory>>> {
    if !trace
        .retrieval
        .outcomes()
        .iter()
        .any(|outcome| outcome.trace.is_some())
    {
        return None;
    }
    let mut categories: BTreeMap<String, Vec<RationaleCategory>> = BTreeMap::new();
    for assignment in trace
        .retrieval
        .outcomes()
        .iter()
        .filter_map(|outcome| outcome.trace.as_ref())
        .flat_map(|trace| &trace.section_assignments)
    {
        let values = categories
            .entry(assignment.object.id.to_string())
            .or_default();
        for category in &assignment.rationale_categories {
            if !values.contains(category) {
                values.push(*category);
            }
        }
    }
    Some(categories)
}

fn insert_category_distribution(
    out: &mut Map<String, Value>,
    prefix: &str,
    categories: &[&[RationaleCategory]],
) {
    let total = categories.iter().map(|values| values.len()).sum::<usize>();
    if total == 0 {
        return;
    }
    for category in RATIONALE_CATEGORIES {
        let count = categories
            .iter()
            .flat_map(|values| values.iter())
            .filter(|value| **value == category)
            .count();
        out.insert(
            format!("{prefix}_{}", rationale_category_name(category)),
            Value::from(count as f64 / total as f64),
        );
    }
}

fn ranked_ids_for_gold(items: &[RetrievedItem], gold_ids: &[String]) -> Vec<String> {
    let gold = gold_ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    items
        .iter()
        .map(|item| {
            [item.external_id.as_ref(), item.episode_external_id.as_ref()]
                .into_iter()
                .flatten()
                .find(|external_id| gold.contains(external_id.as_str()))
                .or(item.external_id.as_ref())
                .or(item.episode_external_id.as_ref())
                .cloned()
                .unwrap_or_else(|| format!("internal:{}.{}", item.kind, item.internal_id))
        })
        .collect()
}

fn gap_days(scenario: &ContinuityScenario, trace: &ContinuityQueryTrace) -> Option<f64> {
    let relevant = trace
        .expected
        .relevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Remember {
                external_id,
                timestamp,
                ..
            }
            | InteractionEvent::Link {
                external_id,
                timestamp,
                ..
            } if relevant.contains(external_id.as_str()) && *timestamp <= trace.timestamp => {
                Some(*timestamp)
            }
            InteractionEvent::Correct {
                replacement_external_id,
                timestamp,
                ..
            } if relevant.contains(replacement_external_id.as_str())
                && *timestamp <= trace.timestamp =>
            {
                Some(*timestamp)
            }
            _ => None,
        })
        .map(|timestamp| (trace.timestamp - timestamp).num_seconds() as f64 / 86_400.0)
        .max_by(|left, right| left.total_cmp(right))
}

fn gap_bucket(days: f64) -> &'static str {
    if days < 30.0 {
        "short"
    } else if days < 180.0 {
        "medium"
    } else {
        "long"
    }
}

fn item_matches_any(item: &RetrievedItem, ids: &BTreeSet<&str>) -> bool {
    item.external_id
        .as_deref()
        .is_some_and(|external_id| ids.contains(external_id))
        || item
            .episode_external_id
            .as_deref()
            .is_some_and(|external_id| ids.contains(external_id))
}

fn rate(numerator: usize, denominator: usize) -> Value {
    if denominator == 0 {
        Value::Null
    } else {
        Value::from(numerator as f64 / denominator as f64)
    }
}

fn rationale_category_name(category: RationaleCategory) -> &'static str {
    match category {
        RationaleCategory::Semantic => "semantic",
        RationaleCategory::Entity => "entity",
        RationaleCategory::Thread => "thread",
        RationaleCategory::Temporal => "temporal",
        RationaleCategory::Salience => "salience",
        RationaleCategory::Scope => "scope",
        RationaleCategory::Lifecycle => "lifecycle",
        RationaleCategory::GraphBound => "graph_bound",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContinuityEntityKind, ContinuityScenarioEmbedding, EntityDeclaration, ExpectedRelevance,
    };
    use chrono::{TimeZone, Utc};
    use cmem_eval::character_memory::{
        ContextPackSection, ContinuityContextPack, LifecycleFilterAction, LifecycleFilterDecision,
        LifecycleFilterReason, MemoryObjectRef, RetentionState, RetrievalRationale, RetrievalTrace,
        RetrieveOutcome, SectionAssignment, SectionAssignmentReason, SectionScoreComponents,
    };
    use cmem_eval::{ContextRenderer, ObjectType, RetrievedContextPack};

    fn object(id: &str) -> MemoryObjectRef {
        MemoryObjectRef::new(
            ObjectType::Episode,
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, id.as_bytes()),
        )
    }
    fn assignment(id: &str, categories: Vec<RationaleCategory>) -> SectionAssignment {
        SectionAssignment {
            object: object(id),
            section: ContextPackSection::RelevantEpisodes,
            rank: Some(1),
            reason: SectionAssignmentReason::Selected {
                scores: SectionScoreComponents {
                    final_score: 1.0,
                    vector_score: None,
                    vector_score_source: None,
                    graph_score: None,
                    salience_score: None,
                },
            },
            rationale_categories: categories,
        }
    }
    fn unsafe_decision(id: &str) -> LifecycleFilterDecision {
        LifecycleFilterDecision {
            object: object(id),
            retention_state: Some(RetentionState::Suppressed),
            is_current: Some(false),
            superseded_by: Vec::new(),
            action: LifecycleFilterAction::Included,
            reason: LifecycleFilterReason::SuppressedIncludedByPolicy,
        }
    }

    fn item(id: &str, rank: usize) -> RetrievedItem {
        RetrievedItem {
            kind: ObjectType::Episode,
            internal_id: object(id).id.to_string(),
            external_id: Some(id.to_string()),
            episode_external_id: None,
            score: Some(1.0 / rank as f64),
            rank,
            rationale: vec!["typed trace".to_string()],
            text: None,
        }
    }

    fn scenario(pattern: ScenarioPattern) -> ContinuityScenario {
        ContinuityScenario {
            fixture_id: "fixture".to_string(),
            namespace: "namespace".to_string(),
            pattern,
            entities: vec![EntityDeclaration {
                external_id: "hub-person".to_string(),
                entity_type: ContinuityEntityKind::Person,
                label: "Hub".to_string(),
                is_hub: true,
            }],
            embedding: ContinuityScenarioEmbedding::controllable_similarity_provider(
                cmem_eval::ControllableSimilarityFixture {
                    seed: 1,
                    vector_size: 2,
                    noise_magnitude: 0.0,
                    clusters: BTreeMap::new(),
                    concepts: BTreeMap::new(),
                },
            ),
            events: vec![
                InteractionEvent::Remember {
                    event_id: "remember".to_string(),
                    external_id: "relevant".to_string(),
                    timestamp: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
                    text: "relevant".to_string(),
                    surface_texts: None,
                    entity_external_ids: vec!["hub-person".to_string()],
                    thread: None,
                    salience: 1.0,
                },
                InteractionEvent::Remember {
                    event_id: "negative".to_string(),
                    external_id: "sampled-negative".to_string(),
                    timestamp: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
                    text: "negative".to_string(),
                    surface_texts: None,
                    entity_external_ids: vec!["hub-person".to_string()],
                    thread: None,
                    salience: 0.5,
                },
                InteractionEvent::Correct {
                    event_id: "correct".to_string(),
                    target_external_id: "old".to_string(),
                    replacement_external_id: "relevant".to_string(),
                    timestamp: Utc.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap(),
                    replacement_text: "corrected".to_string(),
                },
            ],
        }
    }

    fn trace(pattern: ScenarioPattern) -> ContinuityQueryTrace {
        let items = vec![item("relevant", 1), item("sampled-negative", 2)];
        let mut native_trace = RetrievalTrace::empty();
        native_trace.section_assignments = vec![
            assignment(
                "relevant",
                vec![RationaleCategory::Entity, RationaleCategory::Temporal],
            ),
            assignment("sampled-negative", vec![RationaleCategory::Semantic]),
        ];
        ContinuityQueryTrace {
            schema_version: crate::CONTINUITY_TRACE_SCHEMA_VERSION.to_string(),
            fixture_id: "fixture".to_string(),
            namespace: "namespace".to_string(),
            pattern,
            event_id: "query".to_string(),
            query_id: "query".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            query: "query".to_string(),
            expected: ExpectedRelevance {
                relevant_external_ids: vec!["relevant".to_string()],
                irrelevant_external_ids: vec!["sampled-negative".to_string()],
            },
            history_text: String::new(),
            retrieval: RetrievedContextPack::from_ranked_items(
                items,
                vec![RetrieveOutcome {
                    pack: ContinuityContextPack::empty(),
                    rationale: RetrievalRationale::new("test"),
                    trace: Some(native_trace),
                }],
                ContextRenderer::PlainText,
            ),
            write_outcomes: Vec::new(),
            link_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
        }
    }

    fn mutate_trace(
        trace: &mut ContinuityQueryTrace,
        mutate: impl FnOnce(&mut Option<RetrievalTrace>),
    ) {
        let (items, _, _, _, mut outcomes) = std::mem::take(&mut trace.retrieval).into_parts();
        mutate(&mut outcomes[0].trace);
        trace.retrieval =
            RetrievedContextPack::from_ranked_items(items, outcomes, ContextRenderer::PlainText);
    }

    fn replace_items(trace: &mut ContinuityQueryTrace, items: Vec<RetrievedItem>) {
        let (_, _, _, _, telemetry) = std::mem::take(&mut trace.retrieval).into_parts();
        trace.retrieval =
            RetrievedContextPack::from_ranked_items(items, telemetry, ContextRenderer::PlainText);
    }

    fn metrics(pattern: ScenarioPattern) -> Map<String, Value> {
        let scenario = scenario(pattern);
        let trace = trace(pattern);
        let mut out = Map::new();
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        out
    }

    #[test]
    fn continuity_recall_is_bucketed_by_hand_computed_gap() {
        let out = metrics(ScenarioPattern::LongGapRecall);
        assert_eq!(out["continuity_gap_days"], 365.0);
        assert_eq!(out["continuity_recall_fraction_gap_long@5"], 1.0);
        assert!(out.get("continuity_recall_fraction_gap_short@5").is_none());
    }

    #[test]
    fn entity_continuity_measures_share_hits_and_cap_utilization() {
        let scenario = scenario(ScenarioPattern::RecurringHubEntity);
        let mut trace = trace(ScenarioPattern::RecurringHubEntity);
        mutate_trace(&mut trace, |telemetry| {
            telemetry.as_mut().unwrap().section_assignments[1]
                .rationale_categories
                .push(RationaleCategory::Entity);
        });
        let mut out = Map::new();
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        assert_eq!(out["hub_context_share"], 1.0);
        assert_eq!(out["hub_expansion_relevant_hit_rate"], 0.5);
    }

    #[test]
    fn temporal_quality_reuses_hand_computed_recall() {
        let out = metrics(ScenarioPattern::TemporalStructure);
        assert_eq!(out["temporal_recall_fraction@5"], 1.0);
    }

    #[test]
    fn temporal_patterns_route_to_temporal_recall_with_hand_computed_expectation() {
        let scenario = scenario(ScenarioPattern::TemporalPatterns);
        let mut trace = trace(ScenarioPattern::TemporalPatterns);
        trace
            .expected
            .relevant_external_ids
            .push("relevant-not-returned".to_string());
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["temporal_recall_fraction@5"], 0.5);
    }

    #[test]
    fn correction_safety_combines_lifecycle_telemetry_and_replacement_labels() {
        let out = metrics(ScenarioPattern::CorrectionChains);
        assert_eq!(out["correction_lifecycle_safe_admission_rate"], 1.0);
        assert_eq!(out["supersession_replacement_recall"], 1.0);
    }

    #[test]
    fn entrenched_correction_routes_to_correction_metrics_with_hand_computed_expectations() {
        let scenario = scenario(ScenarioPattern::EntrenchedCorrection);
        let mut trace = trace(ScenarioPattern::EntrenchedCorrection);
        mutate_trace(&mut trace, |telemetry| {
            telemetry
                .as_mut()
                .unwrap()
                .lifecycle_filter_decisions
                .push(unsafe_decision("relevant"));
        });
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(trace.retrieval.items().len(), 2);
        assert_eq!(out["correction_lifecycle_safe_admission_rate"], 0.5);
        assert_eq!(out["supersession_replacement_recall"], 1.0);
    }

    #[test]
    fn correction_safety_counts_overlapping_lifecycle_failures_once() {
        let scenario = scenario(ScenarioPattern::CorrectionChains);
        let mut trace = trace(ScenarioPattern::CorrectionChains);
        mutate_trace(&mut trace, |telemetry| {
            telemetry
                .as_mut()
                .unwrap()
                .lifecycle_filter_decisions
                .push(unsafe_decision("relevant"));
        });
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(trace.retrieval.items().len(), 2);
        assert_eq!(out["correction_lifecycle_safe_admission_rate"], 0.5);
    }

    #[test]
    fn correction_metrics_stay_unsupported_for_unrelated_scenarios() {
        let scenario = scenario(ScenarioPattern::LongGapRecall);
        let trace = trace(ScenarioPattern::LongGapRecall);
        let family =
            continuity_metric_family(&MetricsConfig::default(), std::slice::from_ref(&scenario));
        let mut out = Map::new();
        cmem_eval::initialize_registry_metrics_for(&mut out, std::slice::from_ref(&family));

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["correction_lifecycle_safe_admission_rate"], Value::Null);
        assert_eq!(out["supersession_replacement_recall"], Value::Null);
    }

    #[test]
    fn rationale_quality_uses_typed_category_assignments() {
        let out = metrics(ScenarioPattern::RecurringHubEntity);
        assert_eq!(out["typed_rationale_coverage"], 1.0);
        assert_eq!(out["rationale_category_share_entity"], 1.0 / 3.0);
        assert_eq!(out["rationale_category_share_temporal"], 1.0 / 3.0);
        assert_eq!(out["rationale_category_share_semantic"], 1.0 / 3.0);
    }

    #[test]
    fn sampled_pollution_does_not_classify_unlabeled_items_as_negative() {
        let scenario = scenario(ScenarioPattern::MixedSalienceAccumulation);
        let mut trace = trace(ScenarioPattern::MixedSalienceAccumulation);
        let mut items = trace.retrieval.items().to_vec();
        items.push(item("unlabeled", 3));
        replace_items(&mut trace, items);
        mutate_trace(&mut trace, |telemetry| {
            telemetry
                .as_mut()
                .unwrap()
                .section_assignments
                .push(assignment("unlabeled", vec![RationaleCategory::Salience]));
        });
        let mut out = Map::new();
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        assert_eq!(out["sampled_context_pollution_rate"], 0.5);
        assert_eq!(out["sampled_event_pollution_rate"], 0.5);
        assert_eq!(out["sampled_pollution_rationale_share_semantic"], 1.0);
    }

    #[test]
    fn abstention_is_scored_with_pollution_metrics_only() {
        let scenario = scenario(ScenarioPattern::Abstention);
        let mut trace = trace(ScenarioPattern::Abstention);
        trace.expected.relevant_external_ids.clear();
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["sampled_context_pollution_rate"], 1.0);
        assert_eq!(out["sampled_event_pollution_rate"], 1.0);
        assert!(out.get("continuity_gap_days").is_none());
        assert!(out.get("hub_context_share").is_none());
        assert!(out.get("typed_rationale_coverage").is_none());
        assert!(out.get("fanout_over_budget_count").is_none());
    }

    #[test]
    fn event_pollution_deduplicates_surfaces_by_episode_root() {
        let scenario = scenario(ScenarioPattern::SurfaceContribution);
        let mut trace = trace(ScenarioPattern::SurfaceContribution);
        replace_items(
            &mut trace,
            vec![
                RetrievedItem {
                    kind: ObjectType::Episode,
                    internal_id: "relevant-episode".to_string(),
                    external_id: Some("relevant".to_string()),
                    episode_external_id: None,
                    score: None,
                    rank: 1,
                    rationale: Vec::new(),
                    text: None,
                },
                RetrievedItem {
                    kind: ObjectType::Observation,
                    internal_id: "relevant-observation".to_string(),
                    external_id: Some("relevant:observation".to_string()),
                    episode_external_id: Some("relevant".to_string()),
                    score: None,
                    rank: 2,
                    rationale: Vec::new(),
                    text: None,
                },
                RetrievedItem {
                    kind: ObjectType::Episode,
                    internal_id: "negative-episode".to_string(),
                    external_id: Some("sampled-negative".to_string()),
                    episode_external_id: None,
                    score: None,
                    rank: 3,
                    rationale: Vec::new(),
                    text: None,
                },
            ],
        );
        mutate_trace(&mut trace, |telemetry| {
            *telemetry = None;
        });
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["sampled_context_pollution_rate"], 1.0 / 3.0);
        assert_eq!(out["sampled_event_pollution_rate"], 0.5);
    }

    #[test]
    fn relevant_surface_identity_wins_over_a_sampled_negative_provenance_root() {
        let scenario = scenario(ScenarioPattern::CorrectionChains);
        let mut trace = trace(ScenarioPattern::CorrectionChains);
        replace_items(
            &mut trace,
            vec![RetrievedItem {
                kind: ObjectType::DerivedMemory,
                internal_id: "replacement".to_string(),
                external_id: Some("relevant".to_string()),
                episode_external_id: Some("sampled-negative".to_string()),
                score: None,
                rank: 1,
                rationale: Vec::new(),
                text: None,
            }],
        );
        mutate_trace(&mut trace, |telemetry| {
            *telemetry = None;
        });
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["sampled_context_pollution_rate"], 0.0);
        assert_eq!(out["sampled_event_pollution_rate"], 0.0);
    }

    #[test]
    fn missing_telemetry_stays_null_in_the_registry_instead_of_false_zero() {
        let scenario = scenario(ScenarioPattern::SelectiveEntity);
        let mut trace = trace(ScenarioPattern::SelectiveEntity);
        mutate_trace(&mut trace, |telemetry| {
            *telemetry = None;
        });
        let family =
            continuity_metric_family(&MetricsConfig::default(), std::slice::from_ref(&scenario));
        let mut out = Map::new();
        cmem_eval::initialize_registry_metrics_for(&mut out, std::slice::from_ref(&family));
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        assert_eq!(out["typed_rationale_coverage"], Value::Null);
    }
}
