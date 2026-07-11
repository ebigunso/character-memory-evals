use crate::commands::{AdapterKind, RunArgs, read_config};
use crate::enrichment;
use anyhow::{Result, bail};
use cmem_eval_adapter_cmem::CharacterMemoryAdapter;
use cmem_eval_core::{
    BenchmarkRunConfig, EpisodeInput, GraphEnrichmentInput, GraphSnapshotInput, MemoryAdapter,
    MetricFamily, MetricsConfig, MockMemoryAdapter, ObservationInput, PerQuestionResult,
    ReaderResult, ResultContextMetrics, RetrieveInput, RetrievedContextPack, RetrievedItem,
    RunAdapterMetadata, Timer, composition_metrics, count_tokens, estimate_word_count,
    initialize_registry_metrics_for, insert_composition_metrics, insert_context_metrics,
    insert_integrity_detail_metrics, insert_retrieval_metrics, insert_telemetry_metrics,
    integrity_details_with_telemetry, summarize_rows, write_jsonl, write_summary,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

pub(crate) async fn run_synthetic(args: RunArgs) -> Result<()> {
    run_pipeline::<SyntheticSpec>(args).await
}

pub(crate) async fn run_longmemeval(args: RunArgs) -> Result<()> {
    run_pipeline::<LongMemEvalSpec>(args).await
}

pub(crate) async fn run_locomo(args: RunArgs) -> Result<()> {
    run_pipeline::<LoCoMoSpec>(args).await
}

pub(crate) fn metric_family_for_config(config: &BenchmarkRunConfig) -> Result<MetricFamily> {
    match config.dataset.as_str() {
        "synthetic" => {
            SyntheticSpec::validate_config(config)?;
            Ok(SyntheticSpec::metric_family(&config.metrics))
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
            adapter.reset_namespace(namespace).await?;
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
}
