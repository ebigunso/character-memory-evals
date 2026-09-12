use crate::commands::{ContinuityRunArgs, RunArgs, read_config};
use crate::enrichment;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use cmem_eval::CharacterMemoryAdapter;
use cmem_eval::{
    BenchmarkRunConfig, ControllableDimensionPolicy, DatasetId, DatasetKind,
    EmbeddingBindingRecord, EmbeddingProviderConfig, EmbeddingRuntimeBinding, EpisodeInput,
    FrozenEmbeddingProvider, FrozenEmbeddingSource, GraphEnrichmentInput, GraphSnapshotInput,
    LiveEmbeddingProvider, MetricFamily, MetricsConfig, MetricsRecord, ObservationInput,
    PerQuestionResult, ResultContextMetrics, RetrieveInput, RetrievedContextPack, RetrievedItem,
    RunAdapterMetadata, Timer, classify_frozen_embedding_dimensions, composition_metrics,
    count_tokens, estimate_word_count, initialize_registry_metrics_for, insert_composition_metrics,
    insert_context_metrics, insert_integrity_detail_metrics, integrity_details_from_outcomes,
    summarize_rows, write_jsonl, write_summary,
};
use cmem_eval_continuity::{
    ContinuityQueryTrace, ContinuityReportInput, ContinuityRuntime, ContinuityScenario,
    InteractionEvent, RestartObservation, assemble_continuity_report, continuity_metric_family,
    insert_continuity_metrics, parse_fixture_bytes, run_continuity_scenario,
    write_continuity_report, write_continuity_traces,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

type FrozenEmbeddingProviders = HashMap<PathBuf, FrozenEmbeddingProvider>;

#[derive(Debug, Clone, Copy)]
struct DatasetDescriptor {
    id: &'static str,
    kind: DatasetKind,
}

const DATASET_REGISTRY: &[DatasetDescriptor] = &[
    DatasetDescriptor {
        id: "continuity",
        kind: DatasetKind::Continuity,
    },
    DatasetDescriptor {
        id: "longmemeval_s",
        kind: DatasetKind::LongMemEvalS,
    },
    DatasetDescriptor {
        id: "locomo",
        kind: DatasetKind::LoCoMo,
    },
];

fn dataset_descriptor(dataset: &DatasetId) -> Result<DatasetDescriptor> {
    DATASET_REGISTRY
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == dataset.as_str())
        .with_context(|| format!("unsupported dataset {:?}", dataset.as_str()))
}

fn live_embedding_binding(config: &BenchmarkRunConfig) -> Result<EmbeddingBindingRecord> {
    let provider = match config.backend.embedding.provider {
        EmbeddingProviderConfig::Deterministic => LiveEmbeddingProvider::Deterministic,
        EmbeddingProviderConfig::OpenAi => LiveEmbeddingProvider::OpenAi,
        provider => bail!(
            "dataset runtime requires a live embedding provider, got {provider}; continuity scenario bindings must be resolved independently"
        ),
    };
    let vector_size = match config.backend.embedding.vector_size {
        Some(vector_size) => vector_size,
        None => cmem_eval::model_native_embedding_vector_size(&config.backend.embedding.model)?,
    };
    Ok(EmbeddingBindingRecord::Live {
        provider,
        model: config.backend.embedding.model.clone(),
        vector_size,
    })
}

pub(crate) async fn run_continuity(args: ContinuityRunArgs) -> Result<()> {
    let config = read_config(&args.run.config)?;
    ContinuitySpec::validate_config(&config)?;
    let fixture = load_continuity_fixture(&args.run.dataset)?;
    let fixture_schema_version = fixture.schema_version;
    let fixture_seed = fixture.seed;
    let scenarios = select_continuity_scenarios(fixture.scenarios, args.scenario.as_deref())?;
    let frozen_embedding_providers = validate_continuity_embedding_sizes(&config, &scenarios)?;
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

fn load_continuity_fixture(path: &Path) -> Result<cmem_eval_continuity::ContinuityFixtureSet> {
    let bytes =
        fs::read(path).with_context(|| format!("read continuity fixture {}", path.display()))?;
    parse_fixture_bytes(&bytes)
}

fn validate_continuity_embedding_sizes(
    config: &BenchmarkRunConfig,
    scenarios: &[ContinuityScenario],
) -> Result<FrozenEmbeddingProviders> {
    let mut frozen_embedding_providers = HashMap::new();
    let configured_size = config.backend.embedding.vector_size.context(
        "continuity dataset requires backend.embedding.vector_size to match every selected fixture scenario",
    )?;
    let scenario_providers = scenarios
        .iter()
        .map(|scenario| scenario.embedding.provider_name())
        .collect::<BTreeSet<_>>();

    for scenario in scenarios {
        if let Some(fixture_size) = scenario.embedding.vector_size()
            && fixture_size > configured_size
        {
            bail!(
                "continuity scenario {:?} controllable embedding vector_size {fixture_size} exceeds backend.embedding.vector_size {configured_size}",
                scenario.fixture_id
            );
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
        {
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

fn continuity_embedding_binding(
    config: &BenchmarkRunConfig,
    scenario: &ContinuityScenario,
    frozen_store: Option<FrozenEmbeddingProvider>,
) -> Result<(EmbeddingRuntimeBinding, EmbeddingBindingRecord)> {
    let configured_size = config
        .backend
        .embedding
        .vector_size
        .context("continuity runtime requires backend.embedding.vector_size")?;
    if let Some(fixture) = scenario.embedding.controllable_similarity() {
        let dimension_policy = if fixture.vector_size == configured_size {
            ControllableDimensionPolicy::FixtureDeclared
        } else {
            ControllableDimensionPolicy::Exact {
                vector_size: configured_size,
            }
        };
        let record = EmbeddingBindingRecord::Controllable {
            fixture_sha256: fixture.canonical_sha256()?,
            vector_size: fixture.vector_size,
            dimension_policy,
        };
        return Ok((
            EmbeddingRuntimeBinding::Controllable {
                fixture: fixture.clone(),
                dimension_policy,
            },
            record,
        ));
    }

    let store = frozen_store.context("frozen continuity runtime is missing its preflight store")?;
    let dimension_policy = store.dimension_policy();
    let record = EmbeddingBindingRecord::Frozen {
        store_sha256: store.store_sha256()?,
        model: store.model().to_string(),
        vector_size: store.vector_size(),
        dimension_policy,
    };
    Ok((
        EmbeddingRuntimeBinding::Frozen {
            store,
            dimension_policy,
        },
        record,
    ))
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
    derived_memories: Vec<cmem_eval::DerivedMemoryInput>,
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
        derived_memories: Vec<cmem_eval::DerivedMemoryInput>,
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
    let dataset = dataset_descriptor(&config.dataset)?;
    let lexical = config.retrieval.mode == cmem_eval::RetrievalMode::Bm25Only;
    let embedding_binding = if lexical {
        EmbeddingBindingRecord::Bm25
    } else {
        live_embedding_binding(&config)?
    };
    let metric_family = S::metric_family(&config.metrics);
    let source_items = S::load(&args.dataset)?;
    let adapter_metadata = if lexical {
        RunAdapterMetadata::bm25()
    } else {
        RunAdapterMetadata::live()
    };
    let adapter = if lexical {
        None
    } else {
        Some(adapter(&config).await?)
    };
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
        let batch = S::memory_inputs(&item, &config);
        let baseline = lexical
            .then(|| cmem_eval::bm25::Bm25Baseline::new(&batch.episodes, &batch.observations));
        let ingest_detail = S::ingest_progress_detail(&batch);
        let episode_count = batch.episodes.len();
        let observation_count = batch.observations.len();
        let mut write_outcomes = Vec::new();
        if let Some(adapter) = &adapter {
            prepare_fresh_namespace(adapter.as_ref(), &namespace).await?;
            if !batch.episodes.is_empty() {
                write_outcomes.push(adapter.remember_episodes(batch.episodes).await?.outcome);
            }
            progress.phase_done(
                item_number,
                &item_label,
                "ingest-episodes",
                &format!("count={episode_count}"),
            );
            if !batch.observations.is_empty() {
                write_outcomes.push(
                    adapter
                        .remember_observations(batch.observations)
                        .await?
                        .outcome,
                );
            }
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
                write_outcomes.extend(adapter.remember_enrichment(enrichment).await?);
                progress.phase_done(item_number, &item_label, "enrichment", "done");
            }
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
            let input = RetrieveInput {
                mode: config.retrieval.mode,
                namespace: namespace.clone(),
                query: S::question_text(question).to_string(),
                query_date: S::query_date(question),
                surface_policy: config.retrieval.surface_policy.clone(),
            };
            let pack = if let Some(baseline) = &baseline {
                baseline.retrieve(&input)
            } else {
                adapter
                    .as_ref()
                    .expect("library retrieval has an adapter")
                    .retrieve(input)
                    .await?
            };
            if S::REPORT_QA_PROGRESS {
                progress.qa_retrieved(
                    item_number,
                    &item_label,
                    question_index + 1,
                    item_question_count,
                    pack.items().len(),
                );
            } else {
                progress.phase_done(
                    item_number,
                    &item_label,
                    "retrieve",
                    &format!("items={}", pack.items().len()),
                );
            }

            let context = context_metrics_with_full_history(&pack, full_history_metrics);
            let composition = composition_metrics(pack.items());
            let integrity = if config.retrieval.mode == cmem_eval::RetrievalMode::Hybrid {
                integrity_details_from_outcomes(pack.items(), pack.outcomes())
            } else {
                // Raw vector and lexical baselines do not surface the graph-validated pack.
                cmem_eval::integrity_details(pack.items())
            };
            let latency_ms = if S::LATENCY_INCLUDES_INGEST {
                item_timer.elapsed_ms()
            } else {
                question_timer.elapsed_ms()
            };
            let metrics = S::score(&item, question, pack.items(), &config);
            let mut metrics = metrics
                .as_object()
                .cloned()
                .context("dataset scorer must return a JSON object before metrics admission")?;
            insert_common_metrics(
                &mut metrics,
                &context,
                &composition,
                &integrity,
                std::slice::from_ref(&metric_family),
            );
            let metrics = MetricsRecord::try_from(metrics)?;
            let (retrieved, context_text, _, _, retrieval_outcomes) = pack.into_parts();
            rows.push(PerQuestionResult {
                schema_version: cmem_eval::RESULT_SCHEMA_VERSION.to_string(),
                run_id: config.run_id.clone(),
                dataset: config.dataset.clone(),
                dataset_kind: dataset.kind,
                embedding_binding: embedding_binding.clone(),
                adapter: adapter_metadata.clone(),
                question_id: S::question_id(question).to_string(),
                question_type: S::question_type(question),
                question: S::question_text(question).to_string(),
                gold_episode_ids: S::gold_episode_ids(&item, question),
                gold_observation_ids: S::gold_observation_ids(&item, question),
                retrieved,
                context_text,
                write_outcomes: write_outcomes.clone(),
                link_outcomes: Vec::new(),
                lifecycle_outcomes: Vec::new(),
                metrics,
                latency_ms,
                context_char_count: context.retrieved_context_chars,
                context_word_count: context.retrieved_context_words,
                context,
                retrieval_outcomes,
                composition,
                integrity,
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
        if let Some(adapter) = &adapter {
            adapter.detach_namespace(&namespace).await?;
        }
        namespaces_to_cleanup.push(namespace);
        progress.item_finished(item_number, &item_label, item_timer.elapsed_ms());
    }

    progress.write_outputs_started(rows.len());
    write_outputs(args, config.clone(), rows, &[metric_family])?;
    progress.cleanup_started(namespaces_to_cleanup.len());
    if let Some(adapter) = &adapter {
        cleanup_namespaces_after_artifacts(adapter.as_ref(), &config, &namespaces_to_cleanup)
            .await?;
    }
    Ok(())
}

async fn prepare_fresh_namespace(adapter: &CharacterMemoryAdapter, namespace: &str) -> Result<()> {
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
        if !config.retrieval.surface_policy.include_debug_rationale {
            bail!(
                "continuity dataset requires retrieval.surface_policy.include_debug_rationale=true because continuity traces and rationale-derived metrics are mandatory"
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
        _derived_memories: Vec<cmem_eval::DerivedMemoryInput>,
        _config: &BenchmarkRunConfig,
        _configured: &HashMap<String, GraphEnrichmentInput>,
        _snapshots: &HashMap<String, GraphSnapshotInput>,
    ) -> Result<Option<GraphEnrichmentInput>> {
        Ok(None)
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
    let adapter_metadata = RunAdapterMetadata::live();
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
        let (embedding_binding, embedding_binding_record) =
            continuity_embedding_binding(&config, scenario, frozen_embedding_provider)?;
        let mut runtime = ContinuityRuntime::new(&config, embedding_binding).await?;
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
                &embedding_binding_record,
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
    cmem_eval::reject_empty_run(&rows)?;
    progress.write_outputs_started(rows.len());
    write_continuity_traces(&args.trace_out, &traces)?;
    let config_value = serde_json::to_value(&config)?;
    let summary = summarize_rows(
        config.run_id.clone(),
        config.dataset.clone(),
        DatasetKind::Continuity,
        adapter_metadata.clone(),
        config_value.clone(),
        &rows,
        std::slice::from_ref(&metric_family),
    )?;
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
    embedding_binding: &EmbeddingBindingRecord,
    latency_ms: u128,
) -> Result<PerQuestionResult> {
    let full_history = full_history_context_metrics(Some(&trace.history_text));
    let context = context_metrics_with_full_history(&trace.retrieval, full_history);
    let composition = composition_metrics(trace.retrieval.items());
    let integrity =
        integrity_details_from_outcomes(trace.retrieval.items(), trace.retrieval.outcomes());
    let mut metrics = Map::new();
    insert_common_metrics(
        &mut metrics,
        &context,
        &composition,
        &integrity,
        std::slice::from_ref(metric_family),
    );
    insert_continuity_metrics(&mut metrics, scenario, trace, &config.metrics);
    let question_type = serde_json::to_value(trace.pattern)?
        .as_str()
        .map(str::to_string);
    Ok(PerQuestionResult {
        schema_version: cmem_eval::RESULT_SCHEMA_VERSION.to_string(),
        run_id: config.run_id.clone(),
        dataset: config.dataset.clone(),
        dataset_kind: DatasetKind::Continuity,
        embedding_binding: embedding_binding.clone(),
        adapter: adapter.clone(),
        question_id: trace.query_id.clone(),
        question_type,
        question: trace.query.clone(),
        gold_episode_ids: Vec::new(),
        gold_observation_ids: Vec::new(),
        retrieved: trace.retrieval.items().to_vec(),
        context_text: trace.retrieval.context_text().to_string(),
        write_outcomes: trace.write_outcomes.clone(),
        link_outcomes: trace.link_outcomes.clone(),
        lifecycle_outcomes: trace.lifecycle_outcomes.clone(),
        metrics: MetricsRecord::try_from(metrics)?,
        latency_ms,
        context_char_count: context.retrieved_context_chars,
        context_word_count: context.retrieved_context_words,
        context,
        retrieval_outcomes: trace.retrieval.outcomes().to_vec(),
        composition,
        integrity,
    })
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
        _derived_memories: Vec<cmem_eval::DerivedMemoryInput>,
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
        derived_memories: Vec<cmem_eval::DerivedMemoryInput>,
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
    let descriptor = dataset_descriptor(&config.dataset)?;
    if descriptor.id != expected {
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
    composition: &cmem_eval::ResultCompositionMetrics,
    integrity: &cmem_eval::ResultIntegrityDetails,
    metric_families: &[MetricFamily],
) {
    initialize_registry_metrics_for(metrics, metric_families);
    insert_context_metrics(metrics, context);
    insert_composition_metrics(metrics, composition);
    insert_integrity_detail_metrics(metrics, integrity);
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
    let retrieved_context_tokens = count_tokens(pack.context_text());
    let compression_ratio = match (full_history.tokens, retrieved_context_tokens) {
        (Some(full), retrieved) if retrieved > 0 => Some(full as f64 / retrieved as f64),
        _ => None,
    };
    let reduction_rate = match (full_history.tokens, retrieved_context_tokens) {
        (Some(full), retrieved) if full > 0 => Some(1.0 - retrieved as f64 / full as f64),
        _ => None,
    };
    ResultContextMetrics {
        retrieved_context_chars: pack.context_char_count(),
        retrieved_context_words: pack.context_word_count(),
        retrieved_context_tokens,
        full_history_chars: full_history.chars,
        full_history_words: full_history.words,
        full_history_tokens: full_history.tokens,
        compression_ratio,
        reduction_rate,
    }
}

async fn adapter(config: &BenchmarkRunConfig) -> Result<Box<CharacterMemoryAdapter>> {
    Ok(Box::new(CharacterMemoryAdapter::new(config).await?))
}

async fn cleanup_namespaces_after_artifacts(
    adapter: &CharacterMemoryAdapter,
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
    cmem_eval::reject_empty_run(&rows)?;
    let summary = summarize_rows(
        config.run_id.clone(),
        config.dataset.clone(),
        dataset_descriptor(&config.dataset)?.kind,
        rows.first()
            .map(|row| row.adapter.clone())
            .unwrap_or_else(RunAdapterMetadata::live),
        serde_json::to_value(&config)?,
        &rows,
        metric_families,
    )?;
    write_jsonl(&args.out, &rows)?;
    write_summary(&args.summary_out, &summary)
}

struct RunProgress {
    dataset: String,
    total_items: usize,
    total_qa: Option<usize>,
    started_at: Instant,
}

impl RunProgress {
    fn new(dataset: &DatasetId, total_items: usize, total_qa: Option<usize>) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run_args(dataset: PathBuf, config: PathBuf, directory: &Path) -> RunArgs {
        RunArgs {
            dataset,
            config,
            out: directory.join("results.jsonl"),
            summary_out: directory.join("summary.json"),
        }
    }

    fn read_rows(path: &Path) -> Vec<PerQuestionResult> {
        cmem_eval::read_jsonl(path).unwrap()
    }

    fn read_traces(path: &Path) -> Vec<ContinuityQueryTrace> {
        cmem_eval_continuity::read_continuity_traces(path).unwrap()
    }

    fn current_continuity_config_text() -> String {
        // The maintained, unsealed config is the one that tracks the live CLI
        // schema; the sealed configs are cited by hash and never edited.
        fs::read_to_string("../../configs/continuity_smoke.toml").unwrap()
    }

    fn current_continuity_config() -> BenchmarkRunConfig {
        toml::from_str(&current_continuity_config_text()).unwrap()
    }

    #[tokio::test]
    async fn embedded_runtime_restart_preserves_data_and_cleans_up() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        let directory = tempfile::tempdir_in(std::path::absolute(root).unwrap()).unwrap();
        let mut config = current_continuity_config();
        config.backend.vector_store_mode = cmem_eval::VectorStoreMode::Embedded;
        config.backend.qdrant_connection_string = None;
        config.backend.namespace_prefix = Some("cmem_eval_restart".into());
        config.backend.cleanup.enabled = true;
        config.backend.cleanup.require_collection_prefix = Some("cmem_eval_restart".into());
        config.backend.identity_registry_dir =
            Some(directory.path().join("identities").display().to_string());
        config.backend.oxigraph_persistence_path =
            Some(directory.path().join("oxigraph").display().to_string());
        config.backend.retrieval_stats_path =
            Some(directory.path().join("stats.sqlite").display().to_string());
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let scenario = fixture
            .scenarios
            .iter()
            .find(|scenario| scenario.fixture_id == "cross-store-stress")
            .unwrap();
        let (binding, _) = continuity_embedding_binding(&config, scenario, None).unwrap();
        let mut runtime = ContinuityRuntime::new(&config, binding).await.unwrap();
        let run = run_continuity_scenario(&mut runtime, scenario, &config.retrieval)
            .await
            .unwrap();
        assert_eq!(run.restart_observations.len(), 1);
        let restart = &run.restart_observations[0];
        assert!(restart.lifecycle.restored_identity_count > 0);
        assert!(!restart.before_restart.returned_object_ids.is_empty());
        assert!(restart.delta.stable_returned_objects);
        runtime
            .adapter()
            .cleanup_namespace(&scenario.namespace)
            .await
            .unwrap();
        for entry in fs::read_dir(directory.path()).unwrap() {
            let path = entry.unwrap().path();
            assert!(path.is_dir(), "store file remains: {}", path.display());
            assert!(fs::read_dir(path).unwrap().next().is_none());
        }
        directory.close().unwrap();
    }

    #[test]
    fn explicit_vector_size_allows_a_custom_live_embedding_model() {
        let mut config: BenchmarkRunConfig = toml::from_str(include_str!(
            "../../../configs/longmemeval_s_retrieval.toml"
        ))
        .unwrap();
        config.backend.embedding.provider = EmbeddingProviderConfig::OpenAi;
        config.backend.embedding.model = "future-custom-embedding-model".to_string();
        config.backend.embedding.vector_size = Some(2_048);

        assert_eq!(
            live_embedding_binding(&config).unwrap(),
            EmbeddingBindingRecord::Live {
                provider: LiveEmbeddingProvider::OpenAi,
                model: "future-custom-embedding-model".to_string(),
                vector_size: 2_048,
            }
        );
    }

    fn continuity_args(directory: &Path) -> ContinuityRunArgs {
        let source_config = current_continuity_config_text();
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
        let mut config: BenchmarkRunConfig = toml::from_str(&config).unwrap();
        isolate_test_config(&mut config, directory);
        fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
        ContinuityRunArgs {
            run: RunArgs {
                dataset: PathBuf::from("../cmem-eval-continuity/fixtures/continuity_v3.json"),
                config: config_path,
                out: directory.join("continuity.jsonl"),
                summary_out: directory.join("continuity-summary.json"),
            },
            trace_out: directory.join("continuity-traces.jsonl"),
            report_out: directory.join("continuity-report.json"),
            scenario: None,
        }
    }

    fn isolate_test_config(config: &mut BenchmarkRunConfig, directory: &Path) {
        config.backend.vector_store_mode = cmem_eval::VectorStoreMode::Embedded;
        config.backend.qdrant_connection_string = Some("http://127.0.0.1:1".into());
        config.backend.identity_registry_dir =
            Some(directory.join("identities").display().to_string());
        config.backend.oxigraph_persistence_path =
            Some(directory.join("graph").display().to_string());
        config.backend.retrieval_stats_path =
            Some(directory.join("stats.sqlite").display().to_string());
    }

    fn service_free_config(source: &str, directory: &Path) -> PathBuf {
        let config = fs::read_to_string(source).unwrap();
        let config = config
            .lines()
            .filter(|line| !line.trim_start().starts_with("enrichment_snapshot_path"))
            .collect::<Vec<_>>()
            .join("\n");
        let path = directory.join("config.toml");
        let mut config: BenchmarkRunConfig = toml::from_str(&config).unwrap();
        isolate_test_config(&mut config, directory);
        config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
        fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
        path
    }

    #[tokio::test]
    async fn fresh_namespace_preparation_discards_stale_state() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = current_continuity_config();
        isolate_test_config(&mut config, directory.path());
        config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
        let adapter = CharacterMemoryAdapter::new(&config).await.unwrap();
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
        let pack = adapter
            .retrieve(RetrieveInput {
                namespace: "stale".into(),
                query: "stale durable state".into(),
                query_date: None,
                mode: cmem_eval::RetrievalMode::Hybrid,
                surface_policy: config.retrieval.surface_policy.clone(),
            })
            .await
            .unwrap();
        assert!(pack.items().is_empty());
        adapter.close().await.unwrap();
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
                "haystack_sessions": [[
                    {
                        "role": "user",
                        "content": "I like jasmine tea",
                        "has_answer": true
                    },
                    {
                        "role": "assistant",
                        "content": "That sounds refreshing",
                        "has_answer": false
                    },
                    {
                        "role": "user",
                        "content": "Especially in spring",
                        "has_answer": false
                    }
                ]],
                "answer_session_ids": ["s1"]
            }]))
            .unwrap(),
        )
        .unwrap();
        let args = run_args(
            dataset,
            service_free_config("../../configs/longmemeval_s_retrieval.toml", dir.path()),
            dir.path(),
        );
        let output = args.out.clone();
        let summary_output = args.summary_out.clone();

        run_longmemeval(args).await.unwrap();

        let rows = read_rows(&output);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].question_id, "q1");
        assert_eq!(rows[0].gold_episode_ids, vec!["s1"]);
        assert_eq!(rows[0].gold_observation_ids, vec!["s1:turn:1"]);
        assert_eq!(
            rows[0]
                .write_outcomes
                .iter()
                .filter(|record| record.outcome.persisted_object_ids.len() == 3)
                .count(),
            1
        );
        assert!(
            !cmem_eval::read_summary(&summary_output)
                .unwrap()
                .degradation
                .any_degradation
        );
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
        let args = run_args(
            dataset,
            service_free_config("../../configs/locomo_retrieval.toml", dir.path()),
            dir.path(),
        );
        let output = args.out.clone();

        run_locomo(args).await.unwrap();

        let rows = read_rows(&output);
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
        let args = continuity_args(directory.path());
        let second_args = continuity_args(second_directory.path());
        let result_path = args.run.out.clone();
        let summary_path = args.run.summary_out.clone();
        let trace_path = args.trace_out.clone();
        let report_path = args.report_out.clone();
        let dataset_path = args.run.dataset.clone();
        let config_path = args.run.config.clone();

        run_continuity(args).await.unwrap();
        run_continuity(second_args).await.unwrap();

        let rows = read_rows(&result_path);
        let traces = read_traces(&trace_path);
        let summary = cmem_eval::read_summary(&summary_path).unwrap();
        let report: cmem_eval_continuity::ContinuityReport =
            serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
        let second_report: cmem_eval_continuity::ContinuityReport = serde_json::from_slice(
            &fs::read(second_directory.path().join("continuity-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 23);
        assert_eq!(traces.len(), 23);
        assert_eq!(summary.num_questions, 23);
        assert_eq!(
            summary.metric_support["temporal_recall_fraction@5"].numeric_rows,
            4
        );
        assert_eq!(
            summary.metric_support["supersession_replacement_recall"].numeric_rows,
            2
        );
        assert!(
            summary
                .registry_coverage
                .missing_required_metrics
                .is_empty()
        );
        assert_eq!(report.content.aggregate.query_count, 23);
        assert_eq!(report.content.aggregate.restart_count, 1);
        crate::diff::run(crate::diff::DiffArgs {
            run_a: result_path.clone(),
            run_b: second_directory.path().join("continuity.jsonl"),
        })
        .unwrap();
        assert_eq!(report.content.aggregate, second_report.content.aggregate);
        assert_eq!(
            report.schema_version,
            cmem_eval_continuity::CONTINUITY_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.metadata.embedding_seeds.len(), 13);
        assert_eq!(
            report.metadata.normalization.nondeterministic_paths,
            vec![
                "metadata.generated_at",
                "content.scenarios.*.rationale_samples.*.context_pack.outcomes.*.pack"
            ]
        );
        assert_eq!(
            report
                .metadata
                .normalization
                .excluded_nondeterministic_sources,
            vec!["measured query retrieval latency in results and summaries",]
        );
        assert_eq!(
            report.metadata.config["retrieval"]["surface_policy"]["max_graph_roots"],
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
                && scenario
                    .registry_coverage
                    .missing_required_metrics
                    .is_empty()
        }));
        assert_eq!(report.content.tuning_observations.len(), 1);
        assert_eq!(
            report.content.tuning_observations[0].id,
            "entity_root_candidate_limit"
        );
        assert!(
            report.content.scenarios["cross-store-stress"].restart_observations[0]
                .delta
                .stable_returned_objects
        );
        let sample = &report.content.scenarios["cross-store-stress"].rationale_samples[0];
        assert_eq!(sample.query, "What marker must survive the restart?");
        assert!(!sample.context_pack.items().is_empty());
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
                .pointer("/context_pack/outcomes/0/trace/selectivity_decisions")
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
            &RunAdapterMetadata::live(),
            &report_metric_family,
            &fixture.scenarios[0],
            &traces[0],
            &rows[0].embedding_binding,
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
        for token in ["traces", "result rows", "23", "22"] {
            assert!(error.contains(token), "missing {token:?} in {error}");
        }
        let mut swapped_rows = read_rows(&result_path);
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
        assert!(error.contains("trace/result mismatch"), "{error}");
        assert!(error.contains('0'), "{error}");

        let retrieval_rows = read_rows(&result_path);
        let altered_retrieval_index = retrieval_rows
            .iter()
            .position(|row| !row.retrieved.is_empty())
            .expect("continuity fixture should produce a retrieved item");
        for constituent in [
            cmem_eval_continuity::RetrievalPayloadConstituent::Items,
            cmem_eval_continuity::RetrievalPayloadConstituent::RenderedContext,
            cmem_eval_continuity::RetrievalPayloadConstituent::CharCount,
            cmem_eval_continuity::RetrievalPayloadConstituent::WordCount,
            cmem_eval_continuity::RetrievalPayloadConstituent::Outcomes,
        ] {
            let mut altered_retrieval_rows = retrieval_rows.clone();
            let row = &mut altered_retrieval_rows[altered_retrieval_index];
            match constituent {
                cmem_eval_continuity::RetrievalPayloadConstituent::Items => {
                    row.retrieved.clear();
                }
                cmem_eval_continuity::RetrievalPayloadConstituent::RenderedContext => {
                    row.context_text.push_str(" tampered");
                }
                cmem_eval_continuity::RetrievalPayloadConstituent::CharCount => {
                    row.context_char_count += 1;
                }
                cmem_eval_continuity::RetrievalPayloadConstituent::WordCount => {
                    row.context_word_count += 1;
                }
                cmem_eval_continuity::RetrievalPayloadConstituent::Outcomes => {
                    row.retrieval_outcomes.clear();
                }
            }
            let error = assemble_continuity_report(ContinuityReportInput {
                generated_at: Utc::now(),
                fixture_schema_version: fixture.schema_version,
                fixture_seed: fixture.seed,
                config: summary.config.clone(),
                adapter: summary.adapter.clone(),
                scenarios: &fixture.scenarios,
                traces: &traces,
                rows: &altered_retrieval_rows,
                summary: &summary,
                metric_family: &report_metric_family,
                restart_observations: &valid_restart_observations,
            })
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<cmem_eval_continuity::RetrievalPayloadMismatchError>(),
                Some(&cmem_eval_continuity::RetrievalPayloadMismatchError {
                    row_index: altered_retrieval_index,
                    trace_query_id: traces[altered_retrieval_index].query_id.clone(),
                    row_question_id: altered_retrieval_rows[altered_retrieval_index]
                        .question_id
                        .clone(),
                    constituents: vec![constituent],
                })
            );
        }

        let mut altered_outcome_rows = read_rows(&result_path);
        let mut invented = altered_outcome_rows[0].write_outcomes[0].clone();
        invented.operation_id = "invented-write-outcome".into();
        altered_outcome_rows[0].write_outcomes.push(invented);
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &altered_outcome_rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("trace/result mismatch"), "{error}");
        assert!(error.contains("write="), "{error}");

        let mut altered_outcome_rows = read_rows(&result_path);
        let mut invented = traces
            .iter()
            .flat_map(|trace| &trace.lifecycle_outcomes)
            .next()
            .unwrap()
            .clone();
        invented.operation_id = "invented-lifecycle-outcome".into();
        altered_outcome_rows[0].lifecycle_outcomes.push(invented);
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &altered_outcome_rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("trace/result mismatch"), "{error}");
        assert!(error.contains("lifecycle="), "{error}");

        let mut altered_kind_rows = read_rows(&result_path);
        altered_kind_rows[0].dataset_kind = DatasetKind::LoCoMo;
        let error = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: summary.config.clone(),
            adapter: summary.adapter.clone(),
            scenarios: &fixture.scenarios,
            traces: &traces,
            rows: &altered_kind_rows,
            summary: &summary,
            metric_family: &report_metric_family,
            restart_observations: &valid_restart_observations,
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("summary/result identity mismatch"),
            "{error}"
        );
        assert!(error.contains("LoCoMo"), "{error}");
        assert!(error.contains("Continuity"), "{error}");

        let mut invented_traces = traces.clone();
        invented_traces[0].query_id = "invented-query".to_string();
        let mut invented_rows = read_rows(&result_path);
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
        assert!(error.contains("scripted query mismatch"), "{error}");
        assert!(error.contains('0'), "{error}");
        let mut duplicate_traces = traces.clone();
        duplicate_traces[1].query_id = duplicate_traces[0].query_id.clone();
        let mut duplicate_rows = read_rows(&result_path);
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
        assert!(error.contains("scripted query mismatch"), "{error}");
        assert!(error.contains('1'), "{error}");

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
        for token in ["restart observations", "restart events", "0", "1"] {
            assert!(error.contains(token), "missing {token:?} in {error}");
        }

        let mut stale_summary = cmem_eval::read_summary(&summary_path).unwrap();
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

        let mut stale_summary = cmem_eval::read_summary(&summary_path).unwrap();
        stale_summary
            .metrics
            .get_mut("continuity_gap_days")
            .unwrap()
            .mean = Some(-1.0);
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
        assert!(
            summary
                .registry_coverage
                .missing_required_metrics
                .is_empty()
        );
        assert!(rows.iter().any(|row| {
            matches!(
                row.metrics.get("typed_rationale_coverage"),
                Some(cmem_eval::MetricValue::Number(_))
            )
        }));
        assert!(rows.iter().any(|row| {
            row.metrics.iter().any(|(key, value)| {
                key.starts_with("continuity_recall_fraction_gap_")
                    && matches!(value, cmem_eval::MetricValue::Number(_))
            })
        }));
        assert!(traces.iter().all(|trace| {
            !trace.history_text.is_empty()
                && trace
                    .retrieval
                    .items()
                    .iter()
                    .all(|item| !item.rationale.is_empty())
        }));
    }

    #[tokio::test]
    async fn continuity_lifecycle_retry_reaches_row_summary_and_report() {
        let directory = tempfile::tempdir().unwrap();
        let mut args = continuity_args(directory.path());
        args.scenario = Some("correction-chains".to_string());
        let result_path = args.run.out.clone();
        let trace_path = args.trace_out.clone();
        let config_path = args.run.config.clone();
        let dataset_path = args.run.dataset.clone();

        run_continuity(args).await.unwrap();

        let mut traces = read_traces(&trace_path);
        let original_rows = read_rows(&result_path);
        assert_eq!(traces.len(), 1);
        assert_eq!(original_rows.len(), 1);
        let converged_retry = {
            let lifecycle = traces[0]
                .lifecycle_outcomes
                .first_mut()
                .expect("correction scenario should emit lifecycle outcomes");
            let failed_internal_id = lifecycle
                .outcome
                .trace
                .as_ref()
                .unwrap()
                .requested_targets
                .first()
                .unwrap()
                .id();
            lifecycle.outcome.stats_update_status =
                cmem_eval::character_memory::StatsUpdateStatus::failed(
                    [],
                    [failed_internal_id],
                    Vec::new(),
                );
            let mut retry = lifecycle.clone();
            retry.outcome.stats_update_status =
                cmem_eval::character_memory::StatsUpdateStatus::succeeded([failed_internal_id]);
            retry
        };
        traces[0].lifecycle_outcomes.push(converged_retry);

        let fixture = parse_fixture_bytes(&fs::read(dataset_path).unwrap()).unwrap();
        let scenarios =
            select_continuity_scenarios(fixture.scenarios, Some("correction-chains")).unwrap();
        let config = read_config(&config_path).unwrap();
        let metric_family = continuity_metric_family(&config.metrics, &scenarios);
        let adapter = original_rows[0].adapter.clone();
        let row = continuity_result_row(
            &config,
            &adapter,
            &metric_family,
            &scenarios[0],
            &traces[0],
            &original_rows[0].embedding_binding,
            original_rows[0].latency_ms,
        )
        .unwrap();
        assert_eq!(row.lifecycle_outcomes, traces[0].lifecycle_outcomes);

        let rows = vec![row];
        let config_value = serde_json::to_value(&config).unwrap();
        let summary = summarize_rows(
            config.run_id.clone(),
            config.dataset.clone(),
            DatasetKind::Continuity,
            adapter.clone(),
            config_value.clone(),
            &rows,
            std::slice::from_ref(&metric_family),
        )
        .unwrap();
        assert!(summary.degradation.any_degradation);

        let report = assemble_continuity_report(ContinuityReportInput {
            generated_at: Utc::now(),
            fixture_schema_version: fixture.schema_version,
            fixture_seed: fixture.seed,
            config: config_value,
            adapter,
            scenarios: &scenarios,
            traces: &traces,
            rows: &rows,
            summary: &summary,
            metric_family: &metric_family,
            restart_observations: &BTreeMap::new(),
        })
        .unwrap();
        assert!(report.metadata.degradation.any_degradation);
    }

    #[test]
    fn continuity_spec_accepts_resource_provider_for_scenario_binding_resolution() {
        let mut config = current_continuity_config();
        config.backend.embedding.provider = EmbeddingProviderConfig::OpenAi;
        ContinuitySpec::validate_config(&config).unwrap();
    }

    #[test]
    fn continuity_spec_requires_debug_rationale_for_mandatory_traces() {
        let mut config = current_continuity_config();
        config.retrieval.surface_policy.include_debug_rationale = false;

        let error = ContinuitySpec::validate_config(&config)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("retrieval.surface_policy.include_debug_rationale=true"),
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
        let mut config = current_continuity_config();
        config.backend.embedding.provider = EmbeddingProviderConfig::ControllableSimilarity;

        config.backend.embedding.vector_size = None;
        let error = validate_continuity_embedding_sizes(&config, scenarios)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("requires backend.embedding.vector_size"),
            "{error}"
        );

        config.backend.embedding.vector_size =
            Some(scenarios[0].embedding.vector_size().unwrap() - 1);
        let error = validate_continuity_embedding_sizes(&config, scenarios)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeds"), "{error}");
        assert!(error.contains(&scenarios[0].fixture_id), "{error}");

        let fixture_size = scenarios[0].embedding.vector_size().unwrap();
        config.backend.embedding.vector_size = Some(fixture_size + 1);
        validate_continuity_embedding_sizes(&config, scenarios).unwrap();
        let (runtime, record) = continuity_embedding_binding(&config, &scenarios[0], None).unwrap();
        assert!(matches!(
            runtime,
            EmbeddingRuntimeBinding::Controllable {
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size },
                ..
            } if vector_size == fixture_size + 1
        ));
        assert!(matches!(
            record,
            EmbeddingBindingRecord::Controllable {
                vector_size,
                dimension_policy: ControllableDimensionPolicy::Exact { .. },
                ..
            } if vector_size == fixture_size
        ));
    }

    #[test]
    fn frozen_scenario_preflight_rejects_test_provenance() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let mut scenario = fixture.scenarios[0].clone();
        scenario.embedding = cmem_eval_continuity::ContinuityScenarioEmbedding::frozen();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cmem-eval-continuity/fixtures/embeddings");
        let mut config = current_continuity_config();
        config.backend.embedding.provider = EmbeddingProviderConfig::Frozen;
        config.backend.embedding.model = "task21-smoke-model".to_string();
        config.backend.embedding.vector_size = Some(3);
        config.backend.embedding.store_path = Some(
            fixtures
                .join("task21_smoke_store.json")
                .display()
                .to_string(),
        );

        let error = validate_continuity_embedding_sizes(&config, &[scenario.clone()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("source=open_ai_api"), "{error}");
        assert!(error.contains("TestFixture"), "{error}");
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
        let store = cmem_eval::FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            cmem_eval::FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            scenario
                .runtime_embedding_inputs()
                .into_iter()
                .map(|text| (text, vec![0.0; 1_024])),
        )
        .unwrap();
        fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();
        let mut config = current_continuity_config();
        config.backend.embedding.provider = EmbeddingProviderConfig::Frozen;
        config.backend.embedding.model = "text-embedding-3-large".to_string();
        config.backend.embedding.vector_size = Some(1_024);
        config.backend.embedding.store_path = Some(store_path.display().to_string());

        let error = validate_continuity_embedding_sizes(&config, &[scenario])
            .unwrap_err()
            .to_string();

        for token in [
            "text-embedding-3-large",
            "1024",
            "3072",
            "--allow-nonstandard-dimensions",
        ] {
            assert!(error.contains(token), "missing {token:?} in {error}");
        }
        assert!(error.contains("live Character Memory"), "{error}");
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
        let manifest = cmem_eval::FrozenEmbeddingManifest::load(
            &fixture_root.join("embeddings/continuity_benchmarks_v1_manifest.json"),
        )
        .unwrap();
        let manifest_texts = manifest
            .texts
            .iter()
            .map(|item| item.text.clone())
            .collect::<BTreeSet<_>>();
        let store = cmem_eval::FrozenEmbeddingStore::load(
            &fixture_root.join("embeddings/continuity_benchmarks_v1_store.json"),
        )
        .unwrap();
        let store_texts = store
            .entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(runtime_texts.len(), 646);
        assert_eq!(runtime_texts, manifest_texts);
        assert_eq!(runtime_texts, store_texts);
    }

    #[test]
    fn committed_canonical_store_is_exactly_the_frozen_runtime_lookup_set() {
        let fixture_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../cmem-eval-continuity/fixtures");
        let fixture =
            parse_fixture_bytes(&fs::read(fixture_root.join("continuity_v3.json")).unwrap())
                .unwrap();
        let runtime_texts = fixture
            .scenarios
            .iter()
            .filter(|scenario| scenario.embedding.provider_name() == "frozen")
            .flat_map(ContinuityScenario::runtime_embedding_inputs)
            .collect::<BTreeSet<_>>();
        let manifest = cmem_eval::FrozenEmbeddingManifest::load(
            &fixture_root.join("embeddings/task22_real_manifest.json"),
        )
        .unwrap();
        let manifest_texts = manifest
            .texts
            .iter()
            .map(|item| item.text.clone())
            .collect::<BTreeSet<_>>();
        let store = cmem_eval::FrozenEmbeddingStore::load(
            &fixture_root.join("embeddings/task22_real_store.json"),
        )
        .unwrap();
        let store_texts = store
            .entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect::<BTreeSet<_>>();

        assert!(!runtime_texts.is_empty());
        assert_eq!(runtime_texts, manifest_texts);
        assert_eq!(runtime_texts, store_texts);
    }

    #[test]
    fn continuity_suite_resolves_each_scenario_to_its_own_runtime_binding() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let mut config = current_continuity_config();
        let configured_store = config.backend.embedding.store_path.as_deref().unwrap();
        let store_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(configured_store);
        config.backend.embedding.store_path = Some(store_path.display().to_string());
        let frozen_stores =
            validate_continuity_embedding_sizes(&config, &fixture.scenarios).unwrap();
        let mut saw_controllable = false;
        let mut saw_frozen = false;

        for scenario in &fixture.scenarios {
            let frozen_store = (scenario.embedding.provider_name() == "frozen")
                .then(|| frozen_stores.get(&store_path).unwrap().clone());
            let (runtime, record) =
                continuity_embedding_binding(&config, scenario, frozen_store).unwrap();
            match (runtime, record) {
                (
                    EmbeddingRuntimeBinding::Controllable { .. },
                    EmbeddingBindingRecord::Controllable { .. },
                ) => saw_controllable = true,
                (EmbeddingRuntimeBinding::Frozen { .. }, EmbeddingBindingRecord::Frozen { .. }) => {
                    saw_frozen = true
                }
                pair => panic!("scenario resolved mismatched runtime/record pair: {pair:?}"),
            }
        }

        assert!(saw_controllable);
        assert!(saw_frozen);
    }
}
