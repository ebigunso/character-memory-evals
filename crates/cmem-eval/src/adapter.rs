use crate::fs_util;

use crate::{
    BenchmarkRunConfig, CommitWriteOptions, CommitWriteResult, ContextRenderer,
    ControllableDimensionPolicy, ControllableSimilarityEmbeddingProvider,
    ControllableSimilarityFixture, CorrectMemoryInput, CorrectionTargetInput,
    DeterministicEmbeddingProvider, EmbeddingProviderConfig, EmbeddingRuntimeBinding, EpisodeInput,
    ExternalSourceRefInput, ForgetMemoryInput, FrozenEmbeddingDimensionPolicy,
    FrozenEmbeddingProvider, FrozenEmbeddingSource, GraphEnrichmentInput, LifecycleMutationResult,
    LinkMemoryInput, LinkMemoryResult, LiveEmbeddingProvider, MemoryEndpointInput,
    NamespaceLifecycleResult, ObservationInput, PrepareWriteInput, PreparedWritePlan,
    RecordedOutcome, ReplacementDerivedMemoryInput, RetrievalMode, RetrievalSurfacePolicy,
    RetrieveInput, RetrievedContextPack, RetrievedItem, SourceProvenanceInput, SupersessionResult,
    VectorStoreMode, WriteResult, deterministic_operation_id,
};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use character_memory::{
    ArchivePolicy, CandidateProvenance, CandidateValidation, CandidateValidationStatus,
    CharacterMemory, CommitOptions, ContinuitySectionLimits, CorrectMemoryDraft,
    CorrectionCascadePolicy, CorrectionLifecyclePolicy, CorrectionTarget, DEFAULT_SCHEMA_VERSION,
    DerivedMemoryCandidate, DerivedMemoryDraft, DerivedType, EmbeddingProvider, EntityCandidate,
    EntityDraft, EpisodeCandidate, EpisodeDraft, ExternalSourceReference, ForgetCascadePolicy,
    ForgetLifecyclePolicy, ForgetMemoryDraft, LifecycleMutationOutcome, LifecycleTargetRef,
    MemoryCandidate, MemoryId, MemoryLinkCandidate, MemoryLinkDraft, MemoryObjectDraft,
    MemoryObjectRef, MemoryThreadCandidate, MemoryThreadDraft, ObjectType, ObservationCandidate,
    ObservationDraft, PrepareOptions, RememberInput, RememberOutcome, RememberWritePlan,
    ReplacementDerivedMemoryDraft, RetrievalContext, Settings, SourceObjectCorrectionTarget,
    SourceProvenanceReference, SuppressionPolicy, VectorIndexCandidate,
};
use chrono::{DateTime, Utc};
use qdrant_client::{Qdrant, config::QdrantConfig};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

const UUID_NAMESPACE: Uuid = Uuid::from_u128(0x9b6af7a4_9076_49bb_9231_84d1ed632cf1);
const QDRANT_REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct CharacterMemoryAdapter {
    config: BenchmarkRunConfig,
    embedding_binding: EmbeddingRuntimeBinding,
    qdrant: Option<Qdrant>,
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
    episode_texts: BTreeMap<String, String>,
    observation_texts: BTreeMap<String, String>,
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
        self.save_with_before_persist(path, |_| Ok(()))
    }

    fn save_with_before_persist<F>(&self, path: &Path, before_persist: F) -> Result<()>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        fs_util::atomic_replace_with_before_persist(
            path,
            &bytes,
            "identity registry",
            before_persist,
        )
    }
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
        let provider = match config.backend.embedding.provider {
            EmbeddingProviderConfig::Deterministic => LiveEmbeddingProvider::Deterministic,
            EmbeddingProviderConfig::OpenAi => LiveEmbeddingProvider::OpenAi,
            EmbeddingProviderConfig::ControllableSimilarity | EmbeddingProviderConfig::Frozen => {
                bail!("continuity embedding providers require an explicit runtime binding")
            }
        };
        Self::new_with_binding(
            config,
            EmbeddingRuntimeBinding::Live {
                provider,
                model: config.backend.embedding.model.clone(),
            },
        )
        .await
    }

    pub async fn new_with_binding(
        config: &BenchmarkRunConfig,
        binding: EmbeddingRuntimeBinding,
    ) -> Result<Self> {
        Self::validate_runtime_binding(config, &binding, false)?;
        Self::new_internal(config, binding).await
    }

    pub async fn new_with_controllable_similarity(
        config: &BenchmarkRunConfig,
        fixture: ControllableSimilarityFixture,
    ) -> Result<Self> {
        Self::new_with_controllable_similarity_internal(config, fixture, false).await
    }

    pub async fn new_with_padded_controllable_similarity(
        config: &BenchmarkRunConfig,
        fixture: ControllableSimilarityFixture,
    ) -> Result<Self> {
        Self::new_with_controllable_similarity_internal(config, fixture, true).await
    }

    async fn new_with_controllable_similarity_internal(
        config: &BenchmarkRunConfig,
        fixture: ControllableSimilarityFixture,
        allow_storage_padding: bool,
    ) -> Result<Self> {
        config.validate()?;
        if config.backend.embedding.provider != EmbeddingProviderConfig::ControllableSimilarity {
            bail!(
                "new_with_controllable_similarity requires backend.embedding.provider=controllable_similarity"
            );
        }
        let provider = ControllableSimilarityEmbeddingProvider::new(fixture.clone())?;
        let configured_size = config.backend.embedding.vector_size;
        let valid_size = if allow_storage_padding {
            configured_size.is_some_and(|size| size >= provider.vector_size())
        } else {
            configured_size == Some(provider.vector_size())
        };
        if !valid_size {
            let requirement = if allow_storage_padding {
                "at least"
            } else {
                "exactly"
            };
            bail!(
                "backend.embedding.vector_size must be {requirement} the controllable similarity fixture vector_size {}; got {:?}",
                provider.vector_size(),
                configured_size
            );
        }
        let dimension_policy = if allow_storage_padding {
            ControllableDimensionPolicy::Exact {
                vector_size: configured_size.expect("validated controllable storage size"),
            }
        } else {
            ControllableDimensionPolicy::FixtureDeclared
        };
        Self::new_with_binding(
            config,
            EmbeddingRuntimeBinding::Controllable {
                fixture,
                dimension_policy,
            },
        )
        .await
    }

    pub async fn new_with_frozen_embeddings(config: &BenchmarkRunConfig) -> Result<Self> {
        Self::new_with_frozen_embeddings_internal(config, false).await
    }

    pub async fn new_with_frozen_embedding_provider(
        config: &BenchmarkRunConfig,
        provider: FrozenEmbeddingProvider,
    ) -> Result<Self> {
        Self::new_with_frozen_embedding_provider_internal(config, provider, false).await
    }

    #[cfg(test)]
    async fn new_with_test_frozen_embeddings(config: &BenchmarkRunConfig) -> Result<Self> {
        Self::new_with_frozen_embeddings_internal(config, true).await
    }

    #[cfg(test)]
    async fn new_with_test_frozen_embedding_provider(
        config: &BenchmarkRunConfig,
        provider: FrozenEmbeddingProvider,
    ) -> Result<Self> {
        Self::new_with_frozen_embedding_provider_internal(config, provider, true).await
    }

    async fn new_with_frozen_embeddings_internal(
        config: &BenchmarkRunConfig,
        allow_test_fixture: bool,
    ) -> Result<Self> {
        config.validate()?;
        if config.backend.embedding.provider != EmbeddingProviderConfig::Frozen {
            bail!("new_with_frozen_embeddings requires backend.embedding.provider=frozen");
        }
        let store_path = config
            .backend
            .embedding
            .store_path
            .as_deref()
            .context("frozen embedding provider requires backend.embedding.store_path")?;
        let vector_size = config
            .backend
            .embedding
            .vector_size
            .context("frozen embedding provider requires backend.embedding.vector_size")?;
        let provider = FrozenEmbeddingProvider::load(
            Path::new(store_path),
            &config.backend.embedding.model,
            vector_size,
        )?;
        Self::new_with_frozen_embedding_provider_internal(config, provider, allow_test_fixture)
            .await
    }

    async fn new_with_frozen_embedding_provider_internal(
        config: &BenchmarkRunConfig,
        provider: FrozenEmbeddingProvider,
        allow_test_fixture: bool,
    ) -> Result<Self> {
        config.validate()?;
        if config.backend.embedding.provider != EmbeddingProviderConfig::Frozen {
            bail!("new_with_frozen_embedding_provider requires backend.embedding.provider=frozen");
        }
        let store_path = config
            .backend
            .embedding
            .store_path
            .as_deref()
            .context("frozen embedding provider requires backend.embedding.store_path")?;
        let vector_size = config
            .backend
            .embedding
            .vector_size
            .context("frozen embedding provider requires backend.embedding.vector_size")?;
        if provider.model() != config.backend.embedding.model {
            bail!(
                "frozen embedding provider model {:?} does not match configured model {:?}",
                provider.model(),
                config.backend.embedding.model
            );
        }
        if provider.vector_size() != vector_size {
            bail!(
                "frozen embedding provider vector_size {} does not match configured vector_size {vector_size}",
                provider.vector_size()
            );
        }
        if !allow_test_fixture && provider.source() != FrozenEmbeddingSource::OpenAiApi {
            bail!(
                "live frozen embedding adapter requires source=open_ai_api; store {store_path} declares source={:?}",
                provider.source()
            );
        }
        let dimension_policy = if provider.source() == FrozenEmbeddingSource::TestFixture {
            FrozenEmbeddingDimensionPolicy::TestFixture
        } else {
            crate::classify_frozen_embedding_dimensions(
                provider.model(),
                provider.vector_size(),
                false,
            )?
        };
        Self::validate_runtime_binding(
            config,
            &EmbeddingRuntimeBinding::Frozen {
                store: provider.clone(),
                dimension_policy,
            },
            allow_test_fixture,
        )?;
        Self::new_internal(
            config,
            EmbeddingRuntimeBinding::Frozen {
                store: provider,
                dimension_policy,
            },
        )
        .await
    }

    async fn new_internal(
        config: &BenchmarkRunConfig,
        embedding_binding: EmbeddingRuntimeBinding,
    ) -> Result<Self> {
        let qdrant = if config.backend.vector_store_mode == VectorStoreMode::Service {
            let qdrant_url = config
                .backend
                .qdrant_connection_string
                .clone()
                .or_else(|| env::var("QDRANT_CONNECTION_STRING").ok())
                .context("QDRANT_CONNECTION_STRING is required for live Character Memory runs")?;
            Some(Qdrant::new(
                QdrantConfig::from_url(&qdrant_url)
                    .timeout(Duration::from_secs(QDRANT_REQUEST_TIMEOUT_SECS)),
            )?)
        } else {
            None
        };
        Ok(Self {
            config: config.clone(),
            embedding_binding,
            qdrant,
            namespaces: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn validate_runtime_binding(
        config: &BenchmarkRunConfig,
        binding: &EmbeddingRuntimeBinding,
        allow_test_fixture: bool,
    ) -> Result<()> {
        config.validate()?;
        let configured_size = match config.backend.embedding.vector_size {
            Some(vector_size) => vector_size,
            None => crate::model_native_embedding_vector_size(&config.backend.embedding.model)?,
        };
        match binding {
            EmbeddingRuntimeBinding::Controllable {
                fixture,
                dimension_policy,
            } => {
                ControllableSimilarityEmbeddingProvider::new(fixture.clone())?;
                let valid = match dimension_policy {
                    ControllableDimensionPolicy::FixtureDeclared => {
                        configured_size == fixture.vector_size
                    }
                    ControllableDimensionPolicy::Exact { vector_size } => {
                        *vector_size == configured_size && fixture.vector_size <= configured_size
                    }
                };
                if !valid {
                    bail!(
                        "controllable runtime binding vector_size {} is incompatible with configured storage vector_size {configured_size} and policy {dimension_policy:?}",
                        fixture.vector_size
                    );
                }
            }
            EmbeddingRuntimeBinding::Frozen {
                store,
                dimension_policy,
            } => {
                if store.model() != config.backend.embedding.model
                    || store.vector_size() != configured_size
                {
                    bail!(
                        "frozen runtime binding model/vector ({:?}, {}) does not match configured ({:?}, {configured_size})",
                        store.model(),
                        store.vector_size(),
                        config.backend.embedding.model
                    );
                }
                if !allow_test_fixture && store.source() != FrozenEmbeddingSource::OpenAiApi {
                    bail!("live frozen embedding adapter requires source=open_ai_api");
                }
                if *dimension_policy == FrozenEmbeddingDimensionPolicy::TestFixture
                    && store.source() != FrozenEmbeddingSource::TestFixture
                {
                    bail!(
                        "frozen runtime binding declares test_fixture dimensions for a live store"
                    );
                }
            }
            EmbeddingRuntimeBinding::Live { provider: _, model } => {
                if model != &config.backend.embedding.model {
                    bail!(
                        "live runtime binding model {model:?} does not match configured model {:?}",
                        config.backend.embedding.model
                    );
                }
            }
        }
        Ok(())
    }

    /// Releases every namespace's stores without deleting their durable data.
    pub async fn close(self) -> Result<()> {
        let mut first_error = None;
        for (namespace, state) in self.namespaces.lock().await.drain() {
            if let Err(error) = state
                .memory
                .close()
                .await
                .with_context(|| format!("close namespace {namespace}"))
            {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub async fn reconstruct(
        config: &BenchmarkRunConfig,
        namespace: &str,
    ) -> Result<(Self, NamespaceLifecycleResult)> {
        config.validate()?;
        let adapter = Self::new(config).await?;
        let lifecycle = adapter.reattach_namespace(namespace).await?;
        Ok((adapter, lifecycle))
    }

    pub async fn reconstruct_with_binding(
        config: &BenchmarkRunConfig,
        namespace: &str,
        binding: EmbeddingRuntimeBinding,
    ) -> Result<(Self, NamespaceLifecycleResult)> {
        let adapter = Self::new_with_binding(config, binding).await?;
        let lifecycle = adapter.reattach_namespace(namespace).await?;
        Ok((adapter, lifecycle))
    }

    pub async fn reconstruct_with_controllable_similarity(
        config: &BenchmarkRunConfig,
        namespace: &str,
        fixture: ControllableSimilarityFixture,
    ) -> Result<(Self, NamespaceLifecycleResult)> {
        config.validate()?;
        let adapter = Self::new_with_controllable_similarity(config, fixture).await?;
        let lifecycle = adapter.reattach_namespace(namespace).await?;
        Ok((adapter, lifecycle))
    }

    pub async fn reconstruct_with_padded_controllable_similarity(
        config: &BenchmarkRunConfig,
        namespace: &str,
        fixture: ControllableSimilarityFixture,
    ) -> Result<(Self, NamespaceLifecycleResult)> {
        config.validate()?;
        let adapter = Self::new_with_padded_controllable_similarity(config, fixture).await?;
        let lifecycle = adapter.reattach_namespace(namespace).await?;
        Ok((adapter, lifecycle))
    }

    pub async fn reconstruct_with_frozen_embeddings(
        config: &BenchmarkRunConfig,
        namespace: &str,
    ) -> Result<(Self, NamespaceLifecycleResult)> {
        config.validate()?;
        let adapter = Self::new_with_frozen_embeddings(config).await?;
        let lifecycle = adapter.reattach_namespace(namespace).await?;
        Ok((adapter, lifecycle))
    }

    pub async fn reconstruct_with_frozen_embedding_provider(
        config: &BenchmarkRunConfig,
        namespace: &str,
        provider: FrozenEmbeddingProvider,
    ) -> Result<(Self, NamespaceLifecycleResult)> {
        config.validate()?;
        let adapter = Self::new_with_frozen_embedding_provider(config, provider).await?;
        let lifecycle = adapter.reattach_namespace(namespace).await?;
        Ok((adapter, lifecycle))
    }

    async fn create_namespace_state(
        &self,
        namespace: &str,
        identities: ExternalIdRegistry,
    ) -> Result<NamespaceState> {
        let collection_name = self.collection_name(namespace);
        let identity_registry_path = self.identity_registry_path(namespace);
        // Embedded directories already isolate namespaces; do not repeat long collection names.
        let vector_collection_name = match self.config.backend.vector_store_mode {
            VectorStoreMode::Embedded => "memory",
            VectorStoreMode::Service => &collection_name,
        };
        let settings = self.settings(namespace)?;
        let memory = match &self.embedding_binding {
            EmbeddingRuntimeBinding::Live {
                provider: LiveEmbeddingProvider::Deterministic,
                ..
            } => {
                let vector_size = self.config.backend.embedding.vector_size.unwrap_or(3072);
                CharacterMemory::new_with_embedding_provider(
                    settings,
                    vector_collection_name.to_owned(),
                    Box::new(CharacterMemoryEmbeddingProvider::new(vector_size)?),
                )
                .await?
            }
            EmbeddingRuntimeBinding::Controllable { fixture, .. } => {
                let storage_vector_size = settings.get_embedding_vector_size()?;
                CharacterMemory::new_with_embedding_provider(
                    settings,
                    vector_collection_name.to_owned(),
                    Box::new(CharacterMemoryControllableSimilarityEmbeddingProvider::new(
                        fixture.clone(),
                        storage_vector_size,
                    )?),
                )
                .await?
            }
            EmbeddingRuntimeBinding::Frozen { store, .. } => {
                CharacterMemory::new_with_embedding_provider(
                    settings,
                    vector_collection_name.to_owned(),
                    Box::new(CharacterMemoryFrozenEmbeddingProvider {
                        inner: store.clone(),
                    }),
                )
                .await?
            }
            EmbeddingRuntimeBinding::Live {
                provider: LiveEmbeddingProvider::OpenAi,
                ..
            } => CharacterMemory::new(settings, vector_collection_name.to_owned()).await?,
        };

        Ok(NamespaceState {
            memory,
            collection_name,
            identity_registry_path,
            identities,
        })
    }

    fn settings(&self, namespace: &str) -> Result<Settings> {
        let openai_api_key = env::var(&self.config.backend.openai_api_key_env)
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .unwrap_or_else(|_| {
                if !matches!(
                    &self.embedding_binding,
                    EmbeddingRuntimeBinding::Live {
                        provider: LiveEmbeddingProvider::OpenAi,
                        ..
                    }
                ) {
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
            .set_override("oxigraph_path", "unused-in-memory")?
            .set_override("openai_api_key", openai_api_key)?
            .set_override(
                "embedding_model",
                self.config.backend.embedding.model.clone(),
            )?;
        builder = match self.config.backend.vector_store_mode {
            VectorStoreMode::Embedded => {
                let path = self.vector_store_path(namespace);
                fs::create_dir_all(&path)
                    .with_context(|| format!("create embedded vector store {}", path.display()))?;
                builder
                    .set_override("vector_store_mode", "embedded")?
                    .set_override(
                        "vector_store_path",
                        path.canonicalize()?.to_string_lossy().into_owned(),
                    )?
            }
            VectorStoreMode::Service => builder
                .set_override("vector_store_mode", "service")?
                .set_override("qdrant_connection_string", self.qdrant_connection_string()?)?,
        };
        if let Some(path) = self.oxigraph_persistence_path(namespace) {
            builder = builder
                .set_override("graph_store_mode", "persistent")?
                .set_override("oxigraph_path", path.to_string_lossy().into_owned())?;
        } else {
            builder = builder.set_override("graph_store_mode", "in_memory")?;
        }
        if let Some(path) = self.retrieval_stats_path(namespace) {
            builder = builder
                .set_override("retrieval_stats_store_mode", "sqlite")?
                .set_override("retrieval_stats_path", path.to_string_lossy().into_owned())?;
        }
        if let Some(overrides) = &self.config.backend.character_memory {
            if let Some(alpha) = overrides.selectivity_smoothing_alpha {
                builder = builder.set_override("selectivity_smoothing_alpha", alpha)?;
            }
            if let Some(gamma) = overrides.selectivity_gamma {
                builder = builder.set_override("selectivity_gamma", gamma)?;
            }
            if let Some(fanout) = overrides
                .retrieval
                .as_ref()
                .and_then(|retrieval| retrieval.fanout.as_ref())
            {
                if let Some(budget) = fanout
                    .about_entity
                    .as_ref()
                    .and_then(|target| target.derived_memory.as_ref())
                {
                    builder = builder
                        .set_override(
                            "retrieval.fanout.about_entity.derived_memory.min",
                            u64::try_from(budget.min)?,
                        )?
                        .set_override(
                            "retrieval.fanout.about_entity.derived_memory.max",
                            u64::try_from(budget.max)?,
                        )?;
                }
                if let Some(budget) = fanout
                    .participant_entity
                    .as_ref()
                    .and_then(|target| target.episode.as_ref())
                {
                    builder = builder
                        .set_override(
                            "retrieval.fanout.participant_entity.episode.min",
                            u64::try_from(budget.min)?,
                        )?
                        .set_override(
                            "retrieval.fanout.participant_entity.episode.max",
                            u64::try_from(budget.max)?,
                        )?;
                }
                if let Some(budget) = fanout
                    .part_of_thread
                    .as_ref()
                    .and_then(|target| target.derived_memory.as_ref())
                {
                    builder = builder
                        .set_override(
                            "retrieval.fanout.part_of_thread.derived_memory.min",
                            u64::try_from(budget.min)?,
                        )?
                        .set_override(
                            "retrieval.fanout.part_of_thread.derived_memory.max",
                            u64::try_from(budget.max)?,
                        )?;
                }
            }
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

    /// Configured durable-store paths are roots/templates, never deletion targets. Each
    /// namespace gets a child path derived from the same prefix/run/namespace identity used by
    /// Qdrant and the external-ID registry, so reset cannot erase another namespace's state.
    fn durable_store_identity(&self, namespace: &str) -> String {
        format!(
            "{}-{}-{}",
            sanitize_collection_segment(self.namespace_prefix()),
            sanitize_collection_segment(namespace),
            self.namespace_identity_suffix(namespace).simple()
        )
    }

    fn oxigraph_persistence_path(&self, namespace: &str) -> Option<PathBuf> {
        self.config
            .backend
            .oxigraph_persistence_path
            .as_deref()
            .map(Path::new)
            .map(|root| {
                root.join(format!(
                    "oxigraph-{}",
                    self.durable_store_identity(namespace)
                ))
            })
    }

    fn retrieval_stats_path(&self, namespace: &str) -> Option<PathBuf> {
        self.config
            .backend
            .retrieval_stats_path
            .as_deref()
            .map(Path::new)
            .map(|template| {
                let parent = template.parent().unwrap_or_else(|| Path::new(""));
                let stem = template
                    .file_stem()
                    .filter(|stem| !stem.is_empty())
                    .unwrap_or_else(|| std::ffi::OsStr::new("retrieval-stats"));
                let mut filename = stem.to_os_string();
                filename.push(format!("-{}", self.durable_store_identity(namespace)));
                if let Some(extension) = template.extension() {
                    filename.push(".");
                    filename.push(extension);
                }
                parent.join(filename)
            })
    }

    fn configured_durable_store_paths(&self, namespace: &str) -> Vec<(&'static str, PathBuf)> {
        let mut stores = Vec::new();
        if self.config.backend.vector_store_mode == VectorStoreMode::Embedded {
            stores.push(("embedded vector store", self.vector_store_path(namespace)));
        }
        if let Some(path) = self.oxigraph_persistence_path(namespace) {
            stores.push(("Oxigraph store", path));
        }
        if let Some(path) = self.retrieval_stats_path(namespace) {
            stores.push(("retrieval stats store", path));
        }
        stores
    }

    fn vector_store_path(&self, namespace: &str) -> PathBuf {
        self.identity_registry_path(namespace)
            .parent()
            .expect("identity registry has a parent directory")
            .join(format!(
                "vectors-{}",
                self.namespace_identity_suffix(namespace).simple()
            ))
    }

    async fn delete_collection_with_prefix(
        &self,
        collection_name: &str,
        required_prefix: &str,
    ) -> Result<()> {
        validate_cleanup_target(collection_name, Some(required_prefix))?;
        let Some(qdrant) = &self.qdrant else {
            return Ok(());
        };
        if qdrant
            .collection_exists(collection_name)
            .await
            .with_context(|| format!("check Qdrant collection {collection_name}"))?
        {
            qdrant
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
        let mut namespaces = self.namespaces.lock().await;
        let (collection_name, identity_registry_path) = {
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
        if let Some(state) = namespaces.remove(namespace) {
            state
                .memory
                .close()
                .await
                .with_context(|| format!("close namespace {namespace} before removing stores"))?;
        }
        if identity_registry_path.exists() {
            fs::remove_file(&identity_registry_path).with_context(|| {
                format!(
                    "remove identity registry {}",
                    identity_registry_path.display()
                )
            })?;
        }
        for (store_name, path) in self.configured_durable_store_paths(namespace) {
            remove_namespace_store(&path, store_name)?;
        }
        Ok(())
    }

    async fn retrieve_vector_only(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        let search_plan = vector_only_search_plan(&input.surface_policy)?;
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| explicit_lifecycle_error(&input.namespace))?;
        let mut hits = Vec::new();
        let mut outcomes = Vec::new();
        for (kind, budget) in search_plan {
            let object_type = match kind {
                "episode" => ObjectType::Episode,
                "observation" => ObjectType::Observation,
                _ => unreachable!("validated vector-only object kind"),
            };
            let mut context = RetrievalContext::new(input.query.clone());
            context.include_trace = true;
            context.object_type_defaults = vec![object_type];
            context.candidate_limits.max_vector_candidates = budget
                .checked_mul(character_memory::max_embedding_surfaces(object_type))
                .context("vector-only candidate limit overflow")?;
            let outcome = state.memory.retrieve(context).await?;
            let trace = outcome
                .trace
                .as_ref()
                .context("vector-only retrieval omitted requested trace")?;
            // The library trace orders surfaces by score, then canonical identity.
            let mut seen = HashSet::new();
            for candidate in trace
                .vector_candidates
                .iter()
                .filter(|candidate| seen.insert(candidate.object.id))
                .take(budget)
            {
                if candidate.object.object_type != object_type {
                    bail!("vector-only trace escaped its singleton object scope");
                }
                let object_id = candidate.object.id;
                let text = match object_type {
                    ObjectType::Episode => state
                        .reverse_episode_ids
                        .get(&object_id)
                        .and_then(|id| state.episode_texts.get(id)),
                    ObjectType::Observation => state
                        .reverse_observation_ids
                        .get(&object_id)
                        .and_then(|(id, _)| state.observation_texts.get(id)),
                    _ => unreachable!(),
                }
                .with_context(|| {
                    format!("vector-only candidate {kind}/{object_id} has no ingested text")
                })?;
                hits.push(VectorHit {
                    kind,
                    object_id,
                    score: candidate.score as f64,
                    text: Some(text.clone()),
                });
            }
            outcomes.push(outcome);
        }

        Ok(vector_hits_to_context_pack(
            &state.identities,
            hits,
            outcomes,
        ))
    }
}

fn vector_only_search_plan(
    surface_policy: &RetrievalSurfacePolicy,
) -> Result<Vec<(&'static str, usize)>> {
    surface_policy.validate_for_vector_only()?;
    let mut plan = Vec::new();
    if surface_policy.object_types.contains(&ObjectType::Episode) {
        plan.push(("episode", surface_policy.sections.relevant_episodes));
    }
    if surface_policy
        .object_types
        .contains(&ObjectType::Observation)
    {
        plan.push(("observation", surface_policy.sections.salient_observations));
    }
    Ok(plan)
}

impl CharacterMemoryAdapter {
    pub async fn open_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
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
        if let Some((store_name, path)) = self
            .configured_durable_store_paths(namespace)
            .into_iter()
            .find(|(_, path)| path.exists())
        {
            bail!(
                "{store_name} {} already exists for namespace {namespace}; reset the namespace or use reattach_namespace",
                path.display()
            );
        }
        let collection_name = self.collection_name(namespace);
        if let Some(qdrant) = &self.qdrant
            && qdrant
                .collection_exists(&collection_name)
                .await
                .with_context(|| format!("check Qdrant collection {collection_name}"))?
        {
            bail!(
                "Qdrant collection already exists for namespace {namespace}; reset the namespace or use reattach_namespace"
            );
        }
        let state = self
            .create_namespace_state(namespace, ExternalIdRegistry::new(namespace))
            .await?;
        namespaces.insert(namespace.to_string(), state);
        Ok(NamespaceLifecycleResult {
            namespace: namespace.to_string(),
            restored_identity_count: 0,
        })
    }

    pub async fn reattach_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
        let mut namespaces = self.namespaces.lock().await;
        if namespaces.contains_key(namespace) {
            bail!("namespace is already open: {namespace}");
        }
        let registry_path = self.identity_registry_path(namespace);
        let collection_name = self.collection_name(namespace);
        let collection_exists = if let Some(qdrant) = &self.qdrant {
            qdrant
                .collection_exists(&collection_name)
                .await
                .with_context(|| {
                    format!("check Qdrant collection {collection_name} for reattach")
                })?
        } else {
            true
        };
        let mut missing_stores = Vec::new();
        if !registry_path.exists() {
            missing_stores.push(format!("identity registry {}", registry_path.display()));
        }
        if !collection_exists {
            missing_stores.push(format!("Qdrant collection {collection_name}"));
        }
        for (store_name, path) in self.configured_durable_store_paths(namespace) {
            if !path.exists() {
                missing_stores.push(format!("{store_name} {}", path.display()));
            }
        }
        if !missing_stores.is_empty() {
            bail!(
                "cannot reattach namespace {namespace}; missing durable store(s): {}; reattach requires the identity registry, vector-store state, and every configured namespace-scoped store",
                missing_stores.join(", ")
            );
        }
        let identities = ExternalIdRegistry::load(&registry_path, namespace)?;
        let state = self.create_namespace_state(namespace, identities).await?;
        let restored_identity_count = state.identities.len();
        namespaces.insert(namespace.to_string(), state);
        Ok(NamespaceLifecycleResult {
            namespace: namespace.to_string(),
            restored_identity_count,
        })
    }

    pub async fn detach_namespace(&self, namespace: &str) -> Result<()> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .remove(namespace)
            .ok_or_else(|| explicit_lifecycle_error(namespace))?;
        state
            .memory
            .close()
            .await
            .with_context(|| format!("close namespace {namespace} for detach"))
    }

    pub async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let namespace_prefix = self.namespace_prefix();
        self.reset_namespace_with_prefix(namespace, namespace_prefix)
            .await
    }

    pub async fn cleanup_namespace(&self, namespace: &str) -> Result<()> {
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

    pub async fn remember_episode(&self, input: EpisodeInput) -> Result<WriteResult<String>> {
        let result = self.remember_episodes(vec![input]).await?;
        let [id]: [String; 1] = result
            .value
            .try_into()
            .expect("single-item episode batch returned one id");
        Ok(WriteResult {
            value: id,
            outcome: result.outcome,
        })
    }

    pub async fn remember_episodes(
        &self,
        inputs: Vec<EpisodeInput>,
    ) -> Result<WriteResult<Vec<String>>> {
        if inputs.is_empty() {
            bail!("episode batch must not be empty");
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
        let state = namespaces
            .get_mut(&namespace)
            .ok_or_else(|| explicit_lifecycle_error(&namespace))?;
        let mut objects = Vec::with_capacity(inputs.len());
        let mut ids = Vec::with_capacity(inputs.len());
        for input in inputs {
            let id = deterministic_id(&input.namespace, "episode", &input.external_id);
            let mut draft = EpisodeDraft::new(input.summary.clone());
            draft.id = Some(id);
            draft.source_conversation_id = Some(input.external_id.clone());
            draft.raw_ref = Some(format!(
                "eval://{}/episode/{}",
                input.namespace, input.external_id
            ));
            draft.started_at = parse_timestamp(input.started_at.as_deref())?;
            draft.ended_at = parse_timestamp(input.ended_at.as_deref())?;
            objects.push(MemoryObjectDraft::Episode(draft));
            ids.push((input.external_id, id, input.summary));
        }

        let outcome = commit_typed_drafts(&state.memory, &namespace, objects, Vec::new()).await?;
        for (external_id, id, text) in &ids {
            state
                .episode_texts
                .insert(external_id.clone(), text.clone());
            state.episode_ids.insert(external_id.clone(), *id);
            state.reverse_episode_ids.insert(*id, external_id.clone());
        }
        state.persist_identities()?;

        Ok(WriteResult {
            value: ids.into_iter().map(|(_, id, _)| id.to_string()).collect(),
            outcome,
        })
    }

    pub async fn remember_observation(
        &self,
        input: ObservationInput,
    ) -> Result<WriteResult<String>> {
        let result = self.remember_observations(vec![input]).await?;
        let [id]: [String; 1] = result
            .value
            .try_into()
            .expect("single-item observation batch returned one id");
        Ok(WriteResult {
            value: id,
            outcome: result.outcome,
        })
    }

    pub async fn remember_observations(
        &self,
        inputs: Vec<ObservationInput>,
    ) -> Result<WriteResult<Vec<String>>> {
        if inputs.is_empty() {
            bail!("observation batch must not be empty");
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
            let mut draft = ObservationDraft::new(episode_id, input.text.clone());
            draft.id = Some(id);
            draft.raw_ref = Some(format!(
                "eval://{}/observation/{}",
                input.namespace, input.external_id
            ));
            draft.observed_at = parse_timestamp(input.observed_at.as_deref())?;
            objects.push(MemoryObjectDraft::Observation(draft));
            ids.push((input.external_id, input.episode_external_id, id, input.text));
        }

        let outcome = commit_typed_drafts(&state.memory, &namespace, objects, Vec::new()).await?;
        for (external_id, episode_external_id, id, text) in &ids {
            state
                .observation_texts
                .insert(external_id.clone(), text.clone());
            state.observation_ids.insert(external_id.clone(), *id);
            state
                .reverse_observation_ids
                .insert(*id, (external_id.clone(), episode_external_id.clone()));
        }
        state.persist_identities()?;

        Ok(WriteResult {
            value: ids
                .into_iter()
                .map(|(_, _, id, _)| id.to_string())
                .collect(),
            outcome,
        })
    }

    pub async fn remember_enrichment(
        &self,
        input: GraphEnrichmentInput,
    ) -> Result<Option<RecordedOutcome<RememberOutcome>>> {
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
            let mut draft = EntityDraft::new(entity.entity_type, entity.name);
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
            draft.status = thread.status;
            draft.last_touched_at = parse_timestamp(thread.last_touched_at.as_deref())?;
            draft.salience_score = thread.salience_score;
            draft.canonical_key = thread.canonical_key;
            objects.push(MemoryObjectDraft::MemoryThread(draft));
        }

        for memory in input.derived_memories {
            let id = pending_derived[&memory.external_id];
            let mut draft = DerivedMemoryDraft::new(memory.derived_type, memory.text);
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
            draft.stability = memory.stability;
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
            let mut draft = MemoryLinkDraft::new(from_type, from_id, link.relation, to_type, to_id);
            let id = deterministic_id(&input.namespace, "memory_link", &link.external_id);
            draft.id = Some(id);
            draft.confidence = link.confidence;
            draft.rationale = link.rationale;
            pending_links.insert(link.external_id, id);
            links.push(draft);
        }

        if objects.is_empty() && links.is_empty() {
            return Ok(None);
        }

        let outcome = commit_typed_drafts(&state.memory, &input.namespace, objects, links).await?;
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
        Ok(Some(outcome))
    }

    pub async fn link(
        &self,
        input: LinkMemoryInput,
    ) -> Result<WriteResult<LinkMemoryResult, character_memory::LinkOutcome>> {
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
        let mut draft =
            MemoryLinkDraft::new(from_type, from_id, input.link.relation, to_type, to_id);
        let id = deterministic_id(&input.namespace, "memory_link", &input.link.external_id);
        draft.id = Some(id);
        draft.confidence = input.link.confidence;
        draft.rationale = input.link.rationale;
        let link_outcome = state.memory.link(draft).await?;
        let link_id = link_outcome.link.id;
        state
            .link_ids
            .insert(input.link.external_id.clone(), link_id);
        state
            .reverse_link_ids
            .insert(link_id, input.link.external_id.clone());
        state.persist_identities()?;
        let value = LinkMemoryResult {
            internal_id: link_id.to_string(),
            external_id: input.link.external_id,
        };
        let outcome = RecordedOutcome {
            operation_id: deterministic_operation_id(
                &input.namespace,
                "link",
                [value.external_id.as_str()],
            ),
            outcome: link_outcome,
        };
        Ok(WriteResult { value, outcome })
    }

    pub async fn correct(&self, input: CorrectMemoryInput) -> Result<LifecycleMutationResult> {
        let operation_identity = serde_json::to_string(&input)
            .context("serialize correction input for deterministic operation identity")?;
        let operation_id =
            deterministic_operation_id(&input.namespace, "correct", [operation_identity.as_str()]);
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
        lifecycle_result(state, outcome, operation_id)
    }

    pub async fn forget(&self, input: ForgetMemoryInput) -> Result<LifecycleMutationResult> {
        let operation_identity = serde_json::to_string(&input)
            .context("serialize forget input for deterministic operation identity")?;
        let operation_id =
            deterministic_operation_id(&input.namespace, "forget", [operation_identity.as_str()]);
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
            target_retention_state: input.target_retention_state,
            target_thread_status: input.target_thread_status,
            include_trace: input.include_trace,
        };
        let outcome = state.memory.forget(draft).await?;
        lifecycle_result(state, outcome, operation_id)
    }

    pub async fn prepare(&self, input: PrepareWriteInput) -> Result<PreparedWritePlan> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| explicit_lifecycle_error(&input.namespace))?;
        let episode_id = deterministic_id(&input.namespace, "episode", &input.episode_external_id);
        let observation_id = deterministic_id(
            &input.namespace,
            "observation",
            &input.observation_external_id,
        );
        let (episode, observation) = staged_source_drafts(&input, episode_id, observation_id)?;
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
        Ok(PreparedWritePlan {
            namespace: input.namespace.clone(),
            input,
            plan: backend_plan,
        })
    }

    pub async fn validate_plan(
        &self,
        plan: &PreparedWritePlan,
    ) -> Result<Vec<CandidateValidation>> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&plan.namespace)
            .ok_or_else(|| anyhow!("namespace has no prepared state: {}", plan.namespace))?;
        Ok(state.memory.validate_plan(&plan.plan).await?)
    }

    pub async fn commit(
        &self,
        plan: PreparedWritePlan,
        options: CommitWriteOptions,
    ) -> Result<CommitWriteResult> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&plan.namespace)
            .ok_or_else(|| anyhow!("namespace has no prepared state: {}", plan.namespace))?;
        let backend_plan = plan.plan;
        let operation_id = backend_plan.idempotency_key.clone();
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
            state.episode_texts.insert(
                plan.input.episode_external_id.clone(),
                plan.input.content.clone(),
            );
            state
                .episode_ids
                .insert(plan.input.episode_external_id.clone(), episode_id);
            state
                .reverse_episode_ids
                .insert(episode_id, plan.input.episode_external_id.clone());
        }
        if outcome.persisted_object_ids.contains(&observation_id) {
            state.observation_texts.insert(
                plan.input.observation_external_id.clone(),
                plan.input.content.clone(),
            );
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
        let persisted_object_refs = outcome
            .persisted_object_ids
            .iter()
            .filter_map(|id| external_endpoint_for_id(state, *id))
            .collect::<Vec<_>>();
        let persisted_link_external_ids = outcome
            .persisted_link_ids
            .iter()
            .filter_map(|id| state.reverse_link_ids.get(id).cloned())
            .collect::<Vec<_>>();
        let vector_indexed_object_refs = outcome
            .vector_indexed_object_ids
            .iter()
            .filter_map(|id| external_endpoint_for_id(state, *id))
            .collect::<Vec<_>>();
        Ok(CommitWriteResult {
            persisted_object_refs,
            persisted_link_external_ids,
            vector_indexed_object_refs,
            repair_needed: outcome.repair_needed.clone(),
            outcome: RecordedOutcome {
                operation_id,
                outcome,
            },
        })
    }

    pub async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        match input.mode {
            RetrievalMode::Bm25Only => bail!("BM25 retrieval uses the ingested-text baseline"),
            RetrievalMode::VectorOnly => return self.retrieve_vector_only(input).await,
            RetrievalMode::Hybrid => {}
        }
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| explicit_lifecycle_error(&input.namespace))?;

        let mut context = RetrievalContext::new(input.query);
        context.include_trace = input.surface_policy.include_debug_rationale;
        if let Some(max_vector_candidates) = input.surface_policy.max_vector_candidates {
            context.candidate_limits.max_vector_candidates = max_vector_candidates;
        }
        if let Some(max_graph_roots) = input.surface_policy.max_graph_roots {
            context.candidate_limits.max_graph_roots = max_graph_roots;
        }
        let sections = input.surface_policy.sections;
        context.section_limits = ContinuitySectionLimits {
            active_threads: sections.active_threads,
            relevant_episodes: sections.relevant_episodes,
            salient_observations: sections.salient_observations,
            derived_memories: sections.derived_memories,
            preferences: sections.preferences,
            relationship_notes: sections.relationship_notes,
            open_loops: sections.open_loops,
            commitments: sections.commitments,
            character_signals: sections.character_signals,
        };
        context.object_type_defaults = input.surface_policy.object_types;

        let outcome = state.memory.retrieve(context).await?;
        Ok(flatten_outcome(state, outcome))
    }
}

fn flatten_outcome(
    state: &ExternalIdRegistry,
    outcome: character_memory::RetrieveOutcome,
) -> RetrievedContextPack {
    let mut trace_score_by_id: HashMap<MemoryId, f64> = HashMap::new();
    if let Some(trace) = &outcome.trace {
        for candidate in &trace.vector_candidates {
            trace_score_by_id.insert(candidate.object.id, candidate.score as f64);
        }
    }

    let mut items = Vec::new();
    for thread in &outcome.pack.active_threads {
        items.push(RetrievedItem {
            kind: ObjectType::MemoryThread,
            internal_id: thread.id.to_string(),
            external_id: state.reverse_thread_ids.get(&thread.id).cloned(),
            episode_external_id: None,
            score: trace_score_by_id.get(&thread.id).copied(),
            rank: items.len() + 1,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(thread.summary.clone()),
        });
    }

    for episode in &outcome.pack.relevant_episodes {
        let external_id = state
            .reverse_episode_ids
            .get(&episode.id)
            .cloned()
            .or(episode.source_conversation_id.clone());
        items.push(RetrievedItem {
            kind: ObjectType::Episode,
            internal_id: episode.id.to_string(),
            external_id,
            episode_external_id: None,
            score: trace_score_by_id.get(&episode.id).copied(),
            rank: items.len() + 1,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(episode.summary.clone()),
        });
    }

    for observation in &outcome.pack.salient_observations {
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
        items.push(RetrievedItem {
            kind: ObjectType::Observation,
            internal_id: observation.id.to_string(),
            external_id,
            episode_external_id,
            score: trace_score_by_id.get(&observation.id).copied(),
            rank: items.len() + 1,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(observation.text.clone()),
        });
    }

    let mut seen_derived = HashSet::new();
    for derived in outcome
        .pack
        .derived_memories
        .iter()
        .chain(&outcome.pack.preferences)
        .chain(&outcome.pack.relationship_notes)
        .chain(&outcome.pack.open_loops)
        .chain(&outcome.pack.commitments)
        .chain(&outcome.pack.character_signals)
    {
        if !seen_derived.insert(derived.memory.id) {
            continue;
        }
        items.push(RetrievedItem {
            kind: ObjectType::DerivedMemory,
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
            score: trace_score_by_id.get(&derived.memory.id).copied(),
            rank: items.len() + 1,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(derived.memory.text.clone()),
        });
    }

    RetrievedContextPack::from_ranked_items(items, vec![outcome], ContextRenderer::WithIdentity)
}

fn vector_hits_to_context_pack(
    snapshot: &ExternalIdRegistry,
    hits: Vec<VectorHit>,
    outcomes: Vec<character_memory::RetrieveOutcome>,
) -> RetrievedContextPack {
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
                    kind: ObjectType::Episode,
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
                    kind: ObjectType::Observation,
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

    RetrievedContextPack::from_ranked_items(items, outcomes, ContextRenderer::WithIdentity)
}

#[derive(Debug, PartialEq, Eq)]
struct RememberTopology {
    object_ids: Vec<MemoryId>,
    link_ids: Vec<MemoryId>,
    vector_ids: Vec<MemoryId>,
}

async fn commit_typed_drafts(
    memory: &CharacterMemory,
    namespace: &str,
    object_drafts: Vec<MemoryObjectDraft>,
    link_drafts: Vec<MemoryLinkDraft>,
) -> Result<crate::RecordedOutcome<crate::RememberOutcome>> {
    let (plan, expected) =
        typed_remember_plan_at(namespace, object_drafts, link_drafts, Utc::now())?;
    let validations = memory.validate_plan(&plan).await?;
    let invalid = validations
        .iter()
        .filter(|validation| validation.status == CandidateValidationStatus::Invalid)
        .map(|validation| {
            format!(
                "candidate {} ({:?}): {}",
                validation.candidate_index,
                validation.candidate_kind,
                validation
                    .errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        bail!(
            "typed remember plan validation failed: {}",
            invalid.join(" | ")
        );
    }

    let operation_id = plan.idempotency_key.clone();
    let outcome = memory.commit(plan, CommitOptions::default()).await?;
    validate_remember_topology(&outcome, &expected)?;
    Ok(RecordedOutcome {
        operation_id,
        outcome,
    })
}

fn typed_remember_plan_at(
    namespace: &str,
    object_drafts: Vec<MemoryObjectDraft>,
    link_drafts: Vec<MemoryLinkDraft>,
    committed_at: DateTime<Utc>,
) -> Result<(RememberWritePlan, RememberTopology)> {
    let provenance = CandidateProvenance::caller("CharacterMemoryEvals typed ingest");
    let mut object_candidates = Vec::new();
    let mut link_candidates = Vec::new();
    let mut vector_candidates = Vec::new();
    let mut object_ids = Vec::new();
    let mut link_ids = Vec::new();
    let mut vector_ids = Vec::new();

    for draft in object_drafts {
        let draft = complete_typed_draft(draft, committed_at)?;
        match draft {
            MemoryObjectDraft::Episode(draft) => {
                let id = required_draft_id(draft.id, "episode")?;
                object_ids.push(id);
                vector_ids.push(id);
                vector_candidates.push(MemoryCandidate::VectorIndex(VectorIndexCandidate::new(
                    MemoryObjectRef::new(ObjectType::Episode, id),
                    prefixed_embedding_text("Episode summary", &draft.summary),
                    provenance.clone(),
                )));
                object_candidates.push(MemoryCandidate::Episode(EpisodeCandidate::new(
                    draft,
                    provenance.clone(),
                )));
            }
            MemoryObjectDraft::Observation(draft) => {
                let id = required_draft_id(draft.id, "observation")?;
                object_ids.push(id);
                vector_ids.push(id);
                vector_candidates.push(MemoryCandidate::VectorIndex(VectorIndexCandidate::new(
                    MemoryObjectRef::new(ObjectType::Observation, id),
                    prefixed_embedding_text("Observation excerpt", &draft.text),
                    provenance.clone(),
                )));
                object_candidates.push(MemoryCandidate::Observation(ObservationCandidate::new(
                    draft,
                    provenance.clone(),
                )));
            }
            MemoryObjectDraft::Entity(draft) => {
                let id = required_draft_id(draft.id, "entity")?;
                let aliases = if draft.aliases.is_empty() {
                    String::new()
                } else {
                    format!("Aliases: {}", draft.aliases.join(", "))
                };
                let content = join_embedding_text([
                    draft.name.as_str(),
                    aliases.as_str(),
                    draft.summary.as_deref().unwrap_or_default(),
                ]);
                object_ids.push(id);
                vector_ids.push(id);
                vector_candidates.push(MemoryCandidate::VectorIndex(VectorIndexCandidate::new(
                    MemoryObjectRef::new(ObjectType::Entity, id),
                    prefixed_embedding_text("Entity", &content),
                    provenance.clone(),
                )));
                object_candidates.push(MemoryCandidate::Entity(EntityCandidate::new(
                    draft,
                    provenance.clone(),
                )));
            }
            MemoryObjectDraft::MemoryThread(draft) => {
                let id = required_draft_id(draft.id, "memory thread")?;
                let content = join_embedding_text([draft.title.as_str(), draft.summary.as_str()]);
                object_ids.push(id);
                vector_ids.push(id);
                vector_candidates.push(MemoryCandidate::VectorIndex(VectorIndexCandidate::new(
                    MemoryObjectRef::new(ObjectType::MemoryThread, id),
                    prefixed_embedding_text("Thread summary", &content),
                    provenance.clone(),
                )));
                object_candidates.push(MemoryCandidate::MemoryThread(MemoryThreadCandidate::new(
                    draft,
                    provenance.clone(),
                )));
            }
            MemoryObjectDraft::DerivedMemory(draft) => {
                let id = required_draft_id(draft.id, "derived memory")?;
                object_ids.push(id);
                vector_ids.push(id);
                vector_candidates.push(MemoryCandidate::VectorIndex(VectorIndexCandidate::new(
                    MemoryObjectRef::new(ObjectType::DerivedMemory, id),
                    prefixed_embedding_text(
                        derived_embedding_label(draft.derived_type),
                        &draft.text,
                    ),
                    provenance.clone(),
                )));
                object_candidates.push(MemoryCandidate::DerivedMemory(
                    DerivedMemoryCandidate::new(draft, provenance.clone()),
                ));
            }
            MemoryObjectDraft::MemoryLink(draft) => {
                let id = required_draft_id(draft.id, "memory link")?;
                link_ids.push(id);
                link_candidates.push(MemoryCandidate::MemoryLink(MemoryLinkCandidate::new(
                    draft,
                    provenance.clone(),
                )));
            }
        }
    }

    for draft in link_drafts {
        let MemoryObjectDraft::MemoryLink(draft) =
            complete_typed_draft(MemoryObjectDraft::MemoryLink(draft), committed_at)?
        else {
            unreachable!("memory link draft remains a memory link")
        };
        let id = required_draft_id(draft.id, "memory link")?;
        link_ids.push(id);
        link_candidates.push(MemoryCandidate::MemoryLink(MemoryLinkCandidate::new(
            draft,
            provenance.clone(),
        )));
    }

    let topology_key = object_ids
        .iter()
        .chain(&link_ids)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\0");
    let operation_id = deterministic_id(namespace, "remember_plan", &topology_key);
    let mut plan = RememberWritePlan::new(operation_id, format!("cmem-eval:{operation_id}"));
    for candidate in object_candidates
        .into_iter()
        .chain(link_candidates)
        .chain(vector_candidates)
    {
        plan = plan.with_candidate(candidate);
    }

    Ok((
        plan,
        RememberTopology {
            object_ids,
            link_ids,
            vector_ids,
        },
    ))
}

fn complete_typed_draft(
    mut draft: MemoryObjectDraft,
    committed_at: DateTime<Utc>,
) -> Result<MemoryObjectDraft> {
    match &mut draft {
        MemoryObjectDraft::Episode(draft) => {
            required_draft_id(draft.id, "episode")?;
            draft.created_at.get_or_insert(committed_at);
            draft
                .schema_version
                .get_or_insert_with(|| DEFAULT_SCHEMA_VERSION.to_owned());
        }
        MemoryObjectDraft::Observation(draft) => {
            required_draft_id(draft.id, "observation")?;
            draft.created_at.get_or_insert(committed_at);
            draft
                .schema_version
                .get_or_insert_with(|| DEFAULT_SCHEMA_VERSION.to_owned());
        }
        MemoryObjectDraft::Entity(draft) => {
            required_draft_id(draft.id, "entity")?;
            let created_at = *draft.created_at.get_or_insert(committed_at);
            draft.updated_at.get_or_insert(created_at);
            draft
                .schema_version
                .get_or_insert_with(|| DEFAULT_SCHEMA_VERSION.to_owned());
        }
        MemoryObjectDraft::MemoryThread(draft) => {
            required_draft_id(draft.id, "memory thread")?;
            let created_at = *draft.created_at.get_or_insert(committed_at);
            let updated_at = *draft.updated_at.get_or_insert(created_at);
            draft.last_touched_at.get_or_insert(updated_at);
            draft
                .schema_version
                .get_or_insert_with(|| DEFAULT_SCHEMA_VERSION.to_owned());
        }
        MemoryObjectDraft::DerivedMemory(draft) => {
            required_draft_id(draft.id, "derived memory")?;
            let created_at = *draft.created_at.get_or_insert(committed_at);
            draft.updated_at.get_or_insert(created_at);
            draft
                .schema_version
                .get_or_insert_with(|| DEFAULT_SCHEMA_VERSION.to_owned());
        }
        MemoryObjectDraft::MemoryLink(draft) => {
            required_draft_id(draft.id, "memory link")?;
            draft.created_at.get_or_insert(committed_at);
            draft
                .schema_version
                .get_or_insert_with(|| DEFAULT_SCHEMA_VERSION.to_owned());
        }
    }
    Ok(draft)
}

fn required_draft_id(id: Option<MemoryId>, kind: &str) -> Result<MemoryId> {
    id.with_context(|| format!("CharacterMemoryEvals {kind} draft must have a deterministic ID"))
}

fn validate_remember_topology(
    outcome: &RememberOutcome,
    expected: &RememberTopology,
) -> Result<()> {
    if outcome.persisted_object_ids != expected.object_ids {
        bail!(
            "typed remember persisted object topology changed: expected {:?}, got {:?}",
            expected.object_ids,
            outcome.persisted_object_ids
        );
    }
    if outcome.persisted_link_ids != expected.link_ids {
        bail!(
            "typed remember persisted link topology changed: expected {:?}, got {:?}",
            expected.link_ids,
            outcome.persisted_link_ids
        );
    }
    if let Some(failure) = &outcome.vector_indexing_failure {
        if failure.unindexed_object_ids() != expected.vector_ids {
            bail!(
                "typed remember failed vector topology changed: expected {:?}, got {:?}",
                expected.vector_ids,
                failure.unindexed_object_ids()
            );
        }
    } else if outcome.vector_indexed_object_ids != expected.vector_ids {
        bail!(
            "typed remember vector topology changed: expected {:?}, got {:?}",
            expected.vector_ids,
            outcome.vector_indexed_object_ids
        );
    }
    Ok(())
}

fn prefixed_embedding_text(label: &str, text: &str) -> String {
    let text = clean_embedding_text(text);
    if text.is_empty() {
        label.to_owned()
    } else {
        format!("{label}: {text}")
    }
}

fn join_embedding_text<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .map(clean_embedding_text)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn clean_embedding_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

const fn derived_embedding_label(derived_type: DerivedType) -> &'static str {
    match derived_type {
        DerivedType::Reflection => "Reflection",
        DerivedType::UserPreference => "User preference",
        DerivedType::AssistantPreference => "Assistant preference",
        DerivedType::Commitment => "Commitment",
        DerivedType::OpenLoop => "Open loop",
        DerivedType::CharacterSignal => "Character signal",
        DerivedType::RelationshipNote => "Relationship note",
        DerivedType::ProjectNote => "Project note",
        DerivedType::Claim => "Claim",
        DerivedType::Correction => "Correction",
    }
}

fn deterministic_id(namespace: &str, kind: &str, external_id: &str) -> MemoryId {
    Uuid::new_v5(
        &UUID_NAMESPACE,
        format!("{namespace}\0{kind}\0{external_id}").as_bytes(),
    )
}

fn explicit_lifecycle_error(namespace: &str) -> anyhow::Error {
    anyhow!(
        "namespace is not open: {namespace}; call open_namespace or reattach_namespace before adapter operations"
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

fn staged_source_drafts(
    input: &PrepareWriteInput,
    episode_id: MemoryId,
    observation_id: MemoryId,
) -> Result<(EpisodeDraft, ObservationDraft)> {
    let mut episode = EpisodeDraft::new(input.content.clone());
    episode.id = Some(episode_id);
    episode.source_conversation_id = Some(input.episode_external_id.clone());
    episode.started_at = parse_timestamp(input.episode_started_at.as_deref())?;
    episode.raw_ref = input.raw_refs.first().cloned().or_else(|| {
        Some(format!(
            "eval://{}/episode/{}",
            input.namespace, input.episode_external_id
        ))
    });
    let mut observation = ObservationDraft::new(episode_id, input.content.clone());
    observation.id = Some(observation_id);
    observation.observed_at = parse_timestamp(input.observation_observed_at.as_deref())?;
    observation.raw_ref = input.raw_refs.first().cloned().or_else(|| {
        Some(format!(
            "eval://{}/observation/{}",
            input.namespace, input.observation_external_id
        ))
    });
    Ok((episode, observation))
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
    let object_type = endpoint.object_type;
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
            let target = match object_type {
                ObjectType::Episode => SourceObjectCorrectionTarget::Episode {
                    id: *state
                        .episode_ids
                        .get(external_id)
                        .ok_or_else(|| anyhow!("unknown episode external_id {external_id}"))?,
                    original_raw_ref: original_raw_ref.clone(),
                    original_source_ref: original_source_ref.clone(),
                },
                ObjectType::Observation => SourceObjectCorrectionTarget::Observation {
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
    match target.object_type {
        ObjectType::Episode => state
            .episode_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::episode),
        ObjectType::Observation => state
            .observation_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::observation),
        ObjectType::DerivedMemory => state
            .derived_memory_ids
            .get(&target.external_id)
            .copied()
            .map(LifecycleTargetRef::derived_memory),
        ObjectType::MemoryThread => state
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
    let mut draft = ReplacementDerivedMemoryDraft::new(memory.derived_type, memory.text.clone());
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
    draft.stability = memory.stability;
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
    operation_id: String,
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
        .iter()
        .flat_map(|trace| &trace.superseded_by)
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
        .collect::<Vec<_>>();
    Ok(LifecycleMutationResult {
        mutated_object_refs,
        mutated_link_external_ids,
        vector_maintained_object_refs,
        superseded,
        outcome: RecordedOutcome {
            operation_id,
            outcome,
        },
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
        ObjectType::Episode => (ObjectType::Episode, episodes.get(&id)?.clone()),
        ObjectType::Observation => (ObjectType::Observation, observations.get(&id)?.0.clone()),
        ObjectType::Entity => (ObjectType::Entity, entities.get(&id)?.clone()),
        ObjectType::MemoryThread => (ObjectType::MemoryThread, threads.get(&id)?.clone()),
        ObjectType::DerivedMemory => (
            ObjectType::DerivedMemory,
            derived_memories.get(&id)?.clone(),
        ),
        ObjectType::MemoryLink => (ObjectType::MemoryLink, links.get(&id)?.clone()),
    };
    Some(MemoryEndpointInput {
        object_type,
        external_id,
    })
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

fn remove_namespace_store(path: &Path, store_name: &str) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("remove {store_name} directory {}", path.display()))?;
    } else if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("remove {store_name} file {}", path.display()))?;
    }
    if store_name == "retrieval stats store" {
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = path.as_os_str().to_os_string();
            sidecar.push(suffix);
            let sidecar = PathBuf::from(sidecar);
            if sidecar.exists() {
                fs::remove_file(&sidecar).with_context(|| {
                    format!("remove retrieval stats store sidecar {}", sidecar.display())
                })?;
            }
        }
    }
    Ok(())
}

struct CharacterMemoryEmbeddingProvider {
    inner: DeterministicEmbeddingProvider,
}

struct CharacterMemoryControllableSimilarityEmbeddingProvider {
    inner: ControllableSimilarityEmbeddingProvider,
    storage_vector_size: usize,
}

struct CharacterMemoryFrozenEmbeddingProvider {
    inner: FrozenEmbeddingProvider,
}

impl CharacterMemoryEmbeddingProvider {
    fn new(vector_size: usize) -> Result<Self> {
        Ok(Self {
            inner: DeterministicEmbeddingProvider::new(vector_size)?,
        })
    }
}

impl CharacterMemoryControllableSimilarityEmbeddingProvider {
    fn new(fixture: ControllableSimilarityFixture, storage_vector_size: usize) -> Result<Self> {
        let inner = ControllableSimilarityEmbeddingProvider::new(fixture)?;
        if inner.vector_size() > storage_vector_size {
            bail!(
                "controllable similarity fixture vector_size {} exceeds configured storage vector size {storage_vector_size}",
                inner.vector_size()
            );
        }
        Ok(Self {
            inner,
            storage_vector_size,
        })
    }

    fn vector_for_text(&self, text: &str) -> Result<Vec<f32>> {
        let mut vector = self.inner.vector_for_text(text).or_else(|original_error| {
            let fixture_text = [
                "Episode summary: ",
                "Observation excerpt: ",
                "Reflection: ",
                "Entity: ",
                "Thread summary: ",
            ]
            .into_iter()
            .find_map(|prefix| text.strip_prefix(prefix));
            fixture_text
                .map(|fixture_text| self.inner.vector_for_text(fixture_text))
                .unwrap_or(Err(original_error))
        })?;
        vector.resize(self.storage_vector_size, 0.0);
        Ok(vector)
    }
}

impl CharacterMemoryFrozenEmbeddingProvider {
    fn vector_for_text(&self, text: &str) -> Result<Vec<f32>> {
        self.inner.vector_for_text(text).or_else(|original_error| {
            runtime_fixture_text(text)
                .map(|fixture_text| self.inner.vector_for_text(fixture_text))
                .unwrap_or(Err(original_error))
        })
    }
}

fn runtime_fixture_text(text: &str) -> Option<&str> {
    [
        "Episode summary: ",
        "Observation excerpt: ",
        "Reflection: ",
        "Entity: ",
        "Thread summary: ",
    ]
    .into_iter()
    .find_map(|prefix| text.strip_prefix(prefix))
}

#[async_trait]
impl EmbeddingProvider for CharacterMemoryEmbeddingProvider {
    fn vector_size(&self) -> usize {
        self.inner.vector_size()
    }

    async fn generate_embedding<'a>(
        &self,
        text: &'a str,
    ) -> std::result::Result<Vec<f32>, character_memory::EmbeddingError> {
        Ok(self.inner.vector_for_text(text))
    }

    async fn bulk_generate_embeddings<'a>(
        &self,
        texts: &'a [&'a str],
    ) -> std::result::Result<Vec<Vec<f32>>, character_memory::EmbeddingError> {
        Ok(texts
            .iter()
            .map(|text| self.inner.vector_for_text(text))
            .collect())
    }
}

#[async_trait]
impl EmbeddingProvider for CharacterMemoryControllableSimilarityEmbeddingProvider {
    fn vector_size(&self) -> usize {
        self.storage_vector_size
    }

    async fn generate_embedding<'a>(
        &self,
        text: &'a str,
    ) -> std::result::Result<Vec<f32>, character_memory::EmbeddingError> {
        self.vector_for_text(text)
            .map_err(|error| character_memory::EmbeddingError::Unrecognized {
                detail: error.to_string(),
            })
    }

    async fn bulk_generate_embeddings<'a>(
        &self,
        texts: &'a [&'a str],
    ) -> std::result::Result<Vec<Vec<f32>>, character_memory::EmbeddingError> {
        texts
            .iter()
            .map(|text| {
                self.vector_for_text(text).map_err(|error| {
                    character_memory::EmbeddingError::Unrecognized {
                        detail: error.to_string(),
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl EmbeddingProvider for CharacterMemoryFrozenEmbeddingProvider {
    fn vector_size(&self) -> usize {
        self.inner.vector_size()
    }

    async fn generate_embedding<'a>(
        &self,
        text: &'a str,
    ) -> std::result::Result<Vec<f32>, character_memory::EmbeddingError> {
        self.vector_for_text(text)
            .map_err(|error| character_memory::EmbeddingError::Unrecognized {
                detail: error.to_string(),
            })
    }

    async fn bulk_generate_embeddings<'a>(
        &self,
        texts: &'a [&'a str],
    ) -> std::result::Result<Vec<Vec<f32>>, character_memory::EmbeddingError> {
        texts
            .iter()
            .map(|text| {
                self.vector_for_text(text).map_err(|error| {
                    character_memory::EmbeddingError::Unrecognized {
                        detail: error.to_string(),
                    }
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RetrievalSectionBudgets;
    use crate::{
        CleanupConfig, DatasetId, DerivedMemoryInput, EmbeddingConfig, EntityInput,
        FrozenEmbeddingStore, MemoryLinkInput, RetrievalSurfacePolicy,
    };
    use character_memory::{
        CURRENT_SCHEMA_VERSION, ContinuityContextPack, EntityType, Episode, LifecycleFilterAction,
        LifecycleFilterDecision, LifecycleFilterReason, MemoryObjectRef, Modality, RelationType,
        RetentionState, RetrievalRationale, RetrievalTrace, RetrieveOutcome, Stability,
        VectorCandidateTrace, VectorSurface,
    };
    use std::io::Write;
    use std::process::Command;
    use tempfile::tempdir;

    static LIVE_QDRANT_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn adapter_config(run_id: String, namespace_prefix: String) -> BenchmarkRunConfig {
        let mut backend = crate::BackendConfig {
            vector_store_mode: VectorStoreMode::Embedded,
            namespace_prefix: Some(namespace_prefix.clone()),
            qdrant_connection_string: Some(
                env::var("QDRANT_CONNECTION_STRING")
                    .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string()),
            ),
            cleanup: CleanupConfig {
                enabled: true,
                require_collection_prefix: Some(namespace_prefix),
            },
            embedding: EmbeddingConfig {
                provider: EmbeddingProviderConfig::Deterministic,
                vector_size: Some(3072),
                ..EmbeddingConfig::default()
            },
            ..crate::BackendConfig::default()
        };
        backend.openai_api_key_env = "CMEM_EVAL_UNUSED_OPENAI_KEY".to_string();
        BenchmarkRunConfig {
            run_id,
            dataset: DatasetId::new("locomo").unwrap(),
            backend,
            retrieval: Default::default(),
            ingest: crate::IngestConfig::default(),
            metrics: Default::default(),
        }
    }

    #[test]
    fn oxigraph_env_cannot_redirect_graph_path() {
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "adapter::tests::oxigraph_env_cannot_redirect_graph_path_probe",
                "--nocapture",
            ])
            .env("CMEM_EVAL_OXIGRAPH_REDIRECT_PROBE", "1")
            .env("OXIGRAPH_PATH", "redirected-by-env")
            .output()
            .unwrap();

        assert!(output.status.success(), "{output:?}");
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("1 passed")
        );
    }

    #[tokio::test]
    async fn embedded_vector_only_keeps_singleton_budgets_and_ingest_text() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        let directory = tempfile::tempdir_in(std::path::absolute(root).unwrap()).unwrap();
        let mut config = adapter_config("vector-budgets".into(), "cmem_eval_budgets".into());
        config.retrieval.mode = RetrievalMode::VectorOnly;
        config.retrieval.surface_policy =
            retrieval_surface_policy(1, 2, false, false, false, false);
        config.backend.vector_store_mode = VectorStoreMode::Embedded;
        config.backend.qdrant_connection_string = None;
        config.backend.identity_registry_dir =
            Some(directory.path().join("identities").display().to_string());
        config.backend.retrieval_stats_path =
            Some(directory.path().join("stats.sqlite").display().to_string());
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
        adapter.open_namespace("n").await.unwrap();
        for id in ["a", "b", "c"] {
            let episode = adapter
                .remember_episode(EpisodeInput {
                    external_id: id.into(),
                    namespace: "n".into(),
                    summary: "same text".into(),
                    started_at: None,
                    ended_at: None,
                    participants: Vec::new(),
                    metadata: serde_json::json!({}),
                })
                .await
                .unwrap();
            assert!(episode.outcome.outcome.vector_indexing_failure.is_none());
            let observation = adapter
                .remember_observation(ObservationInput {
                    external_id: id.into(),
                    episode_external_id: id.into(),
                    namespace: "n".into(),
                    speaker: None,
                    text: "same text".into(),
                    observed_at: None,
                    metadata: serde_json::json!({}),
                })
                .await
                .unwrap();
            assert!(
                observation
                    .outcome
                    .outcome
                    .vector_indexing_failure
                    .is_none()
            );
        }
        let mut query = RetrieveInput {
            mode: RetrievalMode::VectorOnly,
            namespace: "n".into(),
            query: "same text".into(),
            query_date: None,
            surface_policy: retrieval_surface_policy(1, 2, false, false, false, false),
        };
        let result = adapter.retrieve(query.clone()).await.unwrap();
        assert_eq!(result.items().len(), 3);
        assert!(
            result
                .items()
                .iter()
                .all(|item| item.text.as_deref() == Some("same text"))
        );
        assert_eq!(
            result
                .outcomes()
                .iter()
                .map(|outcome| outcome.rationale.telemetry.configured_object_types.clone())
                .collect::<Vec<_>>(),
            vec![vec![ObjectType::Episode], vec![ObjectType::Observation]]
        );
        assert!(
            result
                .outcomes()
                .iter()
                .all(|outcome| outcome.trace.is_some())
        );
        for (kind, budget) in [(ObjectType::Episode, 1), (ObjectType::Observation, 2)] {
            let actual: Vec<_> = result
                .items()
                .iter()
                .filter(|item| item.kind == kind)
                .map(|item| item.internal_id.clone())
                .collect();
            let mut expected: Vec<_> = ["a", "b", "c"]
                .iter()
                .map(|id| {
                    deterministic_id(
                        "n",
                        if kind == ObjectType::Episode {
                            "episode"
                        } else {
                            "observation"
                        },
                        id,
                    )
                    .to_string()
                })
                .collect();
            expected.sort();
            expected.truncate(budget);
            assert_eq!(actual, expected);
        }
        query.surface_policy.sections.relevant_episodes = 0;
        query.surface_policy.sections.salient_observations = 0;
        assert!(adapter.retrieve(query).await.is_err());
        adapter.detach_namespace("n").await.unwrap();
        adapter.cleanup_namespace("n").await.unwrap();
        directory.close().unwrap();
    }

    #[tokio::test]
    async fn embedded_namespace_survives_restart_and_cleanup_is_isolated() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        let directory = tempfile::tempdir_in(std::path::absolute(root).unwrap()).unwrap();
        let mut config = adapter_config("embedded-lifecycle".into(), "cmem_eval_embedded".into());
        config.backend.vector_store_mode = VectorStoreMode::Embedded;
        config.backend.qdrant_connection_string = None;
        config.backend.identity_registry_dir =
            Some(directory.path().join("identities").display().to_string());
        config.backend.oxigraph_persistence_path =
            Some(directory.path().join("oxigraph").display().to_string());
        config.backend.retrieval_stats_path =
            Some(directory.path().join("stats.sqlite").display().to_string());
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
        let vector_a = adapter.vector_store_path("a");
        let vector_b = adapter.vector_store_path("b");
        let stores: Vec<_> = ["a", "b"]
            .into_iter()
            .flat_map(|namespace| adapter.configured_durable_store_paths(namespace))
            .collect();
        assert_ne!(vector_a, vector_b);
        for namespace in ["a", "b"] {
            adapter.open_namespace(namespace).await.unwrap();
            assert_eq!(adapter.namespaces.lock().await.len(), 1);
            adapter
                .remember_episode(EpisodeInput {
                    external_id: "episode".into(),
                    namespace: namespace.into(),
                    summary: "The notebook is blue.".into(),
                    started_at: Some("2025-01-01T00:00:00Z".into()),
                    ended_at: None,
                    participants: Vec::new(),
                    metadata: serde_json::json!({}),
                })
                .await
                .unwrap();
            adapter.detach_namespace(namespace).await.unwrap();
            assert!(adapter.namespaces.lock().await.is_empty());
        }
        assert!(vector_a.exists());
        assert!(vector_b.exists());
        adapter.reattach_namespace("b").await.unwrap();
        let query = RetrieveInput {
            mode: RetrievalMode::Hybrid,
            namespace: "b".into(),
            query: "The notebook is blue.".into(),
            query_date: Some("2025-01-02T00:00:00Z".into()),
            surface_policy: retrieval_surface_policy(8, 0, false, false, false, true),
        };
        let before = adapter.retrieve(query.clone()).await.unwrap();
        assert!(!before.items().is_empty());
        assert!(matches!(
            before.outcomes()[0]
                .rationale
                .telemetry
                .vector_recall_completeness,
            character_memory::VectorRecallCompleteness::Exhaustive { .. }
        ));
        let mut vector_query = query.clone();
        vector_query.mode = RetrievalMode::VectorOnly;
        vector_query.surface_policy.object_types = vec![ObjectType::Episode];
        let vector_before = adapter.retrieve(vector_query.clone()).await.unwrap();
        assert_eq!(
            vector_before.items()[0].text.as_deref(),
            Some("The notebook is blue.")
        );
        adapter.detach_namespace("b").await.unwrap();
        assert!(adapter.namespaces.lock().await.is_empty());
        assert!(adapter.retrieve(query.clone()).await.is_err());
        assert!(vector_b.exists());
        adapter.close().await.unwrap();
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
        adapter.reattach_namespace("b").await.unwrap();
        let after = adapter.retrieve(query).await.unwrap();
        assert_eq!(before.items(), after.items());
        assert_eq!(
            vector_before.items(),
            adapter.retrieve(vector_query).await.unwrap().items()
        );
        adapter.cleanup_namespace("a").await.unwrap();
        assert!(!vector_a.exists());
        assert!(vector_b.exists());
        adapter.cleanup_namespace("b").await.unwrap();
        assert!(!vector_b.exists());
        for (store_name, path) in stores {
            assert!(!path.exists(), "{store_name} remains: {}", path.display());
        }
        adapter.open_namespace("b").await.unwrap();
        adapter.reset_namespace("b").await.unwrap();
        assert!(!vector_b.exists());
        directory.close().unwrap();
    }

    #[tokio::test]
    async fn oxigraph_env_cannot_redirect_graph_path_probe() {
        if env::var_os("CMEM_EVAL_OXIGRAPH_REDIRECT_PROBE").is_none() {
            return;
        }
        let config = adapter_config("env-probe".into(), "bench:env-probe".into());
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();

        assert_eq!(
            adapter
                .settings("namespace")
                .unwrap()
                .get_oxigraph_path()
                .unwrap(),
            PathBuf::from("unused-in-memory")
        );
    }

    fn retrieval_surface_policy(
        top_k_episodes: usize,
        top_k_observations: usize,
        include_derived_memories: bool,
        include_threads: bool,
        include_entities: bool,
        include_debug_rationale: bool,
    ) -> RetrievalSurfacePolicy {
        let mut sections = RetrievalSectionBudgets {
            relevant_episodes: top_k_episodes,
            salient_observations: top_k_observations,
            ..RetrievalSectionBudgets::default()
        };
        if !include_derived_memories {
            sections.derived_memories = 0;
            sections.preferences = 0;
            sections.relationship_notes = 0;
            sections.open_loops = 0;
            sections.commitments = 0;
            sections.character_signals = 0;
        }
        if !include_threads {
            sections.active_threads = 0;
        }
        let mut object_types = vec![ObjectType::Episode, ObjectType::Observation];
        if include_derived_memories {
            object_types.push(ObjectType::DerivedMemory);
        }
        if include_threads {
            object_types.push(ObjectType::MemoryThread);
        }
        if include_entities {
            object_types.push(ObjectType::Entity);
        }
        RetrievalSurfacePolicy {
            sections,
            object_types,
            include_debug_rationale,
            max_vector_candidates: None,
            max_graph_roots: None,
        }
    }

    #[test]
    fn explicit_vector_size_skips_model_width_lookup_for_runtime_bindings() {
        let mut config = adapter_config(
            "custom-embedding-model".to_string(),
            "cmem_eval_custom_embedding_model".to_string(),
        );
        config.backend.embedding.provider = EmbeddingProviderConfig::OpenAi;
        config.backend.embedding.model = "future-custom-embedding-model".to_string();
        config.backend.embedding.vector_size = Some(2_048);
        let binding = EmbeddingRuntimeBinding::Live {
            provider: LiveEmbeddingProvider::OpenAi,
            model: config.backend.embedding.model.clone(),
        };

        CharacterMemoryAdapter::validate_runtime_binding(&config, &binding, false).unwrap();
    }

    #[test]
    fn vector_only_search_plan_honors_the_selected_supported_object_types() {
        let mut policy = retrieval_surface_policy(3, 5, false, false, false, false);
        policy.object_types = vec![ObjectType::Observation];
        assert_eq!(
            vector_only_search_plan(&policy).unwrap(),
            vec![("observation", 5)]
        );

        policy.object_types = vec![ObjectType::Episode];
        assert_eq!(
            vector_only_search_plan(&policy).unwrap(),
            vec![("episode", 3)]
        );
    }

    #[tokio::test]
    async fn frozen_provider_uses_exact_fixture_text_after_runtime_prefixes() {
        let store = FrozenEmbeddingStore::new(
            "task21-smoke-model",
            FrozenEmbeddingSource::TestFixture,
            [(
                "The cobalt notebook is in the east cabinet.".to_string(),
                vec![1.0, 0.0, 0.0],
            )],
        )
        .unwrap();
        let provider = CharacterMemoryFrozenEmbeddingProvider {
            inner: FrozenEmbeddingProvider::from_store(
                store,
                "fixtures/smoke.json",
                "task21-smoke-model",
                3,
            )
            .unwrap(),
        };

        assert_eq!(
            provider
                .generate_embedding("Episode summary: The cobalt notebook is in the east cabinet.",)
                .await
                .unwrap(),
            vec![1.0, 0.0, 0.0]
        );
        let error = provider
            .generate_embedding("Episode summary: This text is absent.")
            .await
            .unwrap_err();
        let character_memory::EmbeddingError::Unrecognized { detail } = error else {
            panic!("frozen provider returned a non-Unrecognized embedding error: {error:?}");
        };
        assert!(detail.contains("frozen embedding cache miss"), "{detail}");
        assert!(detail.contains("cmem-eval embeddings generate"), "{detail}");
    }

    #[tokio::test]
    async fn live_frozen_construction_and_reconstruction_reject_test_fixture_provenance() {
        let directory = tempdir().unwrap();
        let store_path = directory.path().join("test-fixture-store.json");
        let store = FrozenEmbeddingStore::new(
            "text-embedding-3-small",
            FrozenEmbeddingSource::TestFixture,
            [("fixture text".to_string(), vec![0.0; 1536])],
        )
        .unwrap();
        fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();
        let mut config = adapter_config(
            "frozen-provenance".to_string(),
            "cmem_eval_frozen_provenance".to_string(),
        );
        config.backend.embedding.provider = EmbeddingProviderConfig::Frozen;
        config.backend.embedding.model = "text-embedding-3-small".to_string();
        config.backend.embedding.vector_size = Some(1536);
        config.backend.embedding.store_path = Some(store_path.display().to_string());
        let provider =
            FrozenEmbeddingProvider::load(&store_path, "text-embedding-3-small", 1536).unwrap();

        let error = match CharacterMemoryAdapter::new_with_frozen_embeddings(&config).await {
            Ok(_) => panic!("live construction admitted test-fixture provenance"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("source=open_ai_api"), "{error}");
        assert!(error.contains("TestFixture"), "{error}");
        assert!(error.contains(&store_path.display().to_string()), "{error}");

        let error = match CharacterMemoryAdapter::reconstruct_with_frozen_embeddings(
            &config,
            "continuity-frozen-provenance",
        )
        .await
        {
            Ok(_) => panic!("live reconstruction admitted test-fixture provenance"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("source=open_ai_api"), "{error}");
        assert!(error.contains("TestFixture"), "{error}");
        assert!(error.contains(&store_path.display().to_string()), "{error}");

        let error = match CharacterMemoryAdapter::new_with_frozen_embedding_provider(
            &config,
            provider.clone(),
        )
        .await
        {
            Ok(_) => panic!("provider construction admitted test-fixture provenance"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("source=open_ai_api"), "{error}");
        assert!(error.contains("TestFixture"), "{error}");
        assert!(error.contains(&store_path.display().to_string()), "{error}");

        let error = match CharacterMemoryAdapter::reconstruct_with_frozen_embedding_provider(
            &config,
            "continuity-frozen-provider-provenance",
            provider.clone(),
        )
        .await
        {
            Ok(_) => panic!("provider reconstruction admitted test-fixture provenance"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("source=open_ai_api"), "{error}");
        assert!(error.contains("TestFixture"), "{error}");
        assert!(error.contains(&store_path.display().to_string()), "{error}");

        CharacterMemoryAdapter::new_with_test_frozen_embeddings(&config)
            .await
            .expect("the cfg(test)-only constructor should admit explicit test provenance");
        CharacterMemoryAdapter::new_with_test_frozen_embedding_provider(&config, provider)
            .await
            .expect(
                "the cfg(test)-only provider constructor should admit explicit test provenance",
            );
    }

    fn file_contains(path: &Path, needle: &[u8]) -> bool {
        fs::read(path)
            .unwrap()
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn path_with_appended_suffix(path: &Path, suffix: &str) -> PathBuf {
        let mut value = path.as_os_str().to_os_string();
        value.push(suffix);
        PathBuf::from(value)
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

    #[test]
    fn typed_plan_preserves_batch_enrichment_commit_topology_and_surfaces() {
        let namespace = "typed-plan-equivalence";
        let committed_at = DateTime::parse_from_rfc3339("2026-07-21T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let episode_a_id = deterministic_id(namespace, "episode", "episode-a");
        let episode_b_id = deterministic_id(namespace, "episode", "episode-b");
        let observation_a_id = deterministic_id(namespace, "observation", "observation-a");
        let observation_b_id = deterministic_id(namespace, "observation", "observation-b");
        let entity_id = deterministic_id(namespace, "entity", "entity");
        let thread_id = deterministic_id(namespace, "memory_thread", "thread");
        let derived_id = deterministic_id(namespace, "derived_memory", "derived");
        let link_id = deterministic_id(namespace, "memory_link", "link");

        let mut episode_a = EpisodeDraft::new("Episode   one");
        episode_a.id = Some(episode_a_id);
        let mut episode_b = EpisodeDraft::new("Episode two");
        episode_b.id = Some(episode_b_id);
        let mut observation_a = ObservationDraft::new(episode_a_id, "Observation   one");
        observation_a.id = Some(observation_a_id);
        let mut observation_b = ObservationDraft::new(episode_b_id, "Observation two");
        observation_b.id = Some(observation_b_id);
        let mut entity = EntityDraft::new(EntityType::User, "Kohta");
        entity.id = Some(entity_id);
        entity.aliases = vec!["K".to_string(), "Ko".to_string()];
        entity.summary = Some("Fixture   owner".to_string());
        let mut thread = MemoryThreadDraft::new("Continuity", "Thread   summary");
        thread.id = Some(thread_id);
        let mut derived = DerivedMemoryDraft::new(DerivedType::Reflection, "Stable   insight");
        derived.id = Some(derived_id);
        derived.derived_from_episode_ids = vec![episode_a_id];
        derived.derived_from_observation_ids = vec![observation_a_id];
        derived.thread_ids = vec![thread_id];
        derived.entity_ids = vec![entity_id];
        let mut link = MemoryLinkDraft::new(
            ObjectType::DerivedMemory,
            derived_id,
            RelationType::About,
            ObjectType::Entity,
            entity_id,
        );
        link.id = Some(link_id);

        let mut expected_episode_a = episode_a.clone();
        expected_episode_a.created_at = Some(committed_at);
        expected_episode_a.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_episode_b = episode_b.clone();
        expected_episode_b.created_at = Some(committed_at);
        expected_episode_b.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_observation_a = observation_a.clone();
        expected_observation_a.created_at = Some(committed_at);
        expected_observation_a.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_observation_b = observation_b.clone();
        expected_observation_b.created_at = Some(committed_at);
        expected_observation_b.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_entity = entity.clone();
        expected_entity.created_at = Some(committed_at);
        expected_entity.updated_at = Some(committed_at);
        expected_entity.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_thread = thread.clone();
        expected_thread.created_at = Some(committed_at);
        expected_thread.updated_at = Some(committed_at);
        expected_thread.last_touched_at = Some(committed_at);
        expected_thread.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_derived = derived.clone();
        expected_derived.created_at = Some(committed_at);
        expected_derived.updated_at = Some(committed_at);
        expected_derived.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());
        let mut expected_link = link.clone();
        expected_link.created_at = Some(committed_at);
        expected_link.schema_version = Some(DEFAULT_SCHEMA_VERSION.to_owned());

        let (plan, topology) = typed_remember_plan_at(
            namespace,
            vec![
                MemoryObjectDraft::Episode(episode_a),
                MemoryObjectDraft::Episode(episode_b),
                MemoryObjectDraft::Observation(observation_a),
                MemoryObjectDraft::Observation(observation_b),
                MemoryObjectDraft::Entity(entity),
                MemoryObjectDraft::MemoryThread(thread),
                MemoryObjectDraft::DerivedMemory(derived),
            ],
            vec![link],
            committed_at,
        )
        .unwrap();

        assert_eq!(
            topology,
            RememberTopology {
                object_ids: vec![
                    episode_a_id,
                    episode_b_id,
                    observation_a_id,
                    observation_b_id,
                    entity_id,
                    thread_id,
                    derived_id,
                ],
                link_ids: vec![link_id],
                vector_ids: vec![
                    episode_a_id,
                    episode_b_id,
                    observation_a_id,
                    observation_b_id,
                    entity_id,
                    thread_id,
                    derived_id,
                ],
            }
        );

        let provenance = CandidateProvenance::caller("CharacterMemoryEvals typed ingest");
        let expected_object_and_link_candidates = vec![
            MemoryCandidate::Episode(EpisodeCandidate::new(
                expected_episode_a,
                provenance.clone(),
            )),
            MemoryCandidate::Episode(EpisodeCandidate::new(
                expected_episode_b,
                provenance.clone(),
            )),
            MemoryCandidate::Observation(ObservationCandidate::new(
                expected_observation_a,
                provenance.clone(),
            )),
            MemoryCandidate::Observation(ObservationCandidate::new(
                expected_observation_b,
                provenance.clone(),
            )),
            MemoryCandidate::Entity(EntityCandidate::new(expected_entity, provenance.clone())),
            MemoryCandidate::MemoryThread(MemoryThreadCandidate::new(
                expected_thread,
                provenance.clone(),
            )),
            MemoryCandidate::DerivedMemory(DerivedMemoryCandidate::new(
                expected_derived,
                provenance.clone(),
            )),
            MemoryCandidate::MemoryLink(MemoryLinkCandidate::new(expected_link, provenance)),
        ];
        assert_eq!(
            &plan.candidates[..expected_object_and_link_candidates.len()],
            expected_object_and_link_candidates.as_slice()
        );

        let vector_candidates = plan
            .candidates
            .iter()
            .filter_map(|candidate| match candidate {
                MemoryCandidate::VectorIndex(candidate) => Some((
                    candidate.target.object_type,
                    candidate.target.id,
                    candidate.embedding_text.as_str(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            vector_candidates,
            vec![
                (
                    ObjectType::Episode,
                    episode_a_id,
                    "Episode summary: Episode one"
                ),
                (
                    ObjectType::Episode,
                    episode_b_id,
                    "Episode summary: Episode two"
                ),
                (
                    ObjectType::Observation,
                    observation_a_id,
                    "Observation excerpt: Observation one"
                ),
                (
                    ObjectType::Observation,
                    observation_b_id,
                    "Observation excerpt: Observation two"
                ),
                (
                    ObjectType::Entity,
                    entity_id,
                    "Entity: Kohta Aliases: K, Ko Fixture owner"
                ),
                (
                    ObjectType::MemoryThread,
                    thread_id,
                    "Thread summary: Continuity Thread summary"
                ),
                (
                    ObjectType::DerivedMemory,
                    derived_id,
                    "Reflection: Stable insight"
                ),
            ]
        );
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
        let directory = tempdir().unwrap();
        let mut first_config = adapter_config("run-a".to_string(), "cmem_eval_task3".to_string());
        first_config.backend.oxigraph_persistence_path = Some(
            directory
                .path()
                .join("oxigraph-root")
                .to_string_lossy()
                .into_owned(),
        );
        first_config.backend.retrieval_stats_path = Some(
            directory
                .path()
                .join("retrieval.sqlite")
                .to_string_lossy()
                .into_owned(),
        );
        let first_with_stores = CharacterMemoryAdapter::new(&first_config).await.unwrap();
        let oxigraph_path = first_with_stores
            .oxigraph_persistence_path("namespace")
            .unwrap();
        let stats_path = first_with_stores.retrieval_stats_path("namespace").unwrap();
        assert_eq!(
            oxigraph_path.parent(),
            Some(directory.path().join("oxigraph-root").as_path())
        );
        assert!(oxigraph_path.to_string_lossy().contains(&shared_suffix));
        assert_eq!(stats_path.parent(), Some(directory.path()));
        assert!(stats_path.to_string_lossy().contains(&shared_suffix));
        assert_eq!(
            stats_path.extension().and_then(|value| value.to_str()),
            Some("sqlite")
        );
        validate_cleanup_target(&first.collection_name("namespace"), Some("cmem_eval_task3"))
            .unwrap();
    }

    #[tokio::test]
    async fn matched_deterministic_dimension_satisfies_construction_contract() {
        let mut config = adapter_config(
            "dimension-contract".to_string(),
            "cmem_eval_dimension".to_string(),
        );
        config.backend.embedding.model = "text-embedding-3-small".to_string();
        config.backend.embedding.vector_size = Some(1536);
        config.validate().unwrap();

        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
        let settings = adapter.settings("namespace").unwrap();
        let provider = CharacterMemoryEmbeddingProvider::new(1536).unwrap();
        assert_eq!(settings.get_embedding_vector_size().unwrap(), 1536);
        assert_eq!(provider.vector_size(), 1536);
    }

    #[tokio::test]
    async fn character_memory_run_overrides_reach_settings_and_absence_preserves_defaults() {
        let default_config = adapter_config(
            "settings-defaults".to_string(),
            "cmem_eval_settings_defaults".to_string(),
        );
        let default_adapter = CharacterMemoryAdapter::new(&default_config).await.unwrap();
        let default_settings = default_adapter.settings("namespace").unwrap();
        assert_eq!(default_settings.get_selectivity_smoothing_alpha(), 1.0);
        assert_eq!(default_settings.get_selectivity_gamma(), 1.0);

        let mut overridden_config = adapter_config(
            "settings-overrides".to_string(),
            "cmem_eval_settings_overrides".to_string(),
        );
        overridden_config.backend.character_memory = Some(
            serde_json::from_value(serde_json::json!({
                "selectivity_smoothing_alpha": 2.0,
                "selectivity_gamma": 0.5,
                "retrieval": {
                    "fanout": {
                        "about_entity": {"derived_memory": {"min": 2, "max": 8}},
                        "participant_entity": {"episode": {"min": 1, "max": 3}},
                        "part_of_thread": {"derived_memory": {"min": 4, "max": 9}}
                    }
                }
            }))
            .unwrap(),
        );
        overridden_config.validate().unwrap();
        let overridden_adapter = CharacterMemoryAdapter::new(&overridden_config)
            .await
            .unwrap();
        let overridden_settings = overridden_adapter.settings("namespace").unwrap();
        assert_eq!(overridden_settings.get_selectivity_smoothing_alpha(), 2.0);
        assert_eq!(overridden_settings.get_selectivity_gamma(), 0.5);

        for (field, overrides) in [
            (
                "retrieval.fanout.about_entity.derived_memory",
                serde_json::json!({
                    "retrieval": {"fanout": {
                        "about_entity": {"derived_memory": {"min": 9, "max": 8}}
                    }}
                }),
            ),
            (
                "retrieval.fanout.participant_entity.episode",
                serde_json::json!({
                    "retrieval": {"fanout": {
                        "participant_entity": {"episode": {"min": 9, "max": 8}}
                    }}
                }),
            ),
            (
                "retrieval.fanout.part_of_thread.derived_memory",
                serde_json::json!({
                    "retrieval": {"fanout": {
                        "part_of_thread": {"derived_memory": {"min": 9, "max": 8}}
                    }}
                }),
            ),
        ] {
            let mut invalid_config = adapter_config(
                "settings-invalid".to_string(),
                "cmem_eval_settings_invalid".to_string(),
            );
            invalid_config.backend.character_memory =
                Some(serde_json::from_value(overrides).unwrap());
            let error = match CharacterMemoryAdapter::new(&invalid_config).await {
                Ok(_) => panic!("invalid fanout range was admitted for {field}"),
                Err(error) => error.to_string(),
            };
            assert!(error.contains(field), "{field}: {error}");
        }
    }

    #[tokio::test]
    async fn controllable_similarity_provider_preserves_fixture_prefix_at_storage_width() {
        let fixture = ControllableSimilarityFixture {
            seed: 7,
            vector_size: 2,
            noise_magnitude: 0.0,
            clusters: BTreeMap::from([("cluster".to_string(), vec![1.0, -1.0])]),
            concepts: BTreeMap::from([(
                "concept".to_string(),
                crate::SimilarityConceptFixture {
                    cluster: "cluster".to_string(),
                    inputs: vec!["fixture text".to_string()],
                },
            )]),
        };
        let provider =
            CharacterMemoryControllableSimilarityEmbeddingProvider::new(fixture, 1536).unwrap();

        let vector = provider.generate_embedding("fixture text").await.unwrap();
        assert_eq!(provider.vector_size(), 1536);
        assert_eq!(&vector[..2], &[1.0, -1.0]);
        assert!(vector[2..].iter().all(|component| *component == 0.0));
        for surface_text in [
            "Episode summary: fixture text",
            "Observation excerpt: fixture text",
            "Reflection: fixture text",
            "Entity: fixture text",
            "Thread summary: fixture text",
        ] {
            assert_eq!(
                provider.generate_embedding(surface_text).await.unwrap(),
                vector
            );
        }
        let error = provider
            .generate_embedding("unassigned text")
            .await
            .unwrap_err();
        let character_memory::EmbeddingError::Unrecognized { detail } = error else {
            panic!("controllable provider returned a non-Unrecognized embedding error: {error:?}");
        };
        assert!(detail.contains("no assignment"));
    }

    #[tokio::test]
    async fn controllable_similarity_construction_requires_matching_fixture_dimension() {
        let mut config = adapter_config(
            "controllable-contract".to_string(),
            "cmem_eval_controllable".to_string(),
        );
        config.backend.embedding.provider = EmbeddingProviderConfig::ControllableSimilarity;
        config.backend.embedding.vector_size = Some(3);
        let fixture = ControllableSimilarityFixture {
            seed: 7,
            vector_size: 2,
            noise_magnitude: 0.0,
            clusters: BTreeMap::from([("cluster".to_string(), vec![1.0, -1.0])]),
            concepts: BTreeMap::from([(
                "concept".to_string(),
                crate::SimilarityConceptFixture {
                    cluster: "cluster".to_string(),
                    inputs: vec!["fixture text".to_string()],
                },
            )]),
        };

        let error = match CharacterMemoryAdapter::new_with_controllable_similarity(
            &config,
            fixture.clone(),
        )
        .await
        {
            Ok(_) => panic!("mismatched fixture dimension was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("fixture vector_size 2"));
        assert!(error.contains("Some(3)"));

        CharacterMemoryAdapter::new_with_padded_controllable_similarity(&config, fixture)
            .await
            .expect("mixed-provider storage padding should be accepted explicitly");
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

        let mut updated = registry.clone();
        let replacement = deterministic_id("namespace", "episode", "replacement");
        updated
            .episode_ids
            .insert("replacement".to_string(), replacement);
        updated
            .reverse_episode_ids
            .insert(replacement, "replacement".to_string());
        let expected_updated = updated.clone();
        let mut staged_path = None;
        let error = updated
            .save_with_before_persist(&path, |temporary_path| {
                staged_path = Some(temporary_path.to_path_buf());
                assert_eq!(temporary_path.parent(), path.parent());
                assert_eq!(fs::read_to_string(&path).unwrap(), first);
                assert_eq!(
                    ExternalIdRegistry::load(temporary_path, "namespace").unwrap(),
                    expected_updated
                );
                bail!("simulated interruption before atomic registry replacement")
            })
            .unwrap_err();
        assert!(error.to_string().contains("simulated interruption"));
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        assert!(!staged_path.unwrap().exists());

        updated.save(&path).unwrap();
        assert_eq!(
            ExternalIdRegistry::load(&path, "namespace").unwrap(),
            updated
        );
    }

    #[test]
    fn identity_registry_persist_retries_permission_denied_with_same_staged_bytes() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("identity.json");
        fs::write(&path, b"old complete registry\n").unwrap();
        let staged_bytes = b"new complete registry\n";
        let mut temporary = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        temporary.write_all(staged_bytes).unwrap();
        temporary.as_file().sync_all().unwrap();
        let mut attempts = 0;

        fs_util::persist_with_retry(temporary, &path, "identity registry", |temporary, path| {
            attempts += 1;
            assert_eq!(fs::read(temporary.path()).unwrap(), staged_bytes);
            if attempts == 1 {
                return Err(tempfile::PersistError {
                    error: std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected Windows replace contention",
                    ),
                    file: temporary,
                });
            }
            temporary.persist(path)
        })
        .unwrap();

        assert_eq!(attempts, 2);
        assert_eq!(fs::read(&path).unwrap(), staged_bytes);
    }

    #[tokio::test]
    async fn operational_calls_require_explicit_lifecycle_with_surviving_registry() {
        let directory = tempdir().unwrap();
        let namespace = "explicit-lifecycle";
        let mut config = adapter_config(
            "explicit-lifecycle-run".to_string(),
            "cmem_eval_explicit_lifecycle".to_string(),
        );
        config.backend.identity_registry_dir = Some(
            directory
                .path()
                .join("identities")
                .to_string_lossy()
                .into_owned(),
        );
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
        let registry_path = adapter.identity_registry_path(namespace);
        let mut registry = ExternalIdRegistry::new(namespace);
        let existing_id = deterministic_id(namespace, "episode", "existing");
        registry
            .episode_ids
            .insert("existing".to_string(), existing_id);
        registry
            .reverse_episode_ids
            .insert(existing_id, "existing".to_string());
        registry.save(&registry_path).unwrap();

        let mut errors = Vec::new();
        errors.push(
            adapter
                .remember_episode(EpisodeInput {
                    external_id: "new-episode".to_string(),
                    namespace: namespace.to_string(),
                    summary: "must not attach implicitly".to_string(),
                    started_at: None,
                    ended_at: None,
                    participants: Vec::new(),
                    metadata: serde_json::Value::Null,
                })
                .await
                .unwrap_err(),
        );
        errors.push(
            adapter
                .prepare(PrepareWriteInput {
                    namespace: namespace.to_string(),
                    content: "must not attach implicitly".to_string(),
                    episode_external_id: "new-episode".to_string(),
                    observation_external_id: "new-observation".to_string(),
                    episode_started_at: None,
                    observation_observed_at: None,
                    raw_refs: Vec::new(),
                    idempotency_key: None,
                    include_vector_index_candidates: true,
                    include_stats_update_candidates: true,
                })
                .await
                .unwrap_err(),
        );
        for mode in [RetrievalMode::Hybrid, RetrievalMode::VectorOnly] {
            errors.push(
                adapter
                    .retrieve(RetrieveInput {
                        mode,
                        namespace: namespace.to_string(),
                        query: "must not attach implicitly".to_string(),
                        query_date: None,
                        surface_policy: retrieval_surface_policy(4, 4, false, false, false, false),
                    })
                    .await
                    .unwrap_err(),
            );
        }

        for error in errors {
            let message = error.to_string();
            assert!(message.contains("namespace is not open"));
            assert!(message.contains("open_namespace or reattach_namespace"));
        }
        assert!(adapter.namespaces.lock().await.is_empty());
        assert_eq!(
            ExternalIdRegistry::load(&registry_path, namespace).unwrap(),
            registry
        );
    }

    #[tokio::test]
    async fn reconstruct_validates_config_before_qdrant_or_store_io() {
        let mut config = adapter_config(
            "invalid-reconstruct".to_string(),
            "cmem_eval_invalid_reconstruct".to_string(),
        );
        config.backend.qdrant_connection_string = Some("http://127.0.0.1:1".to_string());
        config.backend.embedding.vector_size = Some(0);
        let error = match CharacterMemoryAdapter::reconstruct(&config, "never-opened").await {
            Ok(_) => panic!("invalid reconstruct config was accepted"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("backend.embedding.vector_size"));
        assert!(!error.contains("QDRANT_CONNECTION_STRING"));
        assert!(!error.contains("failed to connect"));
    }

    #[tokio::test]
    async fn embedded_frozen_write_surface_matches_continuity_runtime_normalization() {
        let _live_test_guard = LIVE_QDRANT_TEST_LOCK.lock().await;
        let directory = tempdir().unwrap();
        let token = unique_test_token();
        let prefix = format!("cmem_eval_frozen_drift_{token}");
        let namespace = "frozen-runtime-normalization";
        let content = "  The  cobalt\tnotebook\nis in   the east cabinet.  ";
        let runtime_lookup_text = cmem_eval_continuity::runtime_memory_embedding_text(content);
        let store_path = directory.path().join("strict-runtime-store.json");
        let store = FrozenEmbeddingStore::new(
            "text-embedding-3-small",
            FrozenEmbeddingSource::TestFixture,
            [(runtime_lookup_text, vec![1.0; 1_536])],
        )
        .unwrap();
        fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();

        let mut config = adapter_config(format!("frozen-drift-{token}"), prefix.clone());
        config.backend.embedding.provider = EmbeddingProviderConfig::Frozen;
        config.backend.embedding.model = "text-embedding-3-small".to_string();
        config.backend.embedding.vector_size = Some(1_536);
        config.backend.embedding.store_path = Some(store_path.display().to_string());
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
        let adapter = (CharacterMemoryAdapter::new_with_test_frozen_embeddings(&config).await)
            .expect("frozen drift-guard adapter construction");
        (adapter.open_namespace(namespace).await).expect("frozen drift-guard namespace open");
        let plan = (adapter
            .prepare(PrepareWriteInput {
                namespace: namespace.to_string(),
                content: content.to_string(),
                episode_external_id: "whitespace-episode".to_string(),
                observation_external_id: "whitespace-observation".to_string(),
                episode_started_at: None,
                observation_observed_at: None,
                raw_refs: Vec::new(),
                idempotency_key: Some("whitespace-drift-guard".to_string()),
                include_vector_index_candidates: true,
                include_stats_update_candidates: true,
            })
            .await)
            .expect("frozen drift-guard write preparation");
        let outcome = (adapter.commit(plan, CommitWriteOptions::default()).await)
            .expect("frozen drift-guard write commit");
        assert_eq!(outcome.vector_indexed_object_refs.len(), 2);
        (adapter.reset_namespace(namespace).await).expect("frozen drift-guard namespace cleanup");
    }

    #[tokio::test]
    async fn embedded_adapter_reattaches_with_external_ids() {
        reattach_with_external_ids(VectorStoreMode::Embedded).await;
    }

    #[cfg(feature = "service-tests")]
    #[tokio::test]
    async fn service_mode_reattaches_with_external_ids() {
        reattach_with_external_ids(VectorStoreMode::Service).await;
    }

    async fn reattach_with_external_ids(mode: VectorStoreMode) {
        let _live_test_guard = LIVE_QDRANT_TEST_LOCK.lock().await;
        let directory = tempdir().unwrap();
        let token = unique_test_token();
        let run_id = format!("task3-{token}");
        let prefix = format!("cmem_eval_task3_{token}");
        let namespace = "restart-round-trip";
        let mut config = adapter_config(run_id, prefix);
        config.backend.vector_store_mode = mode;
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
        let path_adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
        let identity_registry_path = path_adapter.identity_registry_path(namespace);
        let oxigraph_path = path_adapter.oxigraph_persistence_path(namespace).unwrap();
        let retrieval_stats_path = path_adapter.retrieval_stats_path(namespace).unwrap();
        drop(path_adapter);

        let adapter_a =
            (CharacterMemoryAdapter::new(&config).await).expect("initial adapter construction");
        (adapter_a.open_namespace(namespace).await).expect("initial fresh namespace open");
        let staged_plan = (adapter_a
            .prepare(PrepareWriteInput {
                namespace: namespace.to_string(),
                content: "The restart-safe drink is jasmine tea.".to_string(),
                episode_external_id: "episode-external".to_string(),
                observation_external_id: "observation-external".to_string(),
                episode_started_at: Some("2025-01-01T00:00:00Z".to_string()),
                observation_observed_at: Some("2025-01-01T00:00:00Z".to_string()),
                raw_refs: vec!["fixture://continuity/restart".to_string()],
                idempotency_key: Some("continuity-restart-write".to_string()),
                include_vector_index_candidates: true,
                include_stats_update_candidates: true,
            })
            .await)
            .expect("staged write preparation");
        let staged_validations =
            (adapter_a.validate_plan(&staged_plan).await).expect("staged write validation");
        assert!(
            staged_validations
                .iter()
                .all(|validation| validation.status == CandidateValidationStatus::Valid)
        );
        let staged_commit = (adapter_a
            .commit(staged_plan, CommitWriteOptions::default())
            .await)
            .expect("staged write commit");
        assert!(staged_commit.persisted_object_refs.iter().any(|reference| {
            reference.object_type == ObjectType::Episode
                && reference.external_id == "episode-external"
        }));
        assert!(staged_commit.persisted_object_refs.iter().any(|reference| {
            reference.object_type == ObjectType::Observation
                && reference.external_id == "observation-external"
        }));
        (adapter_a
            .remember_enrichment(GraphEnrichmentInput {
                namespace: namespace.to_string(),
                entities: vec![EntityInput {
                    external_id: "alice-entity".to_string(),
                    entity_type: EntityType::Person,
                    name: "Alice".to_string(),
                    aliases: Vec::new(),
                    canonical_key: None,
                    summary: Some("A restart-safe graph entity.".to_string()),
                }],
                derived_memories: vec![DerivedMemoryInput {
                    external_id: "pre-correction-memory".to_string(),
                    derived_type: DerivedType::Reflection,
                    text: "The restart-safe drink is jasmine tea.".to_string(),
                    source_episode_external_ids: vec!["episode-external".to_string()],
                    source_observation_external_ids: vec!["observation-external".to_string()],
                    thread_external_ids: Vec::new(),
                    entity_external_ids: vec!["alice-entity".to_string()],
                    confidence: 1.0,
                    salience_score: 0.8,
                    stability: Stability::Medium,
                    is_current: true,
                    supersedes_external_ids: Vec::new(),
                    metadata: serde_json::Value::Null,
                }],
                ..GraphEnrichmentInput::default()
            })
            .await)
            .expect("graph enrichment ingest");
        let link = (adapter_a
            .link(LinkMemoryInput {
                namespace: namespace.to_string(),
                link: MemoryLinkInput {
                    external_id: "alice-episode-link".to_string(),
                    from: MemoryEndpointInput {
                        object_type: ObjectType::Entity,
                        external_id: "alice-entity".to_string(),
                    },
                    relation: RelationType::Involves,
                    to: MemoryEndpointInput {
                        object_type: ObjectType::Episode,
                        external_id: "episode-external".to_string(),
                    },
                    confidence: 1.0,
                    rationale: Some("exercise graph and stats persistence".to_string()),
                },
            })
            .await)
            .expect("public link round-trip");
        assert_eq!(link.value.external_id, "alice-episode-link");
        assert!(link.outcome.outcome.stats_update_status.failure.is_none());

        let origin = SourceProvenanceInput {
            episode_external_ids: vec!["episode-external".to_string()],
            observation_external_ids: vec!["observation-external".to_string()],
            ..SourceProvenanceInput::default()
        };
        let correction_input = CorrectMemoryInput {
            namespace: namespace.to_string(),
            targets: vec![CorrectionTargetInput::DerivedMemory {
                external_id: "pre-correction-memory".to_string(),
            }],
            replacements: vec![ReplacementDerivedMemoryInput {
                memory: DerivedMemoryInput {
                    external_id: "corrected-memory".to_string(),
                    derived_type: DerivedType::Reflection,
                    text: "The restart-safe drink is oolong tea.".to_string(),
                    source_episode_external_ids: vec!["episode-external".to_string()],
                    source_observation_external_ids: vec!["observation-external".to_string()],
                    thread_external_ids: Vec::new(),
                    entity_external_ids: vec!["alice-entity".to_string()],
                    confidence: 1.0,
                    salience_score: 0.8,
                    stability: Stability::Medium,
                    is_current: true,
                    supersedes_external_ids: vec!["pre-correction-memory".to_string()],
                    metadata: serde_json::Value::Null,
                },
                original_source_provenance: origin.clone(),
                correction_origin_provenance: origin.clone(),
            }],
            superseded_derived_memory_external_ids: vec!["pre-correction-memory".to_string()],
            correction_origin: origin,
            rationale: "The fixture scripted a correction.".to_string(),
            lifecycle_policy: Default::default(),
            cascade_policy: Default::default(),
            include_trace: true,
        };
        let correction = (adapter_a.correct(correction_input.clone()).await)
            .expect("public correction round-trip");
        assert!(correction.mutated_object_refs.iter().any(|reference| {
            reference.object_type == ObjectType::DerivedMemory
                && reference.external_id == "corrected-memory"
        }));
        let correction_retry =
            (adapter_a.correct(correction_input).await).expect("identical public correction retry");
        assert_eq!(
            correction_retry.outcome.operation_id,
            correction.outcome.operation_id
        );
        assert!(correction_retry.mutated_object_refs.is_empty());
        assert!(correction_retry.mutated_link_external_ids.is_empty());
        assert!(
            correction_retry
                .outcome
                .outcome
                .graph_mutated_object_ids
                .is_empty()
        );
        assert!(
            correction_retry
                .outcome
                .outcome
                .graph_mutated_link_ids
                .is_empty()
        );
        assert!(correction_retry.superseded.is_empty());
        assert!(
            correction_retry
                .outcome
                .outcome
                .trace
                .as_ref()
                .unwrap()
                .superseded_by
                .is_empty()
        );
        assert!(
            correction_retry
                .outcome
                .outcome
                .vector_maintenance_failure
                .is_none()
        );
        assert!(
            correction_retry
                .outcome
                .outcome
                .stats_update_status
                .failure
                .is_none()
        );
        let mut retried_external_ids = correction_retry
            .vector_maintained_object_refs
            .iter()
            .map(|reference| reference.external_id.as_str())
            .collect::<Vec<_>>();
        retried_external_ids.sort_unstable();
        assert_eq!(
            retried_external_ids,
            vec!["corrected-memory", "pre-correction-memory"]
        );
        let mut retried_internal_ids = correction_retry
            .outcome
            .outcome
            .vector_maintained_object_ids
            .iter()
            .map(|reference| reference.id)
            .collect::<Vec<_>>();
        retried_internal_ids.sort_unstable();
        let mut retried_stats_ids = correction_retry
            .outcome
            .outcome
            .stats_update_status
            .updated_object_ids
            .clone();
        retried_stats_ids.sort_unstable();
        assert_eq!(retried_stats_ids, retried_internal_ids);

        let forgotten = (adapter_a
            .forget(ForgetMemoryInput {
                namespace: namespace.to_string(),
                targets: vec![MemoryEndpointInput {
                    object_type: ObjectType::DerivedMemory,
                    external_id: "corrected-memory".to_string(),
                }],
                rationale: "The fixture scripted suppression.".to_string(),
                suppression_policy: Default::default(),
                archive_policy: Default::default(),
                cascade_policy: Default::default(),
                target_retention_state: RetentionState::Suppressed,
                target_thread_status: None,
                include_trace: true,
            })
            .await)
            .expect("public forget round-trip");
        assert!(forgotten.mutated_object_refs.iter().any(|reference| {
            reference.object_type == ObjectType::DerivedMemory
                && reference.external_id == "corrected-memory"
        }));
        adapter_a.close().await.unwrap();
        assert!(oxigraph_path.exists());
        assert!(retrieval_stats_path.exists());
        let persisted_entity_id = deterministic_id(namespace, "entity", "alice-entity").to_string();
        assert!(file_contains(
            &retrieval_stats_path,
            persisted_entity_id.as_bytes()
        ));

        let (adapter_b, lifecycle) = (CharacterMemoryAdapter::reconstruct(&config, namespace)
            .await)
            .expect("public adapter reconstruction");
        assert_eq!(lifecycle.restored_identity_count, 6);
        {
            let namespaces = adapter_b.namespaces.lock().await;
            let state = namespaces.get(namespace).unwrap();
            let episode_id = state.episode_ids["episode-external"];
            assert_eq!(
                state
                    .reverse_episode_ids
                    .get(&episode_id)
                    .map(String::as_str),
                Some("episode-external")
            );
            let observation_id = state.observation_ids["observation-external"];
            assert_eq!(
                state.reverse_observation_ids.get(&observation_id),
                Some(&(
                    "observation-external".to_string(),
                    "episode-external".to_string()
                ))
            );
            let entity_id = state.entity_ids["alice-entity"];
            assert_eq!(
                state.reverse_entity_ids.get(&entity_id).map(String::as_str),
                Some("alice-entity")
            );
            for external_id in ["pre-correction-memory", "corrected-memory"] {
                let memory_id = state.derived_memory_ids[external_id];
                assert_eq!(
                    state
                        .reverse_derived_memory_ids
                        .get(&memory_id)
                        .map(String::as_str),
                    Some(external_id)
                );
            }
            let link_id = state.link_ids["alice-episode-link"];
            assert_eq!(
                state.reverse_link_ids.get(&link_id).map(String::as_str),
                Some("alice-episode-link")
            );
        }
        let retrieved = (adapter_b
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: namespace.to_string(),
                query: "What is the restart-safe drink?".to_string(),
                query_date: None,
                surface_policy: retrieval_surface_policy(8, 8, false, false, false, true),
            })
            .await)
            .expect("reattached retrieval");
        assert!(retrieved.items().iter().any(|item| {
            item.kind == ObjectType::Episode
                && item.external_id.as_deref() == Some("episode-external")
        }));
        assert!(retrieved.items().iter().any(|item| {
            item.kind == ObjectType::Observation
                && item.external_id.as_deref() == Some("observation-external")
                && item.episode_external_id.as_deref() == Some("episode-external")
        }));
        let suppression_check = (adapter_b
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: namespace.to_string(),
                query: "What is the corrected restart-safe drink?".to_string(),
                query_date: None,
                surface_policy: retrieval_surface_policy(8, 8, true, false, false, true),
            })
            .await)
            .expect("post-reconstruct suppression retrieval");
        assert!(suppression_check.items().iter().all(|item| {
            item.external_id.as_deref() != Some("corrected-memory")
                && item.external_id.as_deref() != Some("pre-correction-memory")
        }));
        adapter_b.close().await.unwrap();

        let oxigraph_backup = oxigraph_path.with_extension("missing-test-backup");
        fs::rename(&oxigraph_path, &oxigraph_backup).unwrap();
        let adapter_missing_oxigraph = (CharacterMemoryAdapter::new(&config).await)
            .expect("missing-Oxigraph adapter construction");
        let missing_oxigraph_error = (adapter_missing_oxigraph.reattach_namespace(namespace).await)
            .expect_err("expected operation failure");
        let missing_oxigraph_message = missing_oxigraph_error.to_string();
        assert!(missing_oxigraph_message.contains("Oxigraph store"));
        assert!(missing_oxigraph_message.contains(&oxigraph_path.display().to_string()));
        drop(adapter_missing_oxigraph);
        fs::rename(&oxigraph_backup, &oxigraph_path).unwrap();

        let retrieval_stats_backup = retrieval_stats_path.with_extension("missing-test-backup");
        fs::rename(&retrieval_stats_path, &retrieval_stats_backup).unwrap();
        let adapter_missing_stats = (CharacterMemoryAdapter::new(&config).await)
            .expect("missing-stats adapter construction");
        let missing_stats_error = (adapter_missing_stats.reattach_namespace(namespace).await)
            .expect_err("expected operation failure");
        let missing_stats_message = missing_stats_error.to_string();
        assert!(missing_stats_message.contains("retrieval stats store"));
        assert!(missing_stats_message.contains(&retrieval_stats_path.display().to_string()));
        drop(adapter_missing_stats);
        fs::rename(&retrieval_stats_backup, &retrieval_stats_path).unwrap();

        let identity_registry_backup = identity_registry_path.with_extension("missing-test-backup");
        fs::rename(&identity_registry_path, &identity_registry_backup).unwrap();
        let adapter_missing_registry = (CharacterMemoryAdapter::new(&config).await)
            .expect("missing-registry adapter construction");
        let missing_registry_error = (adapter_missing_registry.reattach_namespace(namespace).await)
            .expect_err("expected operation failure");
        let missing_registry_message = missing_registry_error.to_string();
        assert!(missing_registry_message.contains("identity registry"));
        assert!(missing_registry_message.contains(&identity_registry_path.display().to_string()));
        assert!(adapter_missing_registry.namespaces.lock().await.is_empty());
        drop(adapter_missing_registry);
        fs::rename(&identity_registry_backup, &identity_registry_path).unwrap();

        let adapter_restored_stores =
            (CharacterMemoryAdapter::new(&config).await).expect("all-stores adapter construction");
        let restored_stores = (adapter_restored_stores.reattach_namespace(namespace).await)
            .expect("all-stores namespace reattach");
        assert_eq!(restored_stores.restored_identity_count, 6);
        if mode == VectorStoreMode::Service {
            adapter_restored_stores
                .qdrant
                .as_ref()
                .unwrap()
                .delete_collection(adapter_restored_stores.collection_name(namespace))
                .await
                .unwrap();
            adapter_restored_stores.close().await.unwrap();
        } else {
            let vector_path = adapter_restored_stores.vector_store_path(namespace);
            adapter_restored_stores.close().await.unwrap();
            remove_namespace_store(&vector_path, "embedded vector").unwrap();
        }

        let adapter_missing_collection = (CharacterMemoryAdapter::new(&config).await)
            .expect("missing-collection adapter construction");
        let missing_collection_error = (adapter_missing_collection
            .reattach_namespace(namespace)
            .await)
            .expect_err("expected operation failure");
        let missing_collection_message = missing_collection_error.to_string();
        assert!(
            missing_collection_message.contains(if mode == VectorStoreMode::Service {
                "Qdrant collection"
            } else {
                "vector"
            }),
            "{missing_collection_message}"
        );
        assert!(missing_collection_message.contains("missing durable store(s)"));
        assert!(missing_collection_message.contains("identity registry"));
        if mode == VectorStoreMode::Service {
            assert!(
                missing_collection_message
                    .contains(&adapter_missing_collection.collection_name(namespace))
            );
        }
        println!("verified reattach rejects a surviving registry without its Qdrant collection");
        drop(adapter_missing_collection);

        let adapter_c =
            (CharacterMemoryAdapter::new(&config).await).expect("fresh adapter construction");
        let stale_open_error =
            (adapter_c.open_namespace(namespace).await).expect_err("expected operation failure");
        assert!(
            stale_open_error
                .to_string()
                .contains("identity registry already exists")
        );
        (adapter_c.reset_namespace(namespace).await).expect("fresh adapter durable reset");
        assert!(!oxigraph_path.exists());
        assert!(!retrieval_stats_path.exists());
        let fresh =
            (adapter_c.open_namespace(namespace).await).expect("post-reset fresh namespace open");
        assert_eq!(fresh.restored_identity_count, 0);
        assert!(oxigraph_path.exists());
        assert!(retrieval_stats_path.exists());
        let fresh_retrieval = (adapter_c
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: namespace.to_string(),
                query: "What is the restart-safe drink?".to_string(),
                query_date: None,
                surface_policy: retrieval_surface_policy(8, 8, true, true, true, true),
            })
            .await)
            .expect("fresh namespace retrieval");
        assert!(fresh_retrieval.items().is_empty());
        assert!(!file_contains(
            &retrieval_stats_path,
            persisted_entity_id.as_bytes()
        ));
        (adapter_c.reset_namespace(namespace).await).expect("final namespace cleanup");
    }

    #[tokio::test]
    async fn embedded_reset_preserves_sibling_namespace_durable_stores() {
        reset_preserves_sibling_stores(VectorStoreMode::Embedded).await;
    }

    #[cfg(feature = "service-tests")]
    #[tokio::test]
    async fn service_mode_reset_preserves_sibling_namespace_durable_stores() {
        reset_preserves_sibling_stores(VectorStoreMode::Service).await;
    }

    async fn reset_preserves_sibling_stores(mode: VectorStoreMode) {
        let _live_test_guard = LIVE_QDRANT_TEST_LOCK.lock().await;
        let directory = tempdir().unwrap();
        let token = unique_test_token();
        let run_id = format!("sibling-isolation-{token}");
        let prefix = format!("cmem_eval_sibling_{token}");
        let namespace_a = "namespace-a";
        let namespace_b = "namespace-b";
        let oxigraph_root = directory.path().join("shared-oxigraph-root");
        let stats_template = directory.path().join("shared-retrieval-stats.sqlite");
        let mut config = adapter_config(run_id, prefix);
        config.backend.vector_store_mode = mode;
        config.backend.cleanup.enabled = false;
        config.backend.identity_registry_dir = Some(
            directory
                .path()
                .join("identities")
                .to_string_lossy()
                .into_owned(),
        );
        config.backend.oxigraph_persistence_path =
            Some(oxigraph_root.to_string_lossy().into_owned());
        config.backend.retrieval_stats_path = Some(stats_template.to_string_lossy().into_owned());

        let writer =
            (CharacterMemoryAdapter::new(&config).await).expect("sibling writer construction");
        (writer.open_namespace(namespace_a).await).expect("namespace A fresh open");
        (writer.open_namespace(namespace_b).await).expect("namespace B fresh open");
        for (namespace, label) in [(namespace_a, "a"), (namespace_b, "b")] {
            (writer
                .remember_episode(EpisodeInput {
                    external_id: format!("episode-{label}"),
                    namespace: namespace.to_string(),
                    summary: format!("Sibling namespace {label} must survive independently."),
                    started_at: None,
                    ended_at: None,
                    participants: Vec::new(),
                    metadata: serde_json::Value::Null,
                })
                .await)
                .expect("sibling episode ingest");
            (writer
                .remember_enrichment(GraphEnrichmentInput {
                    namespace: namespace.to_string(),
                    entities: vec![EntityInput {
                        external_id: format!("entity-{label}"),
                        entity_type: EntityType::Person,
                        name: format!("Sibling {label}"),
                        aliases: Vec::new(),
                        canonical_key: None,
                        summary: Some(format!("Graph sentinel for namespace {label}.")),
                    }],
                    links: vec![MemoryLinkInput {
                        external_id: format!("link-{label}"),
                        from: MemoryEndpointInput {
                            object_type: ObjectType::Entity,
                            external_id: format!("entity-{label}"),
                        },
                        relation: RelationType::Involves,
                        to: MemoryEndpointInput {
                            object_type: ObjectType::Episode,
                            external_id: format!("episode-{label}"),
                        },
                        confidence: 1.0,
                        rationale: Some("sibling isolation sentinel".to_string()),
                    }],
                    ..GraphEnrichmentInput::default()
                })
                .await)
                .expect("sibling graph and stats ingest");
        }

        let registry_a = writer.identity_registry_path(namespace_a);
        let registry_b = writer.identity_registry_path(namespace_b);
        let oxigraph_a = writer.oxigraph_persistence_path(namespace_a).unwrap();
        let oxigraph_b = writer.oxigraph_persistence_path(namespace_b).unwrap();
        let stats_a = writer.retrieval_stats_path(namespace_a).unwrap();
        let stats_b = writer.retrieval_stats_path(namespace_b).unwrap();
        let collection_a = writer.collection_name(namespace_a);
        let collection_b = writer.collection_name(namespace_b);
        assert_eq!(oxigraph_a.parent(), Some(oxigraph_root.as_path()));
        assert_eq!(oxigraph_b.parent(), Some(oxigraph_root.as_path()));
        assert_eq!(stats_a.parent(), stats_template.parent());
        assert_eq!(stats_b.parent(), stats_template.parent());
        assert_ne!(oxigraph_a, oxigraph_b);
        assert_ne!(stats_a, stats_b);
        assert!(!stats_template.exists());
        writer.close().await.unwrap();

        let stats_a_wal = path_with_appended_suffix(&stats_a, "-wal");
        let stats_a_shm = path_with_appended_suffix(&stats_a, "-shm");
        fs::write(&stats_a_wal, b"namespace-a-wal-sentinel").unwrap();
        fs::write(&stats_a_shm, b"namespace-a-shm-sentinel").unwrap();
        let registry_b_before = fs::read(&registry_b).unwrap();
        let stats_b_before = fs::read(&stats_b).unwrap();
        let entity_b_id = deterministic_id(namespace_b, "entity", "entity-b").to_string();
        assert!(file_contains(&stats_b, entity_b_id.as_bytes()));

        let resetter =
            (CharacterMemoryAdapter::new(&config).await).expect("sibling resetter construction");
        (resetter.reset_namespace(namespace_a).await).expect("namespace A production reset");

        assert!(!registry_a.exists());
        assert!(!oxigraph_a.exists());
        assert!(!stats_a.exists());
        assert!(!stats_a_wal.exists());
        assert!(!stats_a_shm.exists());
        assert!(oxigraph_root.exists());
        assert!(registry_b.exists());
        assert!(oxigraph_b.exists());
        assert!(stats_b.exists());
        assert_eq!(fs::read(&registry_b).unwrap(), registry_b_before);
        assert_eq!(fs::read(&stats_b).unwrap(), stats_b_before);
        if mode == VectorStoreMode::Service {
            assert!(
                !resetter
                    .qdrant
                    .as_ref()
                    .unwrap()
                    .collection_exists(&collection_a)
                    .await
                    .unwrap()
            );
            assert!(
                resetter
                    .qdrant
                    .as_ref()
                    .unwrap()
                    .collection_exists(&collection_b)
                    .await
                    .unwrap()
            );
        } else {
            assert!(!resetter.vector_store_path(namespace_a).exists());
            assert!(resetter.vector_store_path(namespace_b).exists());
        }

        let reattached_b = (resetter.reattach_namespace(namespace_b).await)
            .expect("namespace B reattach after sibling reset");
        assert_eq!(reattached_b.restored_identity_count, 3);
        let surviving_b = (resetter
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: namespace_b.to_string(),
                query: "Which sibling namespace must survive?".to_string(),
                query_date: None,
                surface_policy: retrieval_surface_policy(8, 8, false, false, true, true),
            })
            .await)
            .expect("namespace B retrieval after sibling reset");
        assert!(
            surviving_b
                .items()
                .iter()
                .any(|item| item.external_id.as_deref() == Some("episode-b"))
        );
        (resetter.reset_namespace(namespace_b).await).expect("sibling namespace B cleanup");
    }

    #[tokio::test]
    async fn construction_rejects_cleanup_prefix_that_differs_from_namespace_prefix() {
        let mut config = adapter_config("post-run-cleanup".to_string(), "bench:review".to_string());
        config.backend.cleanup.require_collection_prefix = Some("unrelated:prefix".to_string());
        let error = match CharacterMemoryAdapter::new(&config).await {
            Ok(_) => panic!("mismatched cleanup and namespace prefixes were admitted"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("cleanup.require_collection_prefix"));
        assert!(error.contains("namespace_prefix"));
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
            (ObjectType::Episode, episode_id, ObjectType::Episode, "s1"),
            (
                ObjectType::Observation,
                observation_id,
                ObjectType::Observation,
                "o1",
            ),
            (ObjectType::Entity, entity_id, ObjectType::Entity, "e1"),
            (
                ObjectType::MemoryThread,
                thread_id,
                ObjectType::MemoryThread,
                "t1",
            ),
            (
                ObjectType::DerivedMemory,
                derived_id,
                ObjectType::DerivedMemory,
                "d1",
            ),
            (
                ObjectType::MemoryLink,
                link_id,
                ObjectType::MemoryLink,
                "l1",
            ),
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
    fn context_pack_constructor_renders_external_ids() {
        let pack = RetrievedContextPack::from_ranked_items(
            vec![RetrievedItem {
                kind: ObjectType::Observation,
                internal_id: "i".to_string(),
                external_id: Some("s1:turn:1".to_string()),
                episode_external_id: Some("s1".to_string()),
                score: Some(0.5),
                rank: 1,
                rationale: vec![],
                text: Some("hello".to_string()),
            }],
            Vec::new(),
            ContextRenderer::WithIdentity,
        );
        assert!(pack.context_text().contains("observation:s1:turn:1"));
        assert!(pack.context_text().contains("hello"));
    }

    #[test]
    fn vector_only_context_maps_raw_candidates_and_omits_graph_telemetry() {
        let episode_id = deterministic_id("n", "episode", "s1");
        let observation_id = deterministic_id("n", "observation", "s1:turn:1");
        let unmapped_id = deterministic_id("n", "episode", "unmapped");
        let snapshot = ExternalIdRegistry {
            reverse_episode_ids: BTreeMap::from([(episode_id, "s1".to_string())]),
            reverse_observation_ids: BTreeMap::from([(
                observation_id,
                ("s1:turn:1".to_string(), "s1".to_string()),
            )]),
            ..ExternalIdRegistry::default()
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
            Vec::new(),
        );

        assert_eq!(pack.items().len(), 2);
        assert_eq!(pack.items()[0].kind, ObjectType::Observation);
        assert_eq!(pack.items()[0].external_id.as_deref(), Some("s1:turn:1"));
        assert_eq!(pack.items()[0].episode_external_id.as_deref(), Some("s1"));
        assert_eq!(pack.items()[0].rank, 1);
        assert_eq!(pack.items()[1].kind, ObjectType::Episode);
        assert_eq!(pack.items()[1].external_id.as_deref(), Some("s1"));
        assert_eq!(pack.items()[1].rank, 2);
        assert!(pack.context_text().contains("turn text"));
        assert!(pack.context_text().contains("episode summary"));
    }

    #[test]
    fn vector_only_context_dedupes_duplicate_surfaces_to_best_score() {
        let observation_id = deterministic_id("n", "observation", "s1:turn:1");
        let snapshot = ExternalIdRegistry {
            reverse_episode_ids: BTreeMap::new(),
            reverse_observation_ids: BTreeMap::from([(
                observation_id,
                ("s1:turn:1".to_string(), "s1".to_string()),
            )]),
            ..ExternalIdRegistry::default()
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
            Vec::new(),
        );

        assert_eq!(pack.items().len(), 1);
        assert_eq!(pack.items()[0].score, Some(0.8));
        assert_eq!(pack.items()[0].text.as_deref(), Some("higher"));
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
    fn pack_delivery_order_defines_rank_with_or_without_debug_trace() {
        let first_id = deterministic_id("n", "episode", "first");
        let second_id = deterministic_id("n", "episode", "second");
        let pack = ContinuityContextPack {
            relevant_episodes: vec![episode(first_id), episode(second_id)],
            ..ContinuityContextPack::empty()
        };
        let mut trace = RetrievalTrace::empty();
        trace.vector_candidates = vec![
            VectorCandidateTrace {
                object: MemoryObjectRef::new(ObjectType::Episode, second_id),
                surface: VectorSurface::Summary,
                score: 0.9,
                rank: 1,
            },
            VectorCandidateTrace {
                object: MemoryObjectRef::new(ObjectType::Episode, first_id),
                surface: VectorSurface::Summary,
                score: 0.5,
                rank: 2,
            },
        ];
        let registry = ExternalIdRegistry::new("n");
        let traced = flatten_outcome(
            &registry,
            RetrieveOutcome {
                pack: pack.clone(),
                rationale: RetrievalRationale::new("test"),
                trace: Some(trace),
            },
        );
        let untraced = flatten_outcome(
            &registry,
            RetrieveOutcome {
                pack,
                rationale: RetrievalRationale::new("test"),
                trace: None,
            },
        );

        let expected_ids = vec![first_id.to_string(), second_id.to_string()];
        assert_eq!(
            traced
                .items()
                .iter()
                .map(|item| item.internal_id.clone())
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert_eq!(
            untraced
                .items()
                .iter()
                .map(|item| item.internal_id.clone())
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert_eq!(
            traced
                .items()
                .iter()
                .map(|item| item.rank)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            traced
                .items()
                .iter()
                .map(|item| item.score)
                .collect::<Vec<_>>(),
            vec![Some(0.5_f32 as f64), Some(0.9_f32 as f64)]
        );
    }

    #[test]
    fn telemetry_leakage_counts_only_unique_final_returned_items() {
        let returned_id = deterministic_id("n", "episode", "returned");
        let omitted_id = deterministic_id("n", "episode", "omitted");
        let mut trace = RetrievalTrace::empty();
        trace.lifecycle_filter_decisions = vec![
            LifecycleFilterDecision {
                object: MemoryObjectRef::new(ObjectType::Episode, returned_id),
                retention_state: Some(RetentionState::Suppressed),
                is_current: Some(false),
                superseded_by: vec![omitted_id],
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
            LifecycleFilterDecision {
                object: MemoryObjectRef::new(ObjectType::Episode, returned_id),
                retention_state: Some(RetentionState::Suppressed),
                is_current: Some(false),
                superseded_by: vec![omitted_id],
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

        let integrity = crate::integrity_details_from_outcomes(
            &[RetrievedItem {
                kind: ObjectType::Episode,
                internal_id: returned_id.to_string(),
                external_id: Some("returned".to_string()),
                episode_external_id: None,
                score: None,
                rank: 1,
                rationale: Vec::new(),
                text: None,
            }],
            &[outcome],
        );
        assert_eq!(integrity.suppressed_memory_leakage_rate, Some(1.0));
        assert_eq!(integrity.superseded_current_leakage_rate, Some(1.0));
    }

    #[test]
    fn staged_source_drafts_preserve_scripted_timestamps() {
        let input = PrepareWriteInput {
            namespace: "continuity:test".to_string(),
            content: "scripted memory".to_string(),
            episode_external_id: "episode".to_string(),
            observation_external_id: "observation".to_string(),
            episode_started_at: Some("2025-02-03T04:05:06Z".to_string()),
            observation_observed_at: Some("2025-02-03T04:05:06Z".to_string()),
            raw_refs: Vec::new(),
            idempotency_key: None,
            include_vector_index_candidates: true,
            include_stats_update_candidates: true,
        };
        let (episode, observation) = staged_source_drafts(
            &input,
            deterministic_id(&input.namespace, "episode", &input.episode_external_id),
            deterministic_id(
                &input.namespace,
                "observation",
                &input.observation_external_id,
            ),
        )
        .unwrap();

        assert_eq!(
            episode.started_at.unwrap().to_rfc3339(),
            "2025-02-03T04:05:06+00:00"
        );
        assert_eq!(
            observation.observed_at.unwrap().to_rfc3339(),
            "2025-02-03T04:05:06+00:00"
        );
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
