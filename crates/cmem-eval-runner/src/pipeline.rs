#[cfg(test)]
use crate::commands::read_config;
use crate::commands::{ContinuityRunArgs, RunArgs};
use crate::enrichment;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use cmem_eval::CharacterMemoryAdapter;
use cmem_eval::{
    BenchmarkRunConfig, ControllableDimensionPolicy, DatasetId, DatasetKind,
    EmbeddingBindingRecord, EmbeddingProviderConfig, EmbeddingRuntimeBinding, EpisodeInput,
    FrozenEmbeddingProvider, GraphEnrichmentInput, GraphSnapshotInput, LiveEmbeddingProvider,
    MetricFamily, MetricsConfig, MetricsRecord, ObservationInput, PerQuestionResult,
    ResultContextMetrics, RetrieveInput, RetrievedContextPack, RetrievedItem, RunAdapterMetadata,
    Timer, composition_metrics, count_tokens, estimate_word_count, initialize_registry_metrics_for,
    insert_composition_metrics, insert_context_metrics, insert_integrity_detail_metrics,
    integrity_details_from_outcomes, summarize_rows, write_jsonl, write_summary,
};
use cmem_eval_continuity::{
    ContinuityQueryObservation, ContinuityQueryTrace, ContinuityReportInput, ContinuityRuntime,
    ContinuityScenario, InteractionEvent, ScenarioOutcome, assemble_continuity_report,
    continuity_metric_family, insert_continuity_metrics, parse_fixture_bytes, parse_fixture_source,
    run_continuity_scenario, scenario_missing_features, write_continuity_report,
    write_continuity_traces,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
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
    let (config, config_source) = super::read_config_source(&args.run.config)?;
    ContinuitySpec::validate_config(&config)?;
    let input_source = fs::read_to_string(&args.run.dataset)
        .with_context(|| format!("read continuity fixture {}", args.run.dataset.display()))?;
    let fixture = parse_fixture_source(&args.run.dataset, input_source.as_bytes())?;
    let input_sha256 = cmem_eval::text_sha256(&input_source);
    let scenarios = select_continuity_scenarios(fixture.scenarios, args.scenario.as_deref())?;
    let mut executable = Vec::new();
    for scenario in &scenarios {
        if scenario_missing_features(scenario)?.is_empty() {
            executable.push(scenario.clone());
        }
    }
    let frozen_embedding_providers = if executable.is_empty() {
        HashMap::new()
    } else {
        validate_continuity_embedding_sizes(&config, &executable)?
    };
    run_continuity_pipeline(
        args,
        config,
        config_source,
        input_sha256,
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
    let dimension_policy = store.dimension_policy().to_string();
    let record = EmbeddingBindingRecord::Frozen {
        store_sha256: store.store_sha256()?,
        source: store.source().to_string(),
        model: store.model().to_string(),
        vector_size: store.vector_size(),
        dimension_policy,
    };
    Ok((EmbeddingRuntimeBinding::Frozen { store }, record))
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

#[derive(Default)]
struct MemoryBatch {
    episodes: Vec<EpisodeInput>,
    observations: Vec<ObservationInput>,
    derived_memories: Vec<cmem_eval::DerivedMemoryInput>,
    unresolved_evidence_references: usize,
    dropped_observation_entries: usize,
}

trait DatasetSpec {
    type Item;
    type Question: ?Sized;

    const LATENCY_INCLUDES_INGEST: bool;
    const REPORT_QA_PROGRESS: bool;
    const USES_ENRICHMENT: bool;

    fn metric_family(config: &MetricsConfig) -> MetricFamily;
    fn validate_config(config: &BenchmarkRunConfig) -> Result<()>;
    fn load(source: &str) -> Result<Vec<Self::Item>>;
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
    let (config, config_source) = super::read_config_source(&args.config)?;
    S::validate_config(&config)?;
    config.validate()?;
    let lexical = config.retrieval.mode == cmem_eval::RetrievalMode::Bm25Only;
    let embedding_binding = if lexical {
        EmbeddingBindingRecord::Bm25
    } else {
        live_embedding_binding(&config)?
    };
    let metric_family = S::metric_family(&config.metrics);
    let input_source = fs::read_to_string(&args.dataset)
        .with_context(|| format!("read {}", args.dataset.display()))?;
    let input_sha256 = cmem_eval::text_sha256(&input_source);
    let source_items = S::load(&input_source)?;
    let adapter_metadata = if lexical {
        RunAdapterMetadata::bm25()
    } else {
        RunAdapterMetadata::live()
    };
    let enrichment_by_namespace = if S::USES_ENRICHMENT && !lexical {
        load_enrichment_by_namespace(&config)?
    } else {
        HashMap::new()
    };
    let snapshots_by_item = if S::USES_ENRICHMENT && !lexical {
        load_snapshots_by_dataset_item(&config, &input_sha256)?
    } else {
        HashMap::new()
    };
    let run_root = create_run_root(
        &args.out,
        &[
            ("header", &sibling_output(&args.out, "header.json")),
            ("report", &sibling_output(&args.out, "report.json")),
        ],
    )?;
    let mut adapter = None;
    let namespaces_to_cleanup = source_items.iter().map(S::namespace).collect::<Vec<_>>();
    let result = async {
        let mut header = run_header(
            config_source,
            input_sha256,
            &run_root,
            &config,
            adapter_metadata,
        )?;
        header
            .embedding_bindings
            .insert(config.dataset.to_string(), embedding_binding);
        adapter = if lexical {
            None
        } else {
            Some(CharacterMemoryAdapter::new(&run_root, &config).await?)
        };
        let total_questions = S::total_questions(&source_items);
        let progress = RunProgress::new(
            &config.dataset,
            source_items.len(),
            S::REPORT_QA_PROGRESS.then_some(total_questions),
        );
        let mut rows = Vec::with_capacity(total_questions);
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
                prepare_fresh_namespace(adapter, &namespace).await?;
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
            progress.phase_done(item_number, &item_label, "ingest", &ingest_detail);
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
                    run_id: config.run_id.clone(),
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
                    latency_ms: latency_ms
                        .try_into()
                        .context("query latency exceeds u64 milliseconds")?,
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
            progress.item_finished(item_number, &item_label, item_timer.elapsed_ms());
        }

        progress.write_outputs_started(rows.len());
        write_outputs(args, rows, &[metric_family], header)?;
        Ok(())
    }
    .await;
    let mut cleanup_error = None;
    if let Some(adapter) = &adapter {
        for namespace in &namespaces_to_cleanup {
            if let Err(error) = adapter.cleanup_namespace(namespace).await {
                cleanup_error.get_or_insert(error);
            }
        }
        if let Err(error) = adapter.release_namespaces().await {
            cleanup_error.get_or_insert(error);
        }
    }
    finish_run(
        result,
        cleanup_error,
        &run_root,
        config.backend.retain_stores,
    )
}

async fn prepare_fresh_namespace(adapter: &CharacterMemoryAdapter, namespace: &str) -> Result<()> {
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
        if config.retrieval.mode == cmem_eval::RetrievalMode::Bm25Only {
            bail!("continuity does not support retrieval.mode=bm25_only");
        }
        config.validate()?;
        if !config.retrieval.surface_policy.include_debug_rationale {
            bail!(
                "continuity dataset requires retrieval.surface_policy.include_debug_rationale=true because continuity traces and rationale-derived metrics are mandatory"
            );
        }
        Ok(())
    }

    fn load(source: &str) -> Result<Vec<Self::Item>> {
        Ok(parse_fixture_bytes(source.as_bytes())?.scenarios)
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
        MemoryBatch::default()
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
    config_source: String,
    input_sha256: String,
    scenarios: Vec<ContinuityScenario>,
    frozen_embedding_providers: FrozenEmbeddingProviders,
) -> Result<()> {
    let adapter_metadata = RunAdapterMetadata::live();
    let metric_family = continuity_metric_family(&config.metrics, &scenarios);
    let total_queries = ContinuitySpec::total_questions(&scenarios);
    let progress = RunProgress::new(&config.dataset, scenarios.len(), Some(total_queries));
    let mut traces = Vec::with_capacity(total_queries);
    let mut outcomes = BTreeMap::new();
    let mut operation_counts: BTreeMap<String, usize> = BTreeMap::new();
    let run_root = create_run_root(
        &args.run.out,
        &[
            ("header", &sibling_output(&args.run.out, "header.json")),
            ("report", &sibling_output(&args.run.out, "report.json")),
        ],
    )?;
    let mut runtimes = Vec::with_capacity(scenarios.len());
    let result = async {
        let mut header = run_header(
            config_source,
            input_sha256,
            &run_root,
            &config,
            adapter_metadata,
        )?;
        for (index, scenario) in scenarios.iter().enumerate() {
            let item_number = index + 1;
            progress.item_started(item_number, &scenario.fixture_id);
            let missing = scenario_missing_features(scenario)?;
            if !missing.is_empty() {
                outcomes.insert(
                    scenario.fixture_id.clone(),
                    ScenarioOutcome::not_run(scenario, missing),
                );
                progress.item_finished(item_number, &scenario.fixture_id, 0);
                continue;
            }
            let frozen_embedding_provider =
                if scenario.embedding.provider_name() == "frozen" {
                    let store_path = config.backend.embedding.store_path.as_deref().context(
                        "frozen continuity runtime requires backend.embedding.store_path",
                    )?;
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
            header
                .embedding_bindings
                .insert(scenario.fixture_id.clone(), embedding_binding_record);
            let runtime = ContinuityRuntime::new(&run_root, &config, embedding_binding).await?;
            runtimes.push((scenario.namespace.clone(), runtime));
            let runtime = &mut runtimes.last_mut().expect("just stored runtime").1;
            let run = run_continuity_scenario(runtime, scenario, &config.retrieval).await?;
            outcomes.insert(scenario.fixture_id.clone(), run.outcome);
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
                let result =
                    continuity_result_row(&config, &metric_family, scenario, &trace, latency_ms)?;
                let restart_observations = run
                    .restart_observations
                    .iter()
                    .filter(|observation| observation.probe_query_id == trace.query_id)
                    .cloned()
                    .collect();
                traces.push(ContinuityQueryTrace {
                    result,
                    fixture_id: trace.fixture_id,
                    namespace: trace.namespace,
                    event_id: trace.event_id,
                    timestamp: trace.timestamp,
                    expected: trace.expected,
                    history_text: trace.history_text,
                    restart_observations,
                });
            }
            progress.item_finished(item_number, &scenario.fixture_id, 0);
            runtime
                .adapter()
                .detach_namespace(&scenario.namespace)
                .await?;
        }

        progress.write_outputs_started(traces.len());
        write_continuity_traces(&args.run.out, &traces)?;
        write_run_header(&args.run.out, &header)?;
        let report = assemble_continuity_report(ContinuityReportInput {
            config: serde_json::to_value(&config)?,
            traces: &traces,
            outcomes: &outcomes,
            metric_family: &metric_family,
        })?;
        write_continuity_report(&sibling_output(&args.run.out, "report.json"), &report)?;
        eprintln!(
            "[cmem-eval][continuity][operations] {}",
            serde_json::to_string(&operation_counts)?
        );

        Ok(())
    }
    .await;
    progress.cleanup_started(runtimes.len());
    let mut cleanup_error = None;
    for (namespace, runtime) in runtimes {
        if let Err(error) = runtime.cleanup(&namespace).await {
            cleanup_error.get_or_insert(error);
        }
    }
    finish_run(
        result,
        cleanup_error,
        &run_root,
        config.backend.retain_stores,
    )
}

fn continuity_result_row(
    config: &BenchmarkRunConfig,
    metric_family: &MetricFamily,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryObservation,
    latency_ms: u128,
) -> Result<PerQuestionResult> {
    let full_history = full_history_context_metrics(Some(&trace.history_text));
    let context = context_metrics_with_full_history(&trace.retrieval, full_history);
    let composition = composition_metrics(trace.retrieval.items());
    let integrity = if config.retrieval.mode == cmem_eval::RetrievalMode::Hybrid {
        integrity_details_from_outcomes(trace.retrieval.items(), trace.retrieval.outcomes())
    } else {
        cmem_eval::integrity_details(trace.retrieval.items())
    };
    let mut metrics = Map::new();
    insert_common_metrics(
        &mut metrics,
        &context,
        &composition,
        &integrity,
        std::slice::from_ref(metric_family),
    );
    insert_continuity_metrics(
        &mut metrics,
        scenario,
        trace,
        &config.metrics,
        config.retrieval.mode,
    );
    let question_type = serde_json::to_value(trace.pattern)?
        .as_str()
        .map(str::to_string);
    Ok(PerQuestionResult {
        run_id: config.run_id.clone(),
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
        latency_ms: latency_ms
            .try_into()
            .context("query latency exceeds u64 milliseconds")?,
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

    fn load(source: &str) -> Result<Vec<Self::Item>> {
        Ok(cmem_eval_longmemeval::load_value(serde_json::from_str(
            source,
        )?)?)
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
            ..MemoryBatch::default()
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

    fn load(source: &str) -> Result<Vec<Self::Item>> {
        Ok(cmem_eval_locomo::load_value(serde_json::from_str(source)?)?)
    }

    fn item_id(item: &Self::Item) -> &str {
        &item.sample_id
    }

    fn namespace(item: &Self::Item) -> String {
        item.namespace()
    }

    fn memory_inputs(item: &Self::Item, config: &BenchmarkRunConfig) -> MemoryBatch {
        let baseline = matches!(
            config.retrieval.mode,
            cmem_eval::RetrievalMode::Bm25Only | cmem_eval::RetrievalMode::VectorOnly
        );
        let mapped = cmem_eval_locomo::ingest::to_memory_inputs(
            item,
            config.ingest.include_image_captions,
            config.ingest.index_session_summaries && !baseline,
            config.ingest.index_generated_observations && !baseline,
        );
        MemoryBatch {
            episodes: mapped.episodes,
            observations: mapped.observations,
            derived_memories: mapped.derived_memories,
            unresolved_evidence_references: item.unresolved_evidence_references,
            dropped_observation_entries: item.dropped_observation_entries,
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
        if config.retrieval.mode == cmem_eval::RetrievalMode::VectorOnly {
            return Ok(None);
        }
        let mut result = enrichment::empty_namespace(namespace.to_string());
        result.derived_memories = derived_memories;
        if let Some(snapshot) = snapshots.get(&item.sample_id) {
            if snapshot.namespace != namespace {
                bail!(
                    "LoCoMo snapshot {} namespace {} does not match expected {}",
                    snapshot.snapshot_id,
                    snapshot.namespace,
                    namespace
                );
            }
            enrichment::merge_enrichment(&mut result, snapshot.graph.clone())?;
            return Ok(Some(result));
        }
        if config.ingest.enrichment_snapshot_path.is_some() {
            bail!(
                "missing LoCoMo enrichment snapshot for sample_id {}",
                item.sample_id
            );
        }
        if let Some(configured) = configured.get(namespace).cloned() {
            enrichment::merge_enrichment(&mut result, configured)?;
        } else {
            enrichment::validate_enrichment(&result)?;
        }
        Ok(Some(result))
    }

    fn ingest_progress_detail(batch: &MemoryBatch) -> String {
        format!(
            "episodes={} observations={} generated_derived={} unresolved_evidence_references={} dropped_observation_entries={}",
            batch.episodes.len(),
            batch.observations.len(),
            batch.derived_memories.len(),
            batch.unresolved_evidence_references,
            batch.dropped_observation_entries
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
    input_sha256: &str,
) -> Result<HashMap<String, GraphSnapshotInput>> {
    config
        .ingest
        .enrichment_snapshot_path
        .as_ref()
        .map(|path| {
            enrichment::load_snapshot_path(Path::new(path), config.dataset.as_str(), input_sha256)
        })
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

#[derive(Debug)]
struct OutputPathInStores {
    name: &'static str,
    path: PathBuf,
    root: PathBuf,
}

impl std::fmt::Display for OutputPathInStores {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "output {} ({}) must not be inside reserved run stores {}",
            self.name,
            self.path.display(),
            self.root.display()
        )
    }
}

impl std::error::Error for OutputPathInStores {}

#[derive(Debug)]
struct OutputPathExists {
    name: &'static str,
    path: PathBuf,
}

impl std::fmt::Display for OutputPathExists {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "output {} already exists; choose a new output directory or remove it: {}",
            self.name,
            self.path.display()
        )
    }
}

impl std::error::Error for OutputPathExists {}

fn create_run_root(
    results_path: &Path,
    other_outputs: &[(&'static str, &Path)],
) -> Result<PathBuf> {
    if results_path.extension() != Some(std::ffi::OsStr::new("jsonl")) {
        bail!("out must end in .jsonl: {}", results_path.display());
    }
    let output_dir = results_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_dir)?;
    let root = output_dir.join("stores");
    fs::create_dir(&root).with_context(|| format!("create run stores {}; existing retained or crashed stores must be inspected before choosing a new output directory", root.display()))?;
    let admission = (|| -> Result<()> {
        let canonical_root = fs::canonicalize(&root)?;
        for (name, path) in
            std::iter::once(("out", results_path)).chain(other_outputs.iter().copied())
        {
            let parent = path
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            fs::create_dir_all(parent)?;
            let canonical_parent = fs::canonicalize(parent)?;
            match fs::symlink_metadata(path) {
                Ok(_) => {
                    return Err(OutputPathExists {
                        name,
                        path: path.to_path_buf(),
                    }
                    .into());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("inspect {name} {}", path.display()));
                }
            }
            if canonical_parent.starts_with(&canonical_root) {
                return Err(OutputPathInStores {
                    name,
                    path: path.to_path_buf(),
                    root: root.clone(),
                }
                .into());
            }
        }
        Ok(())
    })();
    if let Err(error) = admission {
        // This invocation acquired root before creating any output parents inside it.
        return finish_run(Err(error), None, &root, false).map(|()| root);
    }
    std::path::absolute(root).map_err(Into::into)
}

fn finish_run(
    result: Result<()>,
    mut cleanup_error: Option<anyhow::Error>,
    root: &Path,
    retain: bool,
) -> Result<()> {
    if !retain
        && cleanup_error.is_none()
        && let Err(error) = fs::remove_dir_all(root)
            .with_context(|| format!("remove run stores {}", root.display()))
    {
        cleanup_error = Some(error);
    }
    match (result, cleanup_error) {
        (Ok(()), None) => Ok(()),
        (Err(error), None) => Err(error),
        (Ok(()), Some(error)) => Err(error),
        (Err(error), Some(cleanup)) => Err(error.context(format!(
            "run also failed to clean up {}: {cleanup:#}",
            root.display()
        ))),
    }
}

fn run_header(
    config_source: String,
    input_sha256: String,
    run_root: &Path,
    config: &BenchmarkRunConfig,
    adapter: RunAdapterMetadata,
) -> Result<cmem_eval::RunHeader> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let commit = |path: &Path| -> Result<String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "HEAD"])
            .output()
            .with_context(|| format!("read commit at {}", path.display()))?;
        if !output.status.success() {
            bail!(
                "read commit at {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    };
    Ok(cmem_eval::RunHeader {
        run_id: config.run_id.clone(),
        dataset: config.dataset.clone(),
        dataset_kind: dataset_descriptor(&config.dataset)?.kind,
        input_sha256,
        embedding_bindings: BTreeMap::new(),
        harness_commit: commit(&workspace)?,
        library_commit: commit(&workspace.join("../CharacterMemory"))?,
        generated_at: Utc::now(),
        config_sha256: cmem_eval::text_sha256(&config_source),
        config: config_source,
        adapter,
        storage_root: run_root.to_path_buf(),
        storage_root_sha256: cmem_eval::adapter::run_root_sha256(run_root)?,
        retain_reason: config.backend.retain_reason.clone(),
    })
}

fn write_outputs(
    args: RunArgs,
    rows: Vec<PerQuestionResult>,
    metric_families: &[MetricFamily],
    header: cmem_eval::RunHeader,
) -> Result<()> {
    cmem_eval::reject_empty_run(&rows)?;
    let summary = summarize_rows(&rows, metric_families)?;
    write_jsonl(&args.out, &rows)?;
    write_run_header(&args.out, &header)?;
    write_summary(&sibling_output(&args.out, "report.json"), &summary)
}

fn sibling_output(artifact: &Path, name: &str) -> PathBuf {
    artifact.with_file_name(name)
}

fn write_run_header(artifact: &Path, header: &cmem_eval::RunHeader) -> Result<()> {
    let path = sibling_output(artifact, "header.json");
    let mut bytes = serde_json::to_vec_pretty(header)?;
    bytes.push(b'\n');
    let mut file = fs::File::create_new(&path)
        .with_context(|| format!("create run header {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("write run header {}", path.display()))
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
        }
    }

    fn read_rows(path: &Path) -> Vec<PerQuestionResult> {
        cmem_eval::read_jsonl(path).unwrap()
    }

    fn read_header(path: &Path) -> cmem_eval::RunHeader {
        serde_json::from_slice(&fs::read(sibling_output(path, "header.json")).unwrap()).unwrap()
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
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let scenario = fixture
            .scenarios
            .iter()
            .find(|scenario| scenario.fixture_id == "cross-store-stress")
            .unwrap();
        let (binding, _) = continuity_embedding_binding(&config, scenario, None).unwrap();
        let mut runtime = ContinuityRuntime::new(directory.path(), &config, binding)
            .await
            .unwrap();
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
        isolate_test_config(&mut config);
        fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
        ContinuityRunArgs {
            run: RunArgs {
                dataset: PathBuf::from("../cmem-eval-continuity/fixtures/continuity_v3.json"),
                config: config_path,
                out: directory.join("continuity.jsonl"),
            },

            scenario: None,
        }
    }

    fn isolate_test_config(config: &mut BenchmarkRunConfig) {
        config.backend.vector_store_mode = cmem_eval::VectorStoreMode::Embedded;
        config.backend.qdrant_connection_string = Some("http://127.0.0.1:1".into());
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
        isolate_test_config(&mut config);
        config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
        fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
        path
    }

    #[tokio::test]
    async fn fresh_namespace_preparation_refuses_existing_state() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = current_continuity_config();
        isolate_test_config(&mut config);
        config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
        let adapter = CharacterMemoryAdapter::new(directory.path(), &config)
            .await
            .unwrap();
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

        let error = prepare_fresh_namespace(&adapter, "stale")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stale"), "{error:#}");
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
        assert!(
            pack.items()
                .iter()
                .any(|item| item.external_id.as_deref() == Some("old"))
        );
        adapter.close().await.unwrap();
    }

    #[tokio::test]
    async fn continuity_run_cleans_or_retains_stores_on_success_and_admission_failure() {
        for retain in [false, true] {
            for fail_output in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let mut args = continuity_args(directory.path());
                args.scenario = Some("graded-similarity".into());
                let mut config = read_config(&args.run.config).unwrap();
                config.backend.retain_stores = retain;
                config.backend.retain_reason = retain.then(|| "inspect regression evidence".into());
                let source = toml::to_string(&config).unwrap();
                fs::write(&args.run.config, &source).unwrap();
                if fail_output {
                    fs::create_dir(sibling_output(&args.run.out, "report.json")).unwrap();
                }
                let root = directory.path().join("stores");
                let result = run_continuity(args.clone()).await;
                assert_eq!(result.is_err(), fail_output, "{result:?}");
                assert_eq!(
                    root.exists(),
                    retain && !fail_output,
                    "retain={retain}, fail_output={fail_output}"
                );
                if !fail_output {
                    let header = read_header(&args.run.out);
                    let header_path = sibling_output(&args.run.out, "header.json");
                    let existing = fs::read(&header_path).unwrap();
                    assert!(write_run_header(&args.run.out, &header).is_err());
                    assert_eq!(fs::read(&header_path).unwrap(), existing);
                    assert_eq!(header.run_id, config.run_id);
                    assert_eq!(header.dataset, config.dataset);
                    assert_eq!(header.dataset_kind, DatasetKind::Continuity);
                    assert_eq!(
                        header.input_sha256,
                        cmem_eval::text_sha256(&fs::read_to_string(&args.run.dataset).unwrap())
                    );
                    assert_eq!(header.embedding_bindings.len(), 1);
                    assert!(header.embedding_bindings.contains_key("graded-similarity"));
                    assert_eq!(header.storage_root, root);
                    assert_eq!(header.storage_root_sha256.len(), 64);
                    if retain {
                        assert_eq!(
                            header.storage_root_sha256,
                            cmem_eval::adapter::run_root_sha256(&root).unwrap()
                        );
                    }
                    assert_eq!(header.retain_reason.is_some(), retain);
                    assert!(
                        serde_json::to_value(&header)
                            .unwrap()
                            .get("retain_stores")
                            .is_none()
                    );
                    assert_eq!(header.retain_reason, config.backend.retain_reason);
                    assert_eq!(header.config, source);
                    assert_eq!(header.config_sha256, cmem_eval::text_sha256(&source));
                    assert_eq!(header.harness_commit.len(), 40);
                    assert_eq!(header.library_commit.len(), 40);
                    let report = cmem_eval_continuity::read_continuity_report(&sibling_output(
                        &args.run.out,
                        "report.json",
                    ))
                    .unwrap();
                    assert_eq!(report.aggregate.query_count, 1);
                    assert!(
                        serde_json::to_value(report)
                            .unwrap()
                            .get("header")
                            .is_none()
                    );
                }
                if retain && !fail_output {
                    let sentinel = root.join("preserve-me");
                    fs::write(&sentinel, b"retained run").unwrap();
                    let error = run_continuity(args).await.unwrap_err();
                    assert!(format!("{error:#}").contains("stores"), "{error:#}");
                    assert_eq!(fs::read(sentinel).unwrap(), b"retained run");
                }
            }
        }
    }

    #[test]
    fn output_write_failure_respects_store_retention_after_admission() {
        for retain in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("results.jsonl");
            let root = create_run_root(&output, &[]).unwrap();
            fs::create_dir(&output).unwrap();
            let write_result = write_continuity_traces(&output, &[]);
            assert!(finish_run(write_result, None, &root, retain).is_err());
            assert_eq!(root.exists(), retain);
        }
    }

    #[tokio::test]
    async fn continuity_rejects_bm25_before_fixture_or_embedding_setup() {
        let directory = tempfile::tempdir().unwrap();
        let mut args = continuity_args(directory.path());
        let mut config = read_config(&args.run.config).unwrap();
        config.retrieval.mode = cmem_eval::RetrievalMode::Bm25Only;
        fs::write(&args.run.config, toml::to_string(&config).unwrap()).unwrap();
        args.run.dataset = directory.path().join("missing-fixture.json");
        let error = run_continuity(args).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("continuity does not support retrieval.mode=bm25_only"),
            "{error:#}"
        );
        assert!(!directory.path().join("stores").exists());
    }

    #[test]
    fn derived_outputs_cannot_enter_the_reserved_stores_root() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("not-created/results.jsonl");
        for suffix in [
            "stores/header.json",
            "nested/../stores/header.json",
            "stores/./nested/header.json",
        ] {
            let header = output.with_file_name(suffix);
            let error = create_run_root(&output, &[("header", &header)]).unwrap_err();
            assert_eq!(
                error.downcast_ref::<OutputPathInStores>().unwrap().name,
                "header"
            );
            assert!(!output.exists());
            assert!(!output.parent().unwrap().join("stores").exists());
        }
        if cfg!(windows) {
            let header = output.with_file_name("STORES/header.json");
            assert!(create_run_root(&output, &[("header", &header)]).is_err());
            assert!(!output.exists());
            assert!(!output.parent().unwrap().join("stores").exists());
        }
        let report = output.with_file_name("stores-sibling/report.json");
        let root = create_run_root(&output, &[("report", &report)]).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[tokio::test]
    async fn cli_rejects_non_jsonl_output_before_creating_directories() {
        let directory = tempfile::tempdir().unwrap();
        let dataset = directory.path().join("locomo.json");
        fs::write(
            &dataset,
            r#"[{"sample_id":"p1","conversation":{"session_1":[{"dia_id":"d1","text":""}]},"qa":[{"question":"What?"}]}]"#,
        ).unwrap();
        for filename in ["header.json", "HEADER.JSON", "report.json", "results"] {
            let output_dir = directory.path().join(filename).join("run");
            let mut args = continuity_args(directory.path());
            args.scenario = Some("correction-chains".into());
            args.run.out = output_dir.join(filename);
            assert!(
                run_continuity(args)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("out must end in .jsonl")
            );
            assert!(!output_dir.exists());
            let mut args = run_args(
                dataset.clone(),
                service_free_config("../../configs/locomo_retrieval.toml", directory.path()),
                &output_dir,
            );
            args.out = output_dir.join(filename);
            assert!(
                run_locomo(args)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("out must end in .jsonl")
            );
            assert!(!output_dir.exists());
        }
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn output_leaf_links_are_rejected_before_writing_artifacts() {
        #[cfg(unix)]
        use std::os::unix::fs::symlink as symlink_file;
        #[cfg(windows)]
        use std::os::windows::fs::symlink_file;

        let directory = tempfile::tempdir().unwrap();
        let output_dir = directory.path().join("outputs");
        fs::create_dir(&output_dir).unwrap();
        let target = output_dir.join("stores/header.json");
        let mut args = continuity_args(directory.path());
        args.run.out = output_dir.join("results.jsonl");
        let header = sibling_output(&args.run.out, "header.json");
        symlink_file(&target, &header).unwrap();
        let error = run_continuity(args).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<OutputPathExists>().unwrap().name,
            "header"
        );
        assert!(!output_dir.join("stores").exists());
        assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 1);
        fs::remove_file(header).unwrap();

        // All callers share the guard, including links to ordinary existing files.
        for existing in [false, true] {
            let target = directory.path().join("target.json");
            if existing {
                fs::write(&target, "original").unwrap();
            }
            for name in ["out", "header", "report"] {
                let leaf = output_dir.join("link.jsonl");
                symlink_file(&target, &leaf).unwrap();
                let output = output_dir.join("results.jsonl");
                let error = if name == "out" {
                    create_run_root(&leaf, &[])
                } else {
                    create_run_root(&output, &[(name, &leaf)])
                }
                .unwrap_err();
                assert_eq!(error.downcast_ref::<OutputPathExists>().unwrap().name, name);
                assert!(!output_dir.join("stores").exists());
                fs::remove_file(leaf).unwrap();
            }
            if existing {
                assert_eq!(fs::read_to_string(target).unwrap(), "original");
            }
        }
    }

    #[test]
    fn existing_output_files_and_directories_fail_admission() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("results.jsonl");
        let header = directory.path().join("header.json");
        let report = directory.path().join("report.json");
        for is_directory in [false, true] {
            for (name, path) in [("out", &output), ("header", &header), ("report", &report)] {
                if is_directory {
                    fs::create_dir(path).unwrap();
                } else {
                    fs::write(path, "original").unwrap();
                }
                let error = create_run_root(&output, &[("header", &header), ("report", &report)])
                    .unwrap_err();
                assert_eq!(error.downcast_ref::<OutputPathExists>().unwrap().name, name);
                assert!(!directory.path().join("stores").exists());
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                if is_directory {
                    assert!(fs::read_dir(path).unwrap().next().is_none());
                    fs::remove_dir(path).unwrap();
                } else {
                    assert_eq!(fs::read_to_string(path).unwrap(), "original");
                    fs::remove_file(path).unwrap();
                }
            }
        }
    }

    #[tokio::test]
    async fn hard_linked_outputs_fail_before_writing_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let output_dir = directory.path().join("outputs");
        fs::create_dir(&output_dir).unwrap();
        let mut args = continuity_args(directory.path());
        args.run.out = output_dir.join("traces.jsonl");
        let header = sibling_output(&args.run.out, "header.json");
        fs::write(&args.run.out, b"original").unwrap();
        fs::hard_link(&args.run.out, &header).unwrap();
        let artifact = args.run.out.clone();

        let error = run_continuity(args).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<OutputPathExists>().unwrap().name,
            "out"
        );
        assert_eq!(fs::read(&artifact).unwrap(), b"original");
        assert_eq!(fs::read(&header).unwrap(), b"original");
        assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 2);
        // Both names still point to the original file after rejected admission.
        fs::write(&artifact, b"still linked").unwrap();
        assert_eq!(fs::read(&header).unwrap(), b"still linked");
    }

    #[cfg(windows)]
    #[test]
    fn unicode_case_alias_cannot_place_output_in_stores() {
        let directory = tempfile::tempdir().unwrap();
        let output_dir = directory.path().join("\u{e9}valuation");
        let output = output_dir.join("traces.jsonl");
        let header = directory.path().join("\u{c9}valuation/stores/header.json");
        let error = create_run_root(&output, &[("header", &header)]).unwrap_err();
        assert_eq!(
            error.downcast_ref::<OutputPathInStores>().unwrap().name,
            "header"
        );
        assert!(fs::read_dir(output_dir).unwrap().next().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn junction_alias_cannot_place_output_in_stores() {
        let directory = tempfile::tempdir().unwrap();
        let output_dir = directory.path().join("real-output");
        let alias = directory.path().join("output-alias");
        fs::create_dir(&output_dir).unwrap();
        let created = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(&output_dir)
            .output()
            .unwrap();
        assert!(created.status.success(), "{created:?}");
        let same_identity =
            fs::canonicalize(&alias).unwrap() == fs::canonicalize(&output_dir).unwrap();
        let result = create_run_root(
            &output_dir.join("traces.jsonl"),
            &[("header", &alias.join("stores/header.json"))],
        );
        fs::remove_dir(&alias).unwrap();
        assert!(same_identity);
        let error = result.unwrap_err();
        assert_eq!(
            error.downcast_ref::<OutputPathInStores>().unwrap().name,
            "header"
        );
        assert!(fs::read_dir(output_dir).unwrap().next().is_none());
    }

    #[test]
    fn run_root_admission_preserves_an_existing_directory() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("stores");
        let results_path = directory.path().join("results.jsonl");
        let barrier = std::sync::Barrier::new(2);
        let admissions = std::thread::scope(|scope| {
            let acquire = || {
                barrier.wait();
                create_run_root(&results_path, &[])
            };
            let first = scope.spawn(acquire);
            let second = scope.spawn(acquire);
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(admissions.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        fs::write(root.join("sentinel"), b"earlier run").unwrap();
        let error = create_run_root(&directory.path().join("results.jsonl"), &[]).unwrap_err();
        assert!(
            format!("{error:#}").contains(&root.display().to_string()),
            "{error:#}"
        );
        assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"earlier run");
    }

    #[tokio::test]
    async fn malformed_datasets_fail_before_output_admission() {
        for locomo in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let dataset = directory.path().join("malformed.json");
            fs::write(&dataset, r#"{"unexpected": []}"#).unwrap();
            let config = Path::new(env!("CARGO_MANIFEST_DIR")).join(if locomo {
                "../../configs/locomo_retrieval.toml"
            } else {
                "../../configs/longmemeval_s_retrieval.toml"
            });
            let output = directory.path().join("uncreated-output");
            let args = run_args(dataset, config, &output);
            let error = if locomo {
                run_locomo(args).await.unwrap_err()
            } else {
                run_longmemeval(args).await.unwrap_err()
            };
            if locomo {
                assert!(matches!(
                    error.downcast_ref::<cmem_eval_locomo::LoadError>(),
                    Some(cmem_eval_locomo::LoadError::Admission {
                        location: cmem_eval_locomo::AdmissionLocation::Root,
                        field,
                        ..
                    }) if field == "root"
                ));
            } else {
                assert!(matches!(
                    error.downcast_ref::<cmem_eval_longmemeval::LoadError>(),
                    Some(cmem_eval_longmemeval::LoadError::Admission {
                        location: cmem_eval_longmemeval::AdmissionLocation::Root,
                        field,
                        ..
                    }) if field == "root"
                ));
            }
            assert!(!output.exists(), "load failure created output state");
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }

    #[tokio::test]
    async fn enrichment_admission_distinguishes_locomo_baselines_from_longmemeval_lexical() {
        use cmem_eval::RetrievalMode;

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        for (source, dataset) in [
            (
                include_str!("../../../configs/locomo_bm25.toml"),
                serde_json::json!([{
                    "sample_id": "p1",
                    "conversation": [{"session_id": "s1", "turns": [{"dia_id": "d1", "text": "jasmine tea"}]}],
                    "qa": [{"question_id": "q1", "question": "tea", "evidence": ["d1"]}]
                }]),
            ),
            (
                include_str!("../../../configs/longmemeval_s_bm25.toml"),
                serde_json::json!([{
                    "question_id": "q1", "question": "tea",
                    "haystack_session_ids": ["s1"],
                    "haystack_sessions": [[{"content": "jasmine tea", "has_answer": true}]],
                    "answer_session_ids": ["s1"]
                }]),
            ),
        ] {
            for snapshot in [false, true] {
                for mode in [RetrievalMode::Bm25Only, RetrievalMode::Hybrid] {
                    let directory = tempfile::tempdir_in(&root).unwrap();
                    let mut config: BenchmarkRunConfig = toml::from_str(source).unwrap();
                    config.retrieval.mode = mode;
                    isolate_test_config(&mut config);
                    if mode == RetrievalMode::Hybrid {
                        config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
                    }
                    let missing = directory.path().join("missing-enrichment.jsonl");
                    let missing_path = missing.display().to_string();
                    if snapshot {
                        config.ingest.enrichment_snapshot_path = Some(missing_path.clone());
                    } else {
                        config.ingest.enrichment_path = Some(missing_path.clone());
                    }
                    let config_path = directory.path().join("config.toml");
                    fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
                    let dataset_path = directory.path().join("dataset.json");
                    fs::write(&dataset_path, serde_json::to_vec(&dataset).unwrap()).unwrap();
                    let args = run_args(dataset_path, config_path, directory.path());
                    let output = args.out.clone();
                    let result = if config.dataset.as_str() == "locomo" {
                        run_locomo(args).await
                    } else {
                        run_longmemeval(args).await
                    };
                    if mode == RetrievalMode::Bm25Only && config.dataset.as_str() == "locomo" {
                        let expected_field = if snapshot {
                            "enrichment_snapshot_path"
                        } else {
                            "enrichment_path"
                        };
                        assert_eq!(
                            result
                                .unwrap_err()
                                .downcast_ref::<cmem_eval_locomo::ConfigError>(),
                            Some(&cmem_eval_locomo::ConfigError::BaselineDerivedContent {
                                mode,
                                field: expected_field
                            })
                        );
                        assert!(!output.exists());
                    } else if mode == RetrievalMode::Bm25Only {
                        result.unwrap();
                        let rows = read_rows(&output);
                        assert_eq!(rows.len(), 1);
                        assert!(!rows[0].retrieved.is_empty());
                    } else {
                        let error = result.unwrap_err();
                        if snapshot {
                            assert_eq!(
                                error.downcast_ref::<enrichment::EnrichmentError>(),
                                Some(&enrichment::EnrichmentError::MissingManifest {
                                    path: missing
                                        .with_file_name("missing-enrichment_manifest.json"),
                                })
                            );
                        } else {
                            assert!(format!("{error:#}").contains(&missing_path), "{error:#}");
                        }
                    }
                }
            }
        }
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
        let summary_output = sibling_output(&args.out, "report.json").clone();
        let input_sha256 = cmem_eval::text_sha256(&fs::read_to_string(&args.dataset).unwrap());
        let config = read_config(&args.config).unwrap();

        run_longmemeval(args.clone()).await.unwrap();
        assert!(!dir.path().join("stores").exists());
        let mut failing_args = args;
        failing_args.out = dir.path().join("output-directory");
        fs::create_dir(&failing_args.out).unwrap();
        assert!(run_longmemeval(failing_args).await.is_err());
        assert!(!dir.path().join("stores").exists());

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
        let summary = cmem_eval::read_summary(&summary_output).unwrap();
        assert!(!summary.degradation.any_degradation);
        let header = read_header(&output);
        assert_eq!(header.input_sha256, input_sha256);
        assert_eq!(header.dataset_kind, DatasetKind::LongMemEvalS);
        assert_eq!(
            header.embedding_bindings,
            BTreeMap::from([(
                "longmemeval_s".into(),
                live_embedding_binding(&config).unwrap()
            )])
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
    async fn continuity_integrity_support_follows_retrieval_mode() {
        use cmem_eval::{ObjectType, RetrievalMode};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        for mode in [RetrievalMode::VectorOnly, RetrievalMode::Hybrid] {
            let directory = tempfile::tempdir_in(&root).unwrap();
            let mut args = continuity_args(directory.path());
            let mut fixture = parse_fixture_bytes(&fs::read(&args.run.dataset).unwrap()).unwrap();
            fixture.scenarios.retain(|scenario| {
                matches!(
                    scenario.fixture_id.as_str(),
                    "recurring-hub-entity" | "entrenched-correction"
                )
            });
            args.run.dataset = directory.path().join("fixture.json");
            fs::write(&args.run.dataset, serde_json::to_vec(&fixture).unwrap()).unwrap();
            let mut config = read_config(&args.run.config).unwrap();
            config.retrieval.mode = mode;
            if mode == RetrievalMode::VectorOnly {
                config.retrieval.surface_policy.object_types =
                    vec![ObjectType::Episode, ObjectType::Observation];
            }
            fs::write(&args.run.config, toml::to_string(&config).unwrap()).unwrap();
            let output = args.run.out.clone();
            run_continuity(args).await.unwrap();
            let rows = read_rows(&output);
            assert_eq!(rows.len(), 4);
            for row in &rows {
                assert!(!row.retrieved.is_empty());
                let metrics = row.metrics.to_json_map();
                let integrity = serde_json::to_value(&row.integrity).unwrap();
                for (metric_key, detail_key) in [
                    (
                        "context_validation_pass_rate",
                        "context_validation_pass_rate",
                    ),
                    (
                        "suppressed_memory_leakage_rate",
                        "suppressed_memory_leakage_rate",
                    ),
                    ("orphan_vector_leakage_rate", "orphan_vector_leakage_rate"),
                    (
                        "superseded_current_leakage_rate",
                        "superseded_current_leakage_rate",
                    ),
                    (
                        "suppressed_or_deleted_items_returned",
                        "suppressed_or_deleted_returned_count",
                    ),
                    (
                        "superseded_items_returned_as_current",
                        "superseded_current_returned_count",
                    ),
                ] {
                    for value in [&metrics[metric_key], &integrity[detail_key]] {
                        if mode == RetrievalMode::Hybrid {
                            assert!(value.is_number(), "{mode:?} {metric_key}: {value}");
                        } else {
                            assert!(value.is_null(), "{mode:?} {metric_key}: {value}");
                        }
                    }
                }
                assert!(metrics["returned_items_without_external_id"].is_number());
                let native_metrics = metrics
                    .iter()
                    .filter(|(key, _)| {
                        matches!(
                            key.as_str(),
                            "correction_lifecycle_safe_admission_rate"
                                | "hub_expansion_relevant_hit_rate"
                                | "typed_rationale_coverage"
                        ) || key.starts_with("rationale_category_share_")
                            || key.starts_with("sampled_pollution_rationale_share_")
                    })
                    .collect::<Vec<_>>();
                if mode == RetrievalMode::VectorOnly {
                    for (key, value) in native_metrics {
                        assert!(value.is_null(), "{} {key}: {value}", row.question_id);
                    }
                } else {
                    assert!(metrics["typed_rationale_coverage"].is_number());
                    if row.question_type.as_deref() == Some("entrenched_correction") {
                        assert!(metrics["correction_lifecycle_safe_admission_rate"].is_number());
                    }
                    if row.question_type.as_deref() == Some("recurring_hub_entity") {
                        assert!(metrics["hub_expansion_relevant_hit_rate"].is_number());
                    }
                }
                assert!(metrics["sampled_context_pollution_rate"].is_number());
                assert!(metrics["hub_context_share"].is_number());
            }
        }
    }

    #[tokio::test]
    async fn vector_only_restart_and_report_include_every_kind_outcome() {
        use cmem_eval::{ObjectType, RetrievalMode};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        let directory = tempfile::tempdir_in(root).unwrap();
        let mut args = continuity_args(directory.path());
        let mut fixture = parse_fixture_bytes(&fs::read(&args.run.dataset).unwrap()).unwrap();
        fixture
            .scenarios
            .retain(|scenario| scenario.fixture_id == "recurring-hub-entity");
        let scenario = &mut fixture.scenarios[0];
        let query_index = scenario
            .events
            .iter()
            .position(|event| matches!(event, InteractionEvent::Query { .. }))
            .unwrap();
        let timestamp = scenario.events[query_index].timestamp();
        let links = ["", ":observation"].map(|suffix| InteractionEvent::Link {
            event_id: format!("link-kinds{suffix}"),
            external_id: format!("link-kinds{suffix}"),
            timestamp: timestamp - chrono::Duration::seconds(2),
            from_external_id: format!("hub-memory-0{suffix}"),
            relation: "associated_with".into(),
            to_external_id: format!("hub-memory-5{suffix}"),
        });
        scenario.events.splice(
            query_index..query_index,
            links.into_iter().chain([InteractionEvent::Restart {
                event_id: "restart-multiple-kinds".into(),
                timestamp: timestamp - chrono::Duration::seconds(1),
                reopen_graph: true,
                reopen_stats: true,
            }]),
        );
        args.run.dataset = directory.path().join("fixture.json");
        fs::write(&args.run.dataset, serde_json::to_vec(&fixture).unwrap()).unwrap();
        let mut config = read_config(&args.run.config).unwrap();
        config.retrieval.mode = RetrievalMode::VectorOnly;
        config.retrieval.surface_policy.object_types =
            vec![ObjectType::Episode, ObjectType::Observation];
        fs::write(&args.run.config, toml::to_string(&config).unwrap()).unwrap();
        let trace_path = args.run.out.clone();
        let report_path = sibling_output(&args.run.out, "report.json").clone();

        run_continuity(args).await.unwrap();

        let traces = read_traces(&trace_path);
        assert_eq!(traces.len(), 1);
        let outcomes = &traces[0].result.retrieval_outcomes;
        assert_eq!(
            outcomes
                .iter()
                .map(|outcome| outcome.rationale.telemetry.configured_object_types.clone())
                .collect::<Vec<_>>(),
            vec![vec![ObjectType::Episode], vec![ObjectType::Observation]]
        );
        let native = outcomes
            .iter()
            .map(|outcome| outcome.trace.as_ref().unwrap())
            .collect::<Vec<_>>();
        assert!(
            native
                .iter()
                .all(|trace| !trace.fanout_utilization.is_empty())
        );
        let fanout = native
            .iter()
            .flat_map(|trace| &trace.fanout_utilization)
            .cloned()
            .collect::<Vec<_>>();
        let decisions = native
            .iter()
            .flat_map(|trace| &trace.selectivity_decisions)
            .cloned()
            .collect::<Vec<_>>();
        let scored = decisions
            .iter()
            .filter(|decision| decision.score.is_some())
            .count();
        let fallback = decisions
            .iter()
            .filter(|decision| decision.fallback)
            .count();
        let report = cmem_eval_continuity::read_continuity_report(&report_path).unwrap();
        let restart = &traces[0].restart_observations[0];
        assert_eq!(restart.probe_query_id, traces[0].result.question_id);
        for snapshot in [&restart.before_restart, &restart.after_restart] {
            assert_eq!(
                snapshot.graph_relation_count,
                Some(native.iter().map(|trace| trace.graph_relations.len()).sum())
            );
            assert_eq!(
                snapshot.graph_verified_count,
                Some(
                    outcomes
                        .iter()
                        .map(|outcome| outcome.rationale.graph_verified_count)
                        .sum()
                )
            );
            assert_eq!(snapshot.fanout_decision_count, Some(fanout.len()));
            assert_eq!(snapshot.selectivity_decision_count, Some(decisions.len()));
            assert_eq!(snapshot.scored_selectivity_count, Some(scored));
            assert_eq!(snapshot.fallback_selectivity_count, Some(fallback));
        }
        let observed = &report.tuning_observations[0].observed;
        assert_eq!(observed["root_counter_query_count"], 1);
        assert_eq!(
            observed["unique_graph_root_candidate_count"],
            outcomes
                .iter()
                .map(|outcome| outcome
                    .rationale
                    .telemetry
                    .unique_graph_root_candidate_count)
                .sum::<usize>()
        );
        assert_eq!(
            observed["selected_graph_root_count"],
            outcomes
                .iter()
                .map(|outcome| outcome.rationale.telemetry.selected_graph_root_count)
                .sum::<usize>()
        );
        assert_eq!(
            observed["graph_root_omission_count"],
            outcomes
                .iter()
                .map(|outcome| outcome.rationale.telemetry.graph_root_omission_count)
                .sum::<usize>()
        );
        assert_eq!(observed["selectivity_decision_count"], decisions.len());
        assert_eq!(observed["scored_selectivity_count"], scored);
        assert_eq!(observed["fallback_selectivity_count"], fallback);
    }

    #[tokio::test]
    async fn continuity_command_runs_scripted_scenarios_and_writes_full_traces() {
        let directory = tempfile::tempdir().unwrap();
        let args = continuity_args(directory.path());
        let artifact = args.run.out.clone();
        let input = fs::read_to_string(&args.run.dataset).unwrap();
        let fixture = parse_fixture_bytes(input.as_bytes()).unwrap();
        let config_source = fs::read_to_string(&args.run.config).unwrap();
        let config = read_config(&args.run.config).unwrap();
        run_continuity(args).await.unwrap();

        let rows = read_rows(&artifact);
        let traces = read_traces(&artifact);
        let report =
            cmem_eval_continuity::read_continuity_report(&sibling_output(&artifact, "report.json"))
                .unwrap();
        let header = read_header(&artifact);
        let expected_queries = fixture
            .scenarios
            .iter()
            .flat_map(|scenario| &scenario.events)
            .filter_map(|event| match event {
                InteractionEvent::Query { query_id, .. } => Some(query_id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(rows.len(), expected_queries.len());
        assert_eq!(traces.len(), rows.len());
        assert_eq!(
            rows.iter()
                .map(|row| &row.question_id)
                .collect::<BTreeSet<_>>(),
            expected_queries
        );
        assert_eq!(report.aggregate.query_count, rows.len());
        assert_eq!(
            report.aggregate.restart_count,
            traces
                .iter()
                .map(|trace| trace.restart_observations.len())
                .sum::<usize>()
        );
        for key in [
            "temporal_recall_fraction@5",
            "supersession_replacement_recall",
        ] {
            let numeric_rows = rows
                .iter()
                .filter(|row| row.metrics.to_json_map()[key].is_number())
                .count();
            assert!(numeric_rows > 0);
            assert_eq!(
                report.aggregate.metric_support[key].numeric_rows,
                numeric_rows
            );
        }
        assert!(
            report
                .aggregate
                .registry_coverage
                .missing_required_metrics
                .is_empty()
        );
        let scenario_ids = fixture
            .scenarios
            .iter()
            .map(|scenario| &scenario.fixture_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            header.embedding_bindings.keys().collect::<BTreeSet<_>>(),
            scenario_ids
        );
        assert_eq!(
            report.scenarios.keys().collect::<BTreeSet<_>>(),
            scenario_ids
        );
        assert_eq!(header.run_id, config.run_id);
        assert_eq!(header.dataset, config.dataset);
        assert_eq!(header.dataset_kind, DatasetKind::Continuity);
        assert_eq!(header.input_sha256, cmem_eval::text_sha256(&input));
        assert_eq!(header.config, config_source);
        assert_eq!(header.config_sha256, cmem_eval::text_sha256(&config_source));
        assert!(report.scenarios.values().all(|scenario| {
            scenario.query_count > 0
                && scenario
                    .registry_coverage
                    .missing_required_metrics
                    .is_empty()
        }));
        assert!(
            report
                .tuning_observations
                .iter()
                .any(|observation| observation.id == "entity_root_candidate_limit")
        );
        let probe = traces
            .iter()
            .find(|trace| !trace.restart_observations.is_empty())
            .unwrap();
        let restart = &probe.restart_observations[0];
        assert_eq!(probe.fixture_id, "cross-store-stress");
        assert_eq!(restart.probe_query_id, probe.result.question_id);
        assert!(restart.delta.stable_returned_objects);
        assert!(!probe.result.retrieved.is_empty());
        assert!(traces.iter().all(|trace| {
            !trace.history_text.is_empty()
                && trace
                    .result
                    .retrieved
                    .iter()
                    .all(|item| !item.rationale.is_empty())
        }));
        for (row, trace) in rows.iter().zip(&traces) {
            assert_eq!(row, &trace.result);
            assert_eq!(row.run_id, header.run_id);
        }
        assert!(
            serde_json::to_value(&report)
                .unwrap()
                .get("header")
                .is_none()
        );
        assert!(!directory.path().join("summary.json").exists());
        assert!(!directory.path().join("stores").exists());
    }

    #[tokio::test]
    async fn continuity_lifecycle_retry_reaches_merged_trace_and_report() {
        let directory = tempfile::tempdir().unwrap();
        let mut args = continuity_args(directory.path());
        args.scenario = Some("correction-chains".to_string());
        let result_path = args.run.out.clone();
        let trace_path = args.run.out.clone();
        let config_path = args.run.config.clone();
        let dataset_path = args.run.dataset.clone();

        run_continuity(args).await.unwrap();

        let mut traces = read_traces(&trace_path);
        let original_rows = read_rows(&result_path);
        assert_eq!(traces.len(), 1);
        assert_eq!(original_rows.len(), 1);
        let converged_retry = {
            let lifecycle = traces[0]
                .result
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
        traces[0].result.lifecycle_outcomes.push(converged_retry);

        let fixture = parse_fixture_bytes(&fs::read(dataset_path).unwrap()).unwrap();
        let scenarios =
            select_continuity_scenarios(fixture.scenarios, Some("correction-chains")).unwrap();
        let config = read_config(&config_path).unwrap();
        let metric_family = continuity_metric_family(&config.metrics, &scenarios);
        let rows = vec![traces[0].result.clone()];
        let summary = summarize_rows(&rows, std::slice::from_ref(&metric_family)).unwrap();
        assert!(summary.degradation.any_degradation);

        let report = assemble_continuity_report(ContinuityReportInput {
            config: serde_json::to_value(&config).unwrap(),
            traces: &traces,
            outcomes: &traces
                .iter()
                .map(|trace| (trace.fixture_id.clone(), ScenarioOutcome::executed()))
                .collect(),
            metric_family: &metric_family,
        })
        .unwrap();
        assert!(report.aggregate.degradation.any_degradation);
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
    fn frozen_preflight_accepts_matching_width_and_descriptive_provenance() {
        let fixture =
            cmem_eval_continuity::generate_fixture_set(cmem_eval_continuity::CHECKED_FIXTURE_SEED)
                .unwrap();
        let mut scenario = fixture.scenarios[0].clone();
        scenario.embedding = cmem_eval_continuity::ContinuityScenarioEmbedding::frozen();
        let directory = tempfile::tempdir().unwrap();
        let store_path = directory.path().join("nonstandard-openai-store.json");
        let store = cmem_eval::FrozenEmbeddingStore::new(
            "text-embedding-3-large",
            "test_fixture",
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

        let providers = validate_continuity_embedding_sizes(&config, &[scenario.clone()]).unwrap();
        assert_eq!(providers[&store_path].vector_size(), 1_024);
        assert_eq!(providers[&store_path].source(), "test_fixture");
        config.backend.embedding.vector_size = Some(512);
        assert!(
            validate_continuity_embedding_sizes(&config, &[scenario])
                .unwrap_err()
                .to_string()
                .contains("vector_size")
        );
    }

    #[test]
    fn committed_benchmark_store_covers_runtime_lookups() {
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

        assert!(!runtime_texts.is_empty());
        assert!(runtime_texts.is_subset(&manifest_texts));
        assert!(runtime_texts.is_subset(&store_texts));
    }

    #[test]
    fn committed_canonical_store_covers_frozen_runtime_lookups() {
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
        assert!(runtime_texts.is_subset(&manifest_texts));
        assert!(runtime_texts.is_subset(&store_texts));
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
                (
                    EmbeddingRuntimeBinding::Frozen { store, .. },
                    EmbeddingBindingRecord::Frozen {
                        store_sha256,
                        source,
                        ..
                    },
                ) => {
                    assert_eq!(store_sha256, store.store_sha256().unwrap());
                    assert_eq!(source, store.source());
                    saw_frozen = true;
                }
                pair => panic!("scenario resolved mismatched runtime/record pair: {pair:?}"),
            }
        }

        assert!(saw_controllable);
        assert!(saw_frozen);
    }

    fn derived_test_dir() -> tempfile::TempDir {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker/bdc-task1");
        fs::create_dir_all(&root).unwrap();
        tempfile::tempdir_in(root).unwrap()
    }
    fn derived_locomo_fixture() -> Value {
        serde_json::json!([{
            "sample_id": "p1",
            "conversation": {"session_1": [{"dia_id": "d1", "text": "jasmine tea"}]},
            "session_summary": {"session_1_summary": "UNIQUE_SUMMARY_TOKEN"},
            "observation": {"session_1_observation": {"A": [["UNIQUE_CLAIM_TOKEN", "d1,missing"], null]}},
            "qa": [{"question": "UNIQUE_SUMMARY_TOKEN", "evidence": ["d1"]}]
        }])
    }

    #[test]
    fn locomo_merges_dataset_memories_with_snapshot_and_configured_graph() {
        use enrichment::EnrichmentError;
        let item = cmem_eval_locomo::load_value(derived_locomo_fixture())
            .unwrap()
            .remove(0);
        let mut config: BenchmarkRunConfig =
            toml::from_str(include_str!("../../../configs/locomo_retrieval.toml")).unwrap();
        let memories = LoCoMoSpec::memory_inputs(&item, &config).derived_memories;
        let mut external = memories[0].clone();
        external.external_id = "external-memory".into();
        external.text = "Configured graph content".into();
        let graph = GraphEnrichmentInput {
            namespace: item.namespace(),
            derived_memories: vec![external.clone()],
            threads: vec![serde_json::from_value(serde_json::json!({
                "external_id": "external-thread", "title": "Thread", "summary": "Configured thread"
            })).unwrap()],
            ..Default::default()
        };
        let snapshot = GraphSnapshotInput {
            snapshot_id: "snapshot".into(),
            namespace: item.namespace(),
            dataset_item_id: item.sample_id.clone(),
            cutoff: cmem_eval::SnapshotCutoff {
                cutoff_type: "final_session".into(),
                value: "session_1".into(),
            },
            graph: graph.clone(),
        };
        let dir = derived_test_dir();
        let path = dir.path().join("graph.jsonl");
        fs::write(&path, serde_json::to_vec(&graph).unwrap()).unwrap();
        for use_snapshot in [true, false] {
            let (configured, snapshots) = if use_snapshot {
                (
                    HashMap::new(),
                    HashMap::from([(item.sample_id.clone(), snapshot.clone())]),
                )
            } else {
                config.ingest.enrichment_snapshot_path = None;
                config.ingest.enrichment_path = Some(path.display().to_string());
                (
                    load_enrichment_by_namespace(&config).unwrap(),
                    HashMap::new(),
                )
            };
            // This is the same enrichment value passed directly to remember_enrichment.
            let merged = LoCoMoSpec::enrichment(
                &item,
                &item.namespace(),
                memories.clone(),
                &config,
                &configured,
                &snapshots,
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                serde_json::to_value(&merged.derived_memories).unwrap(),
                serde_json::to_value(
                    memories
                        .iter()
                        .cloned()
                        .chain([external.clone()])
                        .collect::<Vec<_>>()
                )
                .unwrap()
            );
            assert_eq!(
                serde_json::to_value(&merged.threads).unwrap(),
                serde_json::to_value(&graph.threads).unwrap()
            );
            let mut duplicate = graph.clone();
            duplicate.derived_memories = vec![memories[0].clone()];
            let (configured, snapshots) = if use_snapshot {
                (
                    HashMap::new(),
                    HashMap::from([(
                        item.sample_id.clone(),
                        GraphSnapshotInput {
                            graph: duplicate,
                            ..snapshot.clone()
                        },
                    )]),
                )
            } else {
                (
                    HashMap::from([(item.namespace(), duplicate)]),
                    HashMap::new(),
                )
            };
            let error = LoCoMoSpec::enrichment(
                &item,
                &item.namespace(),
                memories.clone(),
                &config,
                &configured,
                &snapshots,
            )
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<EnrichmentError>(),
                Some(&EnrichmentError::DuplicateExternalId {
                    kind: "derived_memory",
                    external_id: memories[0].external_id.clone(),
                })
            );
        }
    }

    #[tokio::test]
    async fn locomo_baseline_projection_matches_written_header_and_excludes_derived_text() {
        use cmem_eval::RetrievalMode;
        let dataset_value = derived_locomo_fixture();
        let item = cmem_eval_locomo::load_value(dataset_value.clone())
            .unwrap()
            .remove(0);
        for mode in [RetrievalMode::Bm25Only, RetrievalMode::VectorOnly] {
            let mut config: BenchmarkRunConfig =
                toml::from_str(include_str!("../../../configs/locomo_bm25.toml")).unwrap();
            config.retrieval.mode = mode;
            config.ingest.index_session_summaries = true;
            config.ingest.index_generated_observations = true;
            let batch = LoCoMoSpec::memory_inputs(&item, &config);
            assert!(batch.derived_memories.is_empty());
            assert_eq!(
                batch.episodes[0].summary,
                "Conversation session session_1 containing messages between ."
            );
            assert!(
                batch
                    .observations
                    .iter()
                    .all(|observation| !observation.text.contains("UNIQUE_"))
            );
            let detail = LoCoMoSpec::ingest_progress_detail(&batch);
            assert!(detail.contains("unresolved_evidence_references=1"));
            assert!(detail.contains("dropped_observation_entries=1"));
            if mode == RetrievalMode::VectorOnly {
                assert!(
                    LoCoMoSpec::enrichment(
                        &item,
                        &item.namespace(),
                        cmem_eval_locomo::ingest::to_memory_inputs(&item, false, true, true)
                            .derived_memories,
                        &config,
                        &HashMap::new(),
                        &HashMap::new()
                    )
                    .unwrap()
                    .is_none()
                );
            }
            config.ingest.index_session_summaries = false;
            config.ingest.index_generated_observations = false;
            config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
            isolate_test_config(&mut config);
            let directory = derived_test_dir();
            let dataset = directory.path().join("data.json");
            let config_path = directory.path().join("config.toml");
            fs::write(&dataset, serde_json::to_vec(&dataset_value).unwrap()).unwrap();
            fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
            let args = run_args(dataset, config_path, directory.path());
            let output = args.out.clone();
            run_locomo(args).await.unwrap();
            let written: BenchmarkRunConfig = toml::from_str(&read_header(&output).config).unwrap();
            assert_eq!(written.retrieval.mode, mode);
            let projection = LoCoMoSpec::memory_inputs(&item, &written);
            assert_eq!(
                serde_json::to_value(&projection.episodes).unwrap(),
                serde_json::to_value(&batch.episodes).unwrap()
            );
            for row in read_rows(&output) {
                assert!(
                    row.retrieved
                        .iter()
                        .all(|item| item.kind != cmem_eval::ObjectType::DerivedMemory)
                );
                assert!(row.retrieved.iter().all(|item| {
                    item.text
                        .as_deref()
                        .is_none_or(|text| !text.contains("UNIQUE_"))
                }));
            }
        }
        let config: BenchmarkRunConfig =
            toml::from_str(include_str!("../../../configs/locomo_retrieval.toml")).unwrap();
        let hybrid = LoCoMoSpec::memory_inputs(&item, &config);
        assert_eq!(hybrid.derived_memories.len(), 2);
        assert_eq!(hybrid.episodes[0].summary, "UNIQUE_SUMMARY_TOKEN");
        assert!(
            LoCoMoSpec::ingest_progress_detail(&hybrid)
                .contains("unresolved_evidence_references=1")
        );
    }

    #[test]
    fn locomo_config_matrix_keeps_hybrid_content_and_baseline_contracts() {
        for (source, derived) in [
            (include_str!("../../../configs/locomo_retrieval.toml"), true),
            (
                include_str!("../../../configs/locomo_crossmode_embedded.toml"),
                true,
            ),
            (
                include_str!("../../../configs/locomo_crossmode_service.toml"),
                true,
            ),
            (
                include_str!("../../../configs/locomo_crossmode_service_repeat.toml"),
                true,
            ),
            (include_str!("../../../configs/locomo_bm25.toml"), false),
            (include_str!("../../../configs/locomo_vector.toml"), false),
        ] {
            let config: BenchmarkRunConfig = toml::from_str(source).unwrap();
            LoCoMoSpec::validate_config(&config).unwrap();
            assert_eq!(config.ingest.index_session_summaries, derived);
            assert_eq!(config.ingest.index_generated_observations, derived);
        }
    }

    #[tokio::test]
    async fn locomo_snapshot_admission_precedes_run_root_and_adapter_creation() {
        snapshot_admission_precedes_run_root_and_adapter_creation(true).await;
    }

    #[tokio::test]
    async fn longmemeval_snapshot_admission_precedes_run_root_and_adapter_creation() {
        snapshot_admission_precedes_run_root_and_adapter_creation(false).await;
    }

    async fn snapshot_admission_precedes_run_root_and_adapter_creation(locomo: bool) {
        let dataset = if locomo {
            derived_locomo_fixture()
        } else {
            serde_json::json!([{"question_id": "q1", "question": "tea?", "haystack_session_ids": ["s1"],
                "haystack_sessions": [[{"content": "tea"}]], "answer_session_ids": ["s1"]}])
        };
        let directory = derived_test_dir();
        let snapshot_path = directory.path().join("snapshot.jsonl");
        let manifest_path = directory.path().join("snapshot_manifest.json");
        fs::write(&snapshot_path, "{}").unwrap();
        let mut config: BenchmarkRunConfig = toml::from_str(if locomo {
            include_str!("../../../configs/locomo_retrieval.toml")
        } else {
            include_str!("../../../configs/longmemeval_s_retrieval.toml")
        })
        .unwrap();
        config.ingest.enrichment_snapshot_path = Some(snapshot_path.display().to_string());
        config.backend.retain_stores = true;
        config.backend.retain_reason = Some("prove admission precedes store creation".into());
        config.backend.embedding.provider = EmbeddingProviderConfig::Deterministic;
        isolate_test_config(&mut config);
        let config_path = directory.path().join("config.toml");
        let dataset_path = directory.path().join("dataset.json");
        fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
        fs::write(&dataset_path, serde_json::to_vec(&dataset).unwrap()).unwrap();
        let output_dir = directory.path().join("run");
        let args = run_args(dataset_path, config_path, &output_dir);
        let error = if locomo {
            run_locomo(args).await
        } else {
            run_longmemeval(args).await
        }
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<enrichment::EnrichmentError>(),
            Some(&enrichment::EnrichmentError::MissingManifest {
                path: manifest_path
            })
        );
        assert!(!output_dir.exists(), "snapshot admission created run state");
    }
}
