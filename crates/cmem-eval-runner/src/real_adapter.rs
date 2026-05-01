use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use character_memory::{
    CharacterMemory, ContinuitySectionLimits, EmbeddingProvider, EpisodeDraft, MemoryId,
    MemoryObjectDraft, ObjectType, ObservationDraft, RememberDraft, RetrievalCandidateLimits,
    RetrievalContext, Settings,
};
use chrono::{DateTime, Utc};
use cmem_eval_core::{
    BenchmarkRunConfig, EpisodeInput, MemoryAdapter, ObservationInput, RetrieveInput,
    RetrievedContextPack, RetrievedItem,
};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

const UUID_NAMESPACE: Uuid = Uuid::from_u128(0x9b6af7a4_9076_49bb_9231_84d1ed632cf1);

pub struct CharacterMemoryAdapter {
    config: BenchmarkRunConfig,
    namespaces: Arc<Mutex<HashMap<String, NamespaceState>>>,
}

struct NamespaceState {
    memory: CharacterMemory,
    episode_ids: HashMap<String, MemoryId>,
    observation_ids: HashMap<String, MemoryId>,
    reverse_episode_ids: HashMap<MemoryId, String>,
    reverse_observation_ids: HashMap<MemoryId, (String, String)>,
}

impl CharacterMemoryAdapter {
    pub async fn new(config: &BenchmarkRunConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            namespaces: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn create_namespace_state(&self, namespace: &str) -> Result<NamespaceState> {
        let collection_name = self.collection_name(namespace);
        let settings = self.settings()?;
        let memory = if self.config.backend.embedding.provider == "deterministic" {
            let vector_size = self.config.backend.embedding.vector_size.unwrap_or(3072);
            CharacterMemory::new_with_embedding_provider(
                settings,
                collection_name.clone(),
                Box::new(DeterministicEmbeddingProvider { vector_size }),
            )
            .await?
        } else {
            CharacterMemory::new(settings, collection_name.clone()).await?
        };

        Ok(NamespaceState {
            memory,
            episode_ids: HashMap::new(),
            observation_ids: HashMap::new(),
            reverse_episode_ids: HashMap::new(),
            reverse_observation_ids: HashMap::new(),
        })
    }

    fn settings(&self) -> Result<Settings> {
        let qdrant = self
            .config
            .backend
            .qdrant_connection_string
            .clone()
            .or_else(|| env::var("QDRANT_CONNECTION_STRING").ok())
            .context("QDRANT_CONNECTION_STRING is required for live Character Memory runs")?;
        let oxigraph = self
            .config
            .backend
            .oxigraph_connection_string
            .clone()
            .or_else(|| env::var("OXIGRAPH_CONNECTION_STRING").ok())
            .unwrap_or_else(|| "memory://in-memory".to_string());
        let openai_api_key = env::var(&self.config.backend.openai_api_key_env)
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .unwrap_or_else(|_| {
                if self.config.backend.embedding.provider == "deterministic" {
                    "deterministic-unused".to_string()
                } else {
                    String::new()
                }
            });
        if openai_api_key.is_empty() {
            bail!(
                "{} is required for OpenAI live embeddings",
                self.config.backend.openai_api_key_env
            );
        }

        let external_config = config::Config::builder()
            .set_override("qdrant_connection_string", qdrant)?
            .set_override("oxigraph_connection_string", oxigraph)?
            .set_override("openai_api_key", openai_api_key)?
            .set_override(
                "embedding_model",
                self.config.backend.embedding.model.clone(),
            )?
            .build()?;

        Settings::new(external_config).map_err(Into::into)
    }

    fn collection_name(&self, namespace: &str) -> String {
        let prefix = self
            .config
            .backend
            .namespace_prefix
            .as_deref()
            .unwrap_or("cmem_eval");
        format!(
            "{}_{}_{}",
            sanitize_collection_segment(prefix),
            sanitize_collection_segment(namespace),
            Uuid::new_v4().simple()
        )
    }
}

#[async_trait]
impl MemoryAdapter for CharacterMemoryAdapter {
    async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let mut namespaces = self.namespaces.lock().await;
        namespaces.remove(namespace);
        Ok(())
    }

    async fn remember_episode(&self, input: EpisodeInput) -> Result<String> {
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&input.namespace) {
            let state = self.create_namespace_state(&input.namespace).await?;
            namespaces.insert(input.namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&input.namespace)
            .expect("namespace state inserted");
        let id = deterministic_id(&input.namespace, "episode", &input.external_id);
        let mut draft = EpisodeDraft::new(input.summary);
        draft.id = Some(id);
        draft.source_conversation_id = Some(input.external_id.clone());
        draft.raw_ref = Some(format!(
            "eval://{}/episode/{}",
            input.namespace, input.external_id
        ));
        draft.started_at = parse_timestamp(input.started_at.as_deref())?;
        draft.ended_at = parse_timestamp(input.ended_at.as_deref())?;

        state
            .memory
            .remember(RememberDraft::new([MemoryObjectDraft::Episode(draft)]))
            .await?;
        state.episode_ids.insert(input.external_id.clone(), id);
        state
            .reverse_episode_ids
            .insert(id, input.external_id.clone());

        Ok(id.to_string())
    }

    async fn remember_observation(&self, input: ObservationInput) -> Result<String> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered episodes: {}", input.namespace))?;
        let episode_id = *state
            .episode_ids
            .get(&input.episode_external_id)
            .ok_or_else(|| {
                anyhow!(
                    "observation {} references unknown episode external_id {}",
                    input.external_id,
                    input.episode_external_id
                )
            })?;
        let id = deterministic_id(&input.namespace, "observation", &input.external_id);
        let mut draft = ObservationDraft::new(episode_id, input.text);
        draft.id = Some(id);
        draft.raw_ref = Some(format!(
            "eval://{}/observation/{}",
            input.namespace, input.external_id
        ));
        draft.observed_at = parse_timestamp(input.observed_at.as_deref())?;

        state
            .memory
            .remember(RememberDraft::new([MemoryObjectDraft::Observation(draft)]))
            .await?;
        state.observation_ids.insert(input.external_id.clone(), id);
        state.reverse_observation_ids.insert(
            id,
            (input.external_id.clone(), input.episode_external_id.clone()),
        );

        Ok(id.to_string())
    }

    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&input.namespace) {
            let state = self.create_namespace_state(&input.namespace).await?;
            namespaces.insert(input.namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&input.namespace)
            .expect("namespace state inserted");

        let mut context = RetrievalContext::new(input.query);
        context.include_trace = true;
        context.candidate_limits = RetrievalCandidateLimits {
            max_vector_candidates: input.top_k_episodes + input.top_k_observations + 16,
            max_graph_roots: (input.top_k_episodes + input.top_k_observations).max(1),
        };
        context.section_limits = ContinuitySectionLimits {
            relevant_episodes: input.top_k_episodes,
            salient_observations: input.top_k_observations,
            derived_memories: if input.include_derived_memories {
                12
            } else {
                0
            },
            preferences: if input.include_derived_memories { 8 } else { 0 },
            relationship_notes: if input.include_derived_memories { 8 } else { 0 },
            open_loops: if input.include_derived_memories { 8 } else { 0 },
            commitments: if input.include_derived_memories { 8 } else { 0 },
            character_signals: if input.include_derived_memories { 8 } else { 0 },
            ..ContinuitySectionLimits::default()
        };
        context.object_type_defaults = vec![ObjectType::Episode, ObjectType::Observation];
        if input.include_derived_memories {
            context.object_type_defaults.push(ObjectType::DerivedMemory);
        }

        let outcome = state.memory.retrieve(context).await?;
        Ok(flatten_outcome(state, outcome))
    }
}

fn flatten_outcome(
    state: &NamespaceState,
    outcome: character_memory::RetrieveOutcome,
) -> RetrievedContextPack {
    let mut trace_by_id: HashMap<MemoryId, (Option<f64>, usize)> = HashMap::new();
    if let Some(trace) = &outcome.trace {
        for candidate in &trace.vector_candidates {
            trace_by_id.insert(
                candidate.object.id,
                (Some(candidate.score as f64), candidate.rank),
            );
        }
    }

    let mut items = Vec::new();
    for episode in outcome.pack.relevant_episodes {
        let external_id = state
            .reverse_episode_ids
            .get(&episode.id)
            .cloned()
            .or(episode.source_conversation_id.clone());
        let (score, rank) = trace_by_id
            .get(&episode.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "episode".to_string(),
            internal_id: episode.id.to_string(),
            external_id,
            episode_external_id: None,
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(episode.summary),
        });
    }

    for observation in outcome.pack.salient_observations {
        let mapped = state.reverse_observation_ids.get(&observation.id).cloned();
        let (external_id, episode_external_id) = mapped
            .map(|(obs_id, ep_id)| (Some(obs_id), Some(ep_id)))
            .unwrap_or_else(|| {
                (
                    observation.raw_ref.clone(),
                    state
                        .reverse_episode_ids
                        .get(&observation.episode_id)
                        .cloned(),
                )
            });
        let (score, rank) = trace_by_id
            .get(&observation.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "observation".to_string(),
            internal_id: observation.id.to_string(),
            external_id,
            episode_external_id,
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(observation.text),
        });
    }

    items.sort_by_key(|item| item.rank);
    let context_text = render_context_text(&items);
    let context_char_count = context_text.chars().count();
    let context_word_count = context_text.split_whitespace().count();

    RetrievedContextPack {
        items,
        context_text,
        context_char_count,
        context_word_count,
    }
}

fn render_context_text(items: &[RetrievedItem]) -> String {
    items
        .iter()
        .filter_map(|item| {
            item.text.as_ref().map(|text| {
                format!(
                    "[{}:{} rank={}] {}",
                    item.kind,
                    item.external_id.as_deref().unwrap_or("unknown"),
                    item.rank,
                    text
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn deterministic_id(namespace: &str, kind: &str, external_id: &str) -> MemoryId {
    Uuid::new_v5(
        &UUID_NAMESPACE,
        format!("{namespace}\0{kind}\0{external_id}").as_bytes(),
    )
}

fn parse_timestamp(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .with_context(|| format!("parse RFC3339 timestamp {value}"))
        })
        .transpose()
}

fn sanitize_collection_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

struct DeterministicEmbeddingProvider {
    vector_size: usize,
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddingProvider {
    async fn generate_embedding<'a>(
        &self,
        text: &'a str,
    ) -> std::result::Result<Vec<f32>, character_memory::CustomError> {
        Ok(self.vector_for_text(text))
    }

    async fn bulk_generate_embeddings<'a>(
        &self,
        texts: &'a [&'a str],
    ) -> std::result::Result<Vec<Vec<f32>>, character_memory::CustomError> {
        Ok(texts
            .iter()
            .map(|text| self.vector_for_text(text))
            .collect())
    }
}

impl DeterministicEmbeddingProvider {
    fn vector_for_text(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; self.vector_size];
        for token in text.split(|ch: char| !ch.is_alphanumeric()) {
            if token.is_empty() {
                continue;
            }
            let idx = stable_hash(token) % self.vector_size;
            embedding[idx] += 1.0;
        }
        if embedding.iter().all(|value| *value == 0.0) {
            embedding[0] = 1.0;
        }
        embedding
    }
}

fn stable_hash(text: &str) -> usize {
    text.bytes().fold(2166136261usize, |hash, byte| {
        hash.wrapping_mul(16777619) ^ usize::from(byte.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_ids_are_stable_and_namespaced() {
        let first = deterministic_id("n1", "episode", "s1");
        assert_eq!(first, deterministic_id("n1", "episode", "s1"));
        assert_ne!(first, deterministic_id("n2", "episode", "s1"));
        assert_ne!(first, deterministic_id("n1", "observation", "s1"));
    }

    #[test]
    fn renders_context_text_with_external_ids() {
        let text = render_context_text(&[RetrievedItem {
            kind: "observation".to_string(),
            internal_id: "i".to_string(),
            external_id: Some("s1:turn:1".to_string()),
            episode_external_id: Some("s1".to_string()),
            score: Some(0.5),
            rank: 1,
            rationale: vec![],
            text: Some("hello".to_string()),
        }]);
        assert!(text.contains("observation:s1:turn:1"));
        assert!(text.contains("hello"));
    }
}
