use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use cmem_eval_continuity::{
    ContinuityFixtureSet, ContinuityScenario, ContinuityScenarioEmbedding, ExpectedRelevance,
    InteractionEvent, LATEST_CONTINUITY_FIXTURE_SCHEMA_VERSION, ScenarioPattern,
    canonical_fixture_bytes, runtime_memory_embedding_text,
};
use cmem_eval_core::{
    FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION, FrozenEmbeddingManifest, FrozenEmbeddingText,
    FrozenSimilarityOrdering,
};
use cmem_eval_locomo::{LoCoMoQa, LoCoMoSample};
use cmem_eval_longmemeval::LongMemEvalInstance;
use serde::{Deserialize, Serialize};

pub const SELECTION_MANIFEST_SCHEMA_VERSION: u32 = 2;
pub const BENCHMARK_FIXTURE_SEED: u64 = 0x0023_2026_0718;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectionManifest {
    pub schema_version: u32,
    pub instances: Vec<InstanceSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstanceSelection {
    pub fixture_id: String,
    pub source: BenchmarkSource,
    pub source_instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_qa_index: Option<usize>,
    pub scenario_kind: ScenarioKind,
    pub expected_question_type: String,
    pub selected_session_ids: Vec<String>,
    pub sampled_negative_turn_ids: Vec<String>,
    pub similarity_nearer_external_id: String,
    pub similarity_farther_external_id: String,
    pub selection_proof: SelectionProof,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSource {
    LongmemevalS,
    Locomo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioKind {
    Update,
    Temporal,
    MultiEvidence,
    SingleHopControl,
    Abstention,
}

impl ScenarioKind {
    fn pattern(self) -> ScenarioPattern {
        match self {
            Self::Update => ScenarioPattern::EntrenchedCorrection,
            Self::Temporal => ScenarioPattern::TemporalPatterns,
            Self::MultiEvidence => ScenarioPattern::MultiEvidenceAssembly,
            Self::SingleHopControl => ScenarioPattern::Autobiographical,
            Self::Abstention => ScenarioPattern::Abstention,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectionProof {
    pub machine_derived: MachineDerivedPredicates,
    pub curator_asserted: CuratorAssertions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MachineDerivedPredicates {
    pub session_count: usize,
    pub evidence_clean: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_img_url_in_evidence: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gold_turn_ids_empty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CuratorAssertions {
    pub self_contained: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversionArtifacts {
    pub fixtures: ContinuityFixtureSet,
    pub embedding_manifest: FrozenEmbeddingManifest,
}

#[derive(Debug, Clone)]
struct SourceTurn {
    external_id: String,
    timestamp: DateTime<Utc>,
    text: String,
}

#[derive(Debug)]
struct ConvertedScenario {
    scenario: ContinuityScenario,
    text_id_by_external_id: BTreeMap<String, String>,
    query_text_id: String,
}

pub fn load_selection_manifest(path: &Path) -> Result<SelectionManifest> {
    let bytes = fs::read(path)
        .with_context(|| format!("read benchmark selection manifest {}", path.display()))?;
    parse_selection_manifest_bytes(&bytes)
        .with_context(|| format!("parse benchmark selection manifest {}", path.display()))
}

pub fn parse_selection_manifest_bytes(bytes: &[u8]) -> Result<SelectionManifest> {
    let manifest: SelectionManifest = serde_json::from_slice(bytes)
        .context("decode benchmark selection manifest as UTF-8 JSON")?;
    manifest.validate()?;
    Ok(manifest)
}

impl SelectionManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SELECTION_MANIFEST_SCHEMA_VERSION {
            bail!(
                "unsupported benchmark selection manifest schema_version {}; expected {}",
                self.schema_version,
                SELECTION_MANIFEST_SCHEMA_VERSION
            );
        }
        if self.instances.is_empty() {
            bail!("benchmark selection manifest must contain at least one instance");
        }
        let mut fixture_ids = BTreeSet::new();
        for selection in &self.instances {
            if selection.fixture_id.trim().is_empty()
                || selection.source_instance_id.trim().is_empty()
                || selection.expected_question_type.trim().is_empty()
            {
                bail!("benchmark selection identity fields must not be empty");
            }
            if !fixture_ids.insert(selection.fixture_id.as_str()) {
                bail!("duplicate benchmark fixture_id {:?}", selection.fixture_id);
            }
            if !(3..=5).contains(&selection.selected_session_ids.len()) {
                bail!(
                    "selection {:?} must contain 3 to 5 sessions, found {}",
                    selection.fixture_id,
                    selection.selected_session_ids.len()
                );
            }
            let unique_sessions = selection
                .selected_session_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if unique_sessions.len() != selection.selected_session_ids.len() {
                bail!(
                    "selection {:?} contains duplicate session IDs",
                    selection.fixture_id
                );
            }
            if selection.selection_proof.machine_derived.session_count
                != selection.selected_session_ids.len()
            {
                bail!(
                    "selection {:?} records session_count {}, but selects {} sessions",
                    selection.fixture_id,
                    selection.selection_proof.machine_derived.session_count,
                    selection.selected_session_ids.len()
                );
            }
            if !selection.selection_proof.machine_derived.evidence_clean {
                bail!(
                    "selection {:?} must record machine-derived evidence_clean=true",
                    selection.fixture_id
                );
            }
            if !selection.selection_proof.curator_asserted.self_contained {
                bail!(
                    "selection {:?} must record curator-asserted self_contained=true",
                    selection.fixture_id
                );
            }
            if selection.sampled_negative_turn_ids.is_empty() {
                bail!(
                    "selection {:?} must include at least one sampled negative",
                    selection.fixture_id
                );
            }
            let unique_negatives = selection
                .sampled_negative_turn_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if unique_negatives.len() != selection.sampled_negative_turn_ids.len() {
                bail!(
                    "selection {:?} contains duplicate sampled negatives",
                    selection.fixture_id
                );
            }
            if selection.similarity_nearer_external_id == selection.similarity_farther_external_id {
                bail!(
                    "selection {:?} similarity candidates must be distinct",
                    selection.fixture_id
                );
            }
            if !unique_negatives.contains(selection.similarity_farther_external_id.as_str()) {
                bail!(
                    "selection {:?} similarity farther candidate must be a sampled negative",
                    selection.fixture_id
                );
            }
            match selection.source {
                BenchmarkSource::LongmemevalS => {
                    if selection.source_qa_index.is_some() {
                        bail!(
                            "LongMemEval-S selection {:?} must not set source_qa_index",
                            selection.fixture_id
                        );
                    }
                    if selection
                        .selection_proof
                        .machine_derived
                        .gold_turn_ids_empty
                        .is_none()
                        || selection
                            .selection_proof
                            .machine_derived
                            .no_img_url_in_evidence
                            .is_some()
                    {
                        bail!(
                            "LongMemEval-S selection {:?} must record gold_turn_ids_empty and omit no_img_url_in_evidence",
                            selection.fixture_id
                        );
                    }
                }
                BenchmarkSource::Locomo => {
                    if selection.source_qa_index.is_none() {
                        bail!(
                            "LoCoMo selection {:?} must set source_qa_index",
                            selection.fixture_id
                        );
                    }
                    if selection
                        .selection_proof
                        .machine_derived
                        .no_img_url_in_evidence
                        .is_none()
                        || selection
                            .selection_proof
                            .machine_derived
                            .gold_turn_ids_empty
                            .is_some()
                    {
                        bail!(
                            "LoCoMo selection {:?} must record no_img_url_in_evidence and omit gold_turn_ids_empty",
                            selection.fixture_id
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn convert_paths(
    manifest: &SelectionManifest,
    longmemeval_path: &Path,
    locomo_path: &Path,
) -> Result<ConversionArtifacts> {
    let longmemeval = cmem_eval_longmemeval::load_path(longmemeval_path)?;
    let locomo = cmem_eval_locomo::load_path(locomo_path)?;
    convert_loaded_datasets(manifest, &longmemeval, &locomo)
}

pub fn convert_loaded_datasets(
    manifest: &SelectionManifest,
    longmemeval: &[LongMemEvalInstance],
    locomo: &[LoCoMoSample],
) -> Result<ConversionArtifacts> {
    manifest.validate()?;
    let longmemeval = longmemeval
        .iter()
        .map(|row| (row.question_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let locomo = locomo
        .iter()
        .map(|row| (row.sample_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut converted = Vec::with_capacity(manifest.instances.len());
    for selection in &manifest.instances {
        let scenario = match selection.source {
            BenchmarkSource::LongmemevalS => {
                let instance = longmemeval
                    .get(selection.source_instance_id.as_str())
                    .with_context(|| {
                        format!(
                            "selection {:?} references missing LongMemEval-S instance {:?}",
                            selection.fixture_id, selection.source_instance_id
                        )
                    })?;
                convert_longmemeval(selection, instance)?
            }
            BenchmarkSource::Locomo => {
                let sample = locomo
                    .get(selection.source_instance_id.as_str())
                    .with_context(|| {
                        format!(
                            "selection {:?} references missing LoCoMo sample {:?}",
                            selection.fixture_id, selection.source_instance_id
                        )
                    })?;
                convert_locomo(selection, sample)?
            }
        };
        converted.push(scenario);
    }

    let fixtures = ContinuityFixtureSet {
        schema_version: LATEST_CONTINUITY_FIXTURE_SCHEMA_VERSION,
        seed: BENCHMARK_FIXTURE_SEED,
        scenarios: converted
            .iter()
            .map(|converted| converted.scenario.clone())
            .collect(),
    };
    fixtures.validate()?;
    let embedding_manifest = build_embedding_manifest(manifest, &converted)?;
    Ok(ConversionArtifacts {
        fixtures,
        embedding_manifest,
    })
}

fn convert_longmemeval(
    selection: &InstanceSelection,
    instance: &LongMemEvalInstance,
) -> Result<ConvertedScenario> {
    require_question_type(selection, instance.question_type.as_deref())?;
    if selection.scenario_kind == ScenarioKind::Abstention
        && !instance.question_id.ends_with("_abs")
    {
        bail!(
            "LongMemEval-S abstention selection {:?} must use an _abs source row",
            selection.fixture_id
        );
    }
    let gold_ids = instance.gold_turn_ids();
    if let Some(expected_empty) = selection
        .selection_proof
        .machine_derived
        .gold_turn_ids_empty
        && gold_ids.is_empty() != expected_empty
    {
        bail!(
            "selection {:?} expected gold_turn_ids_empty={expected_empty}, derived {}",
            selection.fixture_id,
            gold_ids.is_empty()
        );
    }
    if selection.scenario_kind == ScenarioKind::Abstention && !gold_ids.is_empty() {
        bail!(
            "abstention selection {:?} must derive empty LongMemEval-S gold_turn_ids()",
            selection.fixture_id
        );
    }
    if selection.scenario_kind != ScenarioKind::Abstention && gold_ids.is_empty() {
        bail!(
            "non-abstention selection {:?} must derive at least one LongMemEval-S gold turn",
            selection.fixture_id
        );
    }

    let selected_ids = selection
        .selected_session_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let gold_set = gold_ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut turns = Vec::new();
    for session_id in &selection.selected_session_ids {
        let session = instance
            .sessions
            .iter()
            .find(|session| &session.session_id == session_id)
            .with_context(|| {
                format!(
                    "selection {:?} references missing LongMemEval-S session {session_id:?}",
                    selection.fixture_id
                )
            })?;
        let base = require_timestamp(
            session.date.as_deref(),
            &selection.fixture_id,
            &session.session_id,
        )?;
        for turn in &session.turns {
            require_source_text(&selection.fixture_id, &session.session_id, &turn.text)?;
            turns.push(SourceTurn {
                external_id: format!("{}:turn:{}", session.session_id, turn.index),
                timestamp: base + Duration::seconds(turn.index as i64),
                text: turn.text.clone(),
            });
        }
    }
    let evidence_clean = gold_ids.iter().all(|gold_id| {
        turns
            .iter()
            .filter(|turn| turn.external_id == *gold_id)
            .count()
            == 1
    });
    if evidence_clean != selection.selection_proof.machine_derived.evidence_clean {
        bail!(
            "selection {:?} records evidence_clean={}, derived {evidence_clean}",
            selection.fixture_id,
            selection.selection_proof.machine_derived.evidence_clean
        );
    }
    for gold_id in &gold_ids {
        let gold_session = gold_id
            .split_once(":turn:")
            .map(|(session, _)| session)
            .unwrap_or_default();
        if !selected_ids.contains(gold_session) {
            bail!(
                "selection {:?} omits gold session {gold_session:?}",
                selection.fixture_id
            );
        }
    }
    require_negatives_present(selection, &turns)?;

    let (relevant_ids, correction) = if selection.scenario_kind == ScenarioKind::Update {
        if gold_ids.len() != 2 {
            bail!(
                "update selection {:?} must derive exactly two gold turns, found {}",
                selection.fixture_id,
                gold_ids.len()
            );
        }
        let mut updates = turns
            .iter()
            .filter(|turn| gold_set.contains(turn.external_id.as_str()))
            .collect::<Vec<_>>();
        updates.sort_by_key(|turn| (turn.timestamp, turn.external_id.as_str()));
        if updates.len() != 2 {
            bail!(
                "update selection {:?} must select both gold turns",
                selection.fixture_id
            );
        }
        let old = updates[0];
        let new = updates[1];
        if !selection
            .sampled_negative_turn_ids
            .contains(&old.external_id)
        {
            bail!(
                "update selection {:?} must label the old gold turn {:?} as sampled negative",
                selection.fixture_id,
                old.external_id
            );
        }
        (
            vec![new.external_id.clone()],
            Some((
                old.external_id.clone(),
                new.external_id.clone(),
                new.timestamp,
                new.text.clone(),
            )),
        )
    } else if selection.scenario_kind == ScenarioKind::Abstention {
        (Vec::new(), None)
    } else {
        (gold_ids, None)
    };

    let query_source_timestamp = instance
        .question_date
        .as_deref()
        .map(parse_source_timestamp)
        .transpose()
        .with_context(|| {
            format!(
                "selection {:?} has invalid LongMemEval-S question_date",
                selection.fixture_id
            )
        })?;
    assemble_scenario(
        selection,
        &instance.question,
        turns,
        relevant_ids,
        correction,
        query_source_timestamp,
    )
}

fn convert_locomo(
    selection: &InstanceSelection,
    sample: &LoCoMoSample,
) -> Result<ConvertedScenario> {
    let qa_index = selection
        .source_qa_index
        .expect("manifest validation requires LoCoMo source_qa_index");
    let qa = sample.qa.get(qa_index.saturating_sub(1)).with_context(|| {
        format!(
            "selection {:?} references missing LoCoMo QA index {qa_index}",
            selection.fixture_id
        )
    })?;
    if qa.qa_index != qa_index {
        bail!(
            "selection {:?} resolved inconsistent LoCoMo QA index",
            selection.fixture_id
        );
    }
    require_question_type(selection, qa.question_type.as_deref())?;
    let selected_ids = selection
        .selected_session_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let evidence_sessions = sample.evidence_sessions(qa);
    for evidence_session in &evidence_sessions {
        if !selected_ids.contains(evidence_session.as_str()) {
            bail!(
                "selection {:?} omits LoCoMo evidence session {evidence_session:?}",
                selection.fixture_id
            );
        }
    }
    validate_locomo_evidence(selection, sample, qa)?;

    let mut turns = Vec::new();
    for session_id in &selection.selected_session_ids {
        let session = sample
            .sessions
            .iter()
            .find(|session| &session.session_id == session_id)
            .with_context(|| {
                format!(
                    "selection {:?} references missing LoCoMo session {session_id:?}",
                    selection.fixture_id
                )
            })?;
        let base = require_timestamp(
            session.timestamp.as_deref(),
            &selection.fixture_id,
            &session.session_id,
        )?;
        for (index, turn) in session.turns.iter().enumerate() {
            require_source_text(&selection.fixture_id, &session.session_id, &turn.text)?;
            turns.push(SourceTurn {
                external_id: turn.dialog_id.clone(),
                timestamp: base + Duration::seconds(index as i64 + 1),
                text: turn.text.clone(),
            });
        }
    }
    require_negatives_present(selection, &turns)?;
    let relevant_ids = if selection.scenario_kind == ScenarioKind::Abstention {
        for evidence_id in &qa.evidence_dialog_ids {
            if !selection.sampled_negative_turn_ids.contains(evidence_id) {
                bail!(
                    "LoCoMo abstention selection {:?} must label cited evidence {evidence_id:?} as sampled-negative-only",
                    selection.fixture_id
                );
            }
        }
        Vec::new()
    } else {
        qa.evidence_dialog_ids.clone()
    };
    assemble_scenario(selection, &qa.question, turns, relevant_ids, None, None)
}

fn validate_locomo_evidence(
    selection: &InstanceSelection,
    sample: &LoCoMoSample,
    qa: &LoCoMoQa,
) -> Result<()> {
    let mut evidence_clean = !qa.evidence_dialog_ids.is_empty();
    let mut no_img_url = true;
    for evidence_id in &qa.evidence_dialog_ids {
        let matches = sample
            .sessions
            .iter()
            .flat_map(|session| session.turns.iter())
            .filter(|turn| &turn.dialog_id == evidence_id)
            .collect::<Vec<_>>();
        evidence_clean &= matches.len() == 1 && !matches[0].text.is_empty();
        no_img_url &= matches.len() == 1 && matches[0].image_urls.is_empty();
    }
    if evidence_clean != selection.selection_proof.machine_derived.evidence_clean {
        bail!(
            "selection {:?} records evidence_clean={}, derived {evidence_clean}",
            selection.fixture_id,
            selection.selection_proof.machine_derived.evidence_clean
        );
    }
    if let Some(expected) = selection
        .selection_proof
        .machine_derived
        .no_img_url_in_evidence
        && expected != no_img_url
    {
        bail!(
            "selection {:?} records no_img_url_in_evidence={expected}, derived {no_img_url}",
            selection.fixture_id
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn assemble_scenario(
    selection: &InstanceSelection,
    query: &str,
    mut turns: Vec<SourceTurn>,
    relevant_ids: Vec<String>,
    correction: Option<(String, String, DateTime<Utc>, String)>,
    query_source_timestamp: Option<DateTime<Utc>>,
) -> Result<ConvertedScenario> {
    require_source_text(&selection.fixture_id, "query", query)?;
    if selection.scenario_kind == ScenarioKind::Abstention {
        if !selection
            .sampled_negative_turn_ids
            .contains(&selection.similarity_nearer_external_id)
        {
            bail!(
                "abstention selection {:?} similarity nearer candidate must be sampled negative",
                selection.fixture_id
            );
        }
    } else if !relevant_ids.contains(&selection.similarity_nearer_external_id) {
        bail!(
            "selection {:?} similarity nearer candidate must be relevant",
            selection.fixture_id
        );
    }
    turns.sort_by_key(|turn| (turn.timestamp, turn.external_id.clone()));
    let replacement_id = correction
        .as_ref()
        .map(|(_, replacement_id, _, _)| replacement_id.as_str());
    let mut events = Vec::with_capacity(turns.len() + 2);
    let mut text_id_by_external_id = BTreeMap::new();
    for turn in &turns {
        if Some(turn.external_id.as_str()) == replacement_id {
            continue;
        }
        let event_id = format!("{}:remember:{:04}", selection.fixture_id, events.len() + 1);
        text_id_by_external_id.insert(turn.external_id.clone(), format!("{event_id}:embedding"));
        events.push(InteractionEvent::Remember {
            event_id,
            external_id: turn.external_id.clone(),
            timestamp: turn.timestamp,
            text: turn.text.clone(),
            surface_texts: None,
            entity_external_ids: Vec::new(),
            thread: None,
            salience: 0.5,
        });
    }
    if let Some((target_id, replacement_id, timestamp, replacement_text)) = correction {
        let event_id = format!("{}:correct", selection.fixture_id);
        text_id_by_external_id.insert(replacement_id.clone(), format!("{event_id}:embedding"));
        events.push(InteractionEvent::Correct {
            event_id,
            target_external_id: target_id,
            replacement_external_id: replacement_id,
            timestamp,
            replacement_text,
        });
    }
    events.sort_by_key(|event| (event.timestamp(), event.event_id().to_string()));
    let max_event_timestamp = events
        .iter()
        .map(InteractionEvent::timestamp)
        .max()
        .context("selected scenario must contain at least one source turn")?;
    let query_timestamp = query_source_timestamp
        .map(|timestamp| timestamp.max(max_event_timestamp))
        .unwrap_or(max_event_timestamp)
        + Duration::seconds(1);
    let query_event_id = format!("{}:query", selection.fixture_id);
    let query_text_id = format!("{query_event_id}:embedding");
    events.push(InteractionEvent::Query {
        event_id: query_event_id,
        query_id: format!("{}:query", selection.fixture_id),
        timestamp: query_timestamp,
        text: query.to_string(),
        expected: ExpectedRelevance {
            relevant_external_ids: relevant_ids,
            irrelevant_external_ids: selection.sampled_negative_turn_ids.clone(),
        },
    });
    let scenario = ContinuityScenario {
        fixture_id: selection.fixture_id.clone(),
        namespace: format!("continuity-benchmark:{}", selection.fixture_id),
        pattern: selection.scenario_kind.pattern(),
        entities: Vec::new(),
        embedding: ContinuityScenarioEmbedding::frozen(),
        events,
    };
    Ok(ConvertedScenario {
        scenario,
        text_id_by_external_id,
        query_text_id,
    })
}

fn build_embedding_manifest(
    manifest: &SelectionManifest,
    converted: &[ConvertedScenario],
) -> Result<FrozenEmbeddingManifest> {
    let mut texts = Vec::new();
    let mut orderings = Vec::new();
    for (selection, converted) in manifest.instances.iter().zip(converted) {
        for event in &converted.scenario.events {
            match event {
                InteractionEvent::Remember {
                    external_id, text, ..
                } => texts.push(FrozenEmbeddingText {
                    id: converted.text_id_by_external_id[external_id].clone(),
                    text: runtime_memory_embedding_text(text),
                }),
                InteractionEvent::Correct {
                    replacement_external_id,
                    replacement_text,
                    ..
                } => texts.push(FrozenEmbeddingText {
                    id: converted.text_id_by_external_id[replacement_external_id].clone(),
                    text: runtime_memory_embedding_text(replacement_text),
                }),
                InteractionEvent::Query { text, .. } => texts.push(FrozenEmbeddingText {
                    id: converted.query_text_id.clone(),
                    text: text.clone(),
                }),
                InteractionEvent::Forget { .. }
                | InteractionEvent::Link { .. }
                | InteractionEvent::Restart { .. } => {}
            }
        }
        let nearer = converted
            .text_id_by_external_id
            .get(&selection.similarity_nearer_external_id)
            .with_context(|| {
                format!(
                    "selection {:?} similarity nearer candidate {:?} is not admitted",
                    selection.fixture_id, selection.similarity_nearer_external_id
                )
            })?;
        let farther = converted
            .text_id_by_external_id
            .get(&selection.similarity_farther_external_id)
            .with_context(|| {
                format!(
                    "selection {:?} similarity farther candidate {:?} is not admitted",
                    selection.fixture_id, selection.similarity_farther_external_id
                )
            })?;
        orderings.push(FrozenSimilarityOrdering {
            description: format!(
                "{} query-adapted evidence outranks curated background",
                selection.fixture_id
            ),
            anchor_id: converted.query_text_id.clone(),
            descending_ids: vec![nearer.clone(), farther.clone()],
            min_margin: 0.0,
        });
    }
    let embedding_manifest = FrozenEmbeddingManifest {
        schema_version: FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION,
        texts,
        similarity_orderings: orderings,
    };
    embedding_manifest.validate()?;
    let expected_texts = converted
        .iter()
        .flat_map(|converted| converted.scenario.runtime_embedding_inputs())
        .collect::<BTreeSet<_>>();
    let manifest_texts = embedding_manifest
        .texts
        .iter()
        .map(|item| item.text.clone())
        .collect::<BTreeSet<_>>();
    if expected_texts != manifest_texts {
        bail!("embedding manifest text coverage does not match fixture runtime embedding inputs");
    }
    Ok(embedding_manifest)
}

pub fn canonical_embedding_manifest_bytes(manifest: &FrozenEmbeddingManifest) -> Result<Vec<u8>> {
    manifest.validate()?;
    let mut bytes = serde_json::to_vec_pretty(manifest)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn write_or_check(
    artifacts: &ConversionArtifacts,
    fixture_path: &Path,
    embedding_manifest_path: &Path,
    check: bool,
) -> Result<()> {
    let fixture_bytes = canonical_fixture_bytes(&artifacts.fixtures)?;
    let embedding_bytes = canonical_embedding_manifest_bytes(&artifacts.embedding_manifest)?;
    if check {
        require_identical(fixture_path, &fixture_bytes)?;
        require_identical(embedding_manifest_path, &embedding_bytes)?;
        return Ok(());
    }
    write_bytes(fixture_path, &fixture_bytes)?;
    write_bytes(embedding_manifest_path, &embedding_bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn require_identical(path: &Path, expected: &[u8]) -> Result<()> {
    let actual =
        fs::read(path).with_context(|| format!("read generated artifact {}", path.display()))?;
    if actual != expected {
        bail!(
            "generated artifact {} is stale; rerun without --check",
            path.display()
        );
    }
    Ok(())
}

fn require_question_type(selection: &InstanceSelection, actual: Option<&str>) -> Result<()> {
    if actual != Some(selection.expected_question_type.as_str()) {
        bail!(
            "selection {:?} expected question type {:?}, derived {:?}",
            selection.fixture_id,
            selection.expected_question_type,
            actual
        );
    }
    Ok(())
}

fn require_timestamp(
    timestamp: Option<&str>,
    fixture_id: &str,
    session_id: &str,
) -> Result<DateTime<Utc>> {
    timestamp
        .context("selected session timestamp is missing")
        .and_then(parse_source_timestamp)
        .with_context(|| {
            format!("selection {fixture_id:?} session {session_id:?} has invalid timestamp")
        })
}

fn parse_source_timestamp(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        return Ok(timestamp.with_timezone(&Utc));
    }
    let normalized_lme = if let Some((date, rest)) = value.split_once(" (") {
        rest.split_once(") ")
            .map(|(_, time)| format!("{date} {time}"))
    } else {
        None
    };
    if let Some(timestamp) = normalized_lme
        .as_deref()
        .and_then(|value| NaiveDateTime::parse_from_str(value, "%Y/%m/%d %H:%M").ok())
    {
        return Ok(DateTime::from_naive_utc_and_offset(timestamp, Utc));
    }
    if let Ok(timestamp) = NaiveDateTime::parse_from_str(value, "%I:%M %P on %-d %B, %Y") {
        return Ok(DateTime::from_naive_utc_and_offset(timestamp, Utc));
    }
    bail!("unsupported benchmark timestamp {value:?}")
}

fn require_source_text(fixture_id: &str, source_id: &str, text: &str) -> Result<()> {
    if text.is_empty() {
        bail!("selection {fixture_id:?} source text {source_id:?} is empty");
    }
    Ok(())
}

fn require_negatives_present(selection: &InstanceSelection, turns: &[SourceTurn]) -> Result<()> {
    let admitted = turns
        .iter()
        .map(|turn| turn.external_id.as_str())
        .collect::<BTreeSet<_>>();
    for external_id in &selection.sampled_negative_turn_ids {
        if !admitted.contains(external_id.as_str()) {
            bail!(
                "selection {:?} sampled negative {external_id:?} is not in a selected session",
                selection.fixture_id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmem_eval_continuity::InteractionEvent;
    use serde_json::json;

    #[test]
    fn selection_reader_rejects_corrupt_encoding() {
        let error = parse_selection_manifest_bytes(b"{\"schema_version\":1,\xff")
            .unwrap_err()
            .to_string();
        assert!(error.contains("UTF-8 JSON"));
    }

    #[test]
    fn selection_reader_rejects_partial_input() {
        let error = parse_selection_manifest_bytes(b"{\"schema_version\":1")
            .unwrap_err()
            .to_string();
        assert!(error.contains("UTF-8 JSON"));
    }

    #[test]
    fn selection_reader_rejects_unknown_schema() {
        let error = parse_selection_manifest_bytes(br#"{"schema_version":99,"instances":[]}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported benchmark selection manifest schema_version"));
    }

    #[test]
    fn selection_manifest_rejects_more_than_five_sessions() {
        let mut manifest = test_manifest(ScenarioKind::Update, false);
        let selection = &mut manifest.instances[0];
        selection.selected_session_ids = (1..=6).map(|index| format!("session-{index}")).collect();
        selection.selection_proof.machine_derived.session_count = 6;
        let error = manifest.validate().unwrap_err().to_string();
        assert!(error.contains("must contain 3 to 5 sessions"));
    }

    #[test]
    fn update_conversion_preserves_source_bytes_and_maps_current_answer_only() {
        let manifest = test_manifest(ScenarioKind::Update, false);
        let longmemeval = cmem_eval_longmemeval::load_value(json!([{
            "question_id": "lme",
            "question_type": "knowledge-update",
            "question": "What is current?\nExactly.",
            "question_date": "2024/01/04 (Thu) 00:00",
            "haystack_session_ids": ["old", "new", "background"],
            "haystack_dates": [
                "2024/01/01 (Mon) 00:00",
                "2024/01/02 (Tue) 00:00",
                "2024/01/03 (Wed) 00:00"
            ],
            "haystack_sessions": [
                [{"role":"user","content":"old\n  café","has_answer":true}],
                [{"role":"user","content":"new \t東京","has_answer":true}],
                [{"role":"user","content":"background"}]
            ],
            "answer_session_ids": ["old", "new"]
        }]))
        .unwrap();
        let artifacts = convert_loaded_datasets(&manifest, &longmemeval, &[]).unwrap();
        let scenario = &artifacts.fixtures.scenarios[0];
        assert!(scenario.events.iter().any(|event| matches!(
            event,
            InteractionEvent::Remember { external_id, text, .. }
                if external_id == "old:turn:1" && text.as_bytes() == "old\n  café".as_bytes()
        )));
        assert!(scenario.events.iter().any(|event| matches!(
            event,
            InteractionEvent::Correct {
                target_external_id,
                replacement_external_id,
                replacement_text,
                ..
            } if target_external_id == "old:turn:1"
                && replacement_external_id == "new:turn:1"
                && replacement_text.as_bytes() == "new \t東京".as_bytes()
        )));
        let InteractionEvent::Query { text, expected, .. } = scenario.events.last().unwrap() else {
            panic!("last event must be query");
        };
        assert_eq!(text.as_bytes(), "What is current?\nExactly.".as_bytes());
        assert_eq!(expected.relevant_external_ids, ["new:turn:1"]);
        assert!(
            expected
                .irrelevant_external_ids
                .contains(&"old:turn:1".to_string())
        );

        let manifest_texts = artifacts
            .embedding_manifest
            .texts
            .iter()
            .map(|item| (item.id.as_str(), item.text.as_str()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            manifest_texts["fixture:remember:0001:embedding"],
            "old café"
        );
        assert_eq!(manifest_texts["fixture:correct:embedding"], "new 東京");
        assert_eq!(
            manifest_texts["fixture:query:embedding"].as_bytes(),
            "What is current?\nExactly.".as_bytes()
        );
        assert_eq!(
            scenario.runtime_embedding_inputs(),
            artifacts
                .embedding_manifest
                .texts
                .iter()
                .map(|item| item.text.clone())
                .collect()
        );
    }

    #[test]
    fn longmemeval_evidence_clean_is_derived_from_selected_turns() {
        let manifest = test_manifest(ScenarioKind::Update, false);
        let longmemeval = cmem_eval_longmemeval::load_value(json!([{
            "question_id": "lme",
            "question_type": "knowledge-update",
            "question": "What is current?",
            "haystack_session_ids": ["old", "new", "background", "omitted-evidence"],
            "haystack_dates": [
                "2024/01/01 (Mon) 00:00",
                "2024/01/02 (Tue) 00:00",
                "2024/01/03 (Wed) 00:00",
                "2024/01/04 (Thu) 00:00"
            ],
            "haystack_sessions": [
                [{"role":"user","content":"old"}],
                [{"role":"user","content":"new"}],
                [{"role":"user","content":"background"}],
                [{"role":"user","content":"evidence","has_answer":true}]
            ],
            "answer_session_ids": ["omitted-evidence"]
        }]))
        .unwrap();
        let error = convert_loaded_datasets(&manifest, &longmemeval, &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("records evidence_clean=true, derived false"));
    }

    #[test]
    fn abstention_requires_empty_gold() {
        let mut manifest = test_manifest(ScenarioKind::Abstention, true);
        manifest.instances[0].source_instance_id = "lme_abs".to_string();
        let longmemeval = cmem_eval_longmemeval::load_value(json!([{
            "question_id": "lme_abs",
            "question_type": "knowledge-update",
            "question": "Unknown?",
            "haystack_session_ids": ["old", "new", "background"],
            "haystack_dates": [
                "2024/01/01 (Mon) 00:00",
                "2024/01/02 (Tue) 00:00",
                "2024/01/03 (Wed) 00:00"
            ],
            "haystack_sessions": [
                [{"role":"user","content":"old","has_answer":true}],
                [{"role":"user","content":"new"}],
                [{"role":"user","content":"background"}]
            ]
        }]))
        .unwrap();
        let error = convert_loaded_datasets(&manifest, &longmemeval, &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("expected gold_turn_ids_empty=true, derived false"));
    }

    #[test]
    fn committed_manifest_has_the_confirmed_roster() {
        let manifest = parse_selection_manifest_bytes(include_bytes!(
            "../continuity_benchmarks_v1_selection.json"
        ))
        .unwrap();
        let actual = manifest
            .instances
            .iter()
            .map(|selection| {
                (
                    selection.fixture_id.as_str(),
                    selection.source,
                    selection.source_instance_id.as_str(),
                    selection.source_qa_index,
                    selection.scenario_kind,
                    selection.selection_proof.machine_derived.session_count,
                    selection.selection_proof.machine_derived.evidence_clean,
                    selection
                        .selection_proof
                        .machine_derived
                        .no_img_url_in_evidence,
                    selection
                        .selection_proof
                        .machine_derived
                        .gold_turn_ids_empty,
                    selection.selection_proof.curator_asserted.self_contained,
                )
            })
            .collect::<BTreeSet<_>>();
        let expected = [
            (
                "benchmark-lme-update-01493427",
                BenchmarkSource::LongmemevalS,
                "01493427",
                None,
                ScenarioKind::Update,
                3,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-update-06db6396",
                BenchmarkSource::LongmemevalS,
                "06db6396",
                None,
                ScenarioKind::Update,
                3,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-update-18bc8abd",
                BenchmarkSource::LongmemevalS,
                "18bc8abd",
                None,
                ScenarioKind::Update,
                3,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-update-2698e78f",
                BenchmarkSource::LongmemevalS,
                "2698e78f",
                None,
                ScenarioKind::Update,
                3,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-temporal-08f4fc43",
                BenchmarkSource::LongmemevalS,
                "08f4fc43",
                None,
                ScenarioKind::Temporal,
                3,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-temporal-0bb5a684",
                BenchmarkSource::LongmemevalS,
                "0bb5a684",
                None,
                ScenarioKind::Temporal,
                3,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-multi-129d1232",
                BenchmarkSource::LongmemevalS,
                "129d1232",
                None,
                ScenarioKind::MultiEvidence,
                4,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-multi-2ce6a0f2",
                BenchmarkSource::LongmemevalS,
                "2ce6a0f2",
                None,
                ScenarioKind::MultiEvidence,
                5,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-multi-81507db6",
                BenchmarkSource::LongmemevalS,
                "81507db6",
                None,
                ScenarioKind::MultiEvidence,
                4,
                true,
                None,
                Some(false),
                true,
            ),
            (
                "benchmark-lme-abstention-0862e8bf-abs",
                BenchmarkSource::LongmemevalS,
                "0862e8bf_abs",
                None,
                ScenarioKind::Abstention,
                3,
                true,
                None,
                Some(true),
                true,
            ),
            (
                "benchmark-lme-abstention-19b5f2b3-abs",
                BenchmarkSource::LongmemevalS,
                "19b5f2b3_abs",
                None,
                ScenarioKind::Abstention,
                3,
                true,
                None,
                Some(true),
                true,
            ),
            (
                "benchmark-locomo-temporal-conv-30-qa1",
                BenchmarkSource::Locomo,
                "conv-30",
                Some(1),
                ScenarioKind::Temporal,
                3,
                true,
                Some(true),
                None,
                true,
            ),
            (
                "benchmark-locomo-temporal-conv-50-qa1",
                BenchmarkSource::Locomo,
                "conv-50",
                Some(1),
                ScenarioKind::Temporal,
                3,
                true,
                Some(true),
                None,
                true,
            ),
            (
                "benchmark-locomo-multi-conv-26-qa12",
                BenchmarkSource::Locomo,
                "conv-26",
                Some(12),
                ScenarioKind::MultiEvidence,
                3,
                true,
                Some(true),
                None,
                true,
            ),
            (
                "benchmark-locomo-multi-conv-41-qa4",
                BenchmarkSource::Locomo,
                "conv-41",
                Some(4),
                ScenarioKind::MultiEvidence,
                3,
                true,
                Some(true),
                None,
                true,
            ),
            (
                "benchmark-locomo-control-conv-30-qa40",
                BenchmarkSource::Locomo,
                "conv-30",
                Some(40),
                ScenarioKind::SingleHopControl,
                3,
                true,
                Some(true),
                None,
                true,
            ),
            (
                "benchmark-locomo-control-conv-47-qa69",
                BenchmarkSource::Locomo,
                "conv-47",
                Some(69),
                ScenarioKind::SingleHopControl,
                3,
                true,
                Some(true),
                None,
                true,
            ),
            (
                "benchmark-locomo-abstention-conv-26-qa153",
                BenchmarkSource::Locomo,
                "conv-26",
                Some(153),
                ScenarioKind::Abstention,
                3,
                true,
                Some(true),
                None,
                true,
            ),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn conversion_is_byte_deterministic() {
        let manifest = test_manifest(ScenarioKind::Update, false);
        let source = cmem_eval_longmemeval::load_value(json!([{
            "question_id": "lme",
            "question_type": "knowledge-update",
            "question": "Current?",
            "haystack_session_ids": ["old", "new", "background"],
            "haystack_dates": [
                "2024/01/01 (Mon) 00:00",
                "2024/01/02 (Tue) 00:00",
                "2024/01/03 (Wed) 00:00"
            ],
            "haystack_sessions": [
                [{"role":"user","content":"old","has_answer":true}],
                [{"role":"user","content":"new","has_answer":true}],
                [{"role":"user","content":"background"}]
            ]
        }]))
        .unwrap();
        let first = convert_loaded_datasets(&manifest, &source, &[]).unwrap();
        let second = convert_loaded_datasets(&manifest, &source, &[]).unwrap();
        assert_eq!(
            canonical_fixture_bytes(&first.fixtures).unwrap(),
            canonical_fixture_bytes(&second.fixtures).unwrap()
        );
        assert_eq!(
            canonical_embedding_manifest_bytes(&first.embedding_manifest).unwrap(),
            canonical_embedding_manifest_bytes(&second.embedding_manifest).unwrap()
        );
    }

    fn test_manifest(kind: ScenarioKind, gold_empty: bool) -> SelectionManifest {
        SelectionManifest {
            schema_version: SELECTION_MANIFEST_SCHEMA_VERSION,
            instances: vec![InstanceSelection {
                fixture_id: "fixture".to_string(),
                source: BenchmarkSource::LongmemevalS,
                source_instance_id: "lme".to_string(),
                source_qa_index: None,
                scenario_kind: kind,
                expected_question_type: "knowledge-update".to_string(),
                selected_session_ids: vec![
                    "old".to_string(),
                    "new".to_string(),
                    "background".to_string(),
                ],
                sampled_negative_turn_ids: vec![
                    "old:turn:1".to_string(),
                    "background:turn:1".to_string(),
                ],
                similarity_nearer_external_id: if kind == ScenarioKind::Abstention {
                    "old:turn:1".to_string()
                } else {
                    "new:turn:1".to_string()
                },
                similarity_farther_external_id: "background:turn:1".to_string(),
                selection_proof: SelectionProof {
                    machine_derived: MachineDerivedPredicates {
                        session_count: 3,
                        evidence_clean: true,
                        no_img_url_in_evidence: None,
                        gold_turn_ids_empty: Some(gold_empty),
                    },
                    curator_asserted: CuratorAssertions {
                        self_contained: true,
                    },
                },
            }],
        }
    }
}
