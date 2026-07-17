use std::collections::{BTreeMap, BTreeSet};

use cmem_eval_core::{
    MetricFamily, MetricsConfig, RetrievalRationaleCategory, RetrievedItem, mean, percentile,
    retrieval_metrics,
};
use serde_json::{Map, Value};

use crate::{ContinuityQueryTrace, ContinuityScenario, InteractionEvent, ScenarioPattern};

const GAP_BUCKETS: [&str; 3] = ["short", "medium", "long"];
const RATIONALE_CATEGORIES: [RetrievalRationaleCategory; 8] = [
    RetrievalRationaleCategory::Semantic,
    RetrievalRationaleCategory::Entity,
    RetrievalRationaleCategory::Thread,
    RetrievalRationaleCategory::Temporal,
    RetrievalRationaleCategory::Salience,
    RetrievalRationaleCategory::Scope,
    RetrievalRationaleCategory::Lifecycle,
    RetrievalRationaleCategory::GraphBound,
];
const CONTINUITY_METRICS: [&str; 14] = [
    "continuity_gap_days",
    "hub_context_share",
    "hub_expansion_relevant_hit_rate",
    "hub_fanout_utilization_mean",
    "correction_lifecycle_safe_admission_rate",
    "supersession_replacement_recall",
    "typed_rationale_coverage",
    "sampled_context_pollution_rate",
    "sampled_event_pollution_rate",
    "fanout_over_budget_count",
    "conservative_fallback_activation_count",
    "fanout_selected_cap_utilization_mean",
    "fanout_configured_cap_utilization_mean",
    "selectivity_score_mean",
];

pub fn continuity_metric_family(
    config: &MetricsConfig,
    scenarios: &[ContinuityScenario],
) -> MetricFamily {
    let mut required_metrics = CONTINUITY_METRICS
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    required_metrics.extend([
        "selectivity_score_p50".to_string(),
        "selectivity_score_p95".to_string(),
    ]);
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
    for entity_type in scenarios
        .iter()
        .flat_map(|scenario| scenario.entities.iter())
        .map(|entity| metric_slug(entity.entity_type.as_str()))
    {
        required_metrics.insert(format!("selectivity_score_mean_entity_kind_{entity_type}"));
    }
    MetricFamily::new("continuity", required_metrics)
}

pub fn insert_continuity_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
    config: &MetricsConfig,
) {
    let retrieved_ids = ranked_ids_for_gold(
        &trace.retrieval.items,
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
    if trace.pattern == ScenarioPattern::TemporalStructure {
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
    insert_fanout_metrics(out, scenario, trace);
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
                .items
                .iter()
                .filter(|item| item_matches_any(item, &hub_incident_ids))
                .count(),
            trace.retrieval.items.len(),
        ),
    );

    if let Some(categories) = &trace
        .retrieval
        .telemetry
        .rationale_categories_by_internal_id
    {
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
            .items
            .iter()
            .filter(|item| {
                categories
                    .get(&item.internal_id)
                    .is_some_and(|values| values.contains(&RetrievalRationaleCategory::Entity))
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

    if let Some(fanout) = &trace.retrieval.telemetry.fanout_utilization {
        let utilizations = fanout
            .iter()
            .filter(|entry| {
                entry
                    .root_external_id
                    .as_deref()
                    .is_some_and(|external_id| hub_ids.contains(external_id))
            })
            .filter_map(|entry| utilization(entry.retained_count, entry.selected_cap))
            .collect::<Vec<_>>();
        out.insert(
            "hub_fanout_utilization_mean".to_string(),
            option_number(mean(&utilizations)),
        );
    }
}

fn insert_correction_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
    retrieved_ids: &[String],
) {
    if scenario.pattern != ScenarioPattern::CorrectionChains {
        return;
    }
    let telemetry = &trace.retrieval.telemetry;
    if telemetry.trace_available
        && let Some(unsafe_count) = telemetry.unsafe_lifecycle_returned_count
    {
        out.insert(
            "correction_lifecycle_safe_admission_rate".to_string(),
            Value::from(if trace.retrieval.items.is_empty() {
                1.0
            } else {
                1.0 - unsafe_count.min(trace.retrieval.items.len()) as f64
                    / trace.retrieval.items.len() as f64
            }),
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
    let Some(categories) = &trace
        .retrieval
        .telemetry
        .rationale_categories_by_internal_id
    else {
        return;
    };
    let returned_categories = trace
        .retrieval
        .items
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
        .items
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
    if let Some(categories) = &trace
        .retrieval
        .telemetry
        .rationale_categories_by_internal_id
    {
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

fn insert_fanout_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
) {
    if let Some(fanout) = &trace.retrieval.telemetry.fanout_utilization {
        out.insert(
            "fanout_over_budget_count".to_string(),
            Value::from(
                fanout
                    .iter()
                    .filter(|entry| {
                        entry.selected_cap > entry.configured_cap
                            || entry.retained_count > entry.selected_cap
                    })
                    .count(),
            ),
        );
        let selected = fanout
            .iter()
            .filter_map(|entry| utilization(entry.retained_count, entry.selected_cap))
            .collect::<Vec<_>>();
        let configured = fanout
            .iter()
            .filter_map(|entry| utilization(entry.retained_count, entry.configured_cap))
            .collect::<Vec<_>>();
        out.insert(
            "fanout_selected_cap_utilization_mean".to_string(),
            option_number(mean(&selected)),
        );
        out.insert(
            "fanout_configured_cap_utilization_mean".to_string(),
            option_number(mean(&configured)),
        );
    }
    if let Some(selectivity) = &trace.retrieval.telemetry.selectivity_decisions {
        out.insert(
            "conservative_fallback_activation_count".to_string(),
            Value::from(selectivity.iter().filter(|entry| entry.fallback).count()),
        );
        let scores = selectivity
            .iter()
            .filter_map(|entry| entry.score)
            .collect::<Vec<_>>();
        out.insert(
            "selectivity_score_mean".to_string(),
            option_number(mean(&scores)),
        );
        out.insert(
            "selectivity_score_p50".to_string(),
            option_number(percentile(&scores, 50.0)),
        );
        out.insert(
            "selectivity_score_p95".to_string(),
            option_number(percentile(&scores, 95.0)),
        );
        let entity_types = scenario
            .entities
            .iter()
            .map(|entity| (entity.external_id.as_str(), entity.entity_type.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut by_type: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for entry in selectivity {
            if let (Some(external_id), Some(score)) = (&entry.root_external_id, entry.score)
                && let Some(entity_type) = entity_types.get(external_id.as_str())
            {
                by_type
                    .entry(metric_slug(entity_type))
                    .or_default()
                    .push(score);
            }
        }
        for (entity_type, scores) in by_type {
            out.insert(
                format!("selectivity_score_mean_entity_kind_{entity_type}"),
                option_number(mean(&scores)),
            );
        }
    }
}

fn insert_category_distribution(
    out: &mut Map<String, Value>,
    prefix: &str,
    categories: &[&[RetrievalRationaleCategory]],
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

fn utilization(retained: usize, cap: usize) -> Option<f64> {
    (cap > 0).then_some(retained as f64 / cap as f64)
}

fn option_number(value: Option<f64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn rationale_category_name(category: RetrievalRationaleCategory) -> &'static str {
    match category {
        RetrievalRationaleCategory::Semantic => "semantic",
        RetrievalRationaleCategory::Entity => "entity",
        RetrievalRationaleCategory::Thread => "thread",
        RetrievalRationaleCategory::Temporal => "temporal",
        RetrievalRationaleCategory::Salience => "salience",
        RetrievalRationaleCategory::Scope => "scope",
        RetrievalRationaleCategory::Lifecycle => "lifecycle",
        RetrievalRationaleCategory::GraphBound => "graph_bound",
    }
}

fn metric_slug(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContinuityEntityKind, EntityDeclaration, ExpectedRelevance};
    use chrono::{TimeZone, Utc};
    use cmem_eval_core::{
        RetrievalFanoutUtilization, RetrievalSelectivityDecision, RetrievalTelemetry,
        RetrievedContextPack,
    };

    fn item(id: &str, rank: usize) -> RetrievedItem {
        RetrievedItem {
            kind: "episode".to_string(),
            internal_id: format!("internal-{id}"),
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
            embedding: cmem_eval_core::ControllableSimilarityFixture {
                seed: 1,
                vector_size: 2,
                noise_magnitude: 0.0,
                clusters: BTreeMap::new(),
                concepts: BTreeMap::new(),
            },
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
        let rationale_categories_by_internal_id = BTreeMap::from([
            (
                "internal-relevant".to_string(),
                vec![
                    RetrievalRationaleCategory::Entity,
                    RetrievalRationaleCategory::Temporal,
                ],
            ),
            (
                "internal-sampled-negative".to_string(),
                vec![RetrievalRationaleCategory::Semantic],
            ),
        ]);
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
            retrieval: RetrievedContextPack {
                items,
                telemetry: RetrievalTelemetry {
                    trace_available: true,
                    suppressed_or_deleted_returned_count: Some(0),
                    superseded_current_returned_count: Some(0),
                    unsafe_lifecycle_returned_count: Some(0),
                    fanout_utilization: Some(vec![RetrievalFanoutUtilization {
                        root_internal_id: "root".to_string(),
                        root_object_type: "entity".to_string(),
                        root_external_id: Some("hub-person".to_string()),
                        relation: "mentions".to_string(),
                        object_type: "episode".to_string(),
                        configured_cap: 4,
                        selected_cap: 2,
                        retained_count: 1,
                        omitted_by_fanout_count: 3,
                    }]),
                    selectivity_decisions: Some(vec![RetrievalSelectivityDecision {
                        root_internal_id: "root".to_string(),
                        root_object_type: "entity".to_string(),
                        root_external_id: Some("hub-person".to_string()),
                        relation: "mentions".to_string(),
                        object_type: "episode".to_string(),
                        count_scope: "active".to_string(),
                        score: Some(0.25),
                        entity_count: Some(5),
                        global_count: Some(20),
                        support_factor: 0.5,
                        chosen_fanout: 2,
                        max_fanout: 4,
                        decision: "low_selectivity_supported".to_string(),
                        fallback: false,
                    }]),
                    rationale_categories_by_internal_id: Some(rationale_categories_by_internal_id),
                    ..RetrievalTelemetry::default()
                },
                ..RetrievedContextPack::default()
            },
        }
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
        trace
            .retrieval
            .telemetry
            .rationale_categories_by_internal_id
            .as_mut()
            .unwrap()
            .get_mut("internal-sampled-negative")
            .unwrap()
            .push(RetrievalRationaleCategory::Entity);
        let mut out = Map::new();
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        assert_eq!(out["hub_context_share"], 1.0);
        assert_eq!(out["hub_expansion_relevant_hit_rate"], 0.5);
        assert_eq!(out["hub_fanout_utilization_mean"], 0.5);
    }

    #[test]
    fn temporal_quality_reuses_hand_computed_recall() {
        let out = metrics(ScenarioPattern::TemporalStructure);
        assert_eq!(out["temporal_recall_fraction@5"], 1.0);
    }

    #[test]
    fn correction_safety_combines_lifecycle_telemetry_and_replacement_labels() {
        let out = metrics(ScenarioPattern::CorrectionChains);
        assert_eq!(out["correction_lifecycle_safe_admission_rate"], 1.0);
        assert_eq!(out["supersession_replacement_recall"], 1.0);
    }

    #[test]
    fn correction_safety_counts_overlapping_lifecycle_failures_once() {
        let scenario = scenario(ScenarioPattern::CorrectionChains);
        let mut trace = trace(ScenarioPattern::CorrectionChains);
        trace
            .retrieval
            .telemetry
            .suppressed_or_deleted_returned_count = Some(1);
        trace.retrieval.telemetry.superseded_current_returned_count = Some(1);
        trace.retrieval.telemetry.unsafe_lifecycle_returned_count = Some(1);
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(trace.retrieval.items.len(), 2);
        assert_eq!(out["correction_lifecycle_safe_admission_rate"], 0.5);
    }

    #[test]
    fn correction_metrics_stay_unsupported_for_unrelated_scenarios() {
        let scenario = scenario(ScenarioPattern::LongGapRecall);
        let trace = trace(ScenarioPattern::LongGapRecall);
        let family =
            continuity_metric_family(&MetricsConfig::default(), std::slice::from_ref(&scenario));
        let mut out = Map::new();
        cmem_eval_core::initialize_registry_metrics_for(&mut out, std::slice::from_ref(&family));

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
        trace.retrieval.items.push(item("unlabeled", 3));
        trace
            .retrieval
            .telemetry
            .rationale_categories_by_internal_id
            .as_mut()
            .unwrap()
            .insert(
                "internal-unlabeled".to_string(),
                vec![RetrievalRationaleCategory::Salience],
            );
        let mut out = Map::new();
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        assert_eq!(out["sampled_context_pollution_rate"], 0.5);
        assert_eq!(out["sampled_event_pollution_rate"], 0.5);
        assert_eq!(out["sampled_pollution_rationale_share_semantic"], 1.0);
    }

    #[test]
    fn event_pollution_deduplicates_surfaces_by_episode_root() {
        let scenario = scenario(ScenarioPattern::SurfaceContribution);
        let mut trace = trace(ScenarioPattern::SurfaceContribution);
        trace.retrieval.items = vec![
            RetrievedItem {
                kind: "episode".to_string(),
                internal_id: "relevant-episode".to_string(),
                external_id: Some("relevant".to_string()),
                episode_external_id: None,
                score: None,
                rank: 1,
                rationale: Vec::new(),
                text: None,
            },
            RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "relevant-observation".to_string(),
                external_id: Some("relevant:observation".to_string()),
                episode_external_id: Some("relevant".to_string()),
                score: None,
                rank: 2,
                rationale: Vec::new(),
                text: None,
            },
            RetrievedItem {
                kind: "episode".to_string(),
                internal_id: "negative-episode".to_string(),
                external_id: Some("sampled-negative".to_string()),
                episode_external_id: None,
                score: None,
                rank: 3,
                rationale: Vec::new(),
                text: None,
            },
        ];
        trace
            .retrieval
            .telemetry
            .rationale_categories_by_internal_id = None;
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["sampled_context_pollution_rate"], 1.0 / 3.0);
        assert_eq!(out["sampled_event_pollution_rate"], 0.5);
    }

    #[test]
    fn relevant_surface_identity_wins_over_a_sampled_negative_provenance_root() {
        let scenario = scenario(ScenarioPattern::CorrectionChains);
        let mut trace = trace(ScenarioPattern::CorrectionChains);
        trace.retrieval.items = vec![RetrievedItem {
            kind: "derived_memory".to_string(),
            internal_id: "replacement".to_string(),
            external_id: Some("relevant".to_string()),
            episode_external_id: Some("sampled-negative".to_string()),
            score: None,
            rank: 1,
            rationale: Vec::new(),
            text: None,
        }];
        trace
            .retrieval
            .telemetry
            .rationale_categories_by_internal_id = None;
        let mut out = Map::new();

        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());

        assert_eq!(out["sampled_context_pollution_rate"], 0.0);
        assert_eq!(out["sampled_event_pollution_rate"], 0.0);
    }

    #[test]
    fn fanout_discipline_reports_budgets_fallbacks_and_selectivity_distribution() {
        let out = metrics(ScenarioPattern::SelectiveEntity);
        assert_eq!(out["fanout_over_budget_count"], 0);
        assert_eq!(out["conservative_fallback_activation_count"], 0);
        assert_eq!(out["fanout_selected_cap_utilization_mean"], 0.5);
        assert_eq!(out["fanout_configured_cap_utilization_mean"], 0.25);
        assert_eq!(out["selectivity_score_mean"], 0.25);
        assert_eq!(out["selectivity_score_mean_entity_kind_person"], 0.25);
    }

    #[test]
    fn missing_telemetry_stays_null_in_the_registry_instead_of_false_zero() {
        let scenario = scenario(ScenarioPattern::SelectiveEntity);
        let mut trace = trace(ScenarioPattern::SelectiveEntity);
        trace.retrieval.telemetry = RetrievalTelemetry::default();
        let family =
            continuity_metric_family(&MetricsConfig::default(), std::slice::from_ref(&scenario));
        let mut out = Map::new();
        cmem_eval_core::initialize_registry_metrics_for(&mut out, std::slice::from_ref(&family));
        insert_continuity_metrics(&mut out, &scenario, &trace, &MetricsConfig::default());
        assert_eq!(out["fanout_over_budget_count"], Value::Null);
        assert_eq!(out["typed_rationale_coverage"], Value::Null);
        assert_eq!(
            out["selectivity_score_mean_entity_kind_person"],
            Value::Null
        );
    }
}
