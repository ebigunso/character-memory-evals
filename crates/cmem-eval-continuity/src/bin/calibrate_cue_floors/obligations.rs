//! Authored prospective stories; the base writes the same stories without roles or due instants.
use super::*;
use std::collections::BTreeSet;

const MORNING: &str = "2025-09-09T08:00:00+09:00";
const OWN_TOPIC: &str = "Finishing my watercolor of the harbor";
pub(super) const METHOD: &str = "Prospective obligations in two new stores, leaving every existing family unchanged. Meeting pressure contains both directions, resolved and fulfilled matters, a future-due trigger control, a self-only undated promise, crowded person state, two present people in both orders, and current/superseded controls. Sixty-four unrelated topic memories and their source occasions compete under native caps; two bounded shared occasions per kind make the cohort reachable within the ordinary root budget. The keyless store has 367 equal-salience daily occasions and several overdue obligations. Authored party assertions and UTC due instants are retained separately; the base writes neither. Every supported parent query executes, while future role/identity/trigger/due behavior stays not_run. After wiring those library inputs, only those role/due fields and the required self constructor argument may differ. Both native ID orders reuse the shared public-ingest census and permutation. F1-F9 are readings for cross-pin comparison, not baseline pass claims; F10 compares all existing families. Native paths, limits and floors are recorded separately from authored expectations. No dates or roles are inferred from text, no gold reaches metadata, no wall clock or behavior threshold changes retrieval.";

pub(super) fn is_family(family: &Family) -> bool {
    matches!(
        family.name.as_str(),
        "prospective-meeting" | "prospective-daily"
    )
}

pub(super) fn falsifiers() -> Value {
    json!({
        "F1":"Each loud-topic retrieval retains the first present person's most salient unresolved obligation; report every other person's count separately.",
        "F2":"At trigger floor two, the first person's two most salient unresolved obligations in opposite directions both survive.",
        "F3":"The most salient unresolved due/overdue obligation survives no-topic and loud-topic queries.",
        "F4":"Due never contributes future-due obligations; Trigger/Due never contribute settled obligations; ordinary admission retains resolved_by. Future-due present-person control comes by Trigger with NotYetDue.",
        "F5":"The self-only undated promise must not appear on every retrieval.",
        "F6":"The self-only undated promise appears when its topic is asked.",
        "F7":"On-topic loss versus topic-alone is at most two; retain counts by section as well as total.",
        "F8":"Present-person non-obligation state count falls by at most one between matching before/after queries.",
        "F9":"Keyless equal-salience latest occasion count falls by at most one between matching before/after queries.",
        "F10":"Existing role-free/due-free families retain ids, sections, order, scores and native admitted_by. Compare the complete companion full runs, not this subset."
    })
}

fn add_obligation(
    family: &mut Family,
    label: &str,
    text: &str,
    parties: (&[&str], &[&str]),
    due: Option<&str>,
    salience: f32,
    kind: DerivedType,
) {
    let at = (timestamp("2025-01-01T12:00:00+09:00").unwrap()
        + Duration::hours(family.graph.derived_memories.len() as i64))
    .to_rfc3339();
    let source = format!("source-{label}");
    experience(family, &source, &at, false, 0.5, -0.1);
    let actors = parties
        .0
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    let counterparts = parties
        .1
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    let subjects = actors
        .iter()
        .chain(&counterparts)
        .cloned()
        .collect::<BTreeSet<_>>();
    family
        .experiences
        .last_mut()
        .unwrap()
        .write
        .scene
        .participants = subjects
        .iter()
        .filter(|s| s.as_str() != "self")
        .map(|s| SceneParticipantInput {
            key: Some(s.clone()),
            ..Default::default()
        })
        .collect();
    let mut memory = derived(
        label,
        &at,
        text.into(),
        kind,
        subjects.into_iter().collect(),
        salience,
    );
    memory.given_by_application = false;
    memory.source_episode_external_ids.push(source.clone());
    assign(&mut family.embedding, text, consolidation::vector(7, 1.0));
    family.graph.derived_memories.push(memory);
    consolidation::link(
        family,
        label,
        RelationType::DerivedFrom,
        ObjectType::Episode,
        &source,
    );
    family.obligations.push(Obligation {
        label: label.into(),
        memory_external_id: label.into(),
        actor_subjects: actors,
        counterpart_subjects: counterparts,
        due_instant: due.map(|at| {
            timestamp(at)
                .unwrap()
                .with_timezone(&Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        }),
    });
}

fn add_probe(
    family: &mut Family,
    config: &BenchmarkRunConfig,
    name: &str,
    people: &[&str],
    topic: Option<&str>,
    floor: usize,
) {
    let mut query = input(config, &family.namespace, false, topic);
    query.scene.time = Some(MORNING.into());
    query.scene.participants = people
        .iter()
        .map(|person| SceneParticipantInput {
            key: Some((*person).into()),
            ..Default::default()
        })
        .collect();
    let mut planned = probe(
        name,
        query,
        &[
            "actor_counterpart",
            "character_identity",
            "trigger",
            "due_instant",
            "due",
            "trigger_floor",
        ],
    );
    planned.trigger_floor = Some(floor);
    family.probes.push(planned);
}

pub(super) fn meeting(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("prospective-meeting");
    family.character_entity = Some("self".into());
    for person in ["rowan", "nia", "bob", "sage"] {
        family.graph.entities.push(EntityInput {
            external_id: person.into(),
        });
    }
    let stories = [
        (
            "iris-outgoing",
            "I promised Iris the blue notebook.",
            &["self"][..],
            &["iris"][..],
            Some("2025-09-08T23:59:59+09:00"),
            0.80,
            DerivedType::Commitment,
        ),
        (
            "iris-incoming",
            "Iris promised me the library receipt.",
            &["iris"],
            &["self"],
            Some("2025-09-09T22:00:00+09:00"),
            0.79,
            DerivedType::OpenLoop,
        ),
        (
            "bob-tomorrow",
            "Bob promised me the garden map.",
            &["bob"],
            &["self"],
            Some("2025-09-10T00:00:00+09:00"),
            0.78,
            DerivedType::OpenLoop,
        ),
        (
            "rowan-outgoing",
            "I promised Rowan the copper compass.",
            &["self"],
            &["rowan"],
            None,
            0.77,
            DerivedType::Commitment,
        ),
        (
            "rowan-incoming",
            "Rowan promised me the parcel receipt.",
            &["rowan"],
            &["self"],
            None,
            0.76,
            DerivedType::OpenLoop,
        ),
        (
            "settled-iris",
            "I promised Iris to mend the canvas bag.",
            &["self"],
            &["iris"],
            Some("2025-09-08T12:00:00+09:00"),
            0.99,
            DerivedType::Commitment,
        ),
        (
            "settled-rowan",
            "Rowan promised to return my paper cutter.",
            &["rowan"],
            &["self"],
            Some("2025-09-08T12:00:00+09:00"),
            0.99,
            DerivedType::OpenLoop,
        ),
        (
            "own-undated",
            "I intend to finish my watercolor of the harbor.",
            &["self"],
            &[],
            None,
            0.65,
            DerivedType::Commitment,
        ),
        (
            "own-due",
            "I intend to sort the postcards in my desk.",
            &["self"],
            &[],
            Some("2025-09-09T00:00:00+09:00"),
            0.70,
            DerivedType::Commitment,
        ),
        (
            "superseded-sage",
            "I promised Sage an early draft of the walk map.",
            &["self"],
            &["sage"],
            Some("2025-09-08T12:00:00+09:00"),
            0.98,
            DerivedType::Commitment,
        ),
        (
            "current-sage",
            "I promised Sage the revised walk map instead.",
            &["self"],
            &["sage"],
            Some("2025-09-09T18:00:00+09:00"),
            0.14,
            DerivedType::Commitment,
        ),
        (
            "nia-low",
            "Nia still needs the spare brass key from me.",
            &["self"],
            &["nia"],
            None,
            0.10,
            DerivedType::OpenLoop,
        ),
    ];
    for (label, text, actors, counterparts, due, salience, kind) in stories {
        add_obligation(
            &mut family,
            label,
            text,
            (actors, counterparts),
            due,
            salience,
            kind,
        );
    }
    family
        .graph
        .derived_memories
        .iter_mut()
        .find(|m| m.external_id == "current-sage")
        .unwrap()
        .supersedes_external_ids
        .push("superseded-sage".into());
    for (label, person, text, relation) in [
        (
            "settled-iris",
            "iris",
            "I mended the canvas bag and gave it to Iris.",
            RelationType::FulfillsCommitment,
        ),
        (
            "settled-rowan",
            "rowan",
            "Rowan returned my paper cutter.",
            RelationType::Resolves,
        ),
    ] {
        let resolver = format!("resolver-{label}");
        family.graph.derived_memories.push(derived(
            &resolver,
            "2025-02-01T12:00:00+09:00",
            text.into(),
            DerivedType::Reflection,
            vec!["self".into(), person.into()],
            0.6,
        ));
        assign(&mut family.embedding, text, consolidation::vector(7, 1.0));
        consolidation::link(
            &mut family,
            &resolver,
            relation,
            ObjectType::DerivedMemory,
            label,
        );
    }
    for person in ["iris", "rowan", "nia"] {
        for n in 0..32 {
            consolidation::belief(
                &mut family,
                format!("{person} prefers quiet meeting detail {n}."),
                None,
                Some(person),
                0.95,
                consolidation::vector(7, 1.0),
            );
            // These are reflections about a person, formed in a keyless scene.
            // Extra Involves edges would test the ordinary hub bound
            // instead of the intended crowded About prefix.
            family
                .experiences
                .last_mut()
                .unwrap()
                .write
                .scene
                .participants
                .clear();
        }
    }
    let mut topic_sources = Vec::new();
    for n in 0..64 {
        let (kind, text) = match n % 3 {
            0 => (
                DerivedType::Reflection,
                format!("Copper bell repair: learned method {n}."),
            ),
            1 => (
                DerivedType::OpenLoop,
                format!("I still need to inspect copper bell component {n}."),
            ),
            _ => (
                DerivedType::Commitment,
                format!("I intend to repair copper bell component {n}."),
            ),
        };
        consolidation::belief(
            &mut family,
            text,
            None,
            None,
            0.5,
            consolidation::vector(0, 0.99 - n as f32 * 0.001),
        );
        family
            .graph
            .derived_memories
            .last_mut()
            .unwrap()
            .derived_type = kind;
        let source = &family.experiences.last().unwrap().write;
        // Keep strong source bodies below beliefs so vector capacity also pressures
        // every derived section, including open loops and commitments.
        assign(
            &mut family.embedding,
            &source.content,
            consolidation::vector(0, 0.8 - n as f32 * 0.001),
        );
        family.topic_targets.extend([
            family
                .graph
                .derived_memories
                .last()
                .unwrap()
                .external_id
                .clone(),
            source.episode_external_id.clone(),
        ]);
        // Two source occasions per kind expose the cohort through twelve roots.
        // Each shared source has at most eleven DerivedFrom links and one ObservedIn,
        // below the ordinary fanout; all original source bodies remain in the story.
        if n < 6 {
            topic_sources.push(source.episode_external_id.clone());
        } else {
            let shared_source = &topic_sources[n % 6];
            let memory = family.graph.derived_memories.last_mut().unwrap();
            memory
                .source_episode_external_ids
                .push(shared_source.clone());
            let id = memory.external_id.clone();
            consolidation::link(
                &mut family,
                &id,
                RelationType::DerivedFrom,
                ObjectType::Episode,
                shared_source,
            );
        }
    }
    assign(
        &mut family.embedding,
        OWN_TOPIC,
        consolidation::vector(2, 1.0),
    );
    let own_text = family
        .graph
        .derived_memories
        .iter()
        .find(|m| m.external_id == "own-undated")
        .unwrap()
        .text
        .clone();
    assign(
        &mut family.embedding,
        &own_text,
        consolidation::vector(2, 1.0),
    );
    let settled_text = family
        .graph
        .derived_memories
        .iter()
        .find(|m| m.external_id == "settled-iris")
        .unwrap()
        .text
        .clone();
    assign(
        &mut family.embedding,
        &settled_text,
        consolidation::vector(3, 1.0),
    );
    for (name, people, topic, floor) in [
        ("topic-alone", &[][..], Some(TOPIC), 1),
        ("iris-no-topic", &["iris"][..], None, 1),
        ("iris-loud-floor-1", &["iris"][..], Some(TOPIC), 1),
        ("iris-loud-floor-2", &["iris"][..], Some(TOPIC), 2),
        ("iris-rowan-loud", &["iris", "rowan"][..], Some(TOPIC), 1),
        ("rowan-iris-loud", &["rowan", "iris"][..], Some(TOPIC), 1),
        ("nia-crowded-loud", &["nia"][..], Some(TOPIC), 1),
        ("bob-tomorrow-loud", &["bob"][..], Some(TOPIC), 1),
        ("sage-current-only", &["sage"][..], Some(TOPIC), 1),
        ("due-nothing-said", &[][..], None, 1),
        ("due-loud", &[][..], Some(TOPIC), 1),
        ("own-topic", &[][..], Some(OWN_TOPIC), 1),
        ("settled-topic", &[][..], Some(settled_text.as_str()), 1),
    ] {
        add_probe(&mut family, config, name, people, topic, floor);
    }
    add_probe(
        &mut family,
        config,
        "include-superseded",
        &["sage"],
        Some(TOPIC),
        1,
    );
    family
        .probes
        .last_mut()
        .unwrap()
        .supported_input
        .lifecycle_policy = Some(native::RetrievalLifecyclePolicy {
        include_superseded: true,
        ..Default::default()
    });
    family
}

pub(super) fn daily(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("prospective-daily");
    family.character_entity = Some("self".into());
    for n in 0..4 {
        add_obligation(
            &mut family,
            &format!("daily-overdue-{n}"),
            &format!("I promised Iris the archive card numbered {n}."),
            (&["self"], &["iris"]),
            Some("2025-09-08T12:00:00+09:00"),
            0.8 - n as f32 * 0.01,
            DerivedType::Commitment,
        );
        // This is a keyless daily store, including the obligations' source scenes.
        family
            .experiences
            .last_mut()
            .unwrap()
            .write
            .scene
            .participants
            .clear();
    }
    let last = timestamp("2025-09-09T07:00:00+09:00").unwrap();
    for n in 0..367 {
        experience(
            &mut family,
            &format!("day-{n:03}"),
            &(last - Duration::days(366 - n)).to_rfc3339(),
            false,
            0.5,
            -0.1,
        );
    }
    add_probe(&mut family, config, "daily-nothing-said", &[], None, 1);
    family
}

fn settled_or_superseded(family: &Family, id: &str) -> (bool, bool) {
    let settled = family.graph.links.iter().any(|l| {
        l.to.external_id == id
            && matches!(
                l.relation,
                RelationType::Resolves | RelationType::FulfillsCommitment
            )
    });
    let superseded = family
        .graph
        .derived_memories
        .iter()
        .any(|m| m.supersedes_external_ids.iter().any(|old| old == id));
    (settled, superseded)
}

pub(super) fn topic_control(input: &RetrieveInput) -> RetrieveInput {
    let mut control = input.clone();
    control.scene.participants.clear();
    control.topic = Some(TOPIC.into());
    control
}

pub(super) fn ensure_healthy(
    family: &Family,
    input: &RetrieveInput,
    observed: &Value,
) -> Result<()> {
    ensure!(
        observed["telemetry"]["graph_expansion"]["bounded_failure_count"] == 0,
        "degraded obligations graph recall: family={} input={} graph={} expansions={}",
        family.name,
        serde_json::to_string(input)?,
        observed["telemetry"]["graph_expansion"],
        json!(
            observed["trace"]["graph_expansions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|expansion| !expansion["bounded_failure"].is_null())
                .collect::<Vec<_>>()
        )
    );
    ensure!(
        matches!(
            observed["telemetry"]["vector_recall_completeness"]["kind"].as_str(),
            Some("exhaustive" | "not_requested")
        ),
        "incomplete obligations vector recall"
    );
    if family.name == "prospective-meeting"
        && input.topic.as_deref() == Some(TOPIC)
        && input.scene.participants.is_empty()
    {
        let limits = &input.surface_policy.sections;
        for (section, cap) in [
            ("derived_memories", limits.derived_memories),
            ("open_loops", limits.open_loops),
            ("commitments", limits.commitments),
        ] {
            let selected = observed["selected"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["section"] == section)
                .count();
            ensure!(
                selected == cap,
                "unsaturated obligations pressure fixture: section={section} selected={selected} cap={cap}"
            );
        }
    }
    Ok(())
}

pub(super) fn reading(
    family: &Family,
    input: &RetrieveInput,
    pack: &RetrievedContextPack,
    observed: &Value,
    control: &Value,
) -> Result<Value> {
    ensure_healthy(family, input, observed)?;
    let selected = observed["selected"].as_array().unwrap();
    let contains = |id: &str| selected.iter().any(|s| s["external_id"] == id);
    let mut obligations = obligation_reading(pack, family, input)?;
    let included_superseded = input.lifecycle_policy.is_some_and(|p| p.include_superseded);
    let reference = timestamp(input.scene.time.as_deref().unwrap())?;
    let mut eligible = Vec::new();
    for (spec, row) in family
        .obligations
        .iter()
        .zip(obligations.as_array_mut().unwrap())
    {
        let memory = family
            .graph
            .derived_memories
            .iter()
            .find(|m| m.external_id == spec.memory_external_id)
            .unwrap();
        let (settled, superseded) = settled_or_superseded(family, &spec.memory_external_id);
        row["authored_settled"] = json!(settled);
        row["authored_superseded"] = json!(superseded);
        row["authored_salience"] = json!(memory.salience_score);
        row["source_episode_external_ids"] = json!(memory.source_episode_external_ids);
        row["expected_direction_if_forwarded"] =
            json!(if spec.actor_subjects.iter().any(|p| p == "self") {
                Some("owed_by_character")
            } else if spec.counterpart_subjects.iter().any(|p| p == "self") {
                Some("owed_to_character")
            } else {
                None
            });
        row["native_root_sources"] = json!(
            observed["roots"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["external_id"] == spec.memory_external_id)
                .map(|r| &r["source"])
                .collect::<Vec<_>>()
        );
        row["native_admitted_by"] = json!(
            pack.outcomes()[0]
                .memory_scenes
                .iter()
                .filter(|m| pack
                    .object_refs()
                    .get(&m.memory.id.to_string())
                    .is_some_and(|r| r.external_id == spec.memory_external_id))
                .flat_map(|m| &m.admitted_by)
                .collect::<BTreeSet<_>>()
        );
        if !settled && (!superseded || included_superseded) {
            eligible.push((spec, memory, superseded));
        }
    }
    eligible.sort_by(|a, b| {
        a.2.cmp(&b.2)
            .then_with(|| b.1.salience_score.total_cmp(&a.1.salience_score))
            .then_with(|| b.1.created_at.cmp(&a.1.created_at))
    });
    let person_rows = input.scene.participants.iter().filter_map(|p| p.key.as_deref()).map(|person| {
        let expected = eligible.iter().filter(|(o, _, _)| o.actor_subjects.iter().chain(&o.counterpart_subjects).any(|p| p == person)).collect::<Vec<_>>();
        let all = family.obligations.iter().filter(|o| o.actor_subjects.iter().chain(&o.counterpart_subjects).any(|p| p == person));
        let state = family.graph.derived_memories.iter().filter(|m| !matches!(m.derived_type, DerivedType::OpenLoop | DerivedType::Commitment) && m.entity_external_ids.iter().any(|p| p == person)).filter(|m| contains(&m.external_id)).map(|m| &m.external_id).collect::<Vec<_>>();
        json!({"person":person,"most_salient_unresolved":expected.first().map(|(o,_,_)| &o.label),
            "first_two_unresolved":expected.iter().take(2).map(|(o,_,_)| json!({"label":o.label,"external_id":o.memory_external_id,"selected":contains(&o.memory_external_id)})).collect::<Vec<_>>(),
            "selected_obligation_count":all.filter(|o| contains(&o.memory_external_id)).count(),"non_obligation_state_count":state.len(),"non_obligation_state_ids":state})
    }).collect::<Vec<_>>();
    let top_due = eligible.iter().find(|(o, _, _)| {
        o.due_instant.as_deref().is_some_and(|due| {
            timestamp(due)
                .unwrap()
                .with_timezone(reference.offset())
                .date_naive()
                <= reference.date_naive()
        })
    });
    let topic_ids = family
        .topic_targets
        .iter()
        .chain(
            family
                .experiences
                .iter()
                .filter(|e| family.topic_targets.contains(&e.write.episode_external_id))
                .map(|e| &e.write.observation_external_id),
        )
        .collect::<BTreeSet<_>>();
    let on_topic = selected
        .iter()
        .filter(|s| topic_ids.iter().any(|id| s["external_id"] == **id))
        .collect::<Vec<_>>();
    let topic_counts = |snapshot: &Value| {
        let mut counts = BTreeMap::<String, usize>::new();
        for row in snapshot["selected"].as_array().unwrap() {
            if topic_ids.iter().any(|id| row["external_id"] == **id) {
                *counts
                    .entry(row["section"].as_str().unwrap().to_owned())
                    .or_default() += 1;
            }
        }
        counts
    };
    let mut latest = family
        .experiences
        .iter()
        .filter(|e| {
            e.write.salience == Some(0.5)
                && timestamp(e.write.scene.time.as_deref().unwrap()).unwrap() <= reference
        })
        .collect::<Vec<_>>();
    latest.sort_by_key(|e| {
        std::cmp::Reverse(timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
    });
    let latest_ids = latest
        .iter()
        .take(input.surface_policy.sections.relevant_episodes)
        .map(|e| &e.write.episode_external_id)
        .collect::<Vec<_>>();
    Ok(
        json!({"first_person":person_rows.first(),"people":person_rows,"obligations":obligations,
        "most_salient_due":top_due.map(|(o,_,_)| json!({"label":o.label,"external_id":o.memory_external_id,"selected":contains(&o.memory_external_id)})),
        "on_topic_memory_count":on_topic.len(),"on_topic_memories":on_topic,
        "on_topic_by_section":topic_counts(observed),"topic_alone_by_section":topic_counts(control),
        "on_topic_cohort":"Authored topical reflections, open loops and commitments, their source episodes and companion observations; each selected object counts once.",
        "latest_equal_salience_ids":latest_ids,"latest_equal_salience_selected":latest_ids.iter().filter(|id| contains(id)).count(),
        "reference_scene":pack.outcomes()[0].scene,"native_memory_scenes":pack.outcomes()[0].memory_scenes,
        "native_candidate_limits":native::RetrievalCandidateLimits::default(),"native_graph_limits":native::RetrievalGraphLimits::default(),
        "native_section_limits":native::ContinuitySectionLimits::default(),"executed_floors":input.cue_floors.unwrap_or_default(),
        "executed_surface_policy":input.surface_policy,
        "full_prospective_status":"not_run","role_and_due_fields_forwarded":false,
        "basis":"Counts and eligibility are authored comparison cohorts; selection, root source, admission road, scores and resolution are native observations. First-person order is the caller's order. Supersession priority describes selector/floor expectations, never a claim about final pack order."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_admission_requires_actual_section_saturation() {
        let family = meeting(&config());
        let query = topic_control(&family.probes[0].supported_input);
        let snapshot = |counts: [usize; 3]| {
            json!({
                "telemetry":{"graph_expansion":{"bounded_failure_count":0},
                             "vector_recall_completeness":{"kind":"exhaustive"}},
                "selected":(["derived_memories", "open_loops", "commitments"].into_iter()
                    .zip(counts).flat_map(|(section, count)|
                        (0..count).map(move |_| json!({"section":section}))).collect::<Vec<_>>())
            })
        };
        assert!(ensure_healthy(&family, &query, &snapshot([4, 4, 4])).is_err());
        assert!(ensure_healthy(&family, &query, &snapshot([12, 8, 8])).is_ok());
    }

    #[test]
    fn stories_cover_direction_day_boundaries_pressure_and_opposed_references() {
        let family = meeting(&config());
        let reference = timestamp(MORNING).unwrap();
        assert_ne!(
            reference.date_naive(),
            reference.with_timezone(&Utc).date_naive()
        );
        assert_eq!(family.character_entity.as_deref(), Some("self"));
        assert_eq!(family.topic_targets.len(), 128);
        for person in ["iris", "rowan", "nia"] {
            let state = family
                .graph
                .derived_memories
                .iter()
                .filter(|m| m.salience_score == 0.95 && m.entity_external_ids == [person])
                .collect::<Vec<_>>();
            assert_eq!(state.len(), 32);
            assert!(
                state
                    .iter()
                    .flat_map(|m| &m.source_episode_external_ids)
                    .all(|id| {
                        family
                            .experiences
                            .iter()
                            .find(|e| &e.write.episode_external_id == id)
                            .unwrap()
                            .write
                            .scene
                            .participants
                            .is_empty()
                    })
            );
        }
        let limits = native::ContinuitySectionLimits::default();
        for (kind, cap) in [
            (DerivedType::Reflection, limits.derived_memories),
            (DerivedType::OpenLoop, limits.open_loops),
            (DerivedType::Commitment, limits.commitments),
        ] {
            assert!(
                family
                    .graph
                    .derived_memories
                    .iter()
                    .filter(
                        |m| m.derived_type == kind && family.topic_targets.contains(&m.external_id)
                    )
                    .count()
                    > cap
            );
        }
        let mut source_degrees = BTreeMap::<&str, usize>::new();
        for memory in family
            .graph
            .derived_memories
            .iter()
            .filter(|m| family.topic_targets.contains(&m.external_id))
        {
            for source in &memory.source_episode_external_ids {
                *source_degrees.entry(source).or_default() += 1;
                assert!(
                    family
                        .graph
                        .links
                        .iter()
                        .any(|link| link.from.external_id == memory.external_id
                            && link.to.external_id == *source
                            && link.relation == RelationType::DerivedFrom)
                );
            }
        }
        assert_eq!(
            source_degrees.values().filter(|count| **count > 1).count(),
            6
        );
        assert!(
            source_degrees
                .values()
                .all(|count| *count < native::RetrievalGraphLimits::default().max_fanout_per_node)
        );
        assert!(family.probes.iter().any(|p| p.trigger_floor == Some(2)));
        assert!(family.probes.iter().any(|p| {
            p.supported_input
                .lifecycle_policy
                .is_some_and(|v| v.include_superseded)
        }));
        for probe in &family.probes {
            let mut reference = probe.supported_input.clone();
            reference.scene.time = Some("2025-09-09T22:00:00+09:00".into());
            let mut expected = serde_json::to_value(&reference).unwrap();
            expected["scene"]["participants"] = json!([]);
            expected["topic"] = json!(TOPIC);
            assert_eq!(
                serde_json::to_value(topic_control(&reference)).unwrap(),
                expected
            );
        }
        for obligation in &family.obligations {
            let memory = family
                .graph
                .derived_memories
                .iter()
                .find(|m| m.external_id == obligation.memory_external_id)
                .unwrap();
            assert!(memory.assertions.is_empty() && memory.metadata.is_null());
            assert!(
                obligation
                    .actor_subjects
                    .iter()
                    .chain(&obligation.counterpart_subjects)
                    .all(|p| memory.entity_external_ids.contains(p))
            );
            assert!(
                obligation
                    .actor_subjects
                    .iter()
                    .all(|p| !obligation.counterpart_subjects.contains(p))
            );
        }
        let future = family
            .obligations
            .iter()
            .find(|o| o.label == "bob-tomorrow")
            .unwrap();
        assert_eq!(
            timestamp(future.due_instant.as_deref().unwrap())
                .unwrap()
                .with_timezone(reference.offset())
                .date_naive(),
            reference.date_naive().succ_opt().unwrap()
        );
        let ids = family
            .experiences
            .iter()
            .map(|e| &e.write.episode_external_id)
            .chain(family.graph.derived_memories.iter().map(|m| &m.external_id))
            .enumerate()
            .map(|(n, id)| (id.clone(), format!("native-{n:04}")))
            .collect();
        let (changed, order) = opposed(&family, &ids).unwrap();
        for (old, new) in family
            .graph
            .derived_memories
            .iter()
            .zip(&changed.graph.derived_memories)
        {
            assert_eq!(old.text, new.text);
            assert_eq!(old.created_at, new.created_at);
            assert_eq!(old.salience_score, new.salience_score);
            for predecessor in &old.supersedes_external_ids {
                let expected = order
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["original_external_id"] == *predecessor)
                    .unwrap();
                assert!(
                    new.supersedes_external_ids
                        .iter()
                        .any(|id| expected["external_id"] == *id)
                );
            }
        }
        let daily = daily(&config());
        assert_eq!(
            daily
                .experiences
                .iter()
                .filter(|e| e.write.episode_external_id.starts_with("day-"))
                .count(),
            367
        );
        assert!(
            daily
                .experiences
                .iter()
                .all(|e| e.write.scene.participants.is_empty() && e.write.salience == Some(0.5))
        );
        assert!(daily.obligations.iter().all(|o| {
            timestamp(o.due_instant.as_deref().unwrap())
                .unwrap()
                .with_timezone(reference.offset())
                .date_naive()
                < reference.date_naive()
        }));
        assert_eq!(falsifiers().as_object().unwrap().len(), 10);
    }
}
