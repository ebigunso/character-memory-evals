use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInput {
    pub external_id: String,
    pub namespace: String,
    pub summary: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub participants: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationInput {
    pub external_id: String,
    pub episode_external_id: String,
    pub namespace: String,
    pub speaker: Option<String>,
    pub text: String,
    pub observed_at: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphEnrichmentInput {
    pub namespace: String,
    #[serde(default)]
    pub entities: Vec<EntityInput>,
    #[serde(default)]
    pub threads: Vec<MemoryThreadInput>,
    #[serde(default)]
    pub derived_memories: Vec<DerivedMemoryInput>,
    #[serde(default)]
    pub links: Vec<MemoryLinkInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInput {
    pub external_id: String,
    pub entity_type: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub canonical_key: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryThreadInput {
    pub external_id: String,
    pub title: String,
    pub summary: String,
    #[serde(default = "default_thread_status")]
    pub status: String,
    pub last_touched_at: Option<String>,
    #[serde(default = "default_salience_score")]
    pub salience_score: f32,
    pub canonical_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedMemoryInput {
    pub external_id: String,
    pub derived_type: String,
    pub text: String,
    #[serde(default)]
    pub source_episode_external_ids: Vec<String>,
    #[serde(default)]
    pub source_observation_external_ids: Vec<String>,
    #[serde(default)]
    pub thread_external_ids: Vec<String>,
    #[serde(default)]
    pub entity_external_ids: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default = "default_salience_score")]
    pub salience_score: f32,
    #[serde(default = "default_stability")]
    pub stability: String,
    #[serde(default = "default_true")]
    pub is_current: bool,
    #[serde(default)]
    pub supersedes_external_ids: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLinkInput {
    pub external_id: String,
    pub from: MemoryEndpointInput,
    pub relation: String,
    pub to: MemoryEndpointInput,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEndpointInput {
    pub object_type: String,
    pub external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveInput {
    pub namespace: String,
    pub query: String,
    pub query_date: Option<String>,
    pub top_k_episodes: usize,
    pub top_k_observations: usize,
    pub include_derived_memories: bool,
    pub include_threads: bool,
    pub include_entities: bool,
    pub include_debug_rationale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievedItem {
    pub kind: String,
    pub internal_id: String,
    pub external_id: Option<String>,
    pub episode_external_id: Option<String>,
    pub score: Option<f64>,
    pub rank: usize,
    pub rationale: Vec<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RetrievedContextPack {
    pub items: Vec<RetrievedItem>,
    pub context_text: String,
    pub context_char_count: usize,
    pub context_word_count: usize,
    #[serde(default)]
    pub telemetry: RetrievalTelemetry,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RetrievalTelemetry {
    #[serde(default)]
    pub trace_available: bool,
    #[serde(default)]
    pub vector_candidate_count: Option<usize>,
    #[serde(default)]
    pub graph_relation_count: Option<usize>,
    #[serde(default)]
    pub graph_verified_count: Option<usize>,
    #[serde(default)]
    pub stale_candidate_omission_count: Option<usize>,
    #[serde(default)]
    pub lifecycle_omission_count: Option<usize>,
    #[serde(default)]
    pub lifecycle_filter_decision_count: Option<usize>,
    #[serde(default)]
    pub suppressed_or_deleted_returned_count: Option<usize>,
    #[serde(default)]
    pub superseded_current_returned_count: Option<usize>,
    #[serde(default)]
    pub graph_object_missing_omitted_count: Option<usize>,
    #[serde(default)]
    pub graph_object_missing_returned_count: Option<usize>,
    #[serde(default)]
    pub section_assignment_count: Option<usize>,
    #[serde(default)]
    pub section_assignment_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub stale_candidate_omission_reasons: BTreeMap<String, usize>,
    #[serde(default)]
    pub lifecycle_omission_reasons: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IngestedObjectRefs {
    pub episode_internal_ids: Vec<String>,
    pub observation_internal_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievedExternalRef {
    pub kind: String,
    pub external_id: Option<String>,
    pub episode_external_id: Option<String>,
    pub rank: usize,
}

#[async_trait]
pub trait MemoryAdapter: Send + Sync {
    async fn reset_namespace(&self, namespace: &str) -> Result<()>;
    async fn remember_episode(&self, input: EpisodeInput) -> Result<String>;
    async fn remember_observation(&self, input: ObservationInput) -> Result<String>;
    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()>;
    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack>;
}

#[derive(Debug, Clone, Default)]
pub struct MockMemoryAdapter {
    state: Arc<Mutex<HashMap<String, NamespaceState>>>,
}

#[derive(Debug, Clone, Default)]
struct NamespaceState {
    episodes: Vec<EpisodeInput>,
    observations: Vec<ObservationInput>,
    derived_memories: Vec<DerivedMemoryInput>,
}

#[async_trait]
impl MemoryAdapter for MockMemoryAdapter {
    async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state.remove(namespace);
        Ok(())
    }

    async fn remember_episode(&self, input: EpisodeInput) -> Result<String> {
        let internal_id = format!("mock:episode:{}", input.external_id);
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state
            .entry(input.namespace.clone())
            .or_default()
            .episodes
            .push(input);
        Ok(internal_id)
    }

    async fn remember_observation(&self, input: ObservationInput) -> Result<String> {
        let internal_id = format!("mock:observation:{}", input.external_id);
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state
            .entry(input.namespace.clone())
            .or_default()
            .observations
            .push(input);
        Ok(internal_id)
    }

    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()> {
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace).or_default();
        namespace.derived_memories.extend(input.derived_memories);
        Ok(())
    }

    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        let state = self.state.lock().expect("mock memory mutex poisoned");
        let Some(ns) = state.get(&input.namespace) else {
            return Ok(RetrievedContextPack::default());
        };

        let mut items = Vec::new();
        let mut episodes = ns.episodes.clone();
        episodes.sort_by(|a, b| {
            score_text(&input.query, &b.summary)
                .partial_cmp(&score_text(&input.query, &a.summary))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for episode in episodes.into_iter().take(input.top_k_episodes) {
            let score = score_text(&input.query, &episode.summary);
            items.push(RetrievedItem {
                kind: "episode".to_string(),
                internal_id: format!("mock:episode:{}", episode.external_id),
                external_id: Some(episode.external_id),
                episode_external_id: None,
                score: Some(score),
                rank: 0,
                rationale: vec!["mock_lexical_overlap".to_string()],
                text: Some(episode.summary),
            });
        }

        let mut observations = ns.observations.clone();
        observations.sort_by(|a, b| {
            score_text(&input.query, &b.text)
                .partial_cmp(&score_text(&input.query, &a.text))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for observation in observations.into_iter().take(input.top_k_observations) {
            let score = score_text(&input.query, &observation.text);
            items.push(RetrievedItem {
                kind: "observation".to_string(),
                internal_id: format!("mock:observation:{}", observation.external_id),
                external_id: Some(observation.external_id),
                episode_external_id: Some(observation.episode_external_id),
                score: Some(score),
                rank: 0,
                rationale: vec!["mock_lexical_overlap".to_string()],
                text: Some(observation.text),
            });
        }

        if input.include_derived_memories {
            let mut derived_memories = ns.derived_memories.clone();
            derived_memories.sort_by(|a, b| {
                score_text(&input.query, &b.text)
                    .partial_cmp(&score_text(&input.query, &a.text))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for memory in derived_memories.into_iter().take(input.top_k_observations) {
                let score = score_text(&input.query, &memory.text);
                items.push(RetrievedItem {
                    kind: "derived_memory".to_string(),
                    internal_id: format!("mock:derived_memory:{}", memory.external_id),
                    external_id: Some(memory.external_id),
                    episode_external_id: memory.source_episode_external_ids.first().cloned(),
                    score: Some(score),
                    rank: 0,
                    rationale: vec!["mock_lexical_overlap".to_string()],
                    text: Some(memory.text),
                });
            }
        }

        items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.internal_id.cmp(&b.internal_id))
        });
        for (idx, item) in items.iter_mut().enumerate() {
            item.rank = idx + 1;
        }

        let context_text = items
            .iter()
            .filter_map(|item| item.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");
        let context_char_count = context_text.chars().count();
        let context_word_count = context_text.split_whitespace().count();

        Ok(RetrievedContextPack {
            items,
            context_text,
            context_char_count,
            context_word_count,
            telemetry: RetrievalTelemetry::default(),
        })
    }
}

fn default_thread_status() -> String {
    "active".to_string()
}

fn default_stability() -> String {
    "medium".to_string()
}

fn default_confidence() -> f32 {
    1.0
}

fn default_salience_score() -> f32 {
    0.5
}

fn default_true() -> bool {
    true
}

fn score_text(query: &str, text: &str) -> f64 {
    let query_words = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<std::collections::BTreeSet<_>>();
    if query_words.is_empty() {
        return 0.0;
    }
    let text_words = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<std::collections::BTreeSet<_>>();
    let hits = query_words
        .iter()
        .filter(|word| text_words.contains(*word))
        .count();
    hits as f64 / query_words.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_adapter_preserves_external_ids() {
        let adapter = MockMemoryAdapter::default();
        adapter.reset_namespace("n").await.unwrap();
        adapter
            .remember_episode(EpisodeInput {
                external_id: "s1".into(),
                namespace: "n".into(),
                summary: "Conversation about chat native design".into(),
                started_at: None,
                ended_at: None,
                participants: vec!["user".into()],
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();
        adapter
            .remember_observation(ObservationInput {
                external_id: "s1:turn:1".into(),
                episode_external_id: "s1".into(),
                namespace: "n".into(),
                speaker: Some("user".into()),
                text: "Keep the first version chat native".into(),
                observed_at: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();

        let pack = adapter
            .retrieve(RetrieveInput {
                namespace: "n".into(),
                query: "chat native first version".into(),
                query_date: None,
                top_k_episodes: 5,
                top_k_observations: 5,
                include_derived_memories: false,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == "observation" && item.external_id.as_deref() == Some("s1:turn:1")
        }));
    }

    #[tokio::test]
    async fn mock_adapter_retrieves_derived_memories_when_enabled() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![DerivedMemoryInput {
                    external_id: "dm1".into(),
                    derived_type: "reflection".into(),
                    text: "The user prefers chat native design.".into(),
                    source_episode_external_ids: vec!["s1".into()],
                    source_observation_external_ids: vec![],
                    thread_external_ids: vec![],
                    entity_external_ids: vec![],
                    confidence: 1.0,
                    salience_score: 0.5,
                    stability: "medium".into(),
                    is_current: true,
                    supersedes_external_ids: vec![],
                    metadata: serde_json::json!({}),
                }],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();

        let pack = adapter
            .retrieve(RetrieveInput {
                namespace: "n".into(),
                query: "chat native".into(),
                query_date: None,
                top_k_episodes: 5,
                top_k_observations: 5,
                include_derived_memories: true,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == "derived_memory" && item.external_id.as_deref() == Some("dm1")
        }));
    }
}
