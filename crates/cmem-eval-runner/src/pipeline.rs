use crate::commands::{AdapterKind, ContinuityRunArgs, RunArgs, read_config};
use crate::enrichment;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use chrono::Utc;
use cmem_eval_adapter_cmem::CharacterMemoryAdapter;
use cmem_eval_continuity::{
    ContinuityQueryTrace, ContinuityReportInput, ContinuityRuntime, ContinuityScenario,
    InteractionEvent, RestartObservation, assemble_continuity_report, continuity_metric_family,
    insert_continuity_metrics, parse_fixture_bytes, run_continuity_scenario,
    write_continuity_report, write_continuity_traces,
};
use cmem_eval_core::{
    BenchmarkRunConfig, DatasetKind, EpisodeInput, FrozenEmbeddingProvider, FrozenEmbeddingSource,
    GraphEnrichmentInput, GraphSnapshotInput, MemoryAdapter, MetricFamily, MetricsConfig,
    MockMemoryAdapter, NamespaceLifecycleResult, ObservationInput, PerQuestionResult, ReaderResult,
    ResultContextMetrics, RetrieveInput, RetrievedContextPack, RetrievedItem, RunAdapterMetadata,
    Timer, classify_frozen_embedding_dimensions, composition_metrics, count_tokens,
    estimate_word_count, initialize_registry_metrics_for, insert_composition_metrics,
    insert_context_metrics, insert_integrity_detail_metrics, insert_retrieval_metrics,
    insert_telemetry_metrics, integrity_details_with_telemetry, summarize_rows, write_jsonl,
    write_summary,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

type FrozenEmbeddingProviders = HashMap<PathBuf, FrozenEmbeddingProvider>;

pub(crate) async fn run_synthetic(args: RunArgs) -> Result<()> {
    run_pipeline::<SyntheticSpec>(args).await
}

pub(crate) async fn run_continuity(args: ContinuityRunArgs) -> Result<()> {
    let config = read_config(&args.run.config)?;
    ContinuitySpec::validate_config(&config)?;
    let fixture = load_continuity_fixture(&args.run.dataset)?;
    let fixture_schema_version = fixture.schema_version;
    let fixture_seed = fixture.seed;
    let scenarios = select_continuity_scenarios(fixture.scenarios, args.scenario.as_deref())?;
    let selected_adapter = args.run.selected_adapter();
    let frozen_embedding_providers =
        validate_continuity_embedding_sizes(&config, &scenarios, Some(selected_adapter))?;
    args.run.validate_adapter_selection(&config)?;
    run_continuity_pipeline(
        args,
        config,
        fixture_schema_version,
        fixture_seed,
        scenarios,
        frozen_embedding_providers,
    )
    .await
}

pub(crate) async fn run_longmemeval(args: RunArgs) -> Result<()> {
    run_pipeline::<LongMemEvalSpec>(args).await
}

pub(crate) async fn run_locomo(args: RunArgs) -> Result<()> {
    run_pipeline::<LoCoMoSpec>(args).await
}

pub(crate) fn metric_family_for_config(
    config: &BenchmarkRunConfig,
    continuity_dataset: Option<&Path>,
    continuity_scenario: Option<&str>,
) -> Result<MetricFamily> {
    match config.dataset.as_str() {
        "synthetic" => {
            SyntheticSpec::validate_config(config)?;
            Ok(SyntheticSpec::metric_family(&config.metrics))
        }
        "continuity" => {
            ContinuitySpec::validate_config(config)?;
            let dataset = continuity_dataset.context(
                "summarizing continuity results requires --dataset with the source fixture path",
            )?;
            let scenarios = load_continuity_scenarios(dataset, continuity_scenario)?;
            validate_continuity_embedding_sizes(config, &scenarios, None)?;
            Ok(continuity_metric_family(&config.metrics, &scenarios))
        }
        "longmemeval_s" => {
            LongMemEvalSpec::validate_config(config)?;
            Ok(LongMemEvalSpec::metric_family(&config.metrics))
        }
        "locomo" => {
            LoCoMoSpec::validate_config(config)?;
            Ok(LoCoMoSpec::metric_family(&config.metrics))
        }
        dataset => bail!("unsupported summarize dataset in config: {dataset}"),
    }
}

pub(crate) fn validate_continuity_summary_rows(
    rows: &[PerQuestionResult],
    dataset: &Path,
    selected_scenario: Option<&str>,
) -> Result<()> {
    let scenarios = load_continuity_scenarios(dataset, selected_scenario)?;
    let expected_query_ids = scenarios
        .iter()
        .flat_map(|scenario| scenario.events.iter())
        .filter_map(|event| match event {
            InteractionEvent::Query { query_id, .. } => Some(query_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if rows.len() != expected_query_ids.len() {
        bail!(
            "continuity summary input has {} rows but selected fixture scope requires {} queries",
            rows.len(),
            expected_query_ids.len()
        );
    }
    for (index, (row, expected_query_id)) in rows.iter().zip(expected_query_ids).enumerate() {
        if row.question_id != expected_query_id {
            bail!(
                "continuity summary row/query mismatch at index {index}: got {:?}, expected {:?} from the selected fixture scope",
                row.question_id,
                expected_query_id
            );
        }
    }
    Ok(())
}

fn load_continuity_scenarios(
    path: &Path,
    selected_scenario: Option<&str>,
) -> Result<Vec<ContinuityScenario>> {
    let fixture = load_continuity_fixture(path)?;
    select_continuity_scenarios(fixture.scenarios, selected_scenario)
}

fn load_continuity_fixture(path: &Path) -> Result<cmem_eval_continuity::ContinuityFixtureSet> {
    let bytes =
        fs::read(path).with_context(|| format!("read continuity fixture {}", path.display()))?;
    parse_fixture_bytes(&bytes)
}

fn validate_continuity_embedding_sizes(
    config: &BenchmarkRunConfig,
    scenarios: &[ContinuityScenario],
    selected_adapter: Option<AdapterKind>,
) -> Result<FrozenEmbeddingProviders> {
    let mut frozen_embedding_providers = HashMap::new();
    let configured_size = config.backend.embedding.vector_size.context(
        "continuity dataset requires backend.embedding.vector_size to match every selected fixture scenario",
    )?;
    let scenario_providers = scenarios
        .iter()
        .map(|scenario| scenario.embedding.provider_name())
        .collect::<BTreeSet<_>>();
    match config.backend.embedding.provider.as_str() {
        "controllable_similarity"
            if scenario_providers != BTreeSet::from(["controllable_similarity"]) =>
        {
            bail!(
                "backend.embedding.provider=controllable_similarity cannot run selected scenario providers {scenario_providers:?}; use frozen for frozen-only selection or mixed for a mixed suite"
            );
        }
        "frozen" if scenario_providers != BTreeSet::from(["frozen"]) => {
            bail!(
                "backend.embedding.provider=frozen cannot run selected scenario providers {scenario_providers:?}; use controllable_similarity for legacy selection or mixed for a mixed suite"
            );
        }
        "controllable_similarity" | "frozen" | "mixed" => {}
        provider => bail!("unsupported continuity embedding provider {provider:?}"),
    }

    for scenario in scenarios {
        if let Some(fixture_size) = scenario.embedding.vector_size() {
            let valid = if config.backend.embedding.provider == "mixed" {
                fixture_size <= configured_size
            } else {
                fixture_size == configured_size
            };
            if !valid {
                bail!(
                    "continuity scenario {:?} controllable embedding vector_size {fixture_size} is incompatible with backend.embedding.vector_size {configured_size} for provider {:?}",
                    scenario.fixture_id,
                    config.backend.embedding.provider
                );
            }
        }
    }

    if scenario_providers.contains("frozen") {
        let store_path = PathBuf::from(
            config
                .backend
                .embedding
                .store_path
                .as_deref()
                .context("frozen continuity scenarios require backend.embedding.store_path")?,
        );
        let provider = FrozenEmbeddingProvider::load(
            &store_path,
            &config.backend.embedding.model,
            configured_size,
        )?;
        if selected_adapter == Some(AdapterKind::Real) {
            if provider.source() != FrozenEmbeddingSource::OpenAiApi {
                bail!(
                    "frozen continuity evaluations require a store with source=open_ai_api; {} declares source={:?}",
                    store_path.display(),
                    provider.source()
                );
            }
            classify_frozen_embedding_dimensions(provider.model(), provider.vector_size(), false)?;
        }
        for scenario in scenarios
            .iter()
            .filter(|scenario| scenario.embedding.provider_name() == "frozen")
        {
            for text in scenario.runtime_embedding_inputs() {
                provider.vector_for_text(&text).map_err(|error| {
                    anyhow::anyhow!(
                        "preflight frozen embeddings for continuity scenario {:?}: {error}",
                        scenario.fixture_id
                    )
                })?;
            }
        }
        frozen_embedding_providers.insert(store_path, provider);
    }
    Ok(frozen_embedding_providers)
}

fn continuity_config_for_scenario(
    config: &BenchmarkRunConfig,
    scenario: &ContinuityScenario,
) -> BenchmarkRunConfig {
    let mut scenario_config = config.clone();
    scenario_config.backend.embedding.provider = scenario.embedding.provider_name().to_string();
    scenario_config
}

fn select_continuity_scenarios(
    mut scenarios: Vec<ContinuityScenario>,
    selected_scenario: Option<&str>,
) -> Result<Vec<ContinuityScenario>> {
    if let Some(selected_scenario) = selected_scenario {
        scenarios.retain(|scenario| scenario.fixture_id == selected_scenario);
        if scenarios.is_empty() {
            bail!("continuity fixture has no scenario {selected_scenario:?}");
        }
    }
    Ok(scenarios)
}

struct MemoryBatch {
    episodes: Vec<EpisodeInput>,
    observations: Vec<ObservationInput>,
    derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
}

trait DatasetSpec {
    type Item;
    type Question: ?Sized;

    const LATENCY_INCLUDES_INGEST: bool;
    const REPORT_QA_PROGRESS: bool;
    const USES_ENRICHMENT: bool;

    fn metric_family(config: &MetricsConfig) -> MetricFamily;
    fn validate_config(config: &BenchmarkRunConfig) -> Result<()>;
    fn load(path: &Path) -> Result<Vec<Self::Item>>;
    fn item_id(item: &Self::Item) -> &str;
    fn namespace(item: &Self::Item) -> String;
    fn memory_inputs(item: &Self::Item, config: &BenchmarkRunConfig) -> MemoryBatch;
    fn questions(item: &Self::Item) -> Vec<&Self::Question>;
    fn question_id(question: &Self::Question) -> &str;
    fn question_type(question: &Self::Question) -> Option<String>;
    fn question_text(question: &Self::Question) -> &str;
    fn query_date(question: &Self::Question) -> Option<String>;
    fn gold_episode_ids(item: &Self::Item, question: &Self::Question) -> Vec<String>;
    fn gold_observation_ids(item: &Self::Item, question: &Self::Question) -> Vec<String>;
    fn score(
        item: &Self::Item,
        question: &Self::Question,
        items: &[RetrievedItem],
        config: &BenchmarkRunConfig,
    ) -> Value;
    fn full_history_text(item: &Self::Item) -> String;
    fn enrichment(
        item: &Self::Item,
        namespace: &str,
        derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
        config: &BenchmarkRunConfig,
        configured: &HashMap<String, GraphEnrichmentInput>,
        snapshots: &HashMap<String, GraphSnapshotInput>,
    ) -> Result<Option<GraphEnrichmentInput>>;

    fn total_questions(items: &[Self::Item]) -> usize {
        items.iter().map(|item| Self::questions(item).len()).sum()
    }

    fn ingest_progress_detail(batch: &MemoryBatch) -> String {
        format!(
            "episodes={} observations={}",
            batch.episodes.len(),
            batch.observations.len()
        )
    }
}

async fn run_pipeline<S: DatasetSpec>(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    config.validate()?;
    S::validate_config(&config)?;
    let metric_family = S::metric_family(&config.metrics);
    let source_items = S::load(&args.dataset)?;
    let selected = args.selected_adapter();
    args.validate_adapter_selection(&config)?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let enrichment_by_namespace = if S::USES_ENRICHMENT {
        load_enrichment_by_namespace(&config)?
    } else {
        HashMap::new()
    };
    let snapshots_by_item = if S::USES_ENRICHMENT {
        load_snapshots_by_dataset_item(&config)?
    } else {
        HashMap::new()
    };
    let total_questions = S::total_questions(&source_items);
    let progress = RunProgress::new(
        &config.dataset,
        source_items.len(),
        S::REPORT_QA_PROGRESS.then_some(total_questions),
    );
    let mut rows = Vec::with_capacity(total_questions);
    let mut namespaces_to_cleanup = Vec::with_capacity(source_items.len());
    let mut completed_questions = 0usize;

    for (item_index, item) in source_items.into_iter().enumerate() {
        let item_number = item_index + 1;
        let namespace = S::namespace(&item);
        let item_label = S::item_id(&item).to_string();
        let item_timer = Timer::start();
        progress.item_started(item_number, &item_label);
        prepare_fresh_namespace(adapter.as_ref(), &namespace).await?;

        let batch = S::memory_inputs(&item, &config);
        let ingest_detail = S::ingest_progress_detail(&batch);
        let episode_count = batch.episodes.len();
        let observation_count = batch.observations.len();
        adapter.remember_episodes(batch.episodes).await?;
        progress.phase_done(
            item_number,
            &item_label,
            "ingest-episodes",
            &format!("count={episode_count}"),
        );
        adapter.remember_observations(batch.observations).await?;
        progress.phase_done(
            item_number,
            &item_label,
            "ingest-observations",
            &format!("count={observation_count}"),
        );
        progress.phase_done(item_number, &item_label, "ingest", &ingest_detail);

        if let Some(enrichment) = S::enrichment(
            &item,
            &namespace,
            batch.derived_memories,
            &config,
            &enrichment_by_namespace,
            &snapshots_by_item,
        )? {
            adapter.remember_enrichment(enrichment).await?;
            progress.phase_done(item_number, &item_label, "enrichment", "done");
        }

        let full_history = S::full_history_text(&item);
        let full_history_metrics = full_history_context_metrics(Some(&full_history));
        let questions = S::questions(&item);
        let item_question_count = questions.len();
        for (question_index, question) in questions.into_iter().enumerate() {
            let question_timer = Timer::start();
            if S::REPORT_QA_PROGRESS {
                progress.qa_started(
                    item_number,
                    &item_label,
                    question_index + 1,
                    item_question_count,
                );
            }
            let pack = adapter
                .retrieve(RetrieveInput {
                    mode: config.retrieval.mode,
                    namespace: namespace.clone(),
                    query: S::question_text(question).to_string(),
                    query_date: S::query_date(question),
                    top_k_episodes: config.retrieval.top_k_episodes,
                    top_k_observations: config.retrieval.top_k_observations,
                    include_derived_memories: config.retrieval.include_derived_memories,
                    include_threads: config.retrieval.include_threads,
                    include_entities: config.retrieval.include_entities,
                    include_debug_rationale: config.retrieval.include_debug_rationale,
                })
                .await?;
            if S::REPORT_QA_PROGRESS {
                progress.qa_retrieved(
                    item_number,
                    &item_label,
                    question_index + 1,
                    item_question_count,
                    pack.items.len(),
                );
            } else {
                progress.phase_done(
                    item_number,
                    &item_label,
                    "retrieve",
                    &format!("items={}", pack.items.len()),
                );
            }

            let context = context_metrics_with_full_history(&pack, full_history_metrics);
            let composition = composition_metrics(&pack.items);
            let integrity = integrity_details_with_telemetry(&pack.items, &pack.telemetry);
            let latency_ms = if S::LATENCY_INCLUDES_INGEST {
                item_timer.elapsed_ms()
            } else {
                question_timer.elapsed_ms()
            };
            let mut metrics = S::score(&item, question, &pack.items, &config);
            if let Some(metrics) = metrics.as_object_mut() {
                insert_common_metrics(
                    metrics,
                    &context,
                    &composition,
                    &integrity,
                    &pack.telemetry,
                    std::slice::from_ref(&metric_family),
                );
            }
            rows.push(PerQuestionResult {
                run_id: config.run_id.clone(),
                dataset: config.dataset.clone(),
                adapter: adapter_metadata.clone(),
                question_id: S::question_id(question).to_string(),
                question_type: S::question_type(question),
                question: S::question_text(question).to_string(),
                gold_episode_ids: S::gold_episode_ids(&item, question),
                gold_observation_ids: S::gold_observation_ids(&item, question),
                retrieved: pack.items,
                metrics,
                latency_ms,
                context_char_count: context.retrieved_context_chars,
                context_word_count: context.retrieved_context_words,
                context,
                telemetry: pack.telemetry,
                composition,
                integrity,
                reader: ReaderResult::default(),
            });
            completed_questions += 1;
            if S::REPORT_QA_PROGRESS {
                progress.qa_finished(
                    item_number,
                    &item_label,
                    completed_questions,
                    question_timer.elapsed_ms(),
                );
            }
        }
        namespaces_to_cleanup.push(namespace);
        progress.item_finished(item_number, &item_label, item_timer.elapsed_ms());
    }

    progress.write_outputs_started(rows.len());
    write_outputs(args, config.clone(), rows, &[metric_family])?;
    progress.cleanup_started(namespaces_to_cleanup.len());
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
}

async fn prepare_fresh_namespace(adapter: &dyn MemoryAdapter, namespace: &str) -> Result<()> {
    adapter.reset_namespace(namespace).await?;
    adapter.open_namespace(namespace).await?;
    Ok(())
}

struct ContinuitySpec;

impl DatasetSpec for ContinuitySpec {
    type Item = ContinuityScenario;
    type Question = InteractionEvent;

    const LATENCY_INCLUDES_INGEST: bool = false;
    const REPORT_QA_PROGRESS: bool = true;
    const USES_ENRICHMENT: bool = false;

    fn metric_family(config: &MetricsConfig) -> MetricFamily {
        continuity_metric_family(config, &[])
    }

    fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
        validate_dataset_name(config, "continuity")?;
        config.validate_for_dataset_kind(DatasetKind::Continuity)?;
        if !config.retrieval.include_debug_rationale {
            bail!(
                "continuity dataset requires retrieval.include_debug_rationale=true because continuity traces and rationale-derived metrics are mandatory"
            );
        }
        Ok(())
    }

    fn load(path: &Path) -> Result<Vec<Self::Item>> {
        let bytes = fs::read(path)
            .with_context(|| format!("read continuity fixture {}", path.display()))?;
        Ok(parse_fixture_bytes(&bytes)?.scenarios)
    }

    fn item_id(item: &Self::Item) -> &str {
        &item.fixture_id
    }

    fn namespace(item: &Self::Item) -> String {
        item.namespace.clone()
    }

    fn memory_inputs(_item: &Self::Item, _config: &BenchmarkRunConfig) -> MemoryBatch {
        // Continuity events are executed in order by the scripted driver rather
        // than flattened into the batch-retrieval ingestion path.
        MemoryBatch {
            episodes: Vec::new(),
            observations: Vec::new(),
            derived_memories: Vec::new(),
        }
    }

    fn questions(item: &Self::Item) -> Vec<&Self::Question> {
        item.events
            .iter()
            .filter(|event| matches!(event, InteractionEvent::Query { .. }))
            .collect()
    }

    fn question_id(question: &Self::Question) -> &str {
        match question {
            InteractionEvent::Query { query_id, .. } => query_id,
            _ => unreachable!("ContinuitySpec::questions returns query events only"),
        }
    }

    fn question_type(_question: &Self::Question) -> Option<String> {
        Some("continuity".to_string())
    }

    fn question_text(question: &Self::Question) -> &str {
        match question {
            InteractionEvent::Query { text, .. } => text,
            _ => unreachable!("ContinuitySpec::questions returns query events only"),
        }
    }

    fn query_date(question: &Self::Question) -> Option<String> {
        match question {
            InteractionEvent::Query { timestamp, .. } => Some(timestamp.to_rfc3339()),
            _ => unreachable!("ContinuitySpec::questions returns query events only"),
        }
    }

    fn gold_episode_ids(_item: &Self::Item, _question: &Self::Question) -> Vec<String> {
        Vec::new()
    }

    fn gold_observation_ids(_item: &Self::Item, _question: &Self::Question) -> Vec<String> {
        Vec::new()
    }

    fn score(
        _item: &Self::Item,
        _question: &Self::Question,
        _items: &[RetrievedItem],
        _config: &BenchmarkRunConfig,
    ) -> Value {
        Value::Object(Map::new())
    }

    fn full_history_text(item: &Self::Item) -> String {
        item.events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember { text, .. } | InteractionEvent::Query { text, .. } => {
                    Some(text.as_str())
                }
                InteractionEvent::Correct {
                    replacement_text, ..
                } => Some(replacement_text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn enrichment(
        _item: &Self::Item,
        _namespace: &str,
        _derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
        _config: &BenchmarkRunConfig,
        _configured: &HashMap<String, GraphEnrichmentInput>,
        _snapshots: &HashMap<String, GraphSnapshotInput>,
    ) -> Result<Option<GraphEnrichmentInput>> {
        Ok(None)
    }
}

enum RunnerContinuityRuntime {
    Mock {
        active: MockMemoryAdapter,
        durable: MockMemoryAdapter,
    },
    Real {
        active: Option<Box<CharacterMemoryAdapter>>,
        config: Box<BenchmarkRunConfig>,
        scenario: Box<ContinuityScenario>,
        allow_controllable_padding: bool,
        frozen_embedding_provider: Option<FrozenEmbeddingProvider>,
    },
}

impl RunnerContinuityRuntime {
    async fn new(
        selected: AdapterKind,
        config: &BenchmarkRunConfig,
        scenario: &ContinuityScenario,
        frozen_embedding_provider: Option<FrozenEmbeddingProvider>,
    ) -> Result<Self> {
        match selected {
            AdapterKind::Mock => {
                let active = MockMemoryAdapter::default();
                Ok(Self::Mock {
                    durable: active.clone(),
                    active,
                })
            }
            AdapterKind::Real => {
                let scenario_config = continuity_config_for_scenario(config, scenario);
                let allow_controllable_padding = config.backend.embedding.provider == "mixed";
                let adapter = if let Some(fixture) = scenario.embedding.controllable_similarity() {
                    if allow_controllable_padding {
                        CharacterMemoryAdapter::new_with_padded_controllable_similarity(
                            &scenario_config,
                            fixture.clone(),
                        )
                        .await?
                    } else {
                        CharacterMemoryAdapter::new_with_controllable_similarity(
                            &scenario_config,
                            fixture.clone(),
                        )
                        .await?
                    }
                } else {
                    CharacterMemoryAdapter::new_with_frozen_embedding_provider(
                        &scenario_config,
                        frozen_embedding_provider.clone().context(
                            "frozen continuity runtime is missing its preflight provider",
                        )?,
                    )
                    .await?
                };
                Ok(Self::Real {
                    active: Some(Box::new(adapter)),
                    config: Box::new(scenario_config),
                    scenario: Box::new(scenario.clone()),
                    allow_controllable_padding,
                    frozen_embedding_provider,
                })
            }
        }
    }
}

#[async_trait]
impl ContinuityRuntime for RunnerContinuityRuntime {
    fn adapter(&self) -> &dyn MemoryAdapter {
        match self {
            Self::Mock { active, .. } => active,
            Self::Real { active, .. } => active
                .as_ref()
                .expect("real continuity runtime always holds an active adapter")
                .as_ref(),
        }
    }

    async fn restart(&mut self, scenario: &ContinuityScenario) -> Result<NamespaceLifecycleResult> {
        match self {
            Self::Mock { active, durable } => {
                let replacement = durable.clone();
                let previous = std::mem::replace(active, replacement);
                drop(previous);
                active.reattach_namespace(&scenario.namespace).await
            }
            Self::Real {
                active,
                config,
                scenario: configured_scenario,
                allow_controllable_padding,
                frozen_embedding_provider,
            } => {
                let previous = active
                    .take()
                    .context("real continuity runtime lost its active adapter")?;
                drop(previous);
                let (replacement, lifecycle) = if let Some(fixture) =
                    configured_scenario.embedding.controllable_similarity()
                {
                    if *allow_controllable_padding {
                        CharacterMemoryAdapter::reconstruct_with_padded_controllable_similarity(
                            config.as_ref(),
                            &scenario.namespace,
                            fixture.clone(),
                        )
                        .await?
                    } else {
                        CharacterMemoryAdapter::reconstruct_with_controllable_similarity(
                            config.as_ref(),
                            &scenario.namespace,
                            fixture.clone(),
                        )
                        .await?
                    }
                } else {
                    CharacterMemoryAdapter::reconstruct_with_frozen_embedding_provider(
                        config.as_ref(),
                        &scenario.namespace,
                        frozen_embedding_provider.clone().context(
                            "frozen continuity runtime is missing its preflight provider",
                        )?,
                    )
                    .await?
                };
                *active = Some(Box::new(replacement));
                Ok(lifecycle)
            }
        }
    }
}

async fn run_continuity_pipeline(
    args: ContinuityRunArgs,
    config: BenchmarkRunConfig,
    fixture_schema_version: u32,
    fixture_seed: u64,
    scenarios: Vec<ContinuityScenario>,
    frozen_embedding_providers: FrozenEmbeddingProviders,
) -> Result<()> {
    let selected = args.run.selected_adapter();
    let adapter_metadata = selected.metadata();
    let metric_family = continuity_metric_family(&config.metrics, &scenarios);
    let total_queries = ContinuitySpec::total_questions(&scenarios);
    let progress = RunProgress::new(&config.dataset, scenarios.len(), Some(total_queries));
    let mut rows = Vec::with_capacity(total_queries);
    let mut traces = Vec::with_capacity(total_queries);
    let mut operation_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut restart_observations: BTreeMap<String, Vec<RestartObservation>> = BTreeMap::new();
    let mut runtimes = config
        .backend
        .cleanup
        .enabled
        .then(|| Vec::with_capacity(scenarios.len()));

    for (index, scenario) in scenarios.iter().enumerate() {
        let item_number = index + 1;
        progress.item_started(item_number, &scenario.fixture_id);
        let frozen_embedding_provider = if scenario.embedding.provider_name() == "frozen" {
            let store_path = config
                .backend
                .embedding
                .store_path
                .as_deref()
                .context("frozen continuity runtime requires backend.embedding.store_path")?;
            let provider = frozen_embedding_providers
                .get(Path::new(store_path))
                .cloned()
                .with_context(|| {
                    format!("frozen continuity runtime has no preflight provider for {store_path}")
                })?;
            Some(provider)
        } else {
            None
        };
        let mut runtime =
            RunnerContinuityRuntime::new(selected, &config, scenario, frozen_embedding_provider)
                .await?;
        let run = run_continuity_scenario(&mut runtime, scenario, &config.retrieval).await?;
        restart_observations.insert(scenario.fixture_id.clone(), run.restart_observations);
        for (operation, count) in run.operation_counts {
            *operation_counts.entry(operation).or_default() += count;
        }
        for trace in run.traces {
            let latency_ms = run
                .query_latencies_ms
                .get(&trace.query_id)
                .copied()
                .with_context(|| {
                    format!(
                        "missing measured retrieval latency for continuity query {:?}",
                        trace.query_id
                    )
                })?;
            rows.push(continuity_result_row(
                &config,
                &adapter_metadata,
                &metric_family,
                scenario,
                &trace,
                latency_ms,
            )?);
            traces.push(trace);
        }
        progress.item_finished(item_number, &scenario.fixture_id, 0);
        if let Some(runtimes) = &mut runtimes {
            runtimes.push((scenario.namespace.clone(), runtime));
        }
    }

    if let Some(parent) = args.trace_out.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = args.report_out.parent() {
        fs::create_dir_all(parent)?;
    }
    progress.write_outputs_started(rows.len());
    write_continuity_traces(&args.trace_out, &traces)?;
    let config_value = serde_json::to_value(&config)?;
    let summary = summarize_rows(
        config.run_id.clone(),
        config.dataset.clone(),
        adapter_metadata.clone(),
        config_value.clone(),
        &rows,
        std::slice::from_ref(&metric_family),
    );
    let report = assemble_continuity_report(ContinuityReportInput {
        generated_at: Utc::now(),
        fixture_schema_version,
        fixture_seed,
        config: config_value,
        adapter: adapter_metadata,
        scenarios: &scenarios,
        traces: &traces,
        rows: &rows,
        summary: &summary,
        metric_family: &metric_family,
        restart_observations: &restart_observations,
    })?;
    write_continuity_report(&args.report_out, &report)?;
    write_outputs(
        args.run,
        config.clone(),
        rows,
        std::slice::from_ref(&metric_family),
    )?;
    eprintln!(
        "[cmem-eval][continuity][operations] {}",
        serde_json::to_string(&operation_counts)?
    );

    progress.cleanup_started(runtimes.as_ref().map_or(0, Vec::len));
    if let Some(runtimes) = runtimes {
        for (namespace, runtime) in runtimes {
            runtime.adapter().cleanup_namespace(&namespace).await?;
        }
    }
    Ok(())
}

fn continuity_result_row(
    config: &BenchmarkRunConfig,
    adapter: &RunAdapterMetadata,
    metric_family: &MetricFamily,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryTrace,
    latency_ms: u128,
) -> Result<PerQuestionResult> {
    let full_history = full_history_context_metrics(Some(&trace.history_text));
    let context = context_metrics_with_full_history(&trace.retrieval, full_history);
    let composition = composition_metrics(&trace.retrieval.items);
    let integrity =
        integrity_details_with_telemetry(&trace.retrieval.items, &trace.retrieval.telemetry);
    let mut metrics = Map::new();
    insert_common_metrics(
        &mut metrics,
        &context,
        &composition,
        &integrity,
        &trace.retrieval.telemetry,
        std::slice::from_ref(metric_family),
    );
    insert_continuity_metrics(&mut metrics, scenario, trace, &config.metrics);
    let question_type = serde_json::to_value(trace.pattern)?
        .as_str()
        .map(str::to_string);
    Ok(PerQuestionResult {
        run_id: config.run_id.clone(),
        dataset: config.dataset.clone(),
        adapter: adapter.clone(),
        question_id: trace.query_id.clone(),
        question_type,
        question: trace.query.clone(),
        gold_episode_ids: Vec::new(),
        gold_observation_ids: Vec::new(),
        retrieved: trace.retrieval.items.clone(),
        metrics: Value::Object(metrics),
        latency_ms,
        context_char_count: context.retrieved_context_chars,
        context_word_count: context.retrieved_context_words,
        context,
        telemetry: trace.retrieval.telemetry.clone(),
        composition,
        integrity,
        reader: ReaderResult::default(),
    })
}

struct SyntheticSpec;

impl DatasetSpec for SyntheticSpec {
    type Item = SyntheticQuestion;
    type Question = SyntheticQuestion;

    const LATENCY_INCLUDES_INGEST: bool = true;
    const REPORT_QA_PROGRESS: bool = false;
    const USES_ENRICHMENT: bool = false;

    fn metric_family(_config: &MetricsConfig) -> MetricFamily {
        cmem_eval_core::retrieval_metric_family(
            "synthetic_retrieval",
            [
                ("session", [5, 10].as_slice()),
                ("turn", [10, 50].as_slice()),
            ],
        )
    }

    fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
        validate_dataset_name(config, "synthetic")
    }

    fn load(path: &Path) -> Result<Vec<Self::Item>> {
        Ok(read_json::<SyntheticFixture>(path)?.questions)
    }

    fn item_id(item: &Self::Item) -> &str {
        &item.question_id
    }

    fn namespace(item: &Self::Item) -> String {
        format!("synthetic:{}", item.question_id)
    }

    fn memory_inputs(item: &Self::Item, _config: &BenchmarkRunConfig) -> MemoryBatch {
        let namespace = Self::namespace(item);
        let mut episodes = Vec::with_capacity(item.sessions.len());
        let mut observations = Vec::new();
        for session in &item.sessions {
            episodes.push(EpisodeInput {
                external_id: session.session_id.clone(),
                namespace: namespace.clone(),
                summary: episode_summary(&session.session_id, &session.turns),
                started_at: session.date.clone(),
                ended_at: session.date.clone(),
                participants: participants(&session.turns),
                metadata: serde_json::json!({"source": "synthetic"}),
            });
            for (index, turn) in session.turns.iter().enumerate() {
                observations.push(ObservationInput {
                    external_id: format!("{}:turn:{}", session.session_id, index + 1),
                    episode_external_id: session.session_id.clone(),
                    namespace: namespace.clone(),
                    speaker: turn.role.clone(),
                    text: turn.content.clone(),
                    observed_at: session.date.clone(),
                    metadata: serde_json::json!({"source": "synthetic"}),
                });
            }
        }
        MemoryBatch {
            episodes,
            observations,
            derived_memories: Vec::new(),
        }
    }

    fn questions(item: &Self::Item) -> Vec<&Self::Question> {
        vec![item]
    }

    fn question_id(question: &Self::Question) -> &str {
        &question.question_id
    }

    fn question_type(_question: &Self::Question) -> Option<String> {
        None
    }

    fn question_text(question: &Self::Question) -> &str {
        &question.question
    }

    fn query_date(_question: &Self::Question) -> Option<String> {
        None
    }

    fn gold_episode_ids(_item: &Self::Item, question: &Self::Question) -> Vec<String> {
        question.gold_episode_ids.clone()
    }

    fn gold_observation_ids(_item: &Self::Item, question: &Self::Question) -> Vec<String> {
        question.gold_observation_ids.clone()
    }

    fn score(
        _item: &Self::Item,
        question: &Self::Question,
        items: &[RetrievedItem],
        _config: &BenchmarkRunConfig,
    ) -> Value {
        Value::Object(score_basic(
            items,
            &question.gold_episode_ids,
            &question.gold_observation_ids,
        ))
    }

    fn full_history_text(item: &Self::Item) -> String {
        synthetic_full_history_text(&item.sessions)
    }

    fn enrichment(
        _item: &Self::Item,
        _namespace: &str,
        _derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
        _config: &BenchmarkRunConfig,
        _configured: &HashMap<String, GraphEnrichmentInput>,
        _snapshots: &HashMap<String, GraphSnapshotInput>,
    ) -> Result<Option<GraphEnrichmentInput>> {
        Ok(None)
    }

    fn ingest_progress_detail(batch: &MemoryBatch) -> String {
        format!(
            "sessions={} turns={}",
            batch.episodes.len(),
            batch.observations.len()
        )
    }
}

struct LongMemEvalSpec;

impl DatasetSpec for LongMemEvalSpec {
    type Item = cmem_eval_longmemeval::LongMemEvalInstance;
    type Question = cmem_eval_longmemeval::LongMemEvalInstance;

    const LATENCY_INCLUDES_INGEST: bool = true;
    const REPORT_QA_PROGRESS: bool = false;
    const USES_ENRICHMENT: bool = true;

    fn metric_family(config: &MetricsConfig) -> MetricFamily {
        cmem_eval_longmemeval::metric_family(config)
    }

    fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
        cmem_eval_longmemeval::validate_config(config)
    }

    fn load(path: &Path) -> Result<Vec<Self::Item>> {
        cmem_eval_longmemeval::load_path(path)
    }

    fn item_id(item: &Self::Item) -> &str {
        &item.question_id
    }

    fn namespace(item: &Self::Item) -> String {
        item.namespace()
    }

    fn memory_inputs(item: &Self::Item, _config: &BenchmarkRunConfig) -> MemoryBatch {
        let mapped = cmem_eval_longmemeval::ingest::to_memory_inputs(item);
        MemoryBatch {
            episodes: mapped.episodes,
            observations: mapped.observations,
            derived_memories: Vec::new(),
        }
    }

    fn questions(item: &Self::Item) -> Vec<&Self::Question> {
        vec![item]
    }

    fn question_id(question: &Self::Question) -> &str {
        &question.question_id
    }

    fn question_type(question: &Self::Question) -> Option<String> {
        question.question_type.clone()
    }

    fn question_text(question: &Self::Question) -> &str {
        &question.question
    }

    fn query_date(question: &Self::Question) -> Option<String> {
        question.question_date.clone()
    }

    fn gold_episode_ids(_item: &Self::Item, question: &Self::Question) -> Vec<String> {
        question.answer_session_ids.clone()
    }

    fn gold_observation_ids(_item: &Self::Item, question: &Self::Question) -> Vec<String> {
        question.gold_turn_ids()
    }

    fn score(
        _item: &Self::Item,
        question: &Self::Question,
        items: &[RetrievedItem],
        config: &BenchmarkRunConfig,
    ) -> Value {
        cmem_eval_longmemeval::scoring::score(
            question,
            items,
            &config.metrics.ks_session,
            &config.metrics.ks_turn,
        )
    }

    fn full_history_text(item: &Self::Item) -> String {
        cmem_eval_longmemeval::full_history_text(item)
    }

    fn enrichment(
        item: &Self::Item,
        namespace: &str,
        _derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
        config: &BenchmarkRunConfig,
        configured: &HashMap<String, GraphEnrichmentInput>,
        snapshots: &HashMap<String, GraphSnapshotInput>,
    ) -> Result<Option<GraphEnrichmentInput>> {
        if let Some(snapshot) = snapshots.get(&item.question_id) {
            if snapshot.namespace != namespace {
                bail!(
                    "LongMemEval-S snapshot {} namespace {} does not match expected {}",
                    snapshot.snapshot_id,
                    snapshot.namespace,
                    namespace
                );
            }
            Ok(Some(snapshot.graph.clone()))
        } else if config.ingest.enrichment_snapshot_path.is_some() {
            bail!(
                "missing LongMemEval-S enrichment snapshot for question_id {}",
                item.question_id
            )
        } else {
            Ok(configured.get(namespace).cloned())
        }
    }
}

struct LoCoMoSpec;

impl DatasetSpec for LoCoMoSpec {
    type Item = cmem_eval_locomo::LoCoMoSample;
    type Question = cmem_eval_locomo::LoCoMoQa;

    const LATENCY_INCLUDES_INGEST: bool = false;
    const REPORT_QA_PROGRESS: bool = true;
    const USES_ENRICHMENT: bool = true;

    fn metric_family(config: &MetricsConfig) -> MetricFamily {
        cmem_eval_locomo::metric_family(config)
    }

    fn validate_config(config: &BenchmarkRunConfig) -> Result<()> {
        cmem_eval_locomo::validate_config(config)
    }

    fn load(path: &Path) -> Result<Vec<Self::Item>> {
        cmem_eval_locomo::load_path(path)
    }

    fn item_id(item: &Self::Item) -> &str {
        &item.sample_id
    }

    fn namespace(item: &Self::Item) -> String {
        item.namespace()
    }

    fn memory_inputs(item: &Self::Item, config: &BenchmarkRunConfig) -> MemoryBatch {
        let mapped = cmem_eval_locomo::ingest::to_memory_inputs(
            item,
            config.ingest.include_image_captions,
            config.ingest.index_session_summaries,
            config.ingest.index_generated_observations,
        );
        MemoryBatch {
            episodes: mapped.episodes,
            observations: mapped.observations,
            derived_memories: mapped.derived_memories,
        }
    }

    fn questions(item: &Self::Item) -> Vec<&Self::Question> {
        item.qa.iter().collect()
    }

    fn question_id(question: &Self::Question) -> &str {
        &question.question_id
    }

    fn question_type(question: &Self::Question) -> Option<String> {
        question.question_type.clone()
    }

    fn question_text(question: &Self::Question) -> &str {
        &question.question
    }

    fn query_date(_question: &Self::Question) -> Option<String> {
        None
    }

    fn gold_episode_ids(item: &Self::Item, question: &Self::Question) -> Vec<String> {
        item.evidence_sessions(question)
    }

    fn gold_observation_ids(_item: &Self::Item, question: &Self::Question) -> Vec<String> {
        question.evidence_dialog_ids.clone()
    }

    fn score(
        item: &Self::Item,
        question: &Self::Question,
        items: &[RetrievedItem],
        config: &BenchmarkRunConfig,
    ) -> Value {
        cmem_eval_locomo::scoring::score(
            item,
            question,
            items,
            &config.metrics.ks_dialog,
            &config.metrics.ks_session,
        )
    }

    fn full_history_text(item: &Self::Item) -> String {
        cmem_eval_locomo::full_history_text(item)
    }

    fn enrichment(
        item: &Self::Item,
        namespace: &str,
        derived_memories: Vec<cmem_eval_core::DerivedMemoryInput>,
        config: &BenchmarkRunConfig,
        configured: &HashMap<String, GraphEnrichmentInput>,
        snapshots: &HashMap<String, GraphSnapshotInput>,
    ) -> Result<Option<GraphEnrichmentInput>> {
        if let Some(snapshot) = snapshots.get(&item.sample_id) {
            if snapshot.namespace != namespace {
                bail!(
                    "LoCoMo snapshot {} namespace {} does not match expected {}",
                    snapshot.snapshot_id,
                    snapshot.namespace,
                    namespace
                );
            }
            return Ok(Some(snapshot.graph.clone()));
        }
        if config.ingest.enrichment_snapshot_path.is_some() {
            bail!(
                "missing LoCoMo enrichment snapshot for sample_id {}",
                item.sample_id
            );
        }
        let mut result = enrichment::empty_namespace(namespace.to_string());
        result.derived_memories = derived_memories;
        if let Some(configured) = configured.get(namespace).cloned() {
            enrichment::merge_enrichment(&mut result, configured)?;
        } else {
            enrichment::validate_enrichment(&result)?;
        }
        Ok(Some(result))
    }

    fn ingest_progress_detail(batch: &MemoryBatch) -> String {
        format!(
            "episodes={} observations={} generated_derived={}",
            batch.episodes.len(),
            batch.observations.len(),
            batch.derived_memories.len()
        )
    }
}

fn validate_dataset_name(config: &BenchmarkRunConfig, expected: &str) -> Result<()> {
    if config.dataset != expected {
        bail!(
            "config dataset {:?} does not match selected {expected} pipeline",
            config.dataset
        );
    }
    Ok(())
}

fn load_enrichment_by_namespace(
    config: &BenchmarkRunConfig,
) -> Result<HashMap<String, GraphEnrichmentInput>> {
    config
        .ingest
        .enrichment_path
        .as_ref()
        .map(|path| enrichment::load_enrichment_path(Path::new(path)))
        .transpose()
        .map(|value| value.unwrap_or_default())
}

fn load_snapshots_by_dataset_item(
    config: &BenchmarkRunConfig,
) -> Result<HashMap<String, GraphSnapshotInput>> {
    config
        .ingest
        .enrichment_snapshot_path
        .as_ref()
        .map(|path| enrichment::load_snapshot_path(Path::new(path)))
        .transpose()
        .map(|value| value.unwrap_or_default())
}

fn insert_common_metrics(
    metrics: &mut Map<String, Value>,
    context: &ResultContextMetrics,
    composition: &cmem_eval_core::ResultCompositionMetrics,
    integrity: &cmem_eval_core::ResultIntegrityDetails,
    telemetry: &cmem_eval_core::RetrievalTelemetry,
    metric_families: &[MetricFamily],
) {
    initialize_registry_metrics_for(metrics, metric_families);
    insert_context_metrics(metrics, context);
    insert_composition_metrics(metrics, composition);
    insert_integrity_detail_metrics(metrics, integrity);
    insert_telemetry_metrics(metrics, telemetry);
}

#[derive(Debug, Clone, Copy)]
struct FullHistoryContextMetrics {
    chars: Option<usize>,
    words: Option<usize>,
    tokens: Option<usize>,
}

fn full_history_context_metrics(full_history_text: Option<&str>) -> FullHistoryContextMetrics {
    FullHistoryContextMetrics {
        chars: full_history_text.map(|text| text.chars().count()),
        words: full_history_text.map(estimate_word_count),
        tokens: full_history_text.map(count_tokens),
    }
}

fn context_metrics_with_full_history(
    pack: &RetrievedContextPack,
    full_history: FullHistoryContextMetrics,
) -> ResultContextMetrics {
    let retrieved_context_tokens = count_tokens(&pack.context_text);
    let compression_ratio = match (full_history.tokens, retrieved_context_tokens) {
        (Some(full), retrieved) if retrieved > 0 => Some(full as f64 / retrieved as f64),
        _ => None,
    };
    let reduction_rate = match (full_history.tokens, retrieved_context_tokens) {
        (Some(full), retrieved) if full > 0 => Some(1.0 - retrieved as f64 / full as f64),
        _ => None,
    };
    ResultContextMetrics {
        retrieved_context_chars: pack.context_char_count,
        retrieved_context_words: pack.context_word_count,
        retrieved_context_tokens,
        full_history_chars: full_history.chars,
        full_history_words: full_history.words,
        full_history_tokens: full_history.tokens,
        compression_ratio,
        reduction_rate,
    }
}

fn score_basic(
    items: &[RetrievedItem],
    gold_episode_ids: &[String],
    gold_observation_ids: &[String],
) -> Map<String, Value> {
    let episode_ids = items
        .iter()
        .filter(|item| item.kind == "episode")
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let observation_ids = items
        .iter()
        .filter(|item| item.kind == "observation")
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let mut out = Map::new();
    insert_retrieval_metrics(&mut out, "session", &episode_ids, gold_episode_ids, 5);
    insert_retrieval_metrics(&mut out, "session", &episode_ids, gold_episode_ids, 10);
    insert_retrieval_metrics(&mut out, "turn", &observation_ids, gold_observation_ids, 10);
    insert_retrieval_metrics(&mut out, "turn", &observation_ids, gold_observation_ids, 50);
    out
}

fn synthetic_full_history_text(sessions: &[SyntheticSession]) -> String {
    sessions
        .iter()
        .flat_map(|session| {
            session.turns.iter().map(|turn| {
                format!(
                    "{}: {}",
                    turn.role.as_deref().unwrap_or("unknown"),
                    turn.content
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn episode_summary(session_id: &str, turns: &[SyntheticTurn]) -> String {
    let roles = participants(turns).join(", ");
    format!("Conversation session {session_id} containing messages between {roles}.")
}

fn participants(turns: &[SyntheticTurn]) -> Vec<String> {
    let mut roles = turns
        .iter()
        .filter_map(|turn| turn.role.clone())
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();
    roles
}

async fn adapter(kind: AdapterKind, config: &BenchmarkRunConfig) -> Result<Box<dyn MemoryAdapter>> {
    match kind {
        AdapterKind::Mock => Ok(Box::<MockMemoryAdapter>::default()),
        AdapterKind::Real => Ok(Box::new(CharacterMemoryAdapter::new(config).await?)),
    }
}

async fn cleanup_namespaces_after_artifacts(
    adapter: &dyn MemoryAdapter,
    config: &BenchmarkRunConfig,
    namespaces: &[String],
) -> Result<()> {
    if config.backend.cleanup.enabled {
        for namespace in namespaces {
            adapter.cleanup_namespace(namespace).await?;
        }
    }
    Ok(())
}

fn write_outputs(
    args: RunArgs,
    config: BenchmarkRunConfig,
    rows: Vec<PerQuestionResult>,
    metric_families: &[MetricFamily],
) -> Result<()> {
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = args.summary_out.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = summarize_rows(
        config.run_id.clone(),
        config.dataset.clone(),
        rows.first()
            .map(|row| row.adapter.clone())
            .unwrap_or_else(RunAdapterMetadata::live),
        serde_json::to_value(&config)?,
        &rows,
        metric_families,
    );
    write_jsonl(&args.out, &rows)?;
    write_summary(&args.summary_out, &summary)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = fs::read_to_string(path)
        .map_err(anyhow::Error::from)
        .map_err(|error| anyhow::anyhow!("read {}: {error}", path.display()))?;
    Ok(serde_json::from_str(&content)?)
}

struct RunProgress {
    dataset: String,
    total_items: usize,
    total_qa: Option<usize>,
    started_at: Instant,
}

impl RunProgress {
    fn new(dataset: &str, total_items: usize, total_qa: Option<usize>) -> Self {
        let progress = Self {
            dataset: dataset.to_string(),
            total_items,
            total_qa,
            started_at: Instant::now(),
        };
        match total_qa {
            Some(total_qa) => eprintln!(
                "[cmem-eval][{}][start] items={} qa={} elapsed_ms=0",
                progress.dataset, total_items, total_qa
            ),
            None => eprintln!(
                "[cmem-eval][{}][start] items={} elapsed_ms=0",
                progress.dataset, total_items
            ),
        }
        progress
    }

    fn item_started(&self, index: usize, label: &str) {
        eprintln!(
            "[cmem-eval][{}][item {}/{}][start] id={} elapsed_ms={}",
            self.dataset,
            index,
            self.total_items,
            label,
            self.elapsed_ms()
        );
    }

    fn phase_done(&self, index: usize, label: &str, phase: &str, detail: &str) {
        eprintln!(
            "[cmem-eval][{}][item {}/{}][{}] id={} {} elapsed_ms={}",
            self.dataset,
            index,
            self.total_items,
            phase,
            label,
            detail,
            self.elapsed_ms()
        );
    }

    fn item_finished(&self, index: usize, label: &str, item_latency_ms: u128) {
        eprintln!(
            "[cmem-eval][{}][item {}/{}][done] id={} item_latency_ms={} elapsed_ms={}",
            self.dataset,
            index,
            self.total_items,
            label,
            item_latency_ms,
            self.elapsed_ms()
        );
    }

    fn qa_started(
        &self,
        sample_index: usize,
        sample_label: &str,
        qa_index: usize,
        sample_qa: usize,
    ) {
        eprintln!(
            "[cmem-eval][{}][item {}/{}][qa {}/{}][start] sample_id={} elapsed_ms={}",
            self.dataset,
            sample_index,
            self.total_items,
            qa_index,
            sample_qa,
            sample_label,
            self.elapsed_ms()
        );
    }

    fn qa_retrieved(
        &self,
        sample_index: usize,
        sample_label: &str,
        qa_index: usize,
        sample_qa: usize,
        retrieved_items: usize,
    ) {
        eprintln!(
            "[cmem-eval][{}][item {}/{}][qa {}/{}][retrieve] sample_id={} items={} elapsed_ms={}",
            self.dataset,
            sample_index,
            self.total_items,
            qa_index,
            sample_qa,
            sample_label,
            retrieved_items,
            self.elapsed_ms()
        );
    }

    fn qa_finished(
        &self,
        sample_index: usize,
        sample_label: &str,
        completed_qa: usize,
        qa_latency_ms: u128,
    ) {
        let total_qa = self.total_qa.unwrap_or(completed_qa);
        eprintln!(
            "[cmem-eval][{}][item {}/{}][qa-progress {}/{}][done] sample_id={} qa_latency_ms={} elapsed_ms={}",
            self.dataset,
            sample_index,
            self.total_items,
            completed_qa,
            total_qa,
            sample_label,
            qa_latency_ms,
            self.elapsed_ms()
        );
    }

    fn write_outputs_started(&self, rows: usize) {
        eprintln!(
            "[cmem-eval][{}][write_outputs] rows={} elapsed_ms={}",
            self.dataset,
            rows,
            self.elapsed_ms()
        );
    }

    fn cleanup_started(&self, namespaces: usize) {
        eprintln!(
            "[cmem-eval][{}][cleanup] namespaces={} elapsed_ms={}",
            self.dataset,
            namespaces,
            self.elapsed_ms()
        );
    }

    fn elapsed_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
    }
}

#[derive(Debug, Deserialize)]
struct SyntheticFixture {
    questions: Vec<SyntheticQuestion>,
}

#[derive(Debug, Deserialize)]
struct SyntheticQuestion {
    question_id: String,
    question: String,
    gold_episode_ids: Vec<String>,
    gold_observation_ids: Vec<String>,
    sessions: Vec<SyntheticSession>,
}

#[derive(Debug, Deserialize)]
struct SyntheticSession {
    session_id: String,
    date: Option<String>,
    turns: Vec<SyntheticTurn>,
}

#[derive(Debug, Deserialize)]
struct SyntheticTurn {
    role: Option<String>,
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mock_args(dataset: PathBuf, config: PathBuf, directory: &Path) -> RunArgs {
        RunArgs {
            dataset,
            config,
            out: directory.join("results.jsonl"),
            summary_out: directory.join("summary.json"),
            adapter: Some(AdapterKind::Mock),
            allow_mock_benchmark: true,
        }
    }

    fn continuity_mock_args(directory: &Path) -> ContinuityRunArgs {
        let source_config = fs::read_to_string("../../configs/continuity_retrieval.toml").unwrap();
        let store_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cmem-eval-continuity/fixtures/embeddings/task22_real_store.json")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        let config = source_config.replace(
            "store_path = \"crates/cmem-eval-continuity/fixtures/embeddings/task22_real_store.json\"",
            &format!("store_path = \"{store_path}\""),
        );
        let config_path = directory.join("continuity-config.toml");
        fs::write(&config_path, config).unwrap();
        ContinuityRunArgs {
            run: RunArgs {
                dataset: PathBuf::from("../cmem-eval-continuity/fixtures/continuity_v3.json"),
                config: config_path,
                out: directory.join("continuity.jsonl"),
                summary_out: directory.join("continuity-summary.json"),
                adapter: Some(AdapterKind::Mock),
                allow_mock_benchmark: true,
            },
            trace_out: directory.join("continuity-traces.jsonl"),
            report_out: directory.join("continuity-report.json"),
            scenario: None,
        }
    }

    fn frozen_mock_args(directory: &Path, omit_runtime_text: bool) -> ContinuityRunArgs {
        let mut fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        fixture.scenarios.truncate(1);
        fixture.scenarios[0].embedding =
            cmem_eval_continuity::ContinuityScenarioEmbedding::frozen();
        let mut runtime_texts = fixture.scenarios[0].runtime_embedding_inputs();
        if omit_runtime_text {
            runtime_texts.pop_last().unwrap();
        }
        let store = cmem_eval_core::FrozenEmbeddingStore::new(
            "test-frozen-model",
            FrozenEmbeddingSource::TestFixture,
            runtime_texts
                .into_iter()
                .map(|text| (text, vec![1.0, 0.0, 0.0])),
        )
        .unwrap();
        let store_path = directory.join("test-provenance-store.json");
        fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();

        let fixture_path = directory.join("frozen-continuity.json");
        fs::write(
            &fixture_path,
            cmem_eval_continuity::canonical_fixture_bytes(&fixture).unwrap(),
        )
        .unwrap();

        let source_config = fs::read_to_string("../../configs/continuity_retrieval.toml").unwrap();
        let store_path = store_path.display().to_string().replace('\\', "/");
        let config = source_config
            .replace("provider = \"mixed\"", "provider = \"frozen\"")
            .replace(
                "model = \"text-embedding-3-large\"",
                "model = \"test-frozen-model\"",
            )
            .replace("vector_size = 3072", "vector_size = 3")
            .replace(
                "crates/cmem-eval-continuity/fixtures/embeddings/task22_real_store.json",
                &store_path,
            );
        let config_path = directory.join("frozen-continuity.toml");
        fs::write(&config_path, config).unwrap();

        ContinuityRunArgs {
            run: RunArgs {
                dataset: fixture_path,
                config: config_path,
                out: directory.join("frozen-continuity.jsonl"),
                summary_out: directory.join("frozen-continuity-summary.json"),
                adapter: Some(AdapterKind::Mock),
                allow_mock_benchmark: true,
            },
            trace_out: directory.join("frozen-continuity-traces.jsonl"),
            report_out: directory.join("frozen-continuity-report.json"),
            scenario: None,
        }
    }

    fn service_free_config(source: &str, directory: &Path) -> PathBuf {
        let config = fs::read_to_string(source).unwrap();
        let config = config
            .lines()
            .filter(|line| {
                !line.trim_start().starts_with("enrichment_snapshot_path")
                    && !line.trim_start().starts_with("enrichment_manifest_path")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let path = directory.join("config.toml");
        fs::write(&path, config).unwrap();
        path
    }

    #[tokio::test]
    async fn fresh_namespace_preparation_discards_stale_state() {
        let adapter = MockMemoryAdapter::default();
        adapter.open_namespace("stale").await.unwrap();
        adapter
            .remember_episode(EpisodeInput {
                external_id: "old".into(),
                namespace: "stale".into(),
                summary: "stale durable state".into(),
                started_at: None,
                ended_at: None,
                participants: Vec::new(),
                metadata: Value::Null,
            })
            .await
            .unwrap();

        prepare_fresh_namespace(&adapter, "stale").await.unwrap();

        assert_eq!(
            adapter
                .reattach_namespace("stale")
                .await
                .unwrap()
                .restored_identity_count,
            0
        );
    }

    #[tokio::test]
    async fn synthetic_command_writes_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("synthetic.jsonl");
        let summary = dir.path().join("synthetic_summary.json");
        let resummary = dir.path().join("synthetic_resummary.json");
        let config = PathBuf::from("../../configs/synthetic_retrieval.toml");
        run_synthetic(RunArgs {
            dataset: PathBuf::from("../../fixtures/synthetic_small.json"),
            config: config.clone(),
            out: out.clone(),
            summary_out: summary.clone(),
            adapter: Some(AdapterKind::Mock),
            allow_mock_benchmark: true,
        })
        .await
        .unwrap();
        assert!(out.exists());
        assert!(summary.exists());

        crate::commands::summarize(crate::commands::SummarizeArgs {
            input: out,
            config,
            out: resummary.clone(),
            dataset: None,
            scenario: None,
        })
        .unwrap();
        let original = cmem_eval_core::read_summary(&summary).unwrap();
        let regenerated = cmem_eval_core::read_summary(&resummary).unwrap();
        assert_eq!(regenerated.embedding_provider.as_deref(), Some("openai"));
        assert_eq!(regenerated.config, original.config);
        assert_eq!(regenerated.registry_coverage, original.registry_coverage);
    }

    #[tokio::test]
    async fn longmemeval_command_runs_through_generic_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let dataset = dir.path().join("longmemeval.json");
        fs::write(
            &dataset,
            serde_json::to_vec(&serde_json::json!([{
                "question_id": "q1",
                "question": "What does the user like?",
                "haystack_session_ids": ["s1"],
                "haystack_sessions": [[{
                    "role": "user",
                    "content": "I like jasmine tea",
                    "has_answer": true
                }]],
                "answer_session_ids": ["s1"]
            }]))
            .unwrap(),
        )
        .unwrap();
        let args = mock_args(
            dataset,
            service_free_config("../../configs/longmemeval_s_retrieval.toml", dir.path()),
            dir.path(),
        );
        let output = args.out.clone();

        run_longmemeval(args).await.unwrap();

        let rows = cmem_eval_core::read_jsonl(&output).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].question_id, "q1");
        assert_eq!(rows[0].gold_episode_ids, vec!["s1"]);
        assert_eq!(rows[0].gold_observation_ids, vec!["s1:turn:1"]);
    }

    #[tokio::test]
    async fn locomo_command_runs_multi_question_item_through_generic_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let dataset = dir.path().join("locomo.json");
        fs::write(
            &dataset,
            serde_json::to_vec(&serde_json::json!([{
                "sample_id": "p1",
                "conversation": [{
                    "session_id": "s1",
                    "turns": [{"dia_id": "d1", "speaker": "A", "text": "likes tea"}]
                }],
                "qa": [
                    {"question_id": "q1", "question": "What?", "evidence": ["d1"]},
                    {"question_id": "q2", "question": "Who?", "evidence": ["d1"]}
                ]
            }]))
            .unwrap(),
        )
        .unwrap();
        let args = mock_args(
            dataset,
            service_free_config("../../configs/locomo_retrieval.toml", dir.path()),
            dir.path(),
        );
        let output = args.out.clone();

        run_locomo(args).await.unwrap();

        let rows = cmem_eval_core::read_jsonl(&output).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].question_id, "q1");
        assert_eq!(rows[1].question_id, "q2");
        assert_eq!(rows[0].gold_episode_ids, vec!["s1"]);
        assert_eq!(rows[0].gold_observation_ids, vec!["d1"]);
    }

    #[tokio::test]
    async fn continuity_command_runs_scripted_scenarios_and_writes_full_traces() {
        let directory = tempfile::tempdir().unwrap();
        let second_directory = tempfile::tempdir().unwrap();
        let args = continuity_mock_args(directory.path());
        let second_args = continuity_mock_args(second_directory.path());
        let result_path = args.run.out.clone();
        let summary_path = args.run.summary_out.clone();
        let resummary_path = directory.path().join("continuity-resummary.json");
        let trace_path = args.trace_out.clone();
        let report_path = args.report_out.clone();
        let config_path = args.run.config.clone();
        let dataset_path = args.run.dataset.clone();

        run_continuity(args).await.unwrap();
        run_continuity(second_args).await.unwrap();

        let rows = cmem_eval_core::read_jsonl(&result_path).unwrap();
        let traces = cmem_eval_continuity::read_continuity_traces(&trace_path).unwrap();
        let wrong_scenario_error =
            validate_continuity_summary_rows(&rows[..1], &dataset_path, Some("cross-store-stress"))
                .unwrap_err()
                .to_string();
        assert!(
            wrong_scenario_error.contains("row/query mismatch at index 0"),
            "{wrong_scenario_error}"
        );
        crate::commands::summarize(crate::commands::SummarizeArgs {
            input: result_path.clone(),
            config: config_path.clone(),
            out: resummary_path.clone(),
            dataset: Some(dataset_path.clone()),
            scenario: None,
        })
        .unwrap();
        let summary = cmem_eval_core::read_summary(&summary_path).unwrap();
        let resummary = cmem_eval_core::read_summary(&resummary_path).unwrap();
        let report: cmem_eval_continuity::ContinuityReport =
            serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
        let second_report: cmem_eval_continuity::ContinuityReport = serde_json::from_slice(
            &fs::read(second_directory.path().join("continuity-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 23);
        assert_eq!(traces.len(), 23);
        assert_eq!(summary.num_questions, 23);
        assert_eq!(resummary.config, summary.config);
        assert_eq!(resummary.metric_support, summary.metric_support);
        assert_eq!(resummary.registry_coverage, summary.registry_coverage);
        assert_eq!(report.content.aggregate.query_count, 23);
        assert_eq!(report.content.aggregate.restart_count, 1);
        assert_eq!(report.content, second_report.content);
        assert_eq!(
            report.schema_version,
            cmem_eval_continuity::CONTINUITY_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.metadata.embedding_seeds.len(), 13);
        assert_eq!(
            report.metadata.normalization.nondeterministic_paths,
            vec!["metadata.generated_at"]
        );
        assert_eq!(
            report
                .metadata
                .normalization
                .excluded_nondeterministic_sources,
            vec![
                "correction and forget library mutation timestamps",
                "measured query retrieval latency in results and summaries",
            ]
        );
        assert_eq!(
            report.metadata.config["retrieval"]["max_graph_roots"],
            serde_json::json!(48)
        );
        assert_eq!(
            report.metadata.schema_versions["continuity_report"],
            cmem_eval_continuity::CONTINUITY_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.content.scenarios.len(), 15);
        assert!(report.content.scenarios.values().all(|scenario| {
            scenario.query_count >= 1
                && scenario.rationale_samples.len() == scenario.query_count
                && scenario.fanout_decisions.len() == scenario.query_count
                && scenario.stats_health_events.len() == scenario.query_count
                && scenario.registry_coverage["missing_required_metrics"] == serde_json::json!([])
        }));
        assert!(report.content.tuning_observations.is_empty());
        assert!(
            report.content.scenarios["cross-store-stress"].restart_observations[0]
                .delta
                .stable_returned_objects
        );
        let sample = &report.content.scenarios["cross-store-stress"].rationale_samples[0];
        assert_eq!(sample.query, "What marker must survive the restart?");
        assert!(!sample.context_pack.items.is_empty());
        let sample_value = serde_json::to_value(sample).unwrap();
        assert!(sample_value.pointer("/context_pack/items/0/kind").is_some());
        assert!(sample_value.pointer("/context_pack/items/0/text").is_some());
        assert!(
            sample_value
                .pointer("/context_pack/items/0/score")
                .is_some()
        );
        assert!(
            sample_value
                .pointer("/context_pack/telemetry/selectivity_decisions")
                .is_some()
        );
        let fixture = parse_fixture_bytes(&fs::read(dataset_path).unwrap()).unwrap();
        let report_config = read_config(&config_path).unwrap();
        let report_metric_family =
            continuity_metric_family(&report_config.metrics, &fixture.scenarios);
        let valid_restart_observations = report
            .content
            .scenarios
            .iter()
            .filter(|(_, scenario)| !scenario.restart_observations.is_empty())
            .map(|(fixture_id, scenario)| {
                (fixture_id.clone(), scenario.restart_observations.clone())
            })
            .collect::<BTreeMap<_, _>>();
        let measured_row = continuity_result_row(
            &report_config,
            &RunAdapterMetadata::mock_smoke(),
            &report_metric_family,
            &fixture.scenarios[0],
            &traces[0],
            37,
        )
        .unwrap();
        assert_eq!(measured_row.latency_ms, 37);
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &rows[..rows.len() - 1],
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("23 traces but 22 result rows"), "{error}");
        let mut swapped_rows = cmem_eval_core::read_jsonl(&result_path).unwrap();
        swapped_rows.swap(0, 1);
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &swapped_rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("trace/result mismatch at index 0"),
            "{error}"
        );
        let mut invented_traces = traces.clone();
        invented_traces[0].query_id = "invented-query".to_string();
        let mut invented_rows = cmem_eval_core::read_jsonl(&result_path).unwrap();
        invented_rows[0].question_id = "invented-query".to_string();
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &invented_traces,
            rows: &invented_rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("scripted query mismatch at index 0"),
            "{error}"
        );
        let mut duplicate_traces = traces.clone();
        duplicate_traces[1].query_id = duplicate_traces[0].query_id.clone();
        let mut duplicate_rows = cmem_eval_core::read_jsonl(&result_path).unwrap();
        duplicate_rows[1].question_id = duplicate_rows[0].question_id.clone();
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &duplicate_traces,
            rows: &duplicate_rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("scripted query mismatch at index 1"),
            "{error}"
        );

        let mut unknown_restart_observations = valid_restart_observations.clone();
        unknown_restart_observations.insert("unknown-fixture".to_string(), Vec::new());
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &unknown_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("unknown-fixture"), "{error}");
        assert!(error.contains("unknown or unselected"), "{error}");

        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &BTreeMap::new(),
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("cross-store-stress"), "{error}");
        assert!(error.contains("0 restart observations"), "{error}");
        assert!(error.contains("scripts 1 restart events"), "{error}");

        let mut stale_summary = cmem_eval_core::read_summary(&summary_path).unwrap();
        stale_summary.num_questions -= 1;
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: stale_summary.config.clone(),
            adapter: stale_summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &rows,
            summary: &stale_summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("identity/count"), "{error}");

        let mut stale_summary = cmem_eval_core::read_summary(&summary_path).unwrap();
        stale_summary.metrics["continuity_gap_days"]["mean"] = serde_json::json!(-1.0);
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: stale_summary.config.clone(),
            adapter: stale_summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &rows,
            summary: &stale_summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("summary metrics"), "{error}");
        assert_eq!(
            summary.registry_coverage["missing_required_metrics"],
            serde_json::json!([])
        );
        assert_eq!(
            summary.metric_support["fanout_over_budget_count"]["unsupported"],
            true
        );
        assert!(rows.iter().all(|row| {
            row.metrics["typed_rationale_coverage"].is_null()
                && row.metrics["fanout_over_budget_count"].is_null()
        }));
        assert!(rows.iter().any(|row| {
            row.metrics.as_object().is_some_and(|metrics| {
                metrics.iter().any(|(key, value)| {
                    key.starts_with("continuity_recall_fraction_gap_") && value.is_number()
                })
            })
        }));
        assert!(traces.iter().all(|trace| {
            !trace.history_text.is_empty()
                && !trace.retrieval.context_text.is_empty()
                && trace
                    .retrieval
                    .items
                    .iter()
                    .all(|item| !item.rationale.is_empty())
        }));
    }

    #[test]
    fn continuity_spec_is_the_production_dataset_kind_validation_caller() {
        let mut config =
            read_config(&PathBuf::from("../../configs/continuity_retrieval.toml")).unwrap();
        config.backend.embedding.provider = "openai".to_string();
        let error = ContinuitySpec::validate_config(&config)
            .unwrap_err()
            .to_string();
        assert!(error.contains("controllable_similarity, frozen, or mixed"));
        assert!(error.contains("openai"));
    }

    #[test]
    fn continuity_spec_requires_debug_rationale_for_mandatory_traces() {
        let mut config =
            read_config(&PathBuf::from("../../configs/continuity_retrieval.toml")).unwrap();
        config.retrieval.include_debug_rationale = false;

        let error = ContinuitySpec::validate_config(&config)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("retrieval.include_debug_rationale=true"),
            "{error}"
        );
        assert!(error.contains("traces"), "{error}");
        assert!(error.contains("metrics"), "{error}");
    }

    #[test]
    fn continuity_fixture_dimensions_must_match_the_config_before_adapter_selection() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let scenarios = &fixture.scenarios[..1];
        let mut config =
            read_config(&PathBuf::from("../../configs/continuity_retrieval.toml")).unwrap();
        config.backend.embedding.provider = "controllable_similarity".to_string();

        config.backend.embedding.vector_size = None;
        let error = validate_continuity_embedding_sizes(&config, scenarios, None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("requires backend.embedding.vector_size"),
            "{error}"
        );

        config.backend.embedding.vector_size =
            Some(scenarios[0].embedding.vector_size().unwrap() + 1);
        let error = validate_continuity_embedding_sizes(&config, scenarios, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("is incompatible"), "{error}");
        assert!(error.contains(&scenarios[0].fixture_id), "{error}");

        config.backend.embedding.vector_size = Some(scenarios[0].embedding.vector_size().unwrap());
        validate_continuity_embedding_sizes(&config, scenarios, None).unwrap();
    }

    #[test]
    fn frozen_scenario_preflight_gates_provenance_on_real_and_coverage_on_mock() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let mut scenario = fixture.scenarios[0].clone();
        scenario.embedding = cmem_eval_continuity::ContinuityScenarioEmbedding::frozen();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cmem-eval-continuity/fixtures/embeddings");
        let mut config =
            read_config(&PathBuf::from("../../configs/continuity_retrieval.toml")).unwrap();
        config.backend.embedding.provider = "frozen".to_string();
        config.backend.embedding.model = "task21-smoke-model".to_string();
        config.backend.embedding.vector_size = Some(3);
        config.backend.embedding.store_path = Some(
            fixtures
                .join("task21_smoke_store.json")
                .display()
                .to_string(),
        );

        let error = validate_continuity_embedding_sizes(
            &config,
            &[scenario.clone()],
            Some(AdapterKind::Real),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("source=open_ai_api"), "{error}");
        assert!(error.contains("TestFixture"), "{error}");

        let directory = tempfile::tempdir().unwrap();
        let store_path = directory.path().join("partial-openai-store.json");
        let store = cmem_eval_core::FrozenEmbeddingStore::new(
            "task21-smoke-model",
            FrozenEmbeddingSource::TestFixture,
            [("unrelated cached text".to_string(), vec![1.0, 0.0, 0.0])],
        )
        .unwrap();
        fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();
        config.backend.embedding.store_path = Some(store_path.display().to_string());
        let error =
            validate_continuity_embedding_sizes(&config, &[scenario], Some(AdapterKind::Mock))
                .unwrap_err()
                .to_string();
        assert!(error.contains("preflight frozen embeddings"), "{error}");
        assert!(error.contains("frozen embedding cache miss"), "{error}");
    }

    #[test]
    fn real_preflight_rejects_explicit_nonstandard_store_width() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let mut scenario = fixture.scenarios[0].clone();
        scenario.embedding = cmem_eval_continuity::ContinuityScenarioEmbedding::frozen();
        let directory = tempfile::tempdir().unwrap();
        let store_path = directory.path().join("nonstandard-openai-store.json");
        let store = cmem_eval_core::FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            cmem_eval_core::FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            scenario
                .runtime_embedding_inputs()
                .into_iter()
                .map(|text| (text, vec![0.0; 1_024])),
        )
        .unwrap();
        fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();
        let mut config =
            read_config(&PathBuf::from("../../configs/continuity_retrieval.toml")).unwrap();
        config.backend.embedding.provider = "frozen".to_string();
        config.backend.embedding.model = "text-embedding-3-large".to_string();
        config.backend.embedding.vector_size = Some(1_024);
        config.backend.embedding.store_path = Some(store_path.display().to_string());

        let error =
            validate_continuity_embedding_sizes(&config, &[scenario], Some(AdapterKind::Real))
                .unwrap_err()
                .to_string();

        assert!(error.contains("vector_size 1024"), "{error}");
        assert!(error.contains("canonical width 3072"), "{error}");
        assert!(error.contains("--allow-nonstandard-dimensions"), "{error}");
        assert!(error.contains("live Character Memory"), "{error}");
    }

    #[tokio::test]
    async fn mock_continuity_accepts_test_provenance_store_with_complete_coverage() {
        let directory = tempfile::tempdir().unwrap();

        run_continuity(frozen_mock_args(directory.path(), false))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn mock_continuity_rejects_test_provenance_store_with_cache_miss() {
        let directory = tempfile::tempdir().unwrap();

        let error = run_continuity(frozen_mock_args(directory.path(), true))
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("preflight frozen embeddings"), "{error}");
        assert!(error.contains("frozen embedding cache miss"), "{error}");
    }

    #[test]
    fn committed_benchmark_store_is_exactly_the_runtime_lookup_set() {
        let fixture_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../cmem-eval-continuity/fixtures");
        let fixture = parse_fixture_bytes(
            &fs::read(fixture_root.join("continuity_benchmarks_v1.json")).unwrap(),
        )
        .unwrap();
        let runtime_texts = fixture
            .scenarios
            .iter()
            .flat_map(ContinuityScenario::runtime_embedding_inputs)
            .collect::<BTreeSet<_>>();
        let manifest = cmem_eval_core::FrozenEmbeddingManifest::load(
            &fixture_root.join("embeddings/continuity_benchmarks_v1_manifest.json"),
        )
        .unwrap();
        let manifest_texts = manifest
            .texts
            .iter()
            .map(|item| item.text.clone())
            .collect::<BTreeSet<_>>();
        let store = cmem_eval_core::FrozenEmbeddingStore::load(
            &fixture_root.join("embeddings/continuity_benchmarks_v1_store.json"),
        )
        .unwrap();
        let store_texts = store
            .entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(runtime_texts.len(), 635);
        assert_eq!(runtime_texts, manifest_texts);
        assert_eq!(runtime_texts, store_texts);
    }

    #[test]
    fn mixed_provider_allows_controllable_fixture_padding() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let mut config =
            read_config(&PathBuf::from("../../configs/continuity_retrieval.toml")).unwrap();
        config.backend.embedding.provider = "mixed".to_string();
        config.backend.embedding.model = "text-embedding-3-large".to_string();
        config.backend.embedding.vector_size = Some(3072);
        config.backend.embedding.store_path = Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../cmem-eval-continuity/fixtures/embeddings/task22_real_store.json")
                .display()
                .to_string(),
        );

        validate_continuity_embedding_sizes(&config, &fixture.scenarios, None).unwrap();
    }
}
