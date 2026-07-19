use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cmem_eval_core::{ControllableSimilarityFixture, SimilarityConceptFixture};

use crate::{
    CONTINUITY_FIXTURE_SCHEMA_VERSION, ContinuityEntityKind, ContinuityFixtureSet,
    ContinuityScenario, ContinuityScenarioEmbedding, EntityDeclaration, ExpectedRelevance,
    InteractionEvent, RememberSurfaceTexts, ScenarioPattern, ThreadMembership,
};

pub const CHECKED_FIXTURE_SEED: u64 = 0x0000_0000_0135_2768;
const EMBEDDING_VECTOR_SIZE: usize = 8;
const HUB_SCALE_INCIDENT_COUNT: usize = 48;

pub fn generate_fixture_set(seed: u64) -> Result<ContinuityFixtureSet> {
    let fixtures = ContinuityFixtureSet {
        schema_version: CONTINUITY_FIXTURE_SCHEMA_VERSION,
        seed,
        scenarios: vec![
            long_gap_recall(seed)?,
            recurring_hub_entity(seed)?,
            hub_scale(seed)?,
            selective_entity(seed)?,
            correction_chains(seed)?,
            thread_drift(seed)?,
            temporal_structure(seed)?,
            mixed_salience_accumulation(seed)?,
            cross_store_stress(seed)?,
            surface_contribution(seed)?,
            graded_similarity(seed)?,
            combined_life(seed)?,
            temporal_patterns(seed)?,
            entrenched_correction(seed)?,
            autobiographical(seed)?,
        ],
    };
    fixtures.validate()?;
    Ok(fixtures)
}

fn long_gap_recall(seed: u64) -> Result<ContinuityScenario> {
    let id = "long-gap-recall";
    let target = "A dormant project uses the cobalt protocol.";
    let distractor = "A recent project uses the amber protocol.";
    let query_text = "Which protocol belongs to the dormant project?";
    scenario(
        seed,
        id,
        ScenarioPattern::LongGapRecall,
        standard_entities(false),
        vec![
            remember(
                1,
                "memory-dormant",
                "2025-01-05T10:00:00Z",
                target,
                vec!["entity-person"],
                None,
                0.8,
            )?,
            remember(
                2,
                "memory-recent",
                "2025-06-05T10:00:00Z",
                distractor,
                vec!["entity-organization"],
                None,
                0.6,
            )?,
            query(
                3,
                "query-long-gap",
                "2025-12-20T10:00:00Z",
                query_text,
                vec!["memory-dormant"],
                vec!["memory-recent"],
            )?,
        ],
        concepts([
            ("dormant", "target", vec![target, query_text]),
            ("recent", "distractor", vec![distractor]),
        ]),
    )
}

fn recurring_hub_entity(seed: u64) -> Result<ContinuityScenario> {
    let id = "recurring-hub-entity";
    let entities = vec![
        entity("entity-person", ContinuityEntityKind::Person, "Hub A", true),
        entity(
            "entity-organization",
            ContinuityEntityKind::Organization,
            "Hub B",
            true,
        ),
        entity(
            "entity-location",
            ContinuityEntityKind::Location,
            "Hub C",
            true,
        ),
    ];
    let mut events = Vec::new();
    let mut hub_inputs = Vec::new();
    for index in 0..6 {
        let text = format!("Hub incident {index} connects all declared entity kinds.");
        hub_inputs.push(text.clone());
        events.push(remember(
            index + 1,
            &format!("hub-memory-{index}"),
            &format!("2025-{:02}-10T10:00:00Z", index + 1),
            &text,
            vec!["entity-person", "entity-organization", "entity-location"],
            None,
            0.5,
        )?);
    }
    let query_text = "Which incident most recently connected every hub?";
    hub_inputs.push(query_text.to_string());
    events.push(query(
        7,
        "query-hub",
        "2025-07-10T10:00:00Z",
        query_text,
        vec!["hub-memory-5"],
        vec![],
    )?);
    scenario(
        seed,
        id,
        ScenarioPattern::RecurringHubEntity,
        entities,
        events,
        concepts([(
            "hub",
            "hub",
            hub_inputs.iter().map(String::as_str).collect(),
        )]),
    )
}

fn hub_scale(seed: u64) -> Result<ContinuityScenario> {
    let id = "hub-scale";
    let entities = vec![
        entity(
            "hub-scale-person",
            ContinuityEntityKind::Person,
            "Scale Hub A",
            true,
        ),
        entity(
            "hub-scale-organization",
            ContinuityEntityKind::Organization,
            "Scale Hub B",
            true,
        ),
        entity(
            "hub-scale-location",
            ContinuityEntityKind::Location,
            "Scale Hub C",
            true,
        ),
    ];
    let cluster_ids = [
        "hub-scale-query",
        "hub-scale-secondary",
        "hub-scale-tertiary",
        "hub-scale-quaternary",
    ];
    let salience_levels = [0.15, 0.35, 0.65, 0.95];
    let mut cluster_inputs: [Vec<String>; 4] = std::array::from_fn(|_| Vec::new());
    let mut events = Vec::with_capacity(HUB_SCALE_INCIDENT_COUNT + 2);

    for index in 0..HUB_SCALE_INCIDENT_COUNT {
        let cluster_index = match index {
            0..=3 => 0,
            4..=13 => 1,
            14..=30 => 2,
            _ => 3,
        };
        let entity_id = match cluster_index {
            0 => "hub-scale-person",
            1 => "hub-scale-location",
            _ => "hub-scale-organization",
        };
        let text =
            format!("Binding-scale hub incident {index:02} records routine continuity context.");
        cluster_inputs[cluster_index].push(text.clone());
        let month = index / 24 + 1;
        let day = index % 24 + 1;
        events.push(remember(
            index + 1,
            &format!("hub-scale-memory-{index:02}"),
            &format!("2025-{month:02}-{day:02}T10:00:00Z"),
            &text,
            vec![entity_id],
            None,
            salience_levels[index % salience_levels.len()],
        )?);
    }

    let probe_id = "hub-scale-dormant-probe";
    let probe_text = "The dormant graph-only marker linked to Scale Hub C is obsidian-seven.";
    events.push(remember(
        HUB_SCALE_INCIDENT_COUNT + 1,
        probe_id,
        "2025-03-01T10:00:00Z",
        probe_text,
        vec!["hub-scale-location"],
        None,
        0.8,
    )?);
    let query_text = "Which dormant graph-only marker is linked to Scale Hub C?";
    cluster_inputs[0].push(query_text.to_string());
    events.push(query(
        HUB_SCALE_INCIDENT_COUNT + 2,
        "query-hub-scale",
        "2025-04-01T10:00:00Z",
        query_text,
        vec![probe_id],
        vec![],
    )?);

    let mut embedding_concepts = cluster_ids
        .into_iter()
        .zip(cluster_inputs)
        .enumerate()
        .map(|(index, (cluster, inputs))| {
            (
                format!("hub-scale-{index}"),
                SimilarityConceptFixture {
                    cluster: cluster.to_string(),
                    inputs,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    embedding_concepts.insert(
        "hub-scale-probe".to_string(),
        SimilarityConceptFixture {
            cluster: "hub-scale-probe".to_string(),
            inputs: vec![probe_text.to_string()],
        },
    );

    let mut scenario = scenario(
        seed,
        id,
        ScenarioPattern::HubScale,
        entities,
        events,
        embedding_concepts,
    )?;
    correlate_cluster(
        &mut scenario,
        "hub-scale-query",
        "hub-scale-secondary",
        0.8,
        0.6,
    )?;
    correlate_cluster(
        &mut scenario,
        "hub-scale-query",
        "hub-scale-tertiary",
        0.6,
        0.8,
    )?;
    correlate_cluster(
        &mut scenario,
        "hub-scale-query",
        "hub-scale-quaternary",
        0.28,
        0.96,
    )?;
    scenario.validate()?;
    Ok(scenario)
}

fn correlate_cluster(
    scenario: &mut ContinuityScenario,
    query_cluster: &str,
    target_cluster: &str,
    query_component: f32,
    target_component: f32,
) -> Result<()> {
    let embedding = scenario
        .embedding
        .controllable_similarity_mut()
        .context("cluster correlation requires a controllable-similarity scenario")?;
    let query_dimension = embedding
        .clusters
        .get(query_cluster)
        .with_context(|| format!("missing query embedding cluster {query_cluster:?}"))?
        .iter()
        .position(|component| *component == 1.0)
        .context("query cluster has no deterministic basis dimension")?;
    let target = embedding
        .clusters
        .get_mut(target_cluster)
        .with_context(|| format!("missing target embedding cluster {target_cluster:?}"))?;
    let target_dimension = target
        .iter()
        .position(|component| *component == 1.0)
        .context("target cluster has no deterministic basis dimension")?;
    target.fill(0.0);
    target[query_dimension] = query_component;
    target[target_dimension] = target_component;
    Ok(())
}

fn selective_entity(seed: u64) -> Result<ContinuityScenario> {
    let id = "selective-entity";
    let rare = "A rare location carries the violet access token.";
    let common = "A frequent organization carries routine status updates.";
    let query_text = "Which access token belongs to the rare location?";
    scenario(
        seed,
        id,
        ScenarioPattern::SelectiveEntity,
        standard_entities(false),
        vec![
            remember(
                1,
                "common-memory",
                "2025-01-01T09:00:00Z",
                common,
                vec!["entity-organization"],
                None,
                0.2,
            )?,
            remember(
                2,
                "rare-memory",
                "2025-05-01T09:00:00Z",
                rare,
                vec!["entity-location"],
                None,
                0.95,
            )?,
            query(
                3,
                "query-selective",
                "2025-09-01T09:00:00Z",
                query_text,
                vec!["rare-memory"],
                vec!["common-memory"],
            )?,
        ],
        concepts([
            ("rare", "target", vec![rare, query_text]),
            ("common", "distractor", vec![common]),
        ]),
    )
}

fn correction_chains(seed: u64) -> Result<ContinuityScenario> {
    let id = "correction-chains";
    let original = "The delivery window is Monday.";
    let first = "The delivery window is Tuesday.";
    let final_text = "The delivery window is Thursday.";
    let query_text = "What is the corrected delivery window?";
    scenario(
        seed,
        id,
        ScenarioPattern::CorrectionChains,
        standard_entities(false),
        vec![
            remember(
                1,
                "delivery-v1",
                "2025-01-01T08:00:00Z",
                original,
                vec!["entity-organization"],
                None,
                0.7,
            )?,
            correct(
                2,
                "delivery-v1",
                "delivery-v2",
                "2025-02-01T08:00:00Z",
                first,
            )?,
            correct(
                3,
                "delivery-v2",
                "delivery-v3",
                "2025-03-01T08:00:00Z",
                final_text,
            )?,
            forget(
                4,
                vec!["delivery-v1", "delivery-v1:observation"],
                "2025-03-02T08:00:00Z",
                false,
                false,
            )?,
            query(
                5,
                "query-correction",
                "2025-04-01T08:00:00Z",
                query_text,
                vec!["delivery-v3"],
                vec!["delivery-v1", "delivery-v2"],
            )?,
        ],
        concepts([
            ("old", "superseded", vec![original, first]),
            ("current", "current", vec![final_text, query_text]),
        ]),
    )
}

fn thread_drift(seed: u64) -> Result<ContinuityScenario> {
    let id = "thread-drift";
    let texts = [
        "Thread starts with a focused migration.",
        "Thread broadens to deployment details.",
        "Thread drifts into unrelated travel planning.",
    ];
    let query_text = "What was the thread's original focus?";
    scenario(
        seed,
        id,
        ScenarioPattern::ThreadDrift,
        standard_entities(false),
        vec![
            remember(
                1,
                "thread-focus",
                "2025-01-01T08:00:00Z",
                texts[0],
                vec!["entity-organization"],
                Some(("thread-1", 0.95)),
                0.8,
            )?,
            remember(
                2,
                "thread-broader",
                "2025-02-01T08:00:00Z",
                texts[1],
                vec!["entity-organization"],
                Some(("thread-1", 0.65)),
                0.5,
            )?,
            remember(
                3,
                "thread-drifted",
                "2025-03-01T08:00:00Z",
                texts[2],
                vec!["entity-location"],
                Some(("thread-1", 0.25)),
                0.2,
            )?,
            query(
                4,
                "query-thread",
                "2025-04-01T08:00:00Z",
                query_text,
                vec!["thread-focus"],
                vec!["thread-drifted"],
            )?,
        ],
        concepts([
            ("focus", "target", vec![texts[0], query_text]),
            ("drift", "distractor", vec![texts[1], texts[2]]),
        ]),
    )
}

fn temporal_structure(seed: u64) -> Result<ContinuityScenario> {
    let id = "temporal-structure";
    let old = "The archive was stored in the west wing in January.";
    let current = "The archive moved to the east wing in October.";
    let query_text = "Where was the archive after the October move?";
    scenario(
        seed,
        id,
        ScenarioPattern::TemporalStructure,
        standard_entities(false),
        vec![
            remember(
                1,
                "archive-january",
                "2025-01-15T12:00:00Z",
                old,
                vec!["entity-location"],
                None,
                0.6,
            )?,
            remember(
                2,
                "archive-october",
                "2025-10-15T12:00:00Z",
                current,
                vec!["entity-location"],
                None,
                0.8,
            )?,
            query(
                3,
                "query-temporal",
                "2025-11-15T12:00:00Z",
                query_text,
                vec!["archive-october"],
                vec![],
            )?,
        ],
        concepts([
            ("old", "past", vec![old]),
            ("current", "current", vec![current, query_text]),
        ]),
    )
}

fn mixed_salience_accumulation(seed: u64) -> Result<ContinuityScenario> {
    let id = "mixed-salience-accumulation";
    let low = "A low-salience routine note was recorded.";
    let medium = "A medium-salience schedule change was recorded.";
    let high = "A high-salience emergency access code is delta-nine.";
    let query_text = "What is the emergency access code?";
    scenario(
        seed,
        id,
        ScenarioPattern::MixedSalienceAccumulation,
        standard_entities(false),
        vec![
            remember(
                1,
                "salience-low",
                "2025-01-01T07:00:00Z",
                low,
                vec!["entity-person"],
                None,
                0.1,
            )?,
            remember(
                2,
                "salience-medium",
                "2025-02-01T07:00:00Z",
                medium,
                vec!["entity-organization"],
                None,
                0.5,
            )?,
            remember(
                3,
                "salience-high",
                "2025-03-01T07:00:00Z",
                high,
                vec!["entity-location"],
                None,
                0.95,
            )?,
            query(
                4,
                "query-salience",
                "2025-08-01T07:00:00Z",
                query_text,
                vec!["salience-high"],
                vec!["salience-low", "salience-medium"],
            )?,
        ],
        concepts([
            ("signal", "target", vec![high, query_text]),
            ("routine", "distractor", vec![low, medium]),
        ]),
    )
}

fn cross_store_stress(seed: u64) -> Result<ContinuityScenario> {
    let id = "cross-store-stress";
    let text = "The persistent cross-store marker is quartz-seven.";
    let query_text = "What marker must survive the restart?";
    scenario(
        seed,
        id,
        ScenarioPattern::CrossStoreStress,
        standard_entities(false),
        vec![
            remember(
                1,
                "restart-marker",
                "2025-01-01T06:00:00Z",
                text,
                vec!["entity-person", "entity-organization"],
                None,
                0.9,
            )?,
            link(
                2,
                "restart-link",
                "2025-01-01T06:05:00Z",
                "entity-person",
                "restart-marker",
            )?,
            InteractionEvent::Restart {
                event_id: "event-003".into(),
                timestamp: timestamp("2025-01-01T06:10:00Z")?,
                reopen_graph: true,
                reopen_stats: true,
            },
            query(
                4,
                "query-restart",
                "2025-01-01T06:15:00Z",
                query_text,
                vec!["restart-marker"],
                vec!["restart-link"],
            )?,
        ],
        concepts([("marker", "target", vec![text, query_text])]),
    )
}

fn surface_contribution(seed: u64) -> Result<ContinuityScenario> {
    let id = "surface-contribution";
    let relevant_episode = "During route planning, the crew reviewed the north beacon passage.";
    let relevant_observation =
        "The navigator said the north beacon passage stays clear after dusk.";
    let relevant_derived = "The character trusts the north beacon passage for night travel.";
    let irrelevant_episode = "During meal planning, the crew reviewed the galley provisions.";
    let irrelevant_observation = "The cook said fennel soup is available after dusk.";
    let irrelevant_derived = "The character prefers fennel soup for the evening meal.";
    let query_text = "Which passage does the character trust for night travel?";
    scenario(
        seed,
        id,
        ScenarioPattern::SurfaceContribution,
        standard_entities(false),
        vec![
            remember_with_surfaces(
                1,
                "surface-relevant",
                "2025-01-01T05:00:00Z",
                RememberSurfaceTexts {
                    episode: relevant_episode.into(),
                    observation: relevant_observation.into(),
                    derived: relevant_derived.into(),
                },
                vec!["entity-location"],
                0.8,
            )?,
            remember_with_surfaces(
                2,
                "surface-irrelevant",
                "2025-01-02T05:00:00Z",
                RememberSurfaceTexts {
                    episode: irrelevant_episode.into(),
                    observation: irrelevant_observation.into(),
                    derived: irrelevant_derived.into(),
                },
                vec!["entity-person"],
                0.5,
            )?,
            query(
                3,
                "query-surface-contribution",
                "2025-02-01T05:00:00Z",
                query_text,
                vec!["surface-relevant"],
                vec!["surface-irrelevant"],
            )?,
        ],
        concepts([
            (
                "relevant-event",
                "target",
                vec![
                    relevant_episode,
                    relevant_observation,
                    relevant_derived,
                    query_text,
                ],
            ),
            (
                "irrelevant-event",
                "distractor",
                vec![
                    irrelevant_episode,
                    irrelevant_observation,
                    irrelevant_derived,
                ],
            ),
        ]),
    )
}

fn graded_similarity(seed: u64) -> Result<ContinuityScenario> {
    let target = "After calibrating the aurora camera, Iris left her blue field notebook in the east instrument cabinet.";
    let near_miss = "After calibrating the meteor camera, Iris left her amber field notebook in the west instrument cabinet.";
    let second_near_miss = "Before the aurora camera test, Mara returned a blue maintenance binder to the east archive shelf.";
    let background =
        "The harbor bakery served cardamom rolls while rain moved across the fishing pier.";
    let query_text =
        "Where did Iris put her blue field notebook after the aurora camera calibration?";
    frozen_scenario(
        seed,
        "graded-similarity",
        ScenarioPattern::GradedSimilarity,
        vec![
            entity(
                "graded-character",
                ContinuityEntityKind::Person,
                "Iris Vale",
                false,
            ),
            entity(
                "graded-colleague",
                ContinuityEntityKind::Person,
                "Mara Chen",
                false,
            ),
            entity(
                "graded-observatory",
                ContinuityEntityKind::Organization,
                "Northlight Observatory",
                true,
            ),
            entity(
                "graded-cabinet",
                ContinuityEntityKind::Location,
                "East instrument cabinet",
                false,
            ),
        ],
        vec![
            remember(
                1,
                "graded-target",
                "2025-01-18T19:10:00Z",
                target,
                vec!["graded-character", "graded-observatory", "graded-cabinet"],
                Some(("aurora-calibration", 0.96)),
                0.9,
            )?,
            remember(
                2,
                "graded-near-miss",
                "2025-02-11T18:45:00Z",
                near_miss,
                vec!["graded-character", "graded-observatory"],
                Some(("meteor-calibration", 0.91)),
                0.7,
            )?,
            remember(
                3,
                "graded-second-near-miss",
                "2025-03-03T10:20:00Z",
                second_near_miss,
                vec!["graded-colleague", "graded-observatory"],
                Some(("aurora-calibration", 0.62)),
                0.5,
            )?,
            remember(
                4,
                "graded-background",
                "2025-04-09T07:30:00Z",
                background,
                vec!["graded-character"],
                None,
                0.2,
            )?,
            query(
                5,
                "query-graded-similarity",
                "2025-07-21T09:00:00Z",
                query_text,
                vec!["graded-target"],
                vec![
                    "graded-near-miss",
                    "graded-second-near-miss",
                    "graded-background",
                ],
            )?,
        ],
    )
}

fn combined_life(seed: u64) -> Result<ContinuityScenario> {
    let entities = vec![
        entity(
            "life-character",
            ContinuityEntityKind::Person,
            "Iris Vale",
            true,
        ),
        entity("life-mara", ContinuityEntityKind::Person, "Mara Chen", true),
        entity(
            "life-elian",
            ContinuityEntityKind::Person,
            "Elian Moss",
            false,
        ),
        entity(
            "life-workshop",
            ContinuityEntityKind::Organization,
            "Aster Workshop",
            true,
        ),
        entity(
            "life-observatory",
            ContinuityEntityKind::Organization,
            "Northlight Observatory",
            true,
        ),
        entity(
            "life-harbor",
            ContinuityEntityKind::Location,
            "Tideglass Harbor",
            true,
        ),
    ];
    let memories = vec![
        (
            "life-first-sight",
            "2024-12-12T17:30:00Z",
            "Iris chose to restore Tideglass Harbor's dark lantern after seeing its unlit tower from the evening ferry.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.98)),
            0.92,
        ),
        (
            "life-key-handover",
            "2025-01-02T09:15:00Z",
            "Northlight Observatory entrusted Iris with the lantern-room key and the surviving maintenance ledger.",
            vec!["life-character", "life-observatory", "life-harbor"],
            Some(("lantern-restoration", 0.95)),
            0.82,
        ),
        (
            "life-maker-mark",
            "2025-01-07T11:40:00Z",
            "Mara found the original lens maker's crescent mark beneath a layer of soot on the brass collar.",
            vec!["life-mara", "life-character", "life-harbor"],
            Some(("lantern-restoration", 0.93)),
            0.76,
        ),
        (
            "life-rosemary-start",
            "2025-01-10T08:20:00Z",
            "Elian gave Iris two rosemary cuttings for the exposed community garden above the ferry shed.",
            vec!["life-elian", "life-character", "life-harbor"],
            Some(("harbor-garden", 0.91)),
            0.48,
        ),
        (
            "life-panel-catalog",
            "2025-01-14T14:10:00Z",
            "Iris catalogued seven dented brass panels before anyone removed them from the lantern housing.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.9)),
            0.52,
        ),
        (
            "life-wind-screen",
            "2025-01-20T07:50:00Z",
            "The first garden wind screen tore loose overnight, so Iris and Elian anchored its corners with sand-filled canvas pockets.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.88)),
            0.44,
        ),
        (
            "life-first-prism",
            "2025-01-28T16:35:00Z",
            "Mara and Iris cleaned the first intact prism with distilled water and a brush softer than the ledger recommended.",
            vec!["life-character", "life-mara", "life-workshop"],
            Some(("lantern-restoration", 0.94)),
            0.66,
        ),
        (
            "life-ferry-tea",
            "2025-01-31T18:05:00Z",
            "Iris began taking Friday tea on the last ferry, where the engine noise gave her a quiet boundary after workshop days.",
            vec!["life-character", "life-harbor"],
            None,
            0.31,
        ),
        (
            "life-padded-cradle",
            "2025-02-05T10:45:00Z",
            "Aster Workshop built a padded oak cradle so the fragile lens assembly could be rotated without lifting it by hand.",
            vec!["life-workshop", "life-character"],
            Some(("lantern-restoration", 0.96)),
            0.7,
        ),
        (
            "life-rosemary-frost",
            "2025-02-10T06:55:00Z",
            "A sharp frost browned one rosemary cutting, but the stem stayed green beneath the bark.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.84)),
            0.37,
        ),
        (
            "life-preserve-initials",
            "2025-02-16T13:25:00Z",
            "Iris promised Mara that the maker's scratched initials would remain visible inside the restored lantern.",
            vec!["life-character", "life-mara"],
            Some(("lantern-restoration", 0.97)),
            0.88,
        ),
        (
            "life-salt-corrosion",
            "2025-02-24T15:10:00Z",
            "Salt corrosion had fused the lower bearing cover, so Iris stopped before forcing the screws and documented their condition.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.92)),
            0.73,
        ),
        (
            "life-soil-test",
            "2025-03-02T09:30:00Z",
            "Elian's soil test showed the garden beds were too alkaline for blueberries but suitable for sea kale and thyme.",
            vec!["life-elian", "life-character", "life-harbor"],
            Some(("harbor-garden", 0.9)),
            0.51,
        ),
        (
            "life-cracked-prism",
            "2025-03-08T14:45:00Z",
            "Iris cracked a replacement prism by tightening its brass frame too quickly during a dry fitting.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.98)),
            0.97,
        ),
        (
            "life-admitted-mistake",
            "2025-03-08T17:20:00Z",
            "Iris told Mara the cracked prism was her own mistake and wrote the failed torque setting into the maintenance ledger.",
            vec!["life-character", "life-mara"],
            Some(("lantern-restoration", 0.96)),
            0.94,
        ),
        (
            "life-cork-gasket",
            "2025-03-18T12:10:00Z",
            "Iris rebuilt the prism frame with a cork gasket and a gentler hand-tightened fit.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.94)),
            0.81,
        ),
        (
            "life-nesting-pause",
            "2025-03-25T08:40:00Z",
            "Work paused for four days when a pair of swallows began nesting beside the lantern-room vent.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.72)),
            0.42,
        ),
        (
            "life-sea-kale",
            "2025-04-01T07:35:00Z",
            "Iris planted sea kale along the garden's windward edge while Elian moved the surviving rosemary behind a low slate wall.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.93)),
            0.55,
        ),
        (
            "life-clear-glass-arrival",
            "2025-04-07T11:15:00Z",
            "The replacement lens glass arrived from the mainland wrapped in wool and marked as optically clear.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.91)),
            0.69,
        ),
        (
            "life-amber-glass",
            "2025-04-12T19:05:00Z",
            "At sunset Iris noticed the new glass cast an amber band that would distort the harbor signal.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.95)),
            0.84,
        ),
        (
            "life-glass-return",
            "2025-04-15T09:00:00Z",
            "Mara backed Iris's decision to return the amber-tinted glass even though it threatened the public schedule.",
            vec!["life-mara", "life-character"],
            Some(("lantern-restoration", 0.94)),
            0.78,
        ),
        (
            "life-watering-roster",
            "2025-04-20T17:25:00Z",
            "The garden adopted a watering roster after Iris found three beds soaked and the thyme bed dry on the same evening.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.9)),
            0.43,
        ),
        (
            "life-opening-date-v1",
            "2025-05-01T10:00:00Z",
            "Northlight Observatory announced that the restored harbor lantern would reopen to the public on May 18.",
            vec!["life-character", "life-observatory", "life-harbor"],
            Some(("lantern-restoration", 0.97)),
            0.86,
        ),
        (
            "life-oral-history",
            "2025-05-05T15:30:00Z",
            "Mara recorded retired keeper Sela Rowan describing how fog once made the old lens appear to breathe.",
            vec!["life-mara", "life-observatory", "life-harbor"],
            Some(("lantern-restoration", 0.78)),
            0.62,
        ),
        (
            "life-public-class",
            "2025-05-10T13:00:00Z",
            "Iris taught a small Aster Workshop class to polish spare brass without erasing tool marks.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.83)),
            0.57,
        ),
        (
            "life-ramp-flood",
            "2025-05-17T22:15:00Z",
            "A spring storm flooded the lantern loading ramp and carried two stacks of staging timber into the harbor.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.96)),
            0.91,
        ),
        (
            "life-sage-bloom",
            "2025-05-21T07:10:00Z",
            "The garden's purple sage bloomed after the storm, drawing bees to the sheltered side of the ferry shed.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.86)),
            0.38,
        ),
        (
            "life-bearing-squeal",
            "2025-05-29T16:50:00Z",
            "The lantern bearing turned freely under load but produced a high squeal near the north stop.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.93)),
            0.71,
        ),
        (
            "life-rotation-test",
            "2025-06-05T20:10:00Z",
            "Iris completed a full manual rotation test and marked the squealing sector for later lubrication.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.94)),
            0.68,
        ),
        (
            "life-winter-rosemary-promise",
            "2025-06-11T08:05:00Z",
            "Iris promised Elian she would keep the strongest rosemary plant in the workshop window through winter and return cuttings in spring.",
            vec!["life-character", "life-elian", "life-workshop"],
            Some(("harbor-garden", 0.95)),
            0.87,
        ),
        (
            "life-neutral-glass",
            "2025-06-18T10:25:00Z",
            "A second glass shipment arrived with a neutral beam and a handwritten apology from the mainland maker.",
            vec!["life-character", "life-workshop"],
            Some(("lantern-restoration", 0.92)),
            0.74,
        ),
        (
            "life-handrail",
            "2025-06-25T12:40:00Z",
            "Aster Workshop installed a lower handrail on the lantern stair after Iris saw a visiting keeper struggle with the final turn.",
            vec!["life-character", "life-workshop", "life-harbor"],
            Some(("lantern-restoration", 0.82)),
            0.63,
        ),
        (
            "life-lens-installed",
            "2025-07-03T14:20:00Z",
            "Iris and Mara seated the neutral lens glass in the restored frame without covering the maker's initials.",
            vec!["life-character", "life-mara"],
            Some(("lantern-restoration", 0.98)),
            0.9,
        ),
        (
            "life-garden-bees",
            "2025-07-09T07:45:00Z",
            "Elian counted three native bee species moving between the sage, thyme, and sea kale flowers.",
            vec!["life-elian", "life-character", "life-harbor"],
            Some(("harbor-garden", 0.84)),
            0.4,
        ),
        (
            "life-beam-alignment",
            "2025-07-17T21:05:00Z",
            "The first powered beam crossed the harbor mouth six degrees too far north, so Iris stopped the motor before the public test.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.97)),
            0.89,
        ),
        (
            "life-fog-test",
            "2025-07-24T04:55:00Z",
            "During a natural fog Iris verified that the corrected beam remained visible from the south breakwater.",
            vec!["life-character", "life-harbor"],
            Some(("lantern-restoration", 0.96)),
            0.85,
        ),
        (
            "life-children-workshop",
            "2025-08-02T12:30:00Z",
            "Iris let harbor children turn a wooden lens model while Mara explained why each prism bends light inward.",
            vec!["life-character", "life-mara", "life-observatory"],
            Some(("lantern-restoration", 0.8)),
            0.58,
        ),
        (
            "life-painted-trim",
            "2025-08-09T15:15:00Z",
            "Mara painted the lantern-room trim in the muted green found beneath its newest coats.",
            vec!["life-mara", "life-harbor"],
            Some(("lantern-restoration", 0.87)),
            0.49,
        ),
        (
            "life-alignment-apology",
            "2025-08-16T10:35:00Z",
            "Iris apologized for dismissing Mara's first alignment reading and asked her to lead the independent recheck.",
            vec!["life-character", "life-mara"],
            Some(("lantern-restoration", 0.91)),
            0.88,
        ),
        (
            "life-tomato-harvest",
            "2025-08-23T08:15:00Z",
            "The garden shared its first tomato harvest at the ferry queue, with the smallest fruit saved for seed.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.82)),
            0.36,
        ),
        (
            "life-final-mount",
            "2025-09-05T13:50:00Z",
            "The restored lens assembly was lifted into its permanent mount and rotated twice without binding.",
            vec!["life-character", "life-workshop", "life-harbor"],
            Some(("lantern-restoration", 0.98)),
            0.9,
        ),
        (
            "life-dress-rehearsal",
            "2025-09-12T20:00:00Z",
            "Mara ran the reopening rehearsal while Iris watched the beam from the south breakwater.",
            vec!["life-character", "life-mara", "life-harbor"],
            Some(("lantern-restoration", 0.96)),
            0.83,
        ),
        (
            "life-reopening",
            "2025-09-14T19:30:00Z",
            "The Northlight harbor lantern reopened on September 14, and Iris invited retired keeper Sela Rowan to start its first rotation.",
            vec!["life-character", "life-observatory", "life-harbor"],
            Some(("lantern-restoration", 0.99)),
            0.98,
        ),
        (
            "life-winter-garden-plan",
            "2025-09-20T09:25:00Z",
            "Iris and Elian moved the garden's thyme into cold frames and chose one rosemary plant for the workshop window.",
            vec!["life-character", "life-elian", "life-workshop"],
            Some(("harbor-garden", 0.93)),
            0.67,
        ),
        (
            "life-visitor-log",
            "2025-10-01T16:45:00Z",
            "The first two weeks of the visitor log contained more sketches of the lens than signatures.",
            vec!["life-character", "life-observatory"],
            Some(("lantern-restoration", 0.76)),
            0.34,
        ),
        (
            "life-mara-retelling",
            "2025-10-08T18:10:00Z",
            "Mara recorded Iris retelling the prism accident without hiding her mistake or exaggerating the repair.",
            vec!["life-character", "life-mara", "life-observatory"],
            Some(("lantern-restoration", 0.89)),
            0.79,
        ),
        (
            "life-rosemary-cuttings",
            "2025-10-15T08:30:00Z",
            "Iris moved the strongest rosemary plant into Aster Workshop and labeled it for Elian's spring cuttings.",
            vec!["life-character", "life-elian", "life-workshop"],
            Some(("harbor-garden", 0.97)),
            0.86,
        ),
        (
            "life-winter-cover",
            "2025-10-22T11:55:00Z",
            "A breathable winter cover replaced the lantern's old tar sheet so trapped salt moisture could escape.",
            vec!["life-character", "life-workshop", "life-harbor"],
            Some(("lantern-restoration", 0.86)),
            0.61,
        ),
        (
            "life-november-storm",
            "2025-11-02T23:10:00Z",
            "After the November gale Iris checked the lantern bearing before checking the garden and found both unharmed.",
            vec!["life-character", "life-harbor"],
            None,
            0.7,
        ),
        (
            "life-garden-shelter",
            "2025-11-12T09:20:00Z",
            "Elian added a clear shelter panel that kept rain off the winter rosemary without blocking the weak morning sun.",
            vec!["life-elian", "life-character", "life-workshop"],
            Some(("harbor-garden", 0.88)),
            0.54,
        ),
        (
            "life-annual-reflection",
            "2025-11-20T17:40:00Z",
            "Iris wrote that the lantern taught her to treat hesitation as information rather than as a failure of nerve.",
            vec!["life-character", "life-observatory"],
            None,
            0.9,
        ),
        (
            "life-ferry-return",
            "2025-12-05T18:25:00Z",
            "From the evening ferry Iris saw the restored beam cross the water and remembered the dark tower that began the year.",
            vec!["life-character", "life-harbor"],
            None,
            0.93,
        ),
        (
            "life-community-supper",
            "2025-12-12T19:00:00Z",
            "The garden volunteers held a winter supper beneath the lantern, using dried thyme from the roof beds in every shared pot.",
            vec!["life-character", "life-elian", "life-harbor"],
            Some(("harbor-garden", 0.83)),
            0.6,
        ),
        (
            "life-ledger-close",
            "2025-12-18T15:30:00Z",
            "Iris closed the year's maintenance ledger with the reopening date, the prism torque limit, and Elian's rosemary plan on separate pages.",
            vec!["life-character", "life-observatory", "life-elian"],
            None,
            0.91,
        ),
    ];
    let mut events = memories
        .into_iter()
        .enumerate()
        .map(|(index, (id, at, text, entity_ids, thread, salience))| {
            remember(index + 1, id, at, text, entity_ids, thread, salience)
        })
        .collect::<Result<Vec<_>>>()?;
    events.extend([
        correct(
            901,
            "life-opening-date-v1",
            "life-opening-date-v2",
            "2025-06-02T09:00:00Z",
            "After the flooded ramp delayed repairs, Northlight moved the public reopening from May 18 to July 6.",
        )?,
        correct(
            902,
            "life-opening-date-v2",
            "life-opening-date-v3",
            "2025-09-01T09:00:00Z",
            "After the final lens and alignment checks, Northlight set the public reopening for September 14.",
        )?,
        link(903, "life-link-character-opening", "2025-05-02T09:00:00Z", "life-character", "life-opening-date-v1")?,
        link(904, "life-link-observatory-opening", "2025-05-02T09:05:00Z", "life-observatory", "life-opening-date-v1")?,
        link(905, "life-link-mara-admission", "2025-03-09T09:00:00Z", "life-mara", "life-admitted-mistake")?,
        query(951, "query-combined-reopening", "2025-12-20T09:00:00Z", "On what date did the Northlight harbor lantern finally reopen after the delays?", vec!["life-opening-date-v3", "life-reopening"], vec!["life-opening-date-v1", "life-opening-date-v2"] )?,
        query(952, "query-combined-rosemary", "2025-12-20T09:05:00Z", "What did Iris promise Elian she would do for the rosemary through winter?", vec!["life-winter-rosemary-promise", "life-rosemary-cuttings"], vec!["life-rosemary-frost", "life-sage-bloom"] )?,
        query(953, "query-combined-mistake", "2025-12-20T09:10:00Z", "Which restoration mistake did Iris openly admit was her own?", vec!["life-cracked-prism", "life-admitted-mistake"], vec!["life-cork-gasket", "life-amber-glass"] )?,
    ]);
    events.sort_by_key(InteractionEvent::timestamp);
    frozen_scenario(
        seed,
        "combined-life",
        ScenarioPattern::CombinedLife,
        entities,
        events,
    )
}

fn temporal_patterns(seed: u64) -> Result<ContinuityScenario> {
    let rehearsal_one = "Iris rehearsed the harbor bell sequence before sunrise on January 9.";
    let rehearsal_two = "Iris repeated the harbor bell rehearsal before sunrise on February 13.";
    let interval_start = "The winter residency at Northlight Observatory began on March 3.";
    let interval_end = "The winter residency at Northlight Observatory ended on April 28.";
    let rehearsal_three =
        "Iris returned for a third harbor bell rehearsal before sunrise on May 8.";
    let one_off = "Iris rang the restored solstice bell once at noon on June 21.";
    let recurrence_query = "Which activity did Iris repeat on several mornings?";
    let interval_query = "Which memories mark the beginning and end of the winter residency?";
    let one_off_query = "What bell event happened only once rather than recurring?";
    scenario(
        seed,
        "temporal-patterns",
        ScenarioPattern::TemporalPatterns,
        vec![
            entity(
                "temporal-character",
                ContinuityEntityKind::Person,
                "Iris Vale",
                false,
            ),
            entity(
                "temporal-observatory",
                ContinuityEntityKind::Organization,
                "Northlight Observatory",
                true,
            ),
            entity(
                "temporal-harbor",
                ContinuityEntityKind::Location,
                "Tideglass Harbor",
                false,
            ),
        ],
        vec![
            remember(
                1,
                "temporal-rehearsal-1",
                "2025-01-09T05:40:00Z",
                rehearsal_one,
                vec!["temporal-character", "temporal-harbor"],
                Some(("harbor-bells", 0.88)),
                0.45,
            )?,
            remember(
                2,
                "temporal-rehearsal-2",
                "2025-02-13T05:35:00Z",
                rehearsal_two,
                vec!["temporal-character", "temporal-harbor"],
                Some(("harbor-bells", 0.9)),
                0.45,
            )?,
            remember(
                3,
                "temporal-residency-start",
                "2025-03-03T09:00:00Z",
                interval_start,
                vec!["temporal-character", "temporal-observatory"],
                Some(("winter-residency", 0.96)),
                0.75,
            )?,
            remember(
                4,
                "temporal-residency-end",
                "2025-04-28T17:00:00Z",
                interval_end,
                vec!["temporal-character", "temporal-observatory"],
                Some(("winter-residency", 0.96)),
                0.75,
            )?,
            remember(
                5,
                "temporal-rehearsal-3",
                "2025-05-08T05:20:00Z",
                rehearsal_three,
                vec!["temporal-character", "temporal-harbor"],
                Some(("harbor-bells", 0.92)),
                0.5,
            )?,
            remember(
                6,
                "temporal-solstice-once",
                "2025-06-21T12:00:00Z",
                one_off,
                vec!["temporal-character", "temporal-harbor"],
                Some(("harbor-bells", 0.7)),
                0.9,
            )?,
            query(
                7,
                "query-temporal-recurrence",
                "2025-11-02T10:00:00Z",
                recurrence_query,
                vec![
                    "temporal-rehearsal-1",
                    "temporal-rehearsal-2",
                    "temporal-rehearsal-3",
                ],
                vec!["temporal-solstice-once"],
            )?,
            query(
                8,
                "query-temporal-interval",
                "2025-11-02T10:05:00Z",
                interval_query,
                vec!["temporal-residency-start", "temporal-residency-end"],
                vec!["temporal-rehearsal-2"],
            )?,
            query(
                9,
                "query-temporal-one-off",
                "2025-11-02T10:10:00Z",
                one_off_query,
                vec!["temporal-solstice-once"],
                vec![
                    "temporal-rehearsal-1",
                    "temporal-rehearsal-2",
                    "temporal-rehearsal-3",
                ],
            )?,
        ],
        concepts([
            (
                "recurrence",
                "recurrence",
                vec![
                    rehearsal_one,
                    rehearsal_two,
                    rehearsal_three,
                    recurrence_query,
                ],
            ),
            (
                "interval",
                "interval",
                vec![interval_start, interval_end, interval_query],
            ),
            ("one-off", "one-off", vec![one_off, one_off_query]),
        ]),
    )
}

fn entrenched_correction(seed: u64) -> Result<ContinuityScenario> {
    let original = "The Northlight map room opens to volunteers at sunrise every Saturday.";
    let reinforcement =
        "Mara scheduled the spring survey team around the map room's Saturday sunrise opening.";
    let corrected = "The Northlight map room now opens to volunteers at noon every Sunday.";
    let prior_query = "When can volunteers enter the Northlight map room?";
    let second_prior_query = "What opening time did the spring survey schedule assume?";
    let final_query = "After Mara's September correction, when does the map room open?";
    scenario(
        seed,
        "entrenched-correction",
        ScenarioPattern::EntrenchedCorrection,
        vec![
            entity(
                "entrenched-character",
                ContinuityEntityKind::Person,
                "Iris Vale",
                true,
            ),
            entity(
                "entrenched-colleague",
                ContinuityEntityKind::Person,
                "Mara Chen",
                false,
            ),
            entity(
                "entrenched-observatory",
                ContinuityEntityKind::Organization,
                "Northlight Observatory",
                true,
            ),
            entity(
                "entrenched-map-room",
                ContinuityEntityKind::Location,
                "Northlight map room",
                false,
            ),
        ],
        vec![
            remember(
                1,
                "map-hours-v1",
                "2025-01-04T08:00:00Z",
                original,
                vec![
                    "entrenched-character",
                    "entrenched-observatory",
                    "entrenched-map-room",
                ],
                Some(("survey-planning", 0.95)),
                0.75,
            )?,
            link(
                2,
                "map-link-character",
                "2025-01-05T09:00:00Z",
                "entrenched-character",
                "map-hours-v1",
            )?,
            link(
                3,
                "map-link-observatory",
                "2025-01-05T09:05:00Z",
                "entrenched-observatory",
                "map-hours-v1",
            )?,
            link(
                4,
                "map-link-room",
                "2025-01-05T09:10:00Z",
                "entrenched-map-room",
                "map-hours-v1",
            )?,
            query(
                5,
                "query-map-hours-before",
                "2025-02-01T08:00:00Z",
                prior_query,
                vec!["map-hours-v1"],
                vec![],
            )?,
            remember(
                6,
                "map-hours-reinforcement",
                "2025-03-17T11:30:00Z",
                reinforcement,
                vec![
                    "entrenched-colleague",
                    "entrenched-observatory",
                    "entrenched-map-room",
                ],
                Some(("survey-planning", 0.88)),
                0.65,
            )?,
            query(
                7,
                "query-map-hours-assumption",
                "2025-04-20T08:00:00Z",
                second_prior_query,
                vec!["map-hours-v1", "map-hours-reinforcement"],
                vec![],
            )?,
            correct(
                8,
                "map-hours-v1",
                "map-hours-v2",
                "2025-09-12T14:00:00Z",
                corrected,
            )?,
            query(
                9,
                "query-map-hours-after",
                "2025-10-03T08:00:00Z",
                final_query,
                vec!["map-hours-v2"],
                vec!["map-hours-v1", "map-hours-reinforcement"],
            )?,
        ],
        concepts([
            (
                "entrenched-old",
                "old",
                vec![original, reinforcement, prior_query, second_prior_query],
            ),
            (
                "entrenched-current",
                "current",
                vec![corrected, final_query],
            ),
        ]),
    )
}

fn autobiographical(seed: u64) -> Result<ContinuityScenario> {
    let first_choice = "Iris chose to restore the neglected harbor lantern after seeing its dark tower from the ferry.";
    let promise =
        "Iris promised Mara she would preserve the maker's scratched initials inside the lantern.";
    let mistake = "Iris cracked a replacement prism by tightening its brass frame too quickly.";
    let admission = "Iris told Mara the cracked prism was her own mistake and recorded the failed torque setting.";
    let repair =
        "Iris rebuilt the prism frame with a cork gasket and a gentler hand-tightened fit.";
    let lesson =
        "Iris now pauses before irreversible adjustments and asks a colleague to check the load.";
    let choice_query = "Why did Iris begin restoring the harbor lantern?";
    let mistake_query = "What mistake does Iris remember making during the restoration?";
    let commitment_query = "Which promise about the lantern's history did Iris keep?";
    scenario(
        seed,
        "autobiographical",
        ScenarioPattern::Autobiographical,
        vec![
            entity(
                "auto-character",
                ContinuityEntityKind::Person,
                "Iris Vale",
                true,
            ),
            entity(
                "auto-colleague",
                ContinuityEntityKind::Person,
                "Mara Chen",
                false,
            ),
            entity(
                "auto-workshop",
                ContinuityEntityKind::Organization,
                "Aster Workshop",
                false,
            ),
            entity(
                "auto-harbor",
                ContinuityEntityKind::Location,
                "Tideglass Harbor",
                true,
            ),
        ],
        vec![
            remember(
                1,
                "auto-first-choice",
                "2024-12-12T17:30:00Z",
                first_choice,
                vec!["auto-character", "auto-harbor"],
                Some(("lantern-restoration", 0.95)),
                0.9,
            )?,
            remember(
                2,
                "auto-promise",
                "2025-01-06T10:00:00Z",
                promise,
                vec!["auto-character", "auto-colleague"],
                Some(("lantern-restoration", 0.92)),
                0.85,
            )?,
            remember(
                3,
                "auto-mistake",
                "2025-03-22T16:20:00Z",
                mistake,
                vec!["auto-character", "auto-workshop"],
                Some(("lantern-restoration", 0.94)),
                0.95,
            )?,
            remember(
                4,
                "auto-admission",
                "2025-03-22T18:05:00Z",
                admission,
                vec!["auto-character", "auto-colleague"],
                Some(("lantern-restoration", 0.9)),
                0.9,
            )?,
            remember(
                5,
                "auto-repair",
                "2025-04-04T13:15:00Z",
                repair,
                vec!["auto-character", "auto-workshop"],
                Some(("lantern-restoration", 0.93)),
                0.8,
            )?,
            remember(
                6,
                "auto-lesson",
                "2025-05-19T09:40:00Z",
                lesson,
                vec!["auto-character", "auto-colleague"],
                None,
                0.75,
            )?,
            query(
                7,
                "query-auto-choice",
                "2025-12-01T09:00:00Z",
                choice_query,
                vec!["auto-first-choice"],
                vec!["auto-repair"],
            )?,
            query(
                8,
                "query-auto-mistake",
                "2025-12-01T09:05:00Z",
                mistake_query,
                vec!["auto-mistake", "auto-admission"],
                vec!["auto-repair"],
            )?,
            query(
                9,
                "query-auto-promise",
                "2025-12-01T09:10:00Z",
                commitment_query,
                vec!["auto-promise"],
                vec!["auto-lesson"],
            )?,
        ],
        concepts([
            ("auto-origin", "origin", vec![first_choice, choice_query]),
            (
                "auto-commitment",
                "commitment",
                vec![promise, commitment_query],
            ),
            (
                "auto-mistake",
                "mistake",
                vec![mistake, admission, mistake_query],
            ),
            ("auto-growth", "growth", vec![repair, lesson]),
        ]),
    )
}

fn frozen_scenario(
    seed: u64,
    id: &str,
    pattern: ScenarioPattern,
    mut entities: Vec<EntityDeclaration>,
    events: Vec<InteractionEvent>,
) -> Result<ContinuityScenario> {
    entities.sort_by(|left, right| left.external_id.cmp(&right.external_id));
    let scenario = ContinuityScenario {
        fixture_id: id.to_string(),
        namespace: format!("continuity-{id}-{seed:016x}"),
        pattern,
        entities,
        embedding: ContinuityScenarioEmbedding::frozen(),
        events,
    };
    scenario.validate()?;
    Ok(scenario)
}

fn scenario(
    seed: u64,
    id: &str,
    pattern: ScenarioPattern,
    mut entities: Vec<EntityDeclaration>,
    events: Vec<InteractionEvent>,
    mut concepts: BTreeMap<String, SimilarityConceptFixture>,
) -> Result<ContinuityScenario> {
    entities.sort_by(|left, right| left.external_id.cmp(&right.external_id));
    assign_entity_embedding_inputs(id, &entities, &events, &mut concepts)?;
    let clusters = concepts
        .values()
        .map(|concept| concept.cluster.clone())
        .collect::<BTreeSet<_>>();
    let cluster_count = clusters.len();
    if cluster_count > EMBEDDING_VECTOR_SIZE {
        bail!(
            "continuity scenario `{id}` declares {cluster_count} embedding clusters, exceeding configured vector_size {EMBEDDING_VECTOR_SIZE}"
        );
    }
    let mut cluster_vectors = BTreeMap::new();
    for (index, cluster) in clusters.into_iter().enumerate() {
        let mut vector = vec![0.0; EMBEDDING_VECTOR_SIZE];
        vector[index] = 1.0;
        cluster_vectors.insert(cluster, vector);
    }
    Ok(ContinuityScenario {
        fixture_id: id.to_string(),
        namespace: format!("continuity-{id}-{seed:016x}"),
        pattern,
        entities,
        embedding: ContinuityScenarioEmbedding::controllable_similarity_provider(
            ControllableSimilarityFixture {
                seed,
                vector_size: EMBEDDING_VECTOR_SIZE,
                noise_magnitude: 1.0 / 1024.0,
                clusters: cluster_vectors,
                concepts,
            },
        ),
        events,
    })
}

fn assign_entity_embedding_inputs(
    scenario_id: &str,
    entities: &[EntityDeclaration],
    events: &[InteractionEvent],
    concepts: &mut BTreeMap<String, SimilarityConceptFixture>,
) -> Result<()> {
    let mut background_labels = Vec::new();
    for entity in entities {
        let first_referencing_text = events.iter().find_map(|event| match event {
            InteractionEvent::Remember {
                text,
                entity_external_ids,
                ..
            } if entity_external_ids.contains(&entity.external_id) => Some(text),
            _ => None,
        });
        if let Some(text) = first_referencing_text {
            let Some(concept) = concepts
                .values_mut()
                .find(|concept| concept.inputs.contains(text))
            else {
                bail!(
                    "continuity scenario {scenario_id:?} Remember text {text:?} is missing from embedding concepts"
                );
            };
            concept.inputs.push(entity.label.clone());
        } else {
            background_labels.push(entity.label.clone());
        }
    }
    if !background_labels.is_empty() {
        if concepts.contains_key("entity_background") {
            bail!(
                "continuity scenario {scenario_id:?} collides with reserved embedding concept ID \"entity_background\""
            );
        }
        concepts.insert(
            "entity_background".to_string(),
            SimilarityConceptFixture {
                cluster: "entity_background".to_string(),
                inputs: background_labels,
            },
        );
    }
    Ok(())
}

fn concepts<const N: usize>(
    values: [(&str, &str, Vec<&str>); N],
) -> BTreeMap<String, SimilarityConceptFixture> {
    values
        .into_iter()
        .map(|(id, cluster, inputs)| {
            (
                id.to_string(),
                SimilarityConceptFixture {
                    cluster: cluster.to_string(),
                    inputs: inputs.into_iter().map(str::to_string).collect(),
                },
            )
        })
        .collect()
}

fn standard_entities(hubs: bool) -> Vec<EntityDeclaration> {
    vec![
        entity(
            "entity-person",
            ContinuityEntityKind::Person,
            "Entity A",
            hubs,
        ),
        entity(
            "entity-organization",
            ContinuityEntityKind::Organization,
            "Entity B",
            hubs,
        ),
        entity(
            "entity-location",
            ContinuityEntityKind::Location,
            "Entity C",
            hubs,
        ),
    ]
}

fn entity(
    external_id: &str,
    entity_type: ContinuityEntityKind,
    label: &str,
    is_hub: bool,
) -> EntityDeclaration {
    EntityDeclaration {
        external_id: external_id.into(),
        entity_type,
        label: label.into(),
        is_hub,
    }
}

fn remember(
    number: usize,
    external_id: &str,
    at: &str,
    text: &str,
    entity_ids: Vec<&str>,
    thread: Option<(&str, f32)>,
    salience: f32,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Remember {
        event_id: format!("event-{number:03}"),
        external_id: external_id.into(),
        timestamp: timestamp(at)?,
        text: text.into(),
        surface_texts: None,
        entity_external_ids: entity_ids.into_iter().map(str::to_string).collect(),
        thread: thread.map(|(id, confidence)| ThreadMembership {
            thread_external_id: id.into(),
            confidence,
        }),
        salience,
    })
}

fn remember_with_surfaces(
    number: usize,
    external_id: &str,
    at: &str,
    surface_texts: RememberSurfaceTexts,
    entity_ids: Vec<&str>,
    salience: f32,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Remember {
        event_id: format!("event-{number:03}"),
        external_id: external_id.into(),
        timestamp: timestamp(at)?,
        text: surface_texts.episode.clone(),
        surface_texts: Some(surface_texts),
        entity_external_ids: entity_ids.into_iter().map(str::to_string).collect(),
        thread: None,
        salience,
    })
}

fn correct(
    number: usize,
    target: &str,
    replacement: &str,
    at: &str,
    text: &str,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Correct {
        event_id: format!("event-{number:03}"),
        target_external_id: target.into(),
        replacement_external_id: replacement.into(),
        timestamp: timestamp(at)?,
        replacement_text: text.into(),
    })
}

fn forget(
    number: usize,
    targets: Vec<&str>,
    at: &str,
    suppress_derived_from_target: bool,
    apply_to_derived_from_target: bool,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Forget {
        event_id: format!("event-{number:03}"),
        target_external_ids: targets.into_iter().map(str::to_string).collect(),
        timestamp: timestamp(at)?,
        suppress_derived_from_target,
        apply_to_derived_from_target,
    })
}

fn link(
    number: usize,
    external_id: &str,
    at: &str,
    from: &str,
    to: &str,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Link {
        event_id: format!("event-{number:03}"),
        external_id: external_id.into(),
        timestamp: timestamp(at)?,
        from_external_id: from.into(),
        relation: "about".into(),
        to_external_id: to.into(),
    })
}

fn query(
    number: usize,
    query_id: &str,
    at: &str,
    text: &str,
    relevant: Vec<&str>,
    irrelevant: Vec<&str>,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Query {
        event_id: format!("event-{number:03}"),
        query_id: query_id.into(),
        timestamp: timestamp(at)?,
        text: text.into(),
        expected: ExpectedRelevance {
            relevant_external_ids: relevant.into_iter().map(str::to_string).collect(),
            irrelevant_external_ids: irrelevant.into_iter().map(str::to_string).collect(),
        },
    })
}

fn timestamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("parse continuity fixture timestamp {value:?}"))
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process::Command};

    use super::*;
    use crate::{canonical_fixture_bytes, parse_fixture_bytes, scenario_patterns};

    const PROCESS_PROBE_PATH: &str = "CMEM_CONTINUITY_FIXTURE_PROBE_PATH";
    const CHECKED_FIXTURE: &[u8] = include_bytes!("../fixtures/continuity_v3.json");

    #[test]
    fn checked_fixture_is_canonical_and_covers_every_scenario_pattern() {
        let generated = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let bytes = canonical_fixture_bytes(&generated).unwrap();
        assert_eq!(bytes, CHECKED_FIXTURE);
        assert_eq!(parse_fixture_bytes(CHECKED_FIXTURE).unwrap(), generated);

        let patterns = scenario_patterns(&generated);
        assert_eq!(patterns.len(), 15);
        assert!(patterns.values().all(|count| *count == 1));
        assert_eq!(
            generated
                .scenarios
                .iter()
                .map(|scenario| scenario.fixture_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "long-gap-recall",
                "recurring-hub-entity",
                "hub-scale",
                "selective-entity",
                "correction-chains",
                "thread-drift",
                "temporal-structure",
                "mixed-salience-accumulation",
                "cross-store-stress",
                "surface-contribution",
                "graded-similarity",
                "combined-life",
                "temporal-patterns",
                "entrenched-correction",
                "autobiographical",
            ]
        );
    }

    #[test]
    fn scenario_rejects_more_clusters_than_the_declared_vector_size() {
        let concepts = (0..=EMBEDDING_VECTOR_SIZE)
            .map(|index| {
                (
                    format!("concept-{index}"),
                    SimilarityConceptFixture {
                        cluster: format!("cluster-{index}"),
                        inputs: Vec::new(),
                    },
                )
            })
            .collect();

        let error = super::scenario(
            CHECKED_FIXTURE_SEED,
            "too-many-clusters",
            ScenarioPattern::LongGapRecall,
            Vec::new(),
            Vec::new(),
            concepts,
        )
        .unwrap_err()
        .to_string();

        assert_eq!(
            error,
            "continuity scenario `too-many-clusters` declares 9 embedding clusters, exceeding configured vector_size 8"
        );
    }

    #[test]
    fn scenario_reports_missing_remember_embedding_input_without_panicking() {
        let scenario_id = "extension-missing-concept";
        let remembered_text = "A newly extended Remember event.";
        let events = vec![
            remember(
                1,
                "memory-new",
                "2025-01-01T00:00:00Z",
                remembered_text,
                vec!["entity-new"],
                None,
                0.5,
            )
            .unwrap(),
        ];
        let error = super::scenario(
            CHECKED_FIXTURE_SEED,
            scenario_id,
            ScenarioPattern::LongGapRecall,
            vec![entity(
                "entity-new",
                ContinuityEntityKind::Person,
                "New Entity",
                false,
            )],
            events,
            BTreeMap::new(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains(scenario_id), "{error}");
        assert!(error.contains(remembered_text), "{error}");
        assert!(error.contains("missing from embedding concepts"), "{error}");
    }

    #[test]
    fn scenario_reports_reserved_background_concept_collision_without_panicking() {
        let scenario_id = "extension-reserved-concept";
        let error = super::scenario(
            CHECKED_FIXTURE_SEED,
            scenario_id,
            ScenarioPattern::LongGapRecall,
            vec![entity(
                "entity-new",
                ContinuityEntityKind::Person,
                "New Entity",
                false,
            )],
            Vec::new(),
            concepts([("entity_background", "custom", vec!["custom input"])]),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains(scenario_id), "{error}");
        assert!(error.contains("entity_background"), "{error}");
        assert!(error.contains("reserved"), "{error}");
    }

    #[test]
    fn invalid_extension_timestamp_returns_contextual_error() {
        let error = timestamp("not-a-timestamp").unwrap_err().to_string();
        assert!(error.contains("continuity fixture timestamp"), "{error}");
        assert!(error.contains("not-a-timestamp"), "{error}");
    }

    #[test]
    fn same_seed_is_byte_identical_across_two_process_runs() {
        let current_exe = env::current_exe().unwrap();
        let mut outputs = Vec::new();
        for run in 0..2 {
            let output_path = env::temp_dir().join(format!(
                "cmem-continuity-fixtures-{}-{run}.json",
                std::process::id()
            ));
            let status = Command::new(&current_exe)
                .args([
                    "--exact",
                    "generator::tests::cross_process_fixture_probe",
                    "--nocapture",
                ])
                .env(PROCESS_PROBE_PATH, &output_path)
                .status()
                .unwrap();
            assert!(status.success());
            outputs.push(fs::read(&output_path).unwrap());
            fs::remove_file(output_path).unwrap();
        }
        assert_eq!(outputs[0], outputs[1]);
        assert_eq!(outputs[0], CHECKED_FIXTURE);
    }

    #[test]
    fn cross_process_fixture_probe() {
        let Ok(output_path) = env::var(PROCESS_PROBE_PATH) else {
            return;
        };
        let bytes =
            canonical_fixture_bytes(&generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap()).unwrap();
        fs::write(output_path, bytes).unwrap();
    }

    #[test]
    fn query_relevance_labels_are_explicit_and_contrast_labels_may_be_empty() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let long_gap = scenario(&fixtures, ScenarioPattern::LongGapRecall);
        let first_write = long_gap.events.first().unwrap().timestamp();
        let query_time = long_gap.events.last().unwrap().timestamp();
        assert!((query_time - first_write).num_days() >= 180);

        for scenario in &fixtures.scenarios {
            for event in &scenario.events {
                if let InteractionEvent::Query { expected, .. } = event {
                    assert!(!expected.relevant_external_ids.is_empty());
                }
            }
        }
        let unlabeled_contrasts = fixtures
            .scenarios
            .iter()
            .flat_map(|scenario| &scenario.events)
            .filter(|event| {
                matches!(
                    event,
                    InteractionEvent::Query { expected, .. }
                        if expected.irrelevant_external_ids.is_empty()
                )
            })
            .count();
        assert_eq!(unlabeled_contrasts, 5);
    }

    #[test]
    fn surface_contribution_events_use_distinct_persisted_texts() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario(&fixtures, ScenarioPattern::SurfaceContribution);
        let surface_events = scenario
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember {
                    surface_texts: Some(surface_texts),
                    ..
                } => Some(surface_texts),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(surface_events.len(), 2);
        for surface_texts in surface_events {
            assert_ne!(surface_texts.episode, surface_texts.observation);
            assert_ne!(surface_texts.episode, surface_texts.derived);
            assert_ne!(surface_texts.observation, surface_texts.derived);
        }
    }

    #[test]
    fn public_parser_requires_every_entity_label_embedding_assignment() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = &mut fixtures.scenarios[0];
        let entity_label = scenario
            .entities
            .iter()
            .find(|entity| entity.external_id == "entity-person")
            .unwrap()
            .label
            .clone();
        for concept in scenario
            .embedding
            .controllable_similarity_mut()
            .unwrap()
            .concepts
            .values_mut()
        {
            concept.inputs.retain(|input| input != &entity_label);
        }
        let bytes = serde_json::to_vec(&fixtures).unwrap();

        let error = parse_fixture_bytes(&bytes).unwrap_err().to_string();
        assert!(error.contains("entity label"), "{error}");
        assert!(error.contains(&entity_label), "{error}");
    }

    #[test]
    fn recurring_hubs_span_three_entity_kinds_at_high_degree() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let hub = scenario(&fixtures, ScenarioPattern::RecurringHubEntity);
        let hubs = hub
            .entities
            .iter()
            .filter(|entity| entity.is_hub)
            .collect::<Vec<_>>();
        assert_eq!(
            hubs.iter()
                .map(|entity| entity.entity_type.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        for entity in hubs {
            let degree = hub
                .events
                .iter()
                .filter(|event| match event {
                    InteractionEvent::Remember {
                        entity_external_ids,
                        ..
                    } => entity_external_ids.contains(&entity.external_id),
                    _ => false,
                })
                .count();
            assert!(degree >= 5, "{} degree was {degree}", entity.external_id);
        }
    }

    #[test]
    fn hub_scale_creates_binding_pressure_and_a_graph_only_probe() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scale = scenario(&fixtures, ScenarioPattern::HubScale);
        let routine = scale
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember {
                    external_id,
                    text,
                    salience,
                    ..
                } if external_id.starts_with("hub-scale-memory-") => {
                    Some((text, salience.to_bits()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(routine.len(), HUB_SCALE_INCIDENT_COUNT);
        assert_eq!(
            routine
                .iter()
                .map(|(_, salience)| *salience)
                .collect::<BTreeSet<_>>()
                .len(),
            4
        );

        let embedding = scale.embedding.controllable_similarity().unwrap();
        let provider =
            cmem_eval_core::ControllableSimilarityEmbeddingProvider::new(embedding.clone())
                .unwrap();
        let routine_cluster_counts = routine
            .iter()
            .map(|(text, _)| {
                let concept = provider.concept_for_text(text).unwrap();
                embedding.concepts[concept].cluster.as_str()
            })
            .fold(BTreeMap::new(), |mut counts, cluster| {
                *counts.entry(cluster).or_insert(0) += 1;
                counts
            });
        assert_eq!(
            routine_cluster_counts,
            BTreeMap::from([
                ("hub-scale-query", 4),
                ("hub-scale-quaternary", 17),
                ("hub-scale-secondary", 10),
                ("hub-scale-tertiary", 17),
            ])
        );
        assert_eq!(
            scale
                .entities
                .iter()
                .map(|entity| (
                    entity.label.as_str(),
                    embedding.concepts[provider.concept_for_text(&entity.label).unwrap()]
                        .cluster
                        .as_str(),
                ))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                ("Scale Hub A", "hub-scale-query"),
                ("Scale Hub B", "hub-scale-tertiary"),
                ("Scale Hub C", "hub-scale-secondary"),
            ])
        );

        let query_text = "Which dormant graph-only marker is linked to Scale Hub C?";
        let probe_text = "The dormant graph-only marker linked to Scale Hub C is obsidian-seven.";
        let query_vector = provider.vector_for_text(query_text).unwrap();
        let probe_vector = provider.vector_for_text(probe_text).unwrap();
        let dot = query_vector
            .iter()
            .zip(&probe_vector)
            .map(|(left, right)| left * right)
            .sum::<f32>();
        assert!(dot.abs() < 0.01, "probe/query dot product was {dot}");
        let query_cluster = &embedding.clusters["hub-scale-query"];
        let probe_cluster = &embedding.clusters["hub-scale-probe"];
        assert_eq!(
            query_cluster
                .iter()
                .zip(probe_cluster)
                .map(|(left, right)| left * right)
                .sum::<f32>(),
            0.0
        );

        let positive_vector_memory_count = routine
            .iter()
            .filter(|(text, _)| {
                provider
                    .vector_for_text(text)
                    .unwrap()
                    .iter()
                    .zip(&query_vector)
                    .map(|(left, right)| left * right)
                    .sum::<f32>()
                    > 0.0
            })
            .count();
        let positive_vector_entity_count = scale
            .entities
            .iter()
            .filter(|entity| {
                provider
                    .vector_for_text(&entity.label)
                    .unwrap()
                    .iter()
                    .zip(&query_vector)
                    .map(|(left, right)| left * right)
                    .sum::<f32>()
                    > 0.0
            })
            .count();
        let positive_vector_object_count =
            positive_vector_memory_count + positive_vector_entity_count;
        assert!(positive_vector_object_count > 48);

        let probe = scale
            .events
            .iter()
            .find(|event| {
                matches!(
                    event,
                    InteractionEvent::Remember { external_id, .. }
                        if external_id == "hub-scale-dormant-probe"
                )
            })
            .unwrap();
        let InteractionEvent::Remember {
            entity_external_ids,
            ..
        } = probe
        else {
            unreachable!("probe lookup only accepts Remember events");
        };
        assert_eq!(entity_external_ids, &["hub-scale-location"]);
        let query = scale.events.last().unwrap();
        let InteractionEvent::Query { expected, .. } = query else {
            panic!("hub-scale must end with its measurement query");
        };
        assert_eq!(
            expected.relevant_external_ids,
            vec!["hub-scale-dormant-probe"]
        );
        assert!(expected.irrelevant_external_ids.is_empty());
    }

    #[test]
    fn scripted_patterns_include_required_lifecycle_and_structure() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let corrections = scenario(&fixtures, ScenarioPattern::CorrectionChains);
        assert_eq!(
            corrections
                .events
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Correct { .. }))
                .count(),
            2
        );
        assert!(
            corrections
                .events
                .iter()
                .any(|event| matches!(event, InteractionEvent::Forget { .. }))
        );

        let drift = scenario(&fixtures, ScenarioPattern::ThreadDrift);
        let confidences = drift
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember {
                    thread: Some(thread),
                    ..
                } => Some(thread.confidence),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(confidences.windows(2).all(|pair| pair[0] > pair[1]));

        let salience = scenario(&fixtures, ScenarioPattern::MixedSalienceAccumulation);
        let values = salience
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember { salience, .. } => Some(*salience),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(values.iter().any(|value| *value < 0.2));
        assert!(values.iter().any(|value| *value > 0.9));

        let restart = scenario(&fixtures, ScenarioPattern::CrossStoreStress);
        let restart_index = restart
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    InteractionEvent::Restart {
                        reopen_graph: true,
                        reopen_stats: true,
                        ..
                    }
                )
            })
            .unwrap();
        assert!(
            restart.events[..restart_index]
                .iter()
                .any(|event| matches!(event, InteractionEvent::Remember { .. }))
        );
        assert!(
            restart.events[restart_index + 1..]
                .iter()
                .any(|event| matches!(event, InteractionEvent::Query { .. }))
        );
    }

    #[test]
    fn catalog_scenarios_pin_realism_structure_and_provider_choices() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let graded = scenario(&fixtures, ScenarioPattern::GradedSimilarity);
        assert_eq!(graded.embedding.provider_name(), "frozen");
        assert_eq!(graded.events.len(), 5);

        let combined = scenario(&fixtures, ScenarioPattern::CombinedLife);
        assert_eq!(combined.embedding.provider_name(), "frozen");
        assert!((60..=100).contains(&combined.events.len()));
        assert_eq!(
            combined
                .events
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Correct { .. }))
                .count(),
            2
        );
        let thread_ids = combined
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember {
                    thread: Some(thread),
                    ..
                } => Some(thread.thread_external_id.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            thread_ids,
            BTreeSet::from(["harbor-garden", "lantern-restoration"])
        );
        let salience = combined
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember { salience, .. } => Some(*salience),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(salience.iter().any(|value| *value < 0.4));
        assert!(salience.iter().any(|value| *value > 0.95));
        assert_eq!(
            combined
                .events
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Query { .. }))
                .count(),
            3
        );

        let temporal = scenario(&fixtures, ScenarioPattern::TemporalPatterns);
        assert_eq!(
            temporal
                .events
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Query { .. }))
                .count(),
            3
        );

        let entrenched = scenario(&fixtures, ScenarioPattern::EntrenchedCorrection);
        let correction_index = entrenched
            .events
            .iter()
            .position(|event| matches!(event, InteractionEvent::Correct { .. }))
            .unwrap();
        assert_eq!(
            entrenched.events[..correction_index]
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Query { .. }))
                .count(),
            2
        );
        assert_eq!(
            entrenched.events[..correction_index]
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Link { .. }))
                .count(),
            3
        );

        let autobiography = scenario(&fixtures, ScenarioPattern::Autobiographical);
        assert!(autobiography.entities.iter().any(|entity| {
            entity.external_id == "auto-character"
                && entity.entity_type == ContinuityEntityKind::Person
        }));
    }

    #[test]
    fn schema_is_role_free_and_all_embedding_inputs_are_fixture_assigned() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let serialized = String::from_utf8(canonical_fixture_bytes(&fixtures).unwrap()).unwrap();
        assert!(!serialized.contains("\"role\""));
        for scenario in &fixtures.scenarios {
            let Some(embedding) = scenario.embedding.controllable_similarity() else {
                assert!(!scenario.embedding_inputs().is_empty());
                continue;
            };
            let provider =
                cmem_eval_core::ControllableSimilarityEmbeddingProvider::new(embedding.clone())
                    .unwrap();
            for event in &scenario.events {
                let text = match event {
                    InteractionEvent::Remember { text, .. }
                    | InteractionEvent::Query { text, .. } => Some(text),
                    InteractionEvent::Correct {
                        replacement_text, ..
                    } => Some(replacement_text),
                    _ => None,
                };
                if let Some(text) = text {
                    provider.vector_for_text(text).unwrap();
                }
                if let InteractionEvent::Remember {
                    surface_texts: Some(surface_texts),
                    ..
                } = event
                {
                    for text in [
                        &surface_texts.episode,
                        &surface_texts.observation,
                        &surface_texts.derived,
                    ] {
                        provider.vector_for_text(text).unwrap();
                    }
                }
            }
            for entity in &scenario.entities {
                provider.vector_for_text(&entity.label).unwrap();
            }
        }
    }

    fn scenario(fixtures: &ContinuityFixtureSet, pattern: ScenarioPattern) -> &ContinuityScenario {
        fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == pattern)
            .unwrap()
    }
}
