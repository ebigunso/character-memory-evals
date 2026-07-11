use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use character_memory::{
    ArchivePolicy, CandidateProvenance, CandidateValidation, CandidateValidationStatus,
    CharacterMemory, CommitOptions, ContinuitySectionLimits, CorrectMemoryDraft,
    CorrectionCascadePolicy, CorrectionLifecyclePolicy, CorrectionTarget, DerivedMemoryDraft,
    DerivedType, EmbeddingProvider, EntityDraft, EntityType, EpisodeDraft, ExternalSourceReference,
    ForgetCascadePolicy, ForgetLifecyclePolicy, ForgetMemoryDraft, LifecycleFilterAction,
    LifecycleFilterReason, LifecycleMutationOutcome, LifecycleTargetRef, MemoryCandidate, MemoryId,
    MemoryLinkDraft, MemoryObjectDraft, MemoryThreadDraft, ObjectType, ObservationDraft,
    PrepareOptions, RelationType, RememberDraft, RememberInput, RememberWritePlan,
    ReplacementDerivedMemoryDraft, RetentionState, RetrievalContext, Settings,
    SourceObjectCorrectionTarget, SourceProvenance, SourceProvenanceReference, Stability,
    SuppressionPolicy, ThreadStatus,
};
use chrono::{DateTime, Utc};
pub use cmem_eval_core::DeterministicEmbeddingProvider;
use cmem_eval_core::{
    BenchmarkRunConfig, CandidateValidationResult, CommitWriteOptions, CommitWriteResult,
    CorrectMemoryInput, CorrectionTargetInput, EpisodeInput, ExternalSourceRefInput,
    ForgetMemoryInput, GraphEnrichmentInput, LifecycleMutationResult, LinkMemoryInput,
    LinkMemoryResult, MemoryAdapter, MemoryEndpointInput, NamespaceLifecycleResult,
    ObservationInput, PrepareWriteInput, PreparedCandidate, PreparedWritePlan,
    ReplacementDerivedMemoryInput, RetrievalMode, RetrievalTelemetry, RetrieveInput,
    RetrievedContextPack, RetrievedItem, SourceProvenanceInput, SupersessionResult,
};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{Condition, Filter, ScoredPoint, SearchPointsBuilder, value::Kind};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

const UUID_NAMESPACE: Uuid = Uuid::from_u128(0x9b6af7a4_9076_49bb_9231_84d1ed632cf1);
const QDRANT_OBJECT_ID_FIELD: &str = "object_id";
const QDRANT_OBJECT_TYPE_FIELD: &str = "object_type";
const QDRANT_CONTENT_TEXT_FIELD: &str = "content_text";

pub struct CharacterMemoryAdapter {
    config: BenchmarkRunConfig,
    qdrant: Qdrant,
    openai_http: reqwest::Client,
    namespaces: Arc<Mutex<HashMap<String, NamespaceState>>>,
}

struct NamespaceState {
    memory: CharacterMemory,
    collection_name: String,
    identity_registry_path: PathBuf,
    identities: ExternalIdRegistry,
}

impl Deref for NamespaceState {
    type Target = ExternalIdRegistry;

    fn deref(&self) -> &Self::Target {
        &self.identities
    }
}

impl DerefMut for NamespaceState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.identities
    }
}

impl NamespaceState {
    fn persist_identities(&self) -> Result<()> {
        self.identities.save(&self.identity_registry_path)
    }
}

/// Durable external-id mapping for a single evaluation namespace.
///
/// Callers assign deterministic `MemoryId`s before a write, register them only
/// after the store accepts the write, and persist this registry in the run
/// directory. Reattaching loads this file as the primary identity source;
/// retrieval from the backing stores may verify it but never reconstructs it.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct ExternalIdRegistry {
    namespace: String,
    episode_ids: BTreeMap<String, MemoryId>,
    observation_ids: BTreeMap<String, MemoryId>,
    entity_ids: BTreeMap<String, MemoryId>,
    reverse_entity_ids: BTreeMap<MemoryId, String>,
    thread_ids: BTreeMap<String, MemoryId>,
    derived_memory_ids: BTreeMap<String, MemoryId>,
    reverse_episode_ids: BTreeMap<MemoryId, String>,
    reverse_observation_ids: BTreeMap<MemoryId, (String, String)>,
    reverse_thread_ids: BTreeMap<MemoryId, String>,
    reverse_derived_memory_ids: BTreeMap<MemoryId, String>,
    link_ids: BTreeMap<String, MemoryId>,
    reverse_link_ids: BTreeMap<MemoryId, String>,
}

impl ExternalIdRegistry {
    fn new(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            ..Self::default()
        }
    }

    fn len(&self) -> usize {
        self.episode_ids.len()
            + self.observation_ids.len()
            + self.entity_ids.len()
            + self.thread_ids.len()
            + self.derived_memory_ids.len()
            + self.link_ids.len()
    }

    fn load(path: &Path, expected_namespace: &str) -> Result<Self> {
        let bytes =
            fs::read(path).with_context(|| format!("read identity registry {}", path.display()))?;
        let registry: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("deserialize identity registry {}", path.display()))?;
        if registry.namespace != expected_namespace {
            bail!(
                "identity registry namespace mismatch: expected {expected_namespace}, found {}",
                registry.namespace
            );
        }
        Ok(registry)
    }

    fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("create identity registry directory {}", parent.display())
            })?;
        }
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        fs::write(path, bytes)
            .with_context(|| format!("write identity registry {}", path.display()))
    }
}

#[derive(Clone)]
struct VectorNamespaceSnapshot {
    collection_name: String,
    reverse_episode_ids: BTreeMap<MemoryId, String>,
    reverse_observation_ids: BTreeMap<MemoryId, (String, String)>,
}

#[derive(Debug, Clone)]
struct VectorHit {
    kind: &'static str,
    object_id: MemoryId,
    score: f64,
    text: Option<String>,
}

impl CharacterMemoryAdapter {
    pub async fn new(config: &BenchmarkRunConfig) -> Result<Self> {
        let qdrant = Qdrant::from_url(
            &config
                .backend
                .qdrant_connection_string
                .clone()
                .or_else(|| env::var("QDRANT_CONNECTION_STRING").ok())
                .context("QDRANT_CONNECTION_STRING is required for live Character Memory runs")?,
        )
        .build()?;
        Ok(Self {
            config: config.clone(),
            qdrant,
            openai_http: reqwest::Client::new(),
            namespaces: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn create_namespace_state(&self, namespace: &str) -> Result<NamespaceState> {
        let collection_name = self.collection_name(namespace);
        let identity_registry_path = self.identity_registry_path(namespace);
        let identities = if identity_registry_path.exists() {
            ExternalIdRegistry::load(&identity_registry_path, namespace)?
        } else {
            ExternalIdRegistry::new(namespace)
        };
        let settings = self.settings()?;
        let memory = if self.config.backend.embedding.provider == "deterministic" {
            let vector_size = self.config.backend.embedding.vector_size.unwrap_or(3072);
            CharacterMemory::new_with_embedding_provider(
                settings,
                collection_name.clone(),
                Box::new(CharacterMemoryEmbeddingProvider::new(vector_size)?),
            )
            .await?
        } else {
            CharacterMemory::new(settings, collection_name.clone()).await?
        };

        Ok(NamespaceState {
            memory,
            collection_name,
            identity_registry_path,
            identities,
        })
    }

    fn settings(&self) -> Result<Settings> {
        let qdrant = self.qdrant_connection_string()?;
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

        let mut builder = config::Config::builder()
            .set_override("qdrant_connection_string", qdrant)?
            .set_override("oxigraph_connection_string", oxigraph)?
            .set_override("openai_api_key", openai_api_key)?
            .set_override(
                "embedding_model",
                self.config.backend.embedding.model.clone(),
            )?;
        if let Some(path) = &self.config.backend.oxigraph_persistence_path {
            builder = builder
                .set_override("graph_store_mode", "persistent")?
                .set_override("oxigraph_connection_string", path.clone())?;
        }
        if let Some(path) = &self.config.backend.retrieval_stats_path {
            builder = builder
                .set_override("retrieval_stats_store_mode", "sqlite")?
                .set_override("retrieval_stats_path", path.clone())?;
        }
        let external_config = builder.build()?;

        Settings::new(external_config).map_err(Into::into)
    }

    fn qdrant_connection_string(&self) -> Result<String> {
        self.config
            .backend
            .qdrant_connection_string
            .clone()
            .or_else(|| env::var("QDRANT_CONNECTION_STRING").ok())
            .context("QDRANT_CONNECTION_STRING is required for live Character Memory runs")
    }

    fn namespace_prefix(&self) -> &str {
        self.config
            .backend
            .namespace_prefix
            .as_deref()
            .unwrap_or("cmem_eval")
    }

    fn namespace_identity_suffix(&self, namespace: &str) -> Uuid {
        let identity = format!(
            "{}\0{}\0{namespace}",
            self.namespace_prefix(),
            self.config.run_id
        );
        Uuid::new_v5(&UUID_NAMESPACE, identity.as_bytes())
    }

    fn collection_name(&self, namespace: &str) -> String {
        let prefix = self.namespace_prefix();
        let suffix = self.namespace_identity_suffix(namespace);
        format!(
            "{}_{}_{}_{}",
            sanitize_collection_segment(prefix),
            sanitize_collection_segment(&self.config.run_id),
            sanitize_collection_segment(namespace),
            suffix.simple()
        )
    }

    /// Registry filenames share the collection's prefix/run/namespace identity. Legacy
    /// prefix-less registry files are ephemeral eval artifacts and are intentionally not migrated.
    fn identity_registry_path(&self, namespace: &str) -> PathBuf {
        let root = self
            .config
            .backend
            .identity_registry_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("runs").join(&self.config.run_id));
        let prefix = self.namespace_prefix();
        let suffix = self.namespace_identity_suffix(namespace);
        root.join(format!(
            "identity-{}-{}-{}.json",
            sanitize_collection_segment(prefix),
            sanitize_collection_segment(namespace),
            suffix.simple()
        ))
    }

    async fn delete_collection_with_prefix(
        &self,
        collection_name: &str,
        required_prefix: &str,
    ) -> Result<()> {
        validate_cleanup_target(collection_name, Some(required_prefix))?;
        if self
            .qdrant
            .collection_exists(collection_name)
            .await
            .with_context(|| format!("check Qdrant collection {collection_name}"))?
        {
            self.qdrant
                .delete_collection(collection_name)
                .await
                .with_context(|| format!("delete Qdrant collection {collection_name}"))?;
        }
        Ok(())
    }

    async fn reset_namespace_with_prefix(
        &self,
        namespace: &str,
        required_prefix: &str,
    ) -> Result<()> {
        let (collection_name, identity_registry_path) = {
            let namespaces = self.namespaces.lock().await;
            namespaces
                .get(namespace)
                .map(|state| {
                    (
                        state.collection_name.clone(),
                        state.identity_registry_path.clone(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        self.collection_name(namespace),
                        self.identity_registry_path(namespace),
                    )
                })
        };
        self.delete_collection_with_prefix(&collection_name, required_prefix)
            .await?;
        if identity_registry_path.exists() {
            fs::remove_file(&identity_registry_path).with_context(|| {
                format!(
                    "remove identity registry {}",
                    identity_registry_path.display()
                )
            })?;
        }
        let mut namespaces = self.namespaces.lock().await;
        namespaces.remove(namespace);
        Ok(())
    }

    async fn vector_namespace_snapshot(&self, namespace: &str) -> Result<VectorNamespaceSnapshot> {
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(namespace) {
            let state = self.create_namespace_state(namespace).await?;
            namespaces.insert(namespace.to_string(), state);
        }
        let state = namespaces.get(namespace).expect("namespace state inserted");
        Ok(VectorNamespaceSnapshot {
            collection_name: state.collection_name.clone(),
            reverse_episode_ids: state.reverse_episode_ids.clone(),
            reverse_observation_ids: state.reverse_observation_ids.clone(),
        })
    }

    async fn retrieve_vector_only(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        let snapshot = self.vector_namespace_snapshot(&input.namespace).await?;
        let query_embedding = self.query_embedding(&input.query).await?;

        let mut hits = Vec::new();
        hits.extend(
            self.search_vector_kind(
                &snapshot.collection_name,
                &query_embedding,
                "episode",
                input.top_k_episodes,
            )
            .await?,
        );
        hits.extend(
            self.search_vector_kind(
                &snapshot.collection_name,
                &query_embedding,
                "observation",
                input.top_k_observations,
            )
            .await?,
        );

        Ok(vector_hits_to_context_pack(&snapshot, hits))
    }

    async fn search_vector_kind(
        &self,
        collection_name: &str,
        query_embedding: &[f32],
        kind: &'static str,
        limit: usize,
    ) -> Result<Vec<VectorHit>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let request =
            SearchPointsBuilder::new(collection_name, query_embedding.to_vec(), limit as u64)
                .with_payload(true)
                .with_vectors(false)
                .filter(Filter::must([Condition::matches(
                    QDRANT_OBJECT_TYPE_FIELD,
                    kind.to_string(),
                )]))
                .build();
        let response =
            self.qdrant.search_points(request).await.with_context(|| {
                format!("vector_only Qdrant search {collection_name} kind={kind}")
            })?;
        response
            .result
            .into_iter()
            .map(|point| scored_point_to_vector_hit(point, kind))
            .collect()
    }

    async fn query_embedding(&self, query: &str) -> Result<Vec<f32>> {
        match self.config.backend.embedding.provider.as_str() {
            "deterministic" => {
                let vector_size = self.config.backend.embedding.vector_size.unwrap_or(3072);
                Ok(DeterministicEmbeddingProvider::new(vector_size)?.vector_for_text(query))
            }
            "openai" => self.openai_query_embedding(query).await,
            provider => bail!("unsupported vector_only embedding provider: {provider}"),
        }
    }

    async fn openai_query_embedding(&self, query: &str) -> Result<Vec<f32>> {
        let api_key = env::var(&self.config.backend.openai_api_key_env)
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .with_context(|| {
                format!(
                    "{} is required for vector_only OpenAI query embeddings",
                    self.config.backend.openai_api_key_env
                )
            })?;
        if api_key.trim().is_empty() {
            bail!(
                "{} is required for vector_only OpenAI query embeddings",
                self.config.backend.openai_api_key_env
            );
        }

        let response = self
            .openai_http
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(api_key)
            .json(&OpenAiEmbeddingRequest {
                model: self.config.backend.embedding.model.clone(),
                input: query.to_string(),
            })
            .send()
            .await
            .context("request OpenAI query embedding for vector_only retrieval")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("OpenAI query embedding request failed with {status}: {body}");
        }
        let body: OpenAiEmbeddingResponse = response
            .json()
            .await
            .context("parse OpenAI query embedding response")?;
        let embedding = body
            .data
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .context("OpenAI query embedding response did not contain an embedding")?;
        if let Some(expected) = self.config.backend.embedding.vector_size
            && embedding.len() != expected
        {
            bail!(
                "OpenAI query embedding length {} did not match configured vector_size {}",
                embedding.len(),
                expected
            );
        }
        Ok(embedding)
    }
}

#[derive(serde::Serialize)]
struct OpenAiEmbeddingRequest {
    model: String,
    input: String,
}

#[derive(serde::Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(serde::Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl MemoryAdapter for CharacterMemoryAdapter {
    async fn open_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
        let mut namespaces = self.namespaces.lock().await;
        if namespaces.contains_key(namespace) {
            bail!("namespace is already open: {namespace}");
        }
        let registry_path = self.identity_registry_path(namespace);
        if registry_path.exists() {
            bail!(
                "identity registry already exists for namespace {namespace}; use reattach_namespace"
            );
        }
        let collection_name = self.collection_name(namespace);
        if self
            .qdrant
            .collection_exists(&collection_name)
            .await
            .with_context(|| format!("check Qdrant collection {collection_name}"))?
        {
            bail!(
                "Qdrant collection already exists for namespace {namespace}; reset the namespace or use reattach_namespace"
            );
        }
        let state = self.create_namespace_state(namespace).await?;
        namespaces.insert(namespace.to_string(), state);
        Ok(NamespaceLifecycleResult {
            namespace: namespace.to_string(),
            restored_identity_count: 0,
        })
    }

    async fn reattach_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
        let mut namespaces = self.namespaces.lock().await;
        if namespaces.contains_key(namespace) {
            bail!("namespace is already open: {namespace}");
        }
        let registry_path = self.identity_registry_path(namespace);
        if !registry_path.exists() {
            bail!(
                "identity registry does not exist for namespace {namespace}; reattach requires both the registry and Qdrant collection"
            );
        }
        let collection_name = self.collection_name(namespace);
        if !self
            .qdrant
            .collection_exists(&collection_name)
            .await
            .with_context(|| format!("check Qdrant collection {collection_name} for reattach"))?
        {
            bail!(
                "Qdrant collection {collection_name} does not exist for namespace {namespace} while identity registry {} exists; reattach requires both durable stores",
                registry_path.display()
            );
        }
        let state = self.create_namespace_state(namespace).await?;
        let restored_identity_count = state.identities.len();
        namespaces.insert(namespace.to_string(), state);
        Ok(NamespaceLifecycleResult {
            namespace: namespace.to_string(),
            restored_identity_count,
        })
    }

    async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let namespace_prefix = self.namespace_prefix();
        self.reset_namespace_with_prefix(namespace, namespace_prefix)
            .await
    }

    async fn cleanup_namespace(&self, namespace: &str) -> Result<()> {
        if !self.config.backend.cleanup.enabled {
            return Ok(());
        }
        let cleanup_prefix = self
            .config
            .backend
            .cleanup
            .require_collection_prefix
            .as_deref()
            .context("post-run cleanup requires a collection prefix")?;
        self.reset_namespace_with_prefix(namespace, cleanup_prefix)
            .await
    }

    async fn remember_episode(&self, input: EpisodeInput) -> Result<String> {
        let mut ids = self.remember_episodes(vec![input]).await?;
        Ok(ids.remove(0))
    }

    async fn remember_episodes(&self, inputs: Vec<EpisodeInput>) -> Result<Vec<String>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let namespace = shared_input_namespace(
            "episode batch",
            inputs
                .first()
                .expect("non-empty inputs already checked")
                .namespace
                .as_str(),
            inputs.iter().map(|input| input.namespace.as_str()),
        )?;
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&namespace) {
            let state = self.create_namespace_state(&namespace).await?;
            namespaces.insert(namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&namespace)
            .expect("namespace state inserted");
        let mut objects = Vec::with_capacity(inputs.len());
        let mut ids = Vec::with_capacity(inputs.len());
        for input in inputs {
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
            objects.push(MemoryObjectDraft::Episode(draft));
            ids.push((input.external_id, id));
        }

        state.memory.remember(RememberDraft::new(objects)).await?;
        for (external_id, id) in &ids {
            state.episode_ids.insert(external_id.clone(), *id);
            state.reverse_episode_ids.insert(*id, external_id.clone());
        }
        state.persist_identities()?;

        Ok(ids.into_iter().map(|(_, id)| id.to_string()).collect())
    }

    async fn remember_observation(&self, input: ObservationInput) -> Result<String> {
        let mut ids = self.remember_observations(vec![input]).await?;
        Ok(ids.remove(0))
    }

    async fn remember_observations(&self, inputs: Vec<ObservationInput>) -> Result<Vec<String>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let namespace = shared_input_namespace(
            "observation batch",
            inputs
                .first()
                .expect("non-empty inputs already checked")
                .namespace
                .as_str(),
            inputs.iter().map(|input| input.namespace.as_str()),
        )?;
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered episodes: {namespace}"))?;
        let mut objects = Vec::with_capacity(inputs.len());
        let mut ids = Vec::with_capacity(inputs.len());
        for input in inputs {
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
            objects.push(MemoryObjectDraft::Observation(draft));
            ids.push((input.external_id, input.episode_external_id, id));
        }

        state.memory.remember(RememberDraft::new(objects)).await?;
        for (external_id, episode_external_id, id) in &ids {
            state.observation_ids.insert(external_id.clone(), *id);
            state
                .reverse_observation_ids
                .insert(*id, (external_id.clone(), episode_external_id.clone()));
        }
        state.persist_identities()?;

        Ok(ids.into_iter().map(|(_, _, id)| id.to_string()).collect())
    }

    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered episodes: {}", input.namespace))?;

        let mut objects = Vec::new();
        let mut links = Vec::new();
        let mut pending_entities = BTreeMap::new();
        let mut pending_threads = BTreeMap::new();
        let mut pending_derived = BTreeMap::new();
        let mut pending_links = BTreeMap::new();

        for entity in &input.entities {
            pending_entities.insert(
                entity.external_id.clone(),
                deterministic_id(&input.namespace, "entity", &entity.external_id),
            );
        }
        for thread in &input.threads {
            pending_threads.insert(
                thread.external_id.clone(),
                deterministic_id(&input.namespace, "memory_thread", &thread.external_id),
            );
        }
        for memory in &input.derived_memories {
            pending_derived.insert(
                memory.external_id.clone(),
                deterministic_id(&input.namespace, "derived_memory", &memory.external_id),
            );
        }

        for entity in input.entities {
            let id = pending_entities[&entity.external_id];
            let mut draft = EntityDraft::new(parse_entity_type(&entity.entity_type)?, entity.name);
            draft.id = Some(id);
            draft.aliases = entity.aliases;
            draft.canonical_key = entity.canonical_key;
            draft.summary = entity.summary;
            objects.push(MemoryObjectDraft::Entity(draft));
        }

        for thread in input.threads {
            let id = pending_threads[&thread.external_id];
            let mut draft = MemoryThreadDraft::new(thread.title, thread.summary);
            draft.id = Some(id);
            draft.status = parse_thread_status(&thread.status)?;
            draft.last_touched_at = parse_timestamp(thread.last_touched_at.as_deref())?;
            draft.salience_score = thread.salience_score;
            draft.canonical_key = thread.canonical_key;
            objects.push(MemoryObjectDraft::MemoryThread(draft));
        }

        for memory in input.derived_memories {
            let id = pending_derived[&memory.external_id];
            let mut draft =
                DerivedMemoryDraft::new(parse_derived_type(&memory.derived_type)?, memory.text);
            draft.id = Some(id);
            draft.derived_from_episode_ids = resolve_ids(
                "episode",
                &memory.source_episode_external_ids,
                &state.episode_ids,
                &BTreeMap::new(),
            )?;
            draft.derived_from_observation_ids = resolve_ids(
                "observation",
                &memory.source_observation_external_ids,
                &state.observation_ids,
                &BTreeMap::new(),
            )?;
            draft.thread_ids = resolve_ids(
                "memory_thread",
                &memory.thread_external_ids,
                &state.thread_ids,
                &pending_threads,
            )?;
            draft.entity_ids = resolve_ids(
                "entity",
                &memory.entity_external_ids,
                &state.entity_ids,
                &pending_entities,
            )?;
            draft.confidence = memory.confidence;
            draft.salience_score = memory.salience_score;
            draft.stability = parse_stability(&memory.stability)?;
            draft.is_current = memory.is_current;
            draft.supersedes = resolve_ids(
                "derived_memory",
                &memory.supersedes_external_ids,
                &state.derived_memory_ids,
                &pending_derived,
            )?;
            objects.push(MemoryObjectDraft::DerivedMemory(draft));
        }

        for link in input.links {
            let (from_type, from_id) = resolve_endpoint(
                &link.from,
                state,
                &pending_entities,
                &pending_threads,
                &pending_derived,
            )?;
            let (to_type, to_id) = resolve_endpoint(
                &link.to,
                state,
                &pending_entities,
                &pending_threads,
                &pending_derived,
            )?;
            let mut draft = MemoryLinkDraft::new(
                from_type,
                from_id,
                parse_relation_type(&link.relation)?,
                to_type,
                to_id,
            );
            let id = deterministic_id(&input.namespace, "memory_link", &link.external_id);
            draft.id = Some(id);
            draft.confidence = link.confidence;
            draft.rationale = link.rationale;
            pending_links.insert(link.external_id, id);
            links.push(draft);
        }

        if objects.is_empty() && links.is_empty() {
            return Ok(());
        }

        state
            .memory
            .remember(RememberDraft::new(objects).with_links(links))
            .await?;
        for (external_id, id) in pending_entities {
            state.entity_ids.insert(external_id.clone(), id);
            state.reverse_entity_ids.insert(id, external_id);
        }
        for (external_id, id) in pending_threads {
            state.thread_ids.insert(external_id.clone(), id);
            state.reverse_thread_ids.insert(id, external_id);
        }
        for (external_id, id) in pending_derived {
            state.derived_memory_ids.insert(external_id.clone(), id);
            state.reverse_derived_memory_ids.insert(id, external_id);
        }
        for (external_id, id) in pending_links {
            state.link_ids.insert(external_id.clone(), id);
            state.reverse_link_ids.insert(id, external_id);
        }
        state.persist_identities()?;
        Ok(())
    }

    async fn link(&self, input: LinkMemoryInput) -> Result<LinkMemoryResult> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered objects: {}", input.namespace))?;
        let (from_type, from_id) = resolve_endpoint(
            &input.link.from,
            state,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )?;
        let (to_type, to_id) = resolve_endpoint(
            &input.link.to,
            state,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )?;
        let mut draft = MemoryLinkDraft::new(
            from_type,
            from_id,
            parse_relation_type(&input.link.relation)?,
            to_type,
            to_id,
        );
        let id = deterministic_id(&input.namespace, "memory_link", &input.link.external_id);
        draft.id = Some(id);
        draft.confidence = input.link.confidence;
        draft.rationale = input.link.rationale;
        let link = state.memory.link(draft).await?;
        state
            .link_ids
            .insert(input.link.external_id.clone(), link.id);
        state
            .reverse_link_ids
            .insert(link.id, input.link.external_id.clone());
        state.persist_identities()?;
        Ok(LinkMemoryResult {
            internal_id: link.id.to_string(),
            external_id: input.link.external_id,
        })
    }

    async fn correct(&self, input: CorrectMemoryInput) -> Result<LifecycleMutationResult> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered objects: {}", input.namespace))?;
        let targets = input
            .targets
            .iter()
            .map(|target| correction_target_to_live(target, state))
            .collect::<Result<Vec<_>>>()?;
        let correction_origin =
            source_provenance_reference_to_live(&input.correction_origin, state)?;
        let superseded_derived_memory_ids = resolve_ids(
            "derived_memory",
            &input.superseded_derived_memory_external_ids,
            &state.derived_memory_ids,
            &BTreeMap::new(),
        )?;
        let mut pending_replacements = Vec::new();
        let mut replacements = Vec::new();
        for replacement in &input.replacements {
            let id = deterministic_id(
                &input.namespace,
                "derived_memory",
                &replacement.memory.external_id,
            );
            replacements.push(replacement_to_live(replacement, id, state)?);
            pending_replacements.push((replacement.memory.external_id.clone(), id));
        }
        let draft = CorrectMemoryDraft {
            targets,
            replacement_derived_memories: replacements,
            superseded_derived_memory_ids,
            correction_origin,
            rationale: input.rationale,
            lifecycle_policy: CorrectionLifecyclePolicy {
                supersede_replaced_derived_memories: input
                    .lifecycle_policy
                    .supersede_replaced_derived_memories,
                suppress_superseded_derived_memories: input
                    .lifecycle_policy
                    .suppress_superseded_derived_memories,
                retain_original_source_objects: input
                    .lifecycle_policy
                    .retain_original_source_objects,
                ..CorrectionLifecyclePolicy::default()
            },
            cascade_policy: CorrectionCascadePolicy {
                apply_to_provenanced_derived_memories: input
                    .cascade_policy
                    .apply_to_provenanced_derived_memories,
                require_original_source_match: input.cascade_policy.require_original_source_match,
                cascade_to_threads: input.cascade_policy.cascade_to_threads,
            },
            include_trace: input.include_trace,
        };
        let outcome = state.memory.correct(draft).await?;
        for (external_id, id) in pending_replacements {
            state.derived_memory_ids.insert(external_id.clone(), id);
            state.reverse_derived_memory_ids.insert(id, external_id);
        }
        state.persist_identities()?;
        lifecycle_result(state, outcome)
    }

    async fn forget(&self, input: ForgetMemoryInput) -> Result<LifecycleMutationResult> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered objects: {}", input.namespace))?;
        let targets = input
            .targets
            .iter()
            .map(|target| lifecycle_target_to_live(target, state))
            .collect::<Result<Vec<_>>>()?;
        let draft = ForgetMemoryDraft {
            targets,
            rationale: input.rationale,
            lifecycle_policy: ForgetLifecyclePolicy {
                suppression: SuppressionPolicy {
                    suppress_target: input.suppression_policy.suppress_target,
                    suppress_derived_from_target: input
                        .suppression_policy
                        .suppress_derived_from_target,
                    preserve_original_raw_refs: input.suppression_policy.preserve_original_raw_refs,
                },
                archive: ArchivePolicy {
                    archive_thread: input.archive_policy.archive_thread,
                    archive_thread_derived_memories: input
                        .archive_policy
                        .archive_thread_derived_memories,
                    preserve_original_raw_refs: input.archive_policy.preserve_original_raw_refs,
                },
                ..ForgetLifecyclePolicy::default()
            },
            cascade_policy: ForgetCascadePolicy {
                apply_to_derived_from_target: input.cascade_policy.apply_to_derived_from_target,
                apply_to_thread_members: input.cascade_policy.apply_to_thread_members,
            },
            target_retention_state: parse_retention_state(&input.target_retention_state)?,
            target_thread_status: input
                .target_thread_status
                .as_deref()
                .map(parse_thread_status)
                .transpose()?,
            include_trace: input.include_trace,
        };
        let outcome = state.memory.forget(draft).await?;
        lifecycle_result(state, outcome)
    }

    async fn prepare(&self, input: PrepareWriteInput) -> Result<PreparedWritePlan> {
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&input.namespace) {
            let state = self.create_namespace_state(&input.namespace).await?;
            namespaces.insert(input.namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&input.namespace)
            .expect("namespace state inserted");
        let episode_id = deterministic_id(&input.namespace, "episode", &input.episode_external_id);
        let observation_id = deterministic_id(
            &input.namespace,
            "observation",
            &input.observation_external_id,
        );
        let mut episode = EpisodeDraft::new(input.content.clone());
        episode.id = Some(episode_id);
        episode.source_conversation_id = Some(input.episode_external_id.clone());
        episode.raw_ref = input.raw_refs.first().cloned().or_else(|| {
            Some(format!(
                "eval://{}/episode/{}",
                input.namespace, input.episode_external_id
            ))
        });
        let mut observation = ObservationDraft::new(episode_id, input.content.clone());
        observation.id = Some(observation_id);
        observation.raw_ref = input.raw_refs.first().cloned().or_else(|| {
            Some(format!(
                "eval://{}/observation/{}",
                input.namespace, input.observation_external_id
            ))
        });
        let mut remember_input = RememberInput::new(input.content.clone());
        remember_input.raw_refs = input.raw_refs.clone();
        remember_input.episode_drafts.push(episode);
        remember_input.observation_drafts.push(observation);
        let backend_plan = state
            .memory
            .prepare(
                remember_input,
                PrepareOptions {
                    idempotency_key: input.idempotency_key.clone(),
                    include_vector_index_candidates: input.include_vector_index_candidates,
                    include_stats_update_candidates: input.include_stats_update_candidates,
                },
            )
            .await?;
        let known_refs = HashMap::from([
            (
                episode_id,
                MemoryEndpointInput {
                    object_type: "episode".to_string(),
                    external_id: input.episode_external_id.clone(),
                },
            ),
            (
                observation_id,
                MemoryEndpointInput {
                    object_type: "observation".to_string(),
                    external_id: input.observation_external_id.clone(),
                },
            ),
        ]);
        let candidates = backend_plan
            .candidates
            .iter()
            .map(|candidate| prepared_candidate_from_live(candidate, state, &known_refs))
            .collect::<Result<Vec<_>>>()?;
        let validations = backend_plan
            .validations
            .iter()
            .map(candidate_validation_from_live)
            .collect();
        Ok(PreparedWritePlan {
            namespace: input.namespace.clone(),
            operation_internal_id: backend_plan.operation_id.to_string(),
            idempotency_key: backend_plan.idempotency_key.clone(),
            input,
            candidates,
            validations,
            backend_plan: serde_json::to_value(backend_plan)?,
        })
    }

    async fn validate_plan(
        &self,
        plan: &PreparedWritePlan,
    ) -> Result<Vec<CandidateValidationResult>> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&plan.namespace)
            .ok_or_else(|| anyhow!("namespace has no prepared state: {}", plan.namespace))?;
        let backend_plan: RememberWritePlan = serde_json::from_value(plan.backend_plan.clone())
            .context("deserialize Character Memory write plan")?;
        Ok(state
            .memory
            .validate_plan(&backend_plan)
            .await?
            .iter()
            .map(candidate_validation_from_live)
            .collect())
    }

    async fn commit(
        &self,
        plan: PreparedWritePlan,
        options: CommitWriteOptions,
    ) -> Result<CommitWriteResult> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&plan.namespace)
            .ok_or_else(|| anyhow!("namespace has no prepared state: {}", plan.namespace))?;
        let backend_plan: RememberWritePlan = serde_json::from_value(plan.backend_plan)
            .context("deserialize Character Memory write plan")?;
        let outcome = state
            .memory
            .commit(
                backend_plan,
                CommitOptions {
                    update_vectors: options.update_vectors,
                    update_stats: options.update_stats,
                },
            )
            .await?;
        let episode_id =
            deterministic_id(&plan.namespace, "episode", &plan.input.episode_external_id);
        let observation_id = deterministic_id(
            &plan.namespace,
            "observation",
            &plan.input.observation_external_id,
        );
        if outcome.persisted_object_ids.contains(&episode_id) {
            state
                .episode_ids
                .insert(plan.input.episode_external_id.clone(), episode_id);
            state
                .reverse_episode_ids
                .insert(episode_id, plan.input.episode_external_id.clone());
        }
        if outcome.persisted_object_ids.contains(&observation_id) {
            state
                .observation_ids
                .insert(plan.input.observation_external_id.clone(), observation_id);
            state.reverse_observation_ids.insert(
                observation_id,
                (
                    plan.input.observation_external_id.clone(),
                    plan.input.episode_external_id.clone(),
                ),
            );
        }
        state.persist_identities()?;
        Ok(CommitWriteResult {
            persisted_object_refs: outcome
                .persisted_object_ids
                .iter()
                .filter_map(|id| external_endpoint_for_id(state, *id))
                .collect(),
            persisted_link_external_ids: outcome
                .persisted_link_ids
                .iter()
                .filter_map(|id| state.reverse_link_ids.get(id).cloned())
                .collect(),
            vector_indexed_object_refs: outcome
                .vector_indexed_object_ids
                .iter()
                .filter_map(|id| external_endpoint_for_id(state, *id))
                .collect(),
            repair_needed: outcome
                .repair_needed
                .iter()
                .map(|marker| format!("{marker:?}"))
                .collect(),
        })
    }

    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        match input.mode {
            RetrievalMode::Bm25Only => {
                bail!(
                    "retrieval.mode=bm25_only is service-free and must be run with `--adapter mock --allow-mock-benchmark`; the live Character Memory adapter would use Qdrant/Oxigraph"
                );
            }
            RetrievalMode::VectorOnly => return self.retrieve_vector_only(input).await,
            RetrievalMode::Hybrid => {}
        }
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&input.namespace) {
            let state = self.create_namespace_state(&input.namespace).await?;
            namespaces.insert(input.namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&input.namespace)
            .expect("namespace state inserted");

        let mut context = RetrievalContext::new(input.query);
        context.include_trace = input.include_debug_rationale;
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
        if input.include_threads {
            context.object_type_defaults.push(ObjectType::MemoryThread);
        }
        if input.include_entities {
            context.object_type_defaults.push(ObjectType::Entity);
        }

        let outcome = state.memory.retrieve(context).await?;
        Ok(flatten_outcome(state, outcome))
    }
}

fn flatten_outcome(
    state: &NamespaceState,
    outcome: character_memory::RetrieveOutcome,
) -> RetrievedContextPack {
    let telemetry = telemetry_from_outcome(&outcome);
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
    for thread in outcome.pack.active_threads {
        let (score, rank) = trace_by_id
            .get(&thread.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "memory_thread".to_string(),
            internal_id: thread.id.to_string(),
            external_id: state.reverse_thread_ids.get(&thread.id).cloned(),
            episode_external_id: None,
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(thread.summary),
        });
    }

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

    let mut seen_derived = HashSet::new();
    for derived in outcome
        .pack
        .derived_memories
        .into_iter()
        .chain(outcome.pack.preferences)
        .chain(outcome.pack.relationship_notes)
        .chain(outcome.pack.open_loops)
        .chain(outcome.pack.commitments)
        .chain(outcome.pack.character_signals)
    {
        if !seen_derived.insert(derived.memory.id) {
            continue;
        }
        let (score, rank) = trace_by_id
            .get(&derived.memory.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "derived_memory".to_string(),
            internal_id: derived.memory.id.to_string(),
            external_id: state
                .reverse_derived_memory_ids
                .get(&derived.memory.id)
                .cloned(),
            episode_external_id: derived
                .source_episode_ids
                .first()
                .and_then(|id| state.reverse_episode_ids.get(id))
                .cloned(),
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(derived.memory.text),
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
        telemetry,
    }
}

fn vector_hits_to_context_pack(
    snapshot: &VectorNamespaceSnapshot,
    hits: Vec<VectorHit>,
) -> RetrievedContextPack {
    let vector_candidate_count = hits.len();
    let mut best_by_key: HashMap<(&'static str, MemoryId), VectorHit> = HashMap::new();
    for hit in hits {
        let key = (hit.kind, hit.object_id);
        match best_by_key.get(&key) {
            Some(existing) if existing.score >= hit.score => {}
            _ => {
                best_by_key.insert(key, hit);
            }
        }
    }

    let mut hits = best_by_key.into_values().collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.kind.cmp(right.kind))
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    let mut items = Vec::new();
    for (idx, hit) in hits.into_iter().enumerate() {
        match hit.kind {
            "episode" => {
                let Some(external_id) = snapshot.reverse_episode_ids.get(&hit.object_id).cloned()
                else {
                    continue;
                };
                items.push(RetrievedItem {
                    kind: "episode".to_string(),
                    internal_id: hit.object_id.to_string(),
                    external_id: Some(external_id),
                    episode_external_id: None,
                    score: Some(hit.score),
                    rank: idx + 1,
                    rationale: vec!["vector_only".to_string()],
                    text: hit.text,
                });
            }
            "observation" => {
                let Some((external_id, episode_external_id)) = snapshot
                    .reverse_observation_ids
                    .get(&hit.object_id)
                    .cloned()
                else {
                    continue;
                };
                items.push(RetrievedItem {
                    kind: "observation".to_string(),
                    internal_id: hit.object_id.to_string(),
                    external_id: Some(external_id),
                    episode_external_id: Some(episode_external_id),
                    score: Some(hit.score),
                    rank: idx + 1,
                    rationale: vec!["vector_only".to_string()],
                    text: hit.text,
                });
            }
            _ => {}
        }
    }
    for (idx, item) in items.iter_mut().enumerate() {
        item.rank = idx + 1;
    }

    let context_text = render_context_text(&items);
    let context_char_count = context_text.chars().count();
    let context_word_count = context_text.split_whitespace().count();

    RetrievedContextPack {
        items,
        context_text,
        context_char_count,
        context_word_count,
        telemetry: RetrievalTelemetry {
            trace_available: false,
            vector_candidate_count: Some(vector_candidate_count),
            graph_relation_count: None,
            graph_verified_count: None,
            stale_candidate_omission_count: None,
            lifecycle_omission_count: None,
            lifecycle_filter_decision_count: None,
            suppressed_or_deleted_returned_count: None,
            superseded_current_returned_count: None,
            graph_object_missing_omitted_count: None,
            graph_object_missing_returned_count: None,
            section_assignment_count: None,
            section_assignment_counts: BTreeMap::new(),
            stale_candidate_omission_reasons: BTreeMap::new(),
            lifecycle_omission_reasons: BTreeMap::new(),
        },
    }
}

fn scored_point_to_vector_hit(
    point: ScoredPoint,
    expected_kind: &'static str,
) -> Result<VectorHit> {
    let kind = payload_string(&point.payload, QDRANT_OBJECT_TYPE_FIELD)?;
    if kind != expected_kind {
        bail!("Qdrant returned object_type={kind} for vector_only {expected_kind} query");
    }
    let object_id = payload_string(&point.payload, QDRANT_OBJECT_ID_FIELD)?
        .parse::<MemoryId>()
        .with_context(|| format!("parse Qdrant payload {QDRANT_OBJECT_ID_FIELD}"))?;
    let text = optional_payload_string(&point.payload, QDRANT_CONTENT_TEXT_FIELD);
    Ok(VectorHit {
        kind: expected_kind,
        object_id,
        score: point.score as f64,
        text,
    })
}

fn payload_string(
    payload: &HashMap<String, qdrant_client::qdrant::Value>,
    field: &str,
) -> Result<String> {
    match payload.get(field).and_then(|value| value.kind.as_ref()) {
        Some(Kind::StringValue(value)) => Ok(value.clone()),
        _ => bail!("missing or invalid Qdrant payload string field: {field}"),
    }
}

fn optional_payload_string(
    payload: &HashMap<String, qdrant_client::qdrant::Value>,
    field: &str,
) -> Option<String> {
    match payload.get(field).and_then(|value| value.kind.as_ref()) {
        Some(Kind::StringValue(value)) => Some(value.clone()),
        _ => None,
    }
}

fn telemetry_from_outcome(outcome: &character_memory::RetrieveOutcome) -> RetrievalTelemetry {
    let trace = outcome.trace.as_ref();
    let returned_ids = returned_object_ids(outcome);
    let suppressed_or_deleted_returned_count = trace.map(|trace| {
        trace
            .lifecycle_filter_decisions
            .iter()
            .filter(|decision| {
                returned_ids.contains(&decision.object.id)
                    && decision.action == LifecycleFilterAction::Included
                    && (matches!(
                        decision.retention_state,
                        Some(RetentionState::Suppressed | RetentionState::Deleted)
                    ) || matches!(
                        decision.reason,
                        LifecycleFilterReason::SuppressedIncludedByPolicy
                            | LifecycleFilterReason::DeletedIncludedByPolicy
                    ))
            })
            .count()
    });
    let superseded_current_returned_count = trace.map(|trace| {
        trace
            .lifecycle_filter_decisions
            .iter()
            .filter(|decision| {
                returned_ids.contains(&decision.object.id)
                    && decision.action == LifecycleFilterAction::Included
                    && (decision.is_current == Some(false)
                        || !decision.superseded_by.is_empty()
                        || matches!(
                            decision.reason,
                            LifecycleFilterReason::NonCurrentIncludedByPolicy
                                | LifecycleFilterReason::SupersededIncludedByPolicy
                        ))
            })
            .count()
    });
    let graph_object_missing_omitted_count = trace.map(|trace| {
        trace
            .stale_candidate_omissions
            .iter()
            .filter(|omission| {
                matches!(
                    omission.reason,
                    character_memory::StaleCandidateReason::GraphObjectMissing
                )
            })
            .count()
            + trace
                .lifecycle_filter_decisions
                .iter()
                .filter(|decision| {
                    !returned_ids.contains(&decision.object.id)
                        && decision.action == LifecycleFilterAction::Omitted
                        && decision.reason == LifecycleFilterReason::GraphObjectMissing
                })
                .count()
    });
    let graph_object_missing_returned_count = trace.map(|trace| {
        trace
            .lifecycle_filter_decisions
            .iter()
            .filter(|decision| {
                returned_ids.contains(&decision.object.id)
                    && decision.action == LifecycleFilterAction::Included
                    && decision.reason == LifecycleFilterReason::GraphObjectMissing
            })
            .count()
    });
    RetrievalTelemetry {
        trace_available: trace.is_some(),
        vector_candidate_count: Some(outcome.rationale.vector_candidate_count),
        graph_relation_count: trace.map(|trace| trace.graph_relations.len()),
        graph_verified_count: Some(outcome.rationale.graph_verified_count),
        stale_candidate_omission_count: Some(outcome.rationale.stale_candidate_omission_count),
        lifecycle_omission_count: Some(outcome.rationale.lifecycle_omission_count),
        lifecycle_filter_decision_count: trace.map(|trace| trace.lifecycle_filter_decisions.len()),
        suppressed_or_deleted_returned_count,
        superseded_current_returned_count,
        graph_object_missing_omitted_count,
        graph_object_missing_returned_count,
        section_assignment_count: trace.map(|trace| trace.section_assignments.len()),
        section_assignment_counts: trace
            .map(|trace| {
                let mut counts = BTreeMap::new();
                for assignment in &trace.section_assignments {
                    *counts
                        .entry(format!("{:?}", assignment.section).to_ascii_snake_case())
                        .or_insert(0) += 1;
                }
                counts
            })
            .unwrap_or_default(),
        stale_candidate_omission_reasons: outcome
            .rationale
            .stale_candidate_omission_reasons
            .iter()
            .map(|summary| {
                (
                    format!("{:?}", summary.reason).to_ascii_snake_case(),
                    summary.count,
                )
            })
            .collect(),
        lifecycle_omission_reasons: outcome
            .rationale
            .lifecycle_omission_reasons
            .iter()
            .map(|summary| {
                (
                    format!("{:?}", summary.reason).to_ascii_snake_case(),
                    summary.count,
                )
            })
            .collect(),
    }
}

fn returned_object_ids(outcome: &character_memory::RetrieveOutcome) -> HashSet<MemoryId> {
    outcome
        .pack
        .active_threads
        .iter()
        .map(|thread| thread.id)
        .chain(
            outcome
                .pack
                .relevant_episodes
                .iter()
                .map(|episode| episode.id),
        )
        .chain(
            outcome
                .pack
                .salient_observations
                .iter()
                .map(|observation| observation.id),
        )
        .chain(
            outcome
                .pack
                .derived_memories
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .preferences
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .relationship_notes
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .open_loops
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .commitments
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .character_signals
                .iter()
                .map(|derived| derived.memory.id),
        )
        .collect()
}

trait SnakeCaseDebug {
    fn to_ascii_snake_case(&self) -> String;
}

impl SnakeCaseDebug for str {
    fn to_ascii_snake_case(&self) -> String {
        let mut out = String::new();
        for (idx, ch) in self.chars().enumerate() {
            if ch.is_ascii_uppercase() {
                if idx > 0 {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch);
            }
        }
        out
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

fn shared_input_namespace<'a>(
    label: &str,
    expected: &'a str,
    namespaces: impl IntoIterator<Item = &'a str>,
) -> Result<String> {
    for namespace in namespaces {
        if namespace != expected {
            bail!("{label} contains multiple namespaces: {expected} and {namespace}");
        }
    }
    Ok(expected.to_string())
}

fn parse_timestamp(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .with_context(|| {
                    format!(
                        "parse RFC3339 timestamp {value}; dataset loaders should normalize official benchmark timestamps before live adapter ingestion"
                    )
                })
        })
        .transpose()
}

fn resolve_ids(
    kind: &str,
    external_ids: &[String],
    persisted: &BTreeMap<String, MemoryId>,
    pending: &BTreeMap<String, MemoryId>,
) -> Result<Vec<MemoryId>> {
    external_ids
        .iter()
        .map(|external_id| {
            persisted
                .get(external_id)
                .or_else(|| pending.get(external_id))
                .copied()
                .ok_or_else(|| {
                    anyhow!("{kind} enrichment references unknown external_id {external_id}")
                })
        })
        .collect()
}

fn resolve_endpoint(
    endpoint: &MemoryEndpointInput,
    state: &NamespaceState,
    pending_entities: &BTreeMap<String, MemoryId>,
    pending_threads: &BTreeMap<String, MemoryId>,
    pending_derived: &BTreeMap<String, MemoryId>,
) -> Result<(ObjectType, MemoryId)> {
    let object_type = parse_object_type(&endpoint.object_type)?;
    if object_type == ObjectType::MemoryLink {
        bail!("memory links cannot be endpoints in enrichment links");
    }
    let id = match object_type {
        ObjectType::Episode => state.episode_ids.get(&endpoint.external_id).copied(),
        ObjectType::Observation => state.observation_ids.get(&endpoint.external_id).copied(),
        ObjectType::Entity => state
            .entity_ids
            .get(&endpoint.external_id)
            .or_else(|| pending_entities.get(&endpoint.external_id))
            .copied(),
        ObjectType::MemoryThread => state
            .thread_ids
            .get(&endpoint.external_id)
            .or_else(|| pending_threads.get(&endpoint.external_id))
            .copied(),
        ObjectType::DerivedMemory => state
            .derived_memory_ids
            .get(&endpoint.external_id)
            .or_else(|| pending_derived.get(&endpoint.external_id))
            .copied(),
        ObjectType::MemoryLink => None,
    }
    .ok_or_else(|| {
        anyhow!(
            "link endpoint {:?} references unknown external_id {}",
            object_type,
            endpoint.external_id
        )
    })?;
    Ok((object_type, id))
}

fn correction_target_to_live(
    target: &CorrectionTargetInput,
    state: &NamespaceState,
) -> Result<CorrectionTarget> {
    match target {
        CorrectionTargetInput::DerivedMemory { external_id } => state
            .derived_memory_ids
            .get(external_id)
            .copied()
            .map(CorrectionTarget::derived_memory)
            .ok_or_else(|| anyhow!("unknown derived_memory external_id {external_id}")),
        CorrectionTargetInput::SourceObject {
            object_type,
            external_id,
            original_raw_ref,
            original_source_ref,
        } => {
            let target = match object_type.as_str() {
                "episode" => SourceObjectCorrectionTarget::Episode {
                    id: *state
                        .episode_ids
                        .get(external_id)
                        .ok_or_else(|| anyhow!("unknown episode external_id {external_id}"))?,
                    original_raw_ref: original_raw_ref.clone(),
                    original_source_ref: original_source_ref.clone(),
                },
                "observation" => SourceObjectCorrectionTarget::Observation {
                    id: *state
                        .observation_ids
                        .get(external_id)
                        .ok_or_else(|| anyhow!("unknown observation external_id {external_id}"))?,
                    original_raw_ref: original_raw_ref.clone(),
                    original_source_ref: original_source_ref.clone(),
                },
                unsupported => {
                    bail!("unsupported correction source object type: {unsupported}")
                }
            };
            Ok(CorrectionTarget::source_object(target))
        }
    }
}

fn lifecycle_target_to_live(
    target: &MemoryEndpointInput,
    state: &NamespaceState,
) -> Result<LifecycleTargetRef> {
    match target.object_type.as_str() {
        "episode" => state
            .episode_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::episode),
        "observation" => state
            .observation_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::observation),
        "derived_memory" => state
            .derived_memory_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::derived_memory),
        "memory_thread" => state
            .thread_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::memory_thread),
        unsupported => bail!("unsupported lifecycle target object type: {unsupported}"),
    }
    .ok_or_else(|| {
        anyhow!(
            "unknown {} external_id {}",
            target.object_type,
            target.external_id
        )
    })
}

fn source_provenance_reference_to_live(
    input: &SourceProvenanceInput,
    state: &NamespaceState,
) -> Result<SourceProvenanceReference> {
    Ok(SourceProvenanceReference {
        episode_ids: resolve_ids(
            "episode",
            &input.episode_external_ids,
            &state.episode_ids,
            &BTreeMap::new(),
        )?,
        observation_ids: resolve_ids(
            "observation",
            &input.observation_external_ids,
            &state.observation_ids,
            &BTreeMap::new(),
        )?,
        external_refs: input
            .external_refs
            .iter()
            .map(external_source_ref_to_live)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn external_source_ref_to_live(input: &ExternalSourceRefInput) -> Result<ExternalSourceReference> {
    match (&input.source_ref, &input.raw_ref) {
        (Some(source_ref), None) => Ok(ExternalSourceReference::source(source_ref.clone())),
        (None, Some(raw_ref)) => Ok(ExternalSourceReference::raw(raw_ref.clone())),
        _ => bail!("external source reference must contain exactly one of source_ref or raw_ref"),
    }
}

fn replacement_to_live(
    input: &ReplacementDerivedMemoryInput,
    id: MemoryId,
    state: &NamespaceState,
) -> Result<ReplacementDerivedMemoryDraft> {
    let memory = &input.memory;
    let mut draft = ReplacementDerivedMemoryDraft::new(
        parse_derived_type(&memory.derived_type)?,
        memory.text.clone(),
    );
    draft.id = Some(id);
    draft.derived_from_episode_ids = resolve_ids(
        "episode",
        &memory.source_episode_external_ids,
        &state.episode_ids,
        &BTreeMap::new(),
    )?;
    draft.derived_from_observation_ids = resolve_ids(
        "observation",
        &memory.source_observation_external_ids,
        &state.observation_ids,
        &BTreeMap::new(),
    )?;
    draft.thread_ids = resolve_ids(
        "memory_thread",
        &memory.thread_external_ids,
        &state.thread_ids,
        &BTreeMap::new(),
    )?;
    draft.entity_ids = resolve_ids(
        "entity",
        &memory.entity_external_ids,
        &state.entity_ids,
        &BTreeMap::new(),
    )?;
    draft.confidence = memory.confidence;
    draft.salience_score = memory.salience_score;
    draft.stability = parse_stability(&memory.stability)?;
    draft.supersedes = resolve_ids(
        "derived_memory",
        &memory.supersedes_external_ids,
        &state.derived_memory_ids,
        &BTreeMap::new(),
    )?;
    draft.original_source_provenance =
        source_provenance_reference_to_live(&input.original_source_provenance, state)?;
    draft.correction_origin_provenance =
        source_provenance_reference_to_live(&input.correction_origin_provenance, state)?;
    Ok(draft)
}

fn lifecycle_result(
    state: &NamespaceState,
    outcome: LifecycleMutationOutcome,
) -> Result<LifecycleMutationResult> {
    let mutated_object_refs = outcome
        .graph_mutated_object_ids
        .iter()
        .filter_map(|object| external_endpoint_for_object(state, object.object_type, object.id))
        .collect();
    let mutated_link_external_ids = outcome
        .graph_mutated_link_ids
        .iter()
        .filter_map(|id| state.reverse_link_ids.get(id).cloned())
        .collect();
    let vector_maintained_object_refs = outcome
        .vector_maintained_object_ids
        .iter()
        .filter_map(|object| external_endpoint_for_object(state, object.object_type, object.id))
        .collect();
    let superseded = outcome
        .trace
        .into_iter()
        .flat_map(|trace| trace.superseded_by)
        .filter_map(|evidence| {
            Some(SupersessionResult {
                superseded_external_id: state
                    .reverse_derived_memory_ids
                    .get(&evidence.superseded_memory_id)?
                    .clone(),
                superseded_by_external_id: state
                    .reverse_derived_memory_ids
                    .get(&evidence.superseded_by_memory_id)?
                    .clone(),
            })
        })
        .collect();
    Ok(LifecycleMutationResult {
        mutated_object_refs,
        mutated_link_external_ids,
        vector_maintained_object_refs,
        superseded,
    })
}

fn external_endpoint_for_id(state: &NamespaceState, id: MemoryId) -> Option<MemoryEndpointInput> {
    for object_type in [
        ObjectType::Episode,
        ObjectType::Observation,
        ObjectType::Entity,
        ObjectType::MemoryThread,
        ObjectType::DerivedMemory,
        ObjectType::MemoryLink,
    ] {
        if let Some(endpoint) = external_endpoint_for_object(state, object_type, id) {
            return Some(endpoint);
        }
    }
    None
}

fn external_endpoint_for_object(
    state: &NamespaceState,
    object_type: ObjectType,
    id: MemoryId,
) -> Option<MemoryEndpointInput> {
    external_endpoint_from_reverse_maps(
        object_type,
        id,
        &state.reverse_episode_ids,
        &state.reverse_observation_ids,
        &state.reverse_entity_ids,
        &state.reverse_thread_ids,
        &state.reverse_derived_memory_ids,
        &state.reverse_link_ids,
    )
}

#[allow(clippy::too_many_arguments)]
fn external_endpoint_from_reverse_maps(
    object_type: ObjectType,
    id: MemoryId,
    episodes: &BTreeMap<MemoryId, String>,
    observations: &BTreeMap<MemoryId, (String, String)>,
    entities: &BTreeMap<MemoryId, String>,
    threads: &BTreeMap<MemoryId, String>,
    derived_memories: &BTreeMap<MemoryId, String>,
    links: &BTreeMap<MemoryId, String>,
) -> Option<MemoryEndpointInput> {
    let (object_type, external_id) = match object_type {
        ObjectType::Episode => ("episode", episodes.get(&id)?.clone()),
        ObjectType::Observation => ("observation", observations.get(&id)?.0.clone()),
        ObjectType::Entity => ("entity", entities.get(&id)?.clone()),
        ObjectType::MemoryThread => ("memory_thread", threads.get(&id)?.clone()),
        ObjectType::DerivedMemory => ("derived_memory", derived_memories.get(&id)?.clone()),
        ObjectType::MemoryLink => ("memory_link", links.get(&id)?.clone()),
    };
    Some(MemoryEndpointInput {
        object_type: object_type.to_string(),
        external_id,
    })
}

fn prepared_candidate_from_live(
    candidate: &MemoryCandidate,
    state: &NamespaceState,
    known_refs: &HashMap<MemoryId, MemoryEndpointInput>,
) -> Result<PreparedCandidate> {
    let (kind, internal_id, object_type, provenance): (
        &str,
        MemoryId,
        Option<ObjectType>,
        &CandidateProvenance,
    ) = match candidate {
        MemoryCandidate::Episode(candidate) => (
            "episode",
            candidate
                .draft
                .id
                .context("prepared episode candidate id")?,
            Some(ObjectType::Episode),
            &candidate.provenance,
        ),
        MemoryCandidate::Observation(candidate) => (
            "observation",
            candidate
                .draft
                .id
                .context("prepared observation candidate id")?,
            Some(ObjectType::Observation),
            &candidate.provenance,
        ),
        MemoryCandidate::Entity(candidate) => (
            "entity",
            candidate.draft.id.context("prepared entity candidate id")?,
            Some(ObjectType::Entity),
            &candidate.provenance,
        ),
        MemoryCandidate::MemoryThread(candidate) => (
            "memory_thread",
            candidate
                .draft
                .id
                .context("prepared memory_thread candidate id")?,
            Some(ObjectType::MemoryThread),
            &candidate.provenance,
        ),
        MemoryCandidate::DerivedMemory(candidate) => (
            "derived_memory",
            candidate
                .draft
                .id
                .context("prepared derived_memory candidate id")?,
            Some(ObjectType::DerivedMemory),
            &candidate.provenance,
        ),
        MemoryCandidate::MemoryLink(candidate) => (
            "memory_link",
            candidate
                .draft
                .id
                .context("prepared memory_link candidate id")?,
            Some(ObjectType::MemoryLink),
            &candidate.provenance,
        ),
        MemoryCandidate::VectorIndex(candidate) => (
            "vector_index",
            candidate.target.id,
            Some(candidate.target.object_type),
            &candidate.provenance,
        ),
        MemoryCandidate::StatsUpdate(candidate) => (
            "stats_update",
            candidate.subject.id,
            Some(candidate.subject.object_type),
            &candidate.provenance,
        ),
    };
    let external_id = known_refs
        .get(&internal_id)
        .cloned()
        .or_else(|| {
            object_type.and_then(|kind| external_endpoint_for_object(state, kind, internal_id))
        })
        .map(|endpoint| endpoint.external_id);
    let (producer_kind, rationale_origin, rationale) = candidate_provenance_summary(provenance);
    Ok(PreparedCandidate {
        kind: kind.to_string(),
        internal_id: internal_id.to_string(),
        external_id,
        producer_kind,
        rationale_origin,
        rationale,
        source: source_provenance_from_live(&provenance.source, state, known_refs),
    })
}

fn candidate_provenance_summary(
    provenance: &CandidateProvenance,
) -> (String, String, Option<String>) {
    (
        format!("{:?}", provenance.producer_kind).to_ascii_snake_case(),
        format!("{:?}", provenance.rationale_origin()).to_ascii_snake_case(),
        provenance.rationale.text().map(str::to_string),
    )
}

fn source_provenance_from_live(
    provenance: &SourceProvenance,
    state: &NamespaceState,
    known_refs: &HashMap<MemoryId, MemoryEndpointInput>,
) -> SourceProvenanceInput {
    let external_for = |object_type, id| {
        known_refs
            .get(&id)
            .cloned()
            .or_else(|| external_endpoint_for_object(state, object_type, id))
            .map(|endpoint| endpoint.external_id)
    };
    SourceProvenanceInput {
        episode_external_ids: provenance
            .episode_ids
            .iter()
            .filter_map(|id| external_for(ObjectType::Episode, *id))
            .collect(),
        observation_external_ids: provenance
            .observation_ids
            .iter()
            .filter_map(|id| external_for(ObjectType::Observation, *id))
            .collect(),
        external_refs: provenance
            .external_refs
            .iter()
            .map(|reference| ExternalSourceRefInput {
                source_ref: reference.source_ref.clone(),
                raw_ref: reference.raw_ref.clone(),
            })
            .collect(),
    }
}

fn candidate_validation_from_live(validation: &CandidateValidation) -> CandidateValidationResult {
    CandidateValidationResult {
        candidate_index: validation.candidate_index,
        candidate_kind: format!("{:?}", validation.candidate_kind).to_ascii_snake_case(),
        status: match validation.status {
            CandidateValidationStatus::Valid => "valid",
            CandidateValidationStatus::Invalid => "invalid",
        }
        .to_string(),
        errors: validation.errors.clone(),
        warnings: validation.warnings.clone(),
    }
}

fn parse_entity_type(value: &str) -> Result<EntityType> {
    parse_snake_enum(value, "entity_type")
}

fn parse_derived_type(value: &str) -> Result<DerivedType> {
    parse_snake_enum(value, "derived_type")
}

fn parse_thread_status(value: &str) -> Result<ThreadStatus> {
    parse_snake_enum(value, "thread.status")
}

fn parse_stability(value: &str) -> Result<Stability> {
    parse_snake_enum(value, "derived_memory.stability")
}

fn parse_relation_type(value: &str) -> Result<RelationType> {
    parse_snake_enum(value, "link.relation")
}

fn parse_object_type(value: &str) -> Result<ObjectType> {
    parse_snake_enum(value, "endpoint.object_type")
}

fn parse_snake_enum<T>(value: &str, field: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .with_context(|| format!("parse {field} value {value:?}"))
}

fn validate_cleanup_target(collection_name: &str, required_prefix: Option<&str>) -> Result<()> {
    let Some(required_prefix) = required_prefix
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
    else {
        bail!("cleanup is enabled but no required collection prefix was configured");
    };
    let sanitized_prefix = sanitize_collection_segment(required_prefix);
    if sanitized_prefix.len() < 3 {
        bail!("cleanup required collection prefix is too broad");
    }
    if !collection_name.starts_with(&sanitized_prefix) {
        bail!(
            "refusing to cleanup collection {collection_name}; it does not start with required eval prefix {sanitized_prefix}"
        );
    }
    Ok(())
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

struct CharacterMemoryEmbeddingProvider {
    inner: DeterministicEmbeddingProvider,
}

fn parse_retention_state(value: &str) -> Result<RetentionState> {
    parse_snake_enum(value, "forget.target_retention_state")
}

impl CharacterMemoryEmbeddingProvider {
    fn new(vector_size: usize) -> Result<Self> {
        Ok(Self {
            inner: DeterministicEmbeddingProvider::new(vector_size)?,
        })
    }
}

#[async_trait]
impl EmbeddingProvider for CharacterMemoryEmbeddingProvider {
    fn vector_size(&self) -> usize {
        self.inner.vector_size()
    }

    async fn generate_embedding<'a>(
        &self,
        text: &'a str,
    ) -> std::result::Result<Vec<f32>, character_memory::CustomError> {
        Ok(self.inner.vector_for_text(text))
    }

    async fn bulk_generate_embeddings<'a>(
        &self,
        texts: &'a [&'a str],
    ) -> std::result::Result<Vec<Vec<f32>>, character_memory::CustomError> {
        Ok(texts
            .iter()
            .map(|text| self.inner.vector_for_text(text))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use character_memory::{
        CURRENT_SCHEMA_VERSION, ContinuityContextPack, Episode, LifecycleFilterDecision,
        MemoryObjectRef, Modality, RetrievalRationale, RetrievalTrace, RetrieveOutcome,
        VectorDatabaseError,
    };
    use cmem_eval_core::{CleanupConfig, EmbeddingConfig};
    use tempfile::tempdir;

    fn adapter_config(run_id: String, namespace_prefix: String) -> BenchmarkRunConfig {
        let mut backend = cmem_eval_core::BackendConfig {
            namespace_prefix: Some(namespace_prefix.clone()),
            qdrant_connection_string: Some(
                env::var("QDRANT_CONNECTION_STRING")
                    .unwrap_or_else(|_| "http://localhost:6334".to_string()),
            ),
            cleanup: CleanupConfig {
                enabled: true,
                require_collection_prefix: Some(namespace_prefix),
            },
            embedding: EmbeddingConfig {
                provider: "deterministic".to_string(),
                vector_size: Some(3072),
                ..EmbeddingConfig::default()
            },
            ..cmem_eval_core::BackendConfig::default()
        };
        backend.openai_api_key_env = "CMEM_EVAL_UNUSED_OPENAI_KEY".to_string();
        BenchmarkRunConfig {
            run_id,
            dataset: "synthetic".to_string(),
            backend,
            retrieval: Default::default(),
            ingest: Default::default(),
            metrics: Default::default(),
        }
    }

    fn is_qdrant_unavailable_error(error: &VectorDatabaseError) -> bool {
        let message = error.message.to_ascii_lowercase();
        error.backend == "qdrant"
            && (error
                .status
                .as_deref()
                .is_some_and(|status| status.to_ascii_lowercase().contains("unavailable"))
                || (error.kind == "response"
                    && message.contains("failed to connect")
                    && message.contains("tcp connect error"))
                || matches!(
                    error.kind.as_str(),
                    "reqwest::connect"
                        | "reqwest::timeout"
                        | "io::ConnectionRefused"
                        | "io::ConnectionReset"
                        | "io::ConnectionAborted"
                        | "io::NotConnected"
                        | "io::TimedOut"
                ))
    }

    fn qdrant_unavailable(error: &anyhow::Error) -> bool {
        let typed_error_is_unavailable = error
            .chain()
            .find_map(|source| source.downcast_ref::<VectorDatabaseError>())
            .is_some_and(is_qdrant_unavailable_error);
        let message = format!("{error:#}").to_ascii_lowercase();
        typed_error_is_unavailable
            || (message.contains("failed to connect") && message.contains("tcp connect error"))
            || message.contains("connection refused")
            || message.contains("timeout expired")
            || message.contains("status: unavailable")
            || message.contains("code: unavailable")
    }

    macro_rules! live_call_or_skip {
        ($service_available:ident, $phase:expr, $confirms_availability:expr, $call:expr) => {{
            match $call {
                Ok(value) => {
                    if $confirms_availability {
                        $service_available = true;
                    }
                    value
                }
                Err(error) if qdrant_unavailable(&error) && !$service_available => {
                    println!(
                        "skipping live adapter reattach test because Qdrant is unavailable during {}: {error:#}",
                        $phase
                    );
                    return;
                }
                Err(error) if qdrant_unavailable(&error) => {
                    panic!(
                        "Qdrant became unavailable during {} after a successful live call: {error:#}",
                        $phase
                    )
                }
                Err(error) => {
                    panic!("unexpected live adapter failure during {}: {error:#}", $phase)
                }
            }
        }};
    }

    macro_rules! live_call_or_skip_without_confirmation {
        ($service_available:ident, $phase:expr, $call:expr) => {{
            match $call {
                Ok(value) => value,
                Err(error) if qdrant_unavailable(&error) && !$service_available => {
                    println!(
                        "skipping live adapter reattach test because Qdrant is unavailable during {}: {error:#}",
                        $phase
                    );
                    return;
                }
                Err(error) if qdrant_unavailable(&error) => {
                    panic!(
                        "Qdrant became unavailable during {} after a successful live call: {error:#}",
                        $phase
                    )
                }
                Err(error) => {
                    panic!("unexpected live adapter failure during {}: {error:#}", $phase)
                }
            }
        }};
    }

    macro_rules! live_teardown_with_one_retry {
        ($service_available:ident, $phase:expr, $call:expr, $retry:expr) => {{
            match $call {
                Ok(value) => value,
                Err(error) if qdrant_unavailable(&error) && !$service_available => {
                    println!(
                        "skipping live adapter reattach test because Qdrant is unavailable during {}: {error:#}",
                        $phase
                    );
                    return;
                }
                Err(error) if qdrant_unavailable(&error) => {
                    println!(
                        "retrying live adapter teardown after Qdrant availability error during {}: {error:#}",
                        $phase
                    );
                    live_call_or_skip_without_confirmation!(
                        $service_available,
                        concat!($phase, " retry"),
                        $retry
                    )
                }
                Err(error) => {
                    panic!("unexpected live adapter failure during {}: {error:#}", $phase)
                }
            }
        }};
    }

    macro_rules! live_error_or_skip {
        ($service_available:ident, $phase:expr, $call:expr) => {{
            match $call {
                Err(error) if qdrant_unavailable(&error) && !$service_available => {
                    println!(
                        "skipping live adapter reattach test because Qdrant is unavailable during {}: {error:#}",
                        $phase
                    );
                    return;
                }
                Err(error) if qdrant_unavailable(&error) => {
                    panic!(
                        "Qdrant became unavailable during {} after a successful live call: {error:#}",
                        $phase
                    )
                }
                Err(error) => error,
                Ok(_) => panic!("expected live adapter failure during {}", $phase),
            }
        }};
    }

    fn unique_test_token() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
            .as_nanos();
        format!("{}-{nanos}", std::process::id())
    }

    #[test]
    fn deterministic_ids_are_stable_and_namespaced() {
        let first = deterministic_id("n1", "episode", "s1");
        assert_eq!(first, deterministic_id("n1", "episode", "s1"));
        assert_ne!(first, deterministic_id("n2", "episode", "s1"));
        assert_ne!(first, deterministic_id("n1", "observation", "s1"));
    }

    #[tokio::test]
    async fn collection_names_are_deterministic_and_run_scoped() {
        let first = CharacterMemoryAdapter::new(&adapter_config(
            "run-a".to_string(),
            "cmem_eval_task3".to_string(),
        ))
        .await
        .unwrap();
        let same = CharacterMemoryAdapter::new(&adapter_config(
            "run-a".to_string(),
            "cmem_eval_task3".to_string(),
        ))
        .await
        .unwrap();
        let parallel = CharacterMemoryAdapter::new(&adapter_config(
            "run-b".to_string(),
            "cmem_eval_task3".to_string(),
        ))
        .await
        .unwrap();
        let other_prefix = CharacterMemoryAdapter::new(&adapter_config(
            "run-a".to_string(),
            "cmem_eval_other".to_string(),
        ))
        .await
        .unwrap();

        assert_eq!(
            first.collection_name("namespace"),
            same.collection_name("namespace")
        );
        assert_ne!(
            first.collection_name("namespace"),
            parallel.collection_name("namespace")
        );
        assert_ne!(
            first.collection_name("namespace"),
            other_prefix.collection_name("namespace")
        );
        assert_eq!(
            first.identity_registry_path("namespace"),
            same.identity_registry_path("namespace")
        );
        assert_ne!(
            first.identity_registry_path("namespace"),
            parallel.identity_registry_path("namespace")
        );
        assert_ne!(
            first.identity_registry_path("namespace"),
            other_prefix.identity_registry_path("namespace")
        );
        let shared_suffix = first
            .namespace_identity_suffix("namespace")
            .simple()
            .to_string();
        assert!(first.collection_name("namespace").ends_with(&shared_suffix));
        assert!(
            first
                .identity_registry_path("namespace")
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(&format!("{shared_suffix}.json"))
        );
        validate_cleanup_target(&first.collection_name("namespace"), Some("cmem_eval_task3"))
            .unwrap();
    }

    #[test]
    fn identity_registry_serialization_is_stable_and_sorted() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("identity.json");
        let mut registry = ExternalIdRegistry::new("namespace");
        let z = deterministic_id("namespace", "episode", "z");
        let a = deterministic_id("namespace", "episode", "a");
        registry.episode_ids.insert("z".to_string(), z);
        registry.episode_ids.insert("a".to_string(), a);
        registry.reverse_episode_ids.insert(z, "z".to_string());
        registry.reverse_episode_ids.insert(a, "a".to_string());

        registry.save(&path).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        registry.save(&path).unwrap();
        let second = fs::read_to_string(&path).unwrap();

        assert_eq!(first, second);
        assert!(first.find("\"a\"").unwrap() < first.find("\"z\"").unwrap());
        assert_eq!(
            ExternalIdRegistry::load(&path, "namespace").unwrap(),
            registry
        );
    }

    #[tokio::test]
    async fn live_adapter_reattaches_with_external_ids() {
        let directory = tempdir().unwrap();
        let token = unique_test_token();
        let run_id = format!("task3-{token}");
        let prefix = format!("cmem_eval_task3_{token}");
        let namespace = "restart-round-trip";
        let mut config = adapter_config(run_id, prefix);
        config.backend.cleanup.enabled = false;
        config.backend.cleanup.require_collection_prefix = Some("unrelated:prefix".to_string());
        config.backend.identity_registry_dir = Some(
            directory
                .path()
                .join("identities")
                .to_string_lossy()
                .into_owned(),
        );
        config.backend.oxigraph_persistence_path = Some(
            directory
                .path()
                .join("oxigraph")
                .to_string_lossy()
                .into_owned(),
        );
        config.backend.retrieval_stats_path = Some(
            directory
                .path()
                .join("retrieval-stats.sqlite")
                .to_string_lossy()
                .into_owned(),
        );

        // Absence before the first successful Qdrant operation skips this gated test. Once
        // availability is demonstrated, later failures are test failures; teardown alone gets
        // one retry for the case where deletion committed but its response timed out.
        let mut qdrant_was_available = false;
        let adapter_a = live_call_or_skip!(
            qdrant_was_available,
            "initial adapter construction",
            false,
            CharacterMemoryAdapter::new(&config).await
        );
        live_call_or_skip!(
            qdrant_was_available,
            "initial fresh namespace open",
            true,
            adapter_a.open_namespace(namespace).await
        );
        live_call_or_skip!(
            qdrant_was_available,
            "episode ingest",
            true,
            adapter_a
                .remember_episode(EpisodeInput {
                    external_id: "episode-external".to_string(),
                    namespace: namespace.to_string(),
                    summary: "Alice remembers a restart-safe cup of tea.".to_string(),
                    started_at: None,
                    ended_at: None,
                    participants: Vec::new(),
                    metadata: serde_json::Value::Null,
                })
                .await
        );
        live_call_or_skip!(
            qdrant_was_available,
            "observation ingest",
            true,
            adapter_a
                .remember_observation(ObservationInput {
                    external_id: "observation-external".to_string(),
                    episode_external_id: "episode-external".to_string(),
                    namespace: namespace.to_string(),
                    speaker: Some("Alice".to_string()),
                    text: "The restart-safe drink is jasmine tea.".to_string(),
                    observed_at: None,
                    metadata: serde_json::Value::Null,
                })
                .await
        );
        drop(adapter_a);

        let adapter_b = live_call_or_skip!(
            qdrant_was_available,
            "reattach adapter construction",
            false,
            CharacterMemoryAdapter::new(&config).await
        );
        let lifecycle = live_call_or_skip!(
            qdrant_was_available,
            "namespace reattach",
            true,
            adapter_b.reattach_namespace(namespace).await
        );
        assert_eq!(lifecycle.restored_identity_count, 2);
        let retrieved = live_call_or_skip!(
            qdrant_was_available,
            "reattached retrieval",
            true,
            adapter_b
                .retrieve(RetrieveInput {
                    mode: RetrievalMode::Hybrid,
                    namespace: namespace.to_string(),
                    query: "What is the restart-safe drink?".to_string(),
                    query_date: None,
                    top_k_episodes: 8,
                    top_k_observations: 8,
                    include_derived_memories: false,
                    include_threads: false,
                    include_entities: false,
                    include_debug_rationale: true,
                })
                .await
        );
        assert!(retrieved.items.iter().any(|item| {
            item.external_id.as_deref() == Some("episode-external")
                || item.external_id.as_deref() == Some("observation-external")
        }));
        assert!(retrieved.items.iter().any(|item| {
            item.external_id.as_deref() == Some("observation-external")
                && item.episode_external_id.as_deref() == Some("episode-external")
        }));
        let collection_name = adapter_b.collection_name(namespace);
        live_call_or_skip!(
            qdrant_was_available,
            "backing collection deletion",
            true,
            adapter_b
                .qdrant
                .delete_collection(&collection_name)
                .await
                .with_context(|| format!("delete Qdrant collection {collection_name}"))
        );
        drop(adapter_b);

        let adapter_missing_collection = live_call_or_skip!(
            qdrant_was_available,
            "missing-collection adapter construction",
            false,
            CharacterMemoryAdapter::new(&config).await
        );
        let missing_collection_error = live_error_or_skip!(
            qdrant_was_available,
            "missing-collection namespace reattach",
            adapter_missing_collection
                .reattach_namespace(namespace)
                .await
        );
        let missing_collection_message = missing_collection_error.to_string();
        assert!(missing_collection_message.contains("Qdrant collection"));
        assert!(missing_collection_message.contains("does not exist"));
        assert!(missing_collection_message.contains("identity registry"));
        assert!(missing_collection_message.contains(&collection_name));
        println!("verified reattach rejects a surviving registry without its Qdrant collection");
        drop(adapter_missing_collection);

        let adapter_c = live_call_or_skip!(
            qdrant_was_available,
            "fresh adapter construction",
            false,
            CharacterMemoryAdapter::new(&config).await
        );
        let stale_open_error = live_error_or_skip!(
            qdrant_was_available,
            "stale fresh namespace rejection",
            adapter_c.open_namespace(namespace).await
        );
        assert!(
            stale_open_error
                .to_string()
                .contains("identity registry already exists")
        );
        live_call_or_skip!(
            qdrant_was_available,
            "fresh adapter durable reset",
            true,
            adapter_c.reset_namespace(namespace).await
        );
        let fresh = live_call_or_skip!(
            qdrant_was_available,
            "post-reset fresh namespace open",
            true,
            adapter_c.open_namespace(namespace).await
        );
        assert_eq!(fresh.restored_identity_count, 0);
        live_teardown_with_one_retry!(
            qdrant_was_available,
            "final namespace cleanup",
            adapter_c.reset_namespace(namespace).await,
            adapter_c.reset_namespace(namespace).await
        );
    }

    #[tokio::test]
    async fn post_run_cleanup_honors_cleanup_required_prefix() {
        let mut config = adapter_config("post-run-cleanup".to_string(), "bench:review".to_string());
        config.backend.cleanup.require_collection_prefix = Some("unrelated:prefix".to_string());
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();

        let error = adapter
            .cleanup_namespace("namespace")
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("refusing to cleanup collection"));
        assert!(error.contains("unrelated_prefix"));
    }

    #[test]
    fn new_operation_results_round_trip_all_external_id_kinds() {
        let episode_id = deterministic_id("n", "episode", "s1");
        let observation_id = deterministic_id("n", "observation", "o1");
        let entity_id = deterministic_id("n", "entity", "e1");
        let thread_id = deterministic_id("n", "memory_thread", "t1");
        let derived_id = deterministic_id("n", "derived_memory", "d1");
        let link_id = deterministic_id("n", "memory_link", "l1");
        let episodes = BTreeMap::from([(episode_id, "s1".to_string())]);
        let observations = BTreeMap::from([(observation_id, ("o1".to_string(), "s1".to_string()))]);
        let entities = BTreeMap::from([(entity_id, "e1".to_string())]);
        let threads = BTreeMap::from([(thread_id, "t1".to_string())]);
        let derived = BTreeMap::from([(derived_id, "d1".to_string())]);
        let links = BTreeMap::from([(link_id, "l1".to_string())]);

        for (object_type, id, expected_type, expected_external_id) in [
            (ObjectType::Episode, episode_id, "episode", "s1"),
            (ObjectType::Observation, observation_id, "observation", "o1"),
            (ObjectType::Entity, entity_id, "entity", "e1"),
            (ObjectType::MemoryThread, thread_id, "memory_thread", "t1"),
            (
                ObjectType::DerivedMemory,
                derived_id,
                "derived_memory",
                "d1",
            ),
            (ObjectType::MemoryLink, link_id, "memory_link", "l1"),
        ] {
            let endpoint = external_endpoint_from_reverse_maps(
                object_type,
                id,
                &episodes,
                &observations,
                &entities,
                &threads,
                &derived,
                &links,
            )
            .unwrap();
            assert_eq!(endpoint.object_type, expected_type);
            assert_eq!(endpoint.external_id, expected_external_id);
        }
    }

    #[test]
    fn prepared_candidate_provenance_preserves_producer_and_rationale_origin() {
        let caller = CandidateProvenance::caller("caller supplied the candidate");
        assert_eq!(
            candidate_provenance_summary(&caller),
            (
                "caller".to_string(),
                "provided_by_caller".to_string(),
                Some("caller supplied the candidate".to_string()),
            )
        );

        let helper = CandidateProvenance::unavailable(
            character_memory::CandidateProducerKind::DeterministicHelper,
        );
        assert_eq!(
            candidate_provenance_summary(&helper),
            (
                "deterministic_helper".to_string(),
                "unavailable".to_string(),
                None,
            )
        );
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

    #[test]
    fn vector_only_context_maps_raw_candidates_and_omits_graph_telemetry() {
        let episode_id = deterministic_id("n", "episode", "s1");
        let observation_id = deterministic_id("n", "observation", "s1:turn:1");
        let unmapped_id = deterministic_id("n", "episode", "unmapped");
        let snapshot = VectorNamespaceSnapshot {
            collection_name: "collection".to_string(),
            reverse_episode_ids: BTreeMap::from([(episode_id, "s1".to_string())]),
            reverse_observation_ids: BTreeMap::from([(
                observation_id,
                ("s1:turn:1".to_string(), "s1".to_string()),
            )]),
        };

        let pack = vector_hits_to_context_pack(
            &snapshot,
            vec![
                VectorHit {
                    kind: "episode",
                    object_id: unmapped_id,
                    score: 0.99,
                    text: Some("skip me".to_string()),
                },
                VectorHit {
                    kind: "observation",
                    object_id: observation_id,
                    score: 0.95,
                    text: Some("turn text".to_string()),
                },
                VectorHit {
                    kind: "episode",
                    object_id: episode_id,
                    score: 0.90,
                    text: Some("episode summary".to_string()),
                },
            ],
        );

        assert_eq!(pack.items.len(), 2);
        assert_eq!(pack.items[0].kind, "observation");
        assert_eq!(pack.items[0].external_id.as_deref(), Some("s1:turn:1"));
        assert_eq!(pack.items[0].episode_external_id.as_deref(), Some("s1"));
        assert_eq!(pack.items[0].rank, 1);
        assert_eq!(pack.items[1].kind, "episode");
        assert_eq!(pack.items[1].external_id.as_deref(), Some("s1"));
        assert_eq!(pack.items[1].rank, 2);
        assert_eq!(pack.telemetry.vector_candidate_count, Some(3));
        assert!(!pack.telemetry.trace_available);
        assert_eq!(pack.telemetry.graph_relation_count, None);
        assert_eq!(pack.telemetry.graph_verified_count, None);
        assert!(pack.context_text.contains("turn text"));
        assert!(pack.context_text.contains("episode summary"));
    }

    #[test]
    fn vector_only_context_dedupes_duplicate_surfaces_to_best_score() {
        let observation_id = deterministic_id("n", "observation", "s1:turn:1");
        let snapshot = VectorNamespaceSnapshot {
            collection_name: "collection".to_string(),
            reverse_episode_ids: BTreeMap::new(),
            reverse_observation_ids: BTreeMap::from([(
                observation_id,
                ("s1:turn:1".to_string(), "s1".to_string()),
            )]),
        };

        let pack = vector_hits_to_context_pack(
            &snapshot,
            vec![
                VectorHit {
                    kind: "observation",
                    object_id: observation_id,
                    score: 0.5,
                    text: Some("lower".to_string()),
                },
                VectorHit {
                    kind: "observation",
                    object_id: observation_id,
                    score: 0.8,
                    text: Some("higher".to_string()),
                },
            ],
        );

        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].score, Some(0.8));
        assert_eq!(pack.items[0].text.as_deref(), Some("higher"));
    }

    #[test]
    fn deterministic_query_embeddings_are_stable_and_sized() {
        let provider = DeterministicEmbeddingProvider::new(8).unwrap();
        let first = provider.vector_for_text("Alice likes tea");
        let second = provider.vector_for_text("Alice likes tea");

        assert_eq!(first, second);
        assert_eq!(first.len(), 8);
        assert!(first.iter().any(|value| *value > 0.0));
    }

    #[test]
    fn telemetry_leakage_counts_only_final_returned_items() {
        let returned_id = deterministic_id("n", "episode", "returned");
        let omitted_id = deterministic_id("n", "episode", "omitted");
        let mut trace = RetrievalTrace::empty();
        trace.lifecycle_filter_decisions = vec![
            LifecycleFilterDecision {
                object: MemoryObjectRef::new(ObjectType::Episode, returned_id),
                retention_state: Some(RetentionState::Suppressed),
                is_current: None,
                superseded_by: Vec::new(),
                action: LifecycleFilterAction::Included,
                reason: LifecycleFilterReason::SuppressedIncludedByPolicy,
            },
            LifecycleFilterDecision {
                object: MemoryObjectRef::new(ObjectType::Episode, omitted_id),
                retention_state: Some(RetentionState::Suppressed),
                is_current: None,
                superseded_by: Vec::new(),
                action: LifecycleFilterAction::Included,
                reason: LifecycleFilterReason::SuppressedIncludedByPolicy,
            },
        ];
        let outcome = RetrieveOutcome {
            pack: ContinuityContextPack {
                relevant_episodes: vec![episode(returned_id)],
                ..ContinuityContextPack::empty()
            },
            rationale: RetrievalRationale::new("test"),
            trace: Some(trace),
        };

        let telemetry = telemetry_from_outcome(&outcome);

        assert_eq!(telemetry.suppressed_or_deleted_returned_count, Some(1));
    }

    #[test]
    fn parses_rfc3339_timestamp_and_ignores_empty() {
        assert!(parse_timestamp(None).unwrap().is_none());
        assert!(parse_timestamp(Some("")).unwrap().is_none());
        assert_eq!(
            parse_timestamp(Some("2023-05-30T23:40:00Z"))
                .unwrap()
                .unwrap()
                .to_rfc3339(),
            "2023-05-30T23:40:00+00:00"
        );
    }

    #[test]
    fn rejects_unormalized_timestamp_with_actionable_context() {
        let err = parse_timestamp(Some("2023/05/30 (Tue) 23:40"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("dataset loaders should normalize"));
    }

    #[test]
    fn cleanup_target_requires_eval_prefix_match() {
        validate_cleanup_target("bench_lme_longmemeval_s_v0_1_lme_q1_abc", Some("bench:lme"))
            .unwrap();

        let err = validate_cleanup_target("prod_collection", Some("bench:lme"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("refusing to cleanup"));
    }

    #[test]
    fn cleanup_target_rejects_missing_prefix() {
        let err = validate_cleanup_target("bench_lme_q1", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no required collection prefix"));
    }

    fn episode(id: MemoryId) -> Episode {
        let now = Utc::now();
        Episode {
            id,
            object_type: ObjectType::Episode,
            modality: Modality::Chat,
            source_conversation_id: Some("external".to_string()),
            started_at: None,
            ended_at: None,
            participant_entity_ids: Vec::new(),
            summary: "summary".to_string(),
            raw_ref: Some("external".to_string()),
            salience_score: 0.5,
            retention_state: RetentionState::Active,
            created_at: now,
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        }
    }
}
