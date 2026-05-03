use crate::{enrichment, official_exports};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use cmem_eval_core::{
    BenchmarkRunConfig, EpisodeInput, MemoryAdapter, MockMemoryAdapter, ObservationInput,
    PerQuestionResult, ReaderResult, ResultContextMetrics, RetrievalMode, RetrieveInput,
    RunAdapterMetadata, Timer, composition_metrics, count_tokens, estimate_word_count,
    initialize_registry_metrics, insert_composition_metrics, insert_context_metrics,
    insert_integrity_detail_metrics, insert_retrieval_metrics, insert_telemetry_metrics,
    integrity_details_with_telemetry, read_jsonl, summarize_rows, write_jsonl, write_summary,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[cfg(feature = "real-character-memory")]
#[path = "real_adapter.rs"]
mod real_adapter;

#[derive(Debug, Parser)]
#[command(name = "cmem-eval")]
#[command(about = "Run Character Memory retrieval benchmarks")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Run(run) => run.run().await,
            Command::ExportOfficial(args) => export_official(args),
            Command::Summarize(args) => summarize(args),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunCommand),
    ExportOfficial(ExportOfficialCommand),
    Summarize(SummarizeArgs),
}

#[derive(Debug, Args)]
struct RunCommand {
    #[command(subcommand)]
    dataset: RunDataset,
}

impl RunCommand {
    async fn run(self) -> Result<()> {
        match self.dataset {
            RunDataset::Synthetic(args) => run_synthetic(args).await,
            RunDataset::LongmemevalS(args) => run_longmemeval(args).await,
            RunDataset::Locomo(args) => run_locomo(args).await,
        }
    }
}

#[derive(Debug, Subcommand)]
enum RunDataset {
    LongmemevalS(RunArgs),
    Locomo(RunArgs),
    Synthetic(RunArgs),
}

#[derive(Debug, Args)]
struct ExportOfficialCommand {
    #[command(subcommand)]
    dataset: ExportOfficialDataset,
}

#[derive(Debug, Subcommand)]
enum ExportOfficialDataset {
    Longmemeval(LongMemEvalExportCommand),
    Locomo(LoCoMoExportArgs),
}

#[derive(Debug, Args)]
struct LongMemEvalExportCommand {
    #[command(subcommand)]
    export: LongMemEvalExportKind,
}

#[derive(Debug, Subcommand)]
enum LongMemEvalExportKind {
    Retrieval(OfficialExportArgs),
    Qa(QaExportArgs),
}

#[derive(Debug, Args)]
struct OfficialExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct QaExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    predictions: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct LoCoMoExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    predictions: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
struct RunArgs {
    #[arg(long)]
    dataset: PathBuf,
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long = "summary-out")]
    summary_out: PathBuf,
    #[arg(long, value_enum)]
    adapter: Option<AdapterKind>,
    #[arg(long)]
    allow_mock_benchmark: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AdapterKind {
    Mock,
    Real,
}

#[derive(Debug, Args)]
struct SummarizeArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

async fn run_synthetic(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    config.validate()?;
    let fixture: SyntheticFixture = read_json(&args.dataset)?;
    let selected = args.selected_adapter();
    args.validate_adapter_selection(&config)?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let mut rows = Vec::new();
    let mut namespaces_to_cleanup = Vec::new();
    let progress = RunProgress::new(&config.dataset, fixture.questions.len(), None);

    for (question_idx, question) in fixture.questions.into_iter().enumerate() {
        let namespace = format!("synthetic:{}", question.question_id);
        let timer = Timer::start();
        let question_label = question.question_id.clone();
        progress.item_started(question_idx + 1, &question_label);
        adapter.reset_namespace(&namespace).await?;
        let mut episodes = Vec::with_capacity(question.sessions.len());
        let mut observations = Vec::new();
        for session in &question.sessions {
            episodes.push(EpisodeInput {
                external_id: session.session_id.clone(),
                namespace: namespace.clone(),
                summary: episode_summary(&session.session_id, &session.turns),
                started_at: session.date.clone(),
                ended_at: session.date.clone(),
                participants: participants(&session.turns),
                metadata: serde_json::json!({"source": "synthetic"}),
            });
            for (idx, turn) in session.turns.iter().enumerate() {
                observations.push(ObservationInput {
                    external_id: format!("{}:turn:{}", session.session_id, idx + 1),
                    episode_external_id: session.session_id.clone(),
                    namespace: namespace.clone(),
                    speaker: turn.role.clone(),
                    text: turn.content.clone(),
                    observed_at: session.date.clone(),
                    metadata: serde_json::json!({"source": "synthetic"}),
                });
            }
        }
        adapter.remember_episodes(episodes).await?;
        adapter.remember_observations(observations).await?;
        progress.phase_done(
            question_idx + 1,
            &question_label,
            "ingest",
            &format!(
                "sessions={} turns={}",
                question.sessions.len(),
                question
                    .sessions
                    .iter()
                    .map(|session| session.turns.len())
                    .sum::<usize>()
            ),
        );
        let pack = adapter
            .retrieve(RetrieveInput {
                mode: config.retrieval.mode,
                namespace: namespace.clone(),
                query: question.question.clone(),
                query_date: None,
                top_k_episodes: config.retrieval.top_k_episodes,
                top_k_observations: config.retrieval.top_k_observations,
                include_derived_memories: config.retrieval.include_derived_memories,
                include_threads: config.retrieval.include_threads,
                include_entities: config.retrieval.include_entities,
                include_debug_rationale: config.retrieval.include_debug_rationale,
            })
            .await?;
        progress.phase_done(
            question_idx + 1,
            &question_label,
            "retrieve",
            &format!("items={}", pack.items.len()),
        );
        let full_history = synthetic_full_history_text(&question.sessions);
        let context = context_metrics(&pack, Some(&full_history));
        let composition = composition_metrics(&pack.items);
        let integrity = integrity_details_with_telemetry(&pack.items, &pack.telemetry);
        let latency_ms = timer.elapsed_ms();
        let mut metrics = score_basic(
            &pack.items,
            &question.gold_episode_ids,
            &question.gold_observation_ids,
        );
        insert_common_metrics(
            &mut metrics,
            &context,
            &composition,
            &integrity,
            &pack.telemetry,
            latency_ms,
        );
        rows.push(PerQuestionResult {
            run_id: config.run_id.clone(),
            dataset: config.dataset.clone(),
            adapter: adapter_metadata.clone(),
            question_id: question.question_id,
            question_type: None,
            question: question.question,
            gold_episode_ids: question.gold_episode_ids,
            gold_observation_ids: question.gold_observation_ids,
            retrieved: pack.items,
            metrics: Value::Object(metrics),
            latency_ms,
            context_char_count: context.retrieved_context_chars,
            context_word_count: context.retrieved_context_words,
            context,
            telemetry: pack.telemetry,
            composition,
            integrity,
            reader: ReaderResult::default(),
        });
        namespaces_to_cleanup.push(namespace);
        progress.item_finished(question_idx + 1, &question_label, timer.elapsed_ms());
    }
    progress.write_outputs_started(rows.len());
    write_outputs(args, config.clone(), rows)?;
    progress.cleanup_started(namespaces_to_cleanup.len());
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
}

async fn run_longmemeval(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    config.validate()?;
    let instances = cmem_eval_longmemeval::load_path(&args.dataset)?;
    let selected = args.selected_adapter();
    args.validate_adapter_selection(&config)?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let enrichment_by_namespace = load_enrichment_by_namespace(&config)?;
    let mut rows = Vec::new();
    let mut namespaces_to_cleanup = Vec::new();
    let progress = RunProgress::new(&config.dataset, instances.len(), None);
    for (instance_idx, instance) in instances.into_iter().enumerate() {
        let namespace = instance.namespace();
        let timer = Timer::start();
        let question_label = instance.question_id.clone();
        progress.item_started(instance_idx + 1, &question_label);
        adapter.reset_namespace(&namespace).await?;
        let mapped = cmem_eval_longmemeval::ingest::to_memory_inputs(&instance);
        let episode_count = mapped.episodes.len();
        let observation_count = mapped.observations.len();
        adapter.remember_episodes(mapped.episodes).await?;
        progress.phase_done(
            instance_idx + 1,
            &question_label,
            "ingest-episodes",
            &format!("count={episode_count}"),
        );
        adapter.remember_observations(mapped.observations).await?;
        progress.phase_done(
            instance_idx + 1,
            &question_label,
            "ingest-observations",
            &format!("count={observation_count}"),
        );
        progress.phase_done(
            instance_idx + 1,
            &question_label,
            "ingest",
            &format!("episodes={episode_count} observations={observation_count}"),
        );
        remember_configured_enrichment(&*adapter, &enrichment_by_namespace, &namespace).await?;
        progress.phase_done(instance_idx + 1, &question_label, "enrichment", "done");
        let pack = adapter
            .retrieve(RetrieveInput {
                mode: config.retrieval.mode,
                namespace: namespace.clone(),
                query: instance.question.clone(),
                query_date: instance.question_date.clone(),
                top_k_episodes: config.retrieval.top_k_episodes,
                top_k_observations: config.retrieval.top_k_observations,
                include_derived_memories: config.retrieval.include_derived_memories,
                include_threads: config.retrieval.include_threads,
                include_entities: config.retrieval.include_entities,
                include_debug_rationale: config.retrieval.include_debug_rationale,
            })
            .await?;
        progress.phase_done(
            instance_idx + 1,
            &question_label,
            "retrieve",
            &format!("items={}", pack.items.len()),
        );
        let full_history = longmemeval_full_history_text(&instance);
        let context = context_metrics(&pack, Some(&full_history));
        let composition = composition_metrics(&pack.items);
        let integrity = integrity_details_with_telemetry(&pack.items, &pack.telemetry);
        let latency_ms = timer.elapsed_ms();
        let mut metrics = cmem_eval_longmemeval::scoring::score(
            &instance,
            &pack.items,
            &config.metrics.ks_session,
            &config.metrics.ks_turn,
        );
        if let Some(metrics) = metrics.as_object_mut() {
            insert_common_metrics(
                metrics,
                &context,
                &composition,
                &integrity,
                &pack.telemetry,
                latency_ms,
            );
        }
        let gold_observation_ids = instance.gold_turn_ids();
        rows.push(PerQuestionResult {
            run_id: config.run_id.clone(),
            dataset: config.dataset.clone(),
            adapter: adapter_metadata.clone(),
            question_id: instance.question_id,
            question_type: instance.question_type,
            question: instance.question,
            gold_episode_ids: instance.answer_session_ids,
            gold_observation_ids,
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
        namespaces_to_cleanup.push(namespace);
        progress.item_finished(instance_idx + 1, &question_label, timer.elapsed_ms());
    }
    progress.write_outputs_started(rows.len());
    write_outputs(args, config.clone(), rows)?;
    progress.cleanup_started(namespaces_to_cleanup.len());
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
}

async fn run_locomo(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    config.validate()?;
    let samples = cmem_eval_locomo::load_path(&args.dataset)?;
    let selected = args.selected_adapter();
    args.validate_adapter_selection(&config)?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let enrichment_by_namespace = load_enrichment_by_namespace(&config)?;
    let mut rows = Vec::new();
    let mut namespaces_to_cleanup = Vec::new();
    let total_qa = samples.iter().map(|sample| sample.qa.len()).sum::<usize>();
    let progress = RunProgress::new(&config.dataset, samples.len(), Some(total_qa));
    let mut completed_qa = 0usize;
    for (sample_idx, sample) in samples.into_iter().enumerate() {
        let namespace = sample.namespace();
        let sample_label = sample.sample_id.clone();
        let sample_timer = Timer::start();
        progress.item_started(sample_idx + 1, &sample_label);
        let mapped = cmem_eval_locomo::ingest::to_memory_inputs(
            &sample,
            config.ingest.include_image_captions,
            config.ingest.index_session_summaries,
            config.ingest.index_generated_observations,
        );
        adapter.reset_namespace(&namespace).await?;
        let episode_count = mapped.episodes.len();
        let observation_count = mapped.observations.len();
        let derived_count = mapped.derived_memories.len();
        adapter.remember_episodes(mapped.episodes).await?;
        progress.phase_done(
            sample_idx + 1,
            &sample_label,
            "ingest-episodes",
            &format!("count={episode_count}"),
        );
        adapter.remember_observations(mapped.observations).await?;
        progress.phase_done(
            sample_idx + 1,
            &sample_label,
            "ingest-observations",
            &format!("count={observation_count}"),
        );
        progress.phase_done(
            sample_idx + 1,
            &sample_label,
            "ingest",
            &format!(
                "episodes={episode_count} observations={observation_count} generated_derived={derived_count}"
            ),
        );
        let mut namespace_enrichment = enrichment::empty_namespace(namespace.clone());
        namespace_enrichment.derived_memories = mapped.derived_memories;
        if let Some(configured) = enrichment_by_namespace.get(&namespace).cloned() {
            enrichment::merge_enrichment(&mut namespace_enrichment, configured)?;
        } else {
            enrichment::validate_enrichment(&namespace_enrichment)?;
        }
        adapter.remember_enrichment(namespace_enrichment).await?;
        progress.phase_done(sample_idx + 1, &sample_label, "enrichment", "done");
        let full_history = locomo_full_history_text(&sample);
        let full_history_metrics = full_history_context_metrics(Some(&full_history));
        let evidence_session_index = sample.evidence_session_index();
        for (qa_idx, qa) in sample.qa.iter().enumerate() {
            let timer = Timer::start();
            progress.qa_started(sample_idx + 1, &sample_label, qa_idx + 1, sample.qa.len());
            let pack = adapter
                .retrieve(RetrieveInput {
                    mode: config.retrieval.mode,
                    namespace: namespace.clone(),
                    query: qa.question.clone(),
                    query_date: None,
                    top_k_episodes: config.retrieval.top_k_episodes,
                    top_k_observations: config.retrieval.top_k_observations,
                    include_derived_memories: config.retrieval.include_derived_memories,
                    include_threads: config.retrieval.include_threads,
                    include_entities: config.retrieval.include_entities,
                    include_debug_rationale: config.retrieval.include_debug_rationale,
                })
                .await?;
            progress.qa_retrieved(
                sample_idx + 1,
                &sample_label,
                qa_idx + 1,
                sample.qa.len(),
                pack.items.len(),
            );
            let context = context_metrics_with_full_history(&pack, full_history_metrics);
            let composition = composition_metrics(&pack.items);
            let integrity = integrity_details_with_telemetry(&pack.items, &pack.telemetry);
            let latency_ms = timer.elapsed_ms();
            let gold_episode_ids = evidence_session_index.evidence_sessions(qa);
            let mut metrics = cmem_eval_locomo::scoring::score_with_gold_sessions(
                qa,
                &gold_episode_ids,
                &pack.items,
                &config.metrics.ks_dialog,
                &config.metrics.ks_session,
            );
            if let Some(metrics) = metrics.as_object_mut() {
                insert_common_metrics(
                    metrics,
                    &context,
                    &composition,
                    &integrity,
                    &pack.telemetry,
                    latency_ms,
                );
            }
            rows.push(PerQuestionResult {
                run_id: config.run_id.clone(),
                dataset: config.dataset.clone(),
                adapter: adapter_metadata.clone(),
                question_id: qa.question_id.clone(),
                question_type: qa.question_type.clone(),
                question: qa.question.clone(),
                gold_episode_ids,
                gold_observation_ids: qa.evidence_dialog_ids.clone(),
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
            completed_qa += 1;
            progress.qa_finished(
                sample_idx + 1,
                &sample_label,
                completed_qa,
                timer.elapsed_ms(),
            );
        }
        namespaces_to_cleanup.push(namespace);
        progress.item_finished(sample_idx + 1, &sample_label, sample_timer.elapsed_ms());
    }
    progress.write_outputs_started(rows.len());
    write_outputs(args, config.clone(), rows)?;
    progress.cleanup_started(namespaces_to_cleanup.len());
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
}

fn load_enrichment_by_namespace(
    config: &BenchmarkRunConfig,
) -> Result<std::collections::HashMap<String, cmem_eval_core::GraphEnrichmentInput>> {
    config
        .ingest
        .enrichment_path
        .as_ref()
        .map(|path| enrichment::load_enrichment_path(std::path::Path::new(path)))
        .transpose()
        .map(|value| value.unwrap_or_default())
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

async fn remember_configured_enrichment(
    adapter: &dyn MemoryAdapter,
    enrichment_by_namespace: &std::collections::HashMap<
        String,
        cmem_eval_core::GraphEnrichmentInput,
    >,
    namespace: &str,
) -> Result<()> {
    if let Some(enrichment) = enrichment_by_namespace.get(namespace).cloned() {
        adapter.remember_enrichment(enrichment).await?;
    }
    Ok(())
}

fn insert_common_metrics(
    metrics: &mut Map<String, Value>,
    context: &ResultContextMetrics,
    composition: &cmem_eval_core::ResultCompositionMetrics,
    integrity: &cmem_eval_core::ResultIntegrityDetails,
    telemetry: &cmem_eval_core::RetrievalTelemetry,
    latency_ms: u128,
) {
    initialize_registry_metrics(metrics);
    metrics.insert(
        "retrieval_latency_ms".to_string(),
        Value::from(latency_ms as f64),
    );
    insert_context_metrics(metrics, context);
    insert_composition_metrics(metrics, composition);
    insert_integrity_detail_metrics(metrics, integrity);
    insert_telemetry_metrics(metrics, telemetry);
}

fn context_metrics(
    pack: &cmem_eval_core::RetrievedContextPack,
    full_history_text: Option<&str>,
) -> ResultContextMetrics {
    context_metrics_with_full_history(pack, full_history_context_metrics(full_history_text))
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
    pack: &cmem_eval_core::RetrievedContextPack,
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

fn longmemeval_full_history_text(instance: &cmem_eval_longmemeval::LongMemEvalInstance) -> String {
    instance
        .sessions
        .iter()
        .flat_map(|session| {
            session.turns.iter().map(|turn| {
                format!(
                    "{}: {}",
                    turn.speaker.as_deref().unwrap_or("unknown"),
                    turn.text
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn locomo_full_history_text(sample: &cmem_eval_locomo::LoCoMoSample) -> String {
    sample
        .sessions
        .iter()
        .flat_map(|session| {
            session.turns.iter().map(|turn| {
                format!(
                    "{}: {}",
                    turn.speaker.as_deref().unwrap_or("unknown"),
                    turn.text
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn export_official(args: ExportOfficialCommand) -> Result<()> {
    match args.dataset {
        ExportOfficialDataset::Longmemeval(args) => match args.export {
            LongMemEvalExportKind::Retrieval(args) => {
                let rows = read_jsonl(&args.input)?;
                official_exports::write_longmemeval_retrieval(&args.out, &rows)
            }
            LongMemEvalExportKind::Qa(args) => {
                let rows = read_jsonl(&args.input)?;
                let predictions = official_exports::read_predictions_jsonl(&args.predictions)?;
                official_exports::write_longmemeval_qa(&args.out, &rows, &predictions)
            }
        },
        ExportOfficialDataset::Locomo(args) => {
            let rows = read_jsonl(&args.input)?;
            let predictions = args
                .predictions
                .as_ref()
                .map(|path| official_exports::read_predictions_jsonl(path))
                .transpose()?;
            official_exports::write_locomo(&args.out, &rows, predictions.as_ref())
        }
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

fn summarize(args: SummarizeArgs) -> Result<()> {
    let rows = read_jsonl(&args.input)?;
    let Some(first) = rows.first() else {
        bail!("cannot summarize empty JSONL: {}", args.input.display());
    };
    let summary = summarize_rows(
        first.run_id.clone(),
        first.dataset.clone(),
        first.adapter.clone(),
        serde_json::json!({}),
        &rows,
    );
    write_summary(&args.out, &summary)
}

fn write_outputs(
    args: RunArgs,
    config: BenchmarkRunConfig,
    rows: Vec<PerQuestionResult>,
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
    );
    write_jsonl(&args.out, &rows)?;
    write_summary(&args.summary_out, &summary)
}

async fn adapter(kind: AdapterKind, config: &BenchmarkRunConfig) -> Result<Box<dyn MemoryAdapter>> {
    match kind {
        AdapterKind::Mock => Ok(Box::<MockMemoryAdapter>::default()),
        AdapterKind::Real => real_adapter(config).await,
    }
}

#[cfg(feature = "real-character-memory")]
async fn real_adapter(config: &BenchmarkRunConfig) -> Result<Box<dyn MemoryAdapter>> {
    Ok(Box::new(
        real_adapter::CharacterMemoryAdapter::new(config).await?,
    ))
}

#[cfg(not(feature = "real-character-memory"))]
async fn real_adapter(_config: &BenchmarkRunConfig) -> Result<Box<dyn MemoryAdapter>> {
    bail!(
        "live Character Memory adapter is the default for benchmark runs, but this binary was built without `real-character-memory`; rebuild with `cargo run -p cmem-eval-runner --features real-character-memory -- ...`. Use `--adapter mock --allow-mock-benchmark` only for smoke tests."
    )
}

impl RunArgs {
    fn selected_adapter(&self) -> AdapterKind {
        self.adapter.unwrap_or(AdapterKind::Real)
    }

    fn validate_adapter_selection(&self, config: &BenchmarkRunConfig) -> Result<()> {
        if self.selected_adapter() == AdapterKind::Mock && !self.allow_mock_benchmark {
            bail!(
                "mock adapter is test/smoke-only; pass `--allow-mock-benchmark` to make mock output explicit, or omit `--adapter` for the default live Character Memory run"
            );
        }
        if config.retrieval.mode == RetrievalMode::Bm25Only
            && self.selected_adapter() != AdapterKind::Mock
        {
            bail!(
                "retrieval.mode=bm25_only is service-free and requires `--adapter mock --allow-mock-benchmark`; refusing to create a live adapter"
            );
        }
        if config.retrieval.mode == RetrievalMode::VectorOnly
            && self.selected_adapter() == AdapterKind::Mock
        {
            bail!(
                "retrieval.mode=vector_only is a live Qdrant baseline and cannot run with `--adapter mock`; omit `--adapter` or pass `--adapter real`"
            );
        }
        Ok(())
    }
}

impl AdapterKind {
    fn metadata(self) -> RunAdapterMetadata {
        match self {
            AdapterKind::Mock => RunAdapterMetadata::mock_smoke(),
            AdapterKind::Real => RunAdapterMetadata::live(),
        }
    }
}

fn read_config(path: &PathBuf) -> Result<BenchmarkRunConfig> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: toml::Value = toml::from_str(&content)?;
    let json = serde_json::to_value(value)?;
    Ok(serde_json::from_value(json)?)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(serde_json::from_str(&content)?)
}

fn score_basic(
    items: &[cmem_eval_core::RetrievedItem],
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

    #[tokio::test]
    async fn synthetic_command_writes_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("synthetic.jsonl");
        let summary = dir.path().join("synthetic_summary.json");
        run_synthetic(RunArgs {
            dataset: PathBuf::from("../../fixtures/synthetic_small.json"),
            config: PathBuf::from("../../configs/synthetic_retrieval.toml"),
            out: out.clone(),
            summary_out: summary.clone(),
            adapter: Some(AdapterKind::Mock),
            allow_mock_benchmark: true,
        })
        .await
        .unwrap();
        assert!(out.exists());
        assert!(summary.exists());
    }

    #[test]
    fn omitted_adapter_selects_real() {
        let args = RunArgs {
            dataset: PathBuf::from("dataset.json"),
            config: PathBuf::from("config.toml"),
            out: PathBuf::from("out.jsonl"),
            summary_out: PathBuf::from("summary.json"),
            adapter: None,
            allow_mock_benchmark: false,
        };

        assert_eq!(args.selected_adapter(), AdapterKind::Real);
        args.validate_adapter_selection(&synthetic_config())
            .unwrap();
    }

    #[test]
    fn mock_adapter_requires_explicit_benchmark_guard() {
        let args = RunArgs {
            dataset: PathBuf::from("dataset.json"),
            config: PathBuf::from("config.toml"),
            out: PathBuf::from("out.jsonl"),
            summary_out: PathBuf::from("summary.json"),
            adapter: Some(AdapterKind::Mock),
            allow_mock_benchmark: false,
        };

        let err = args
            .validate_adapter_selection(&synthetic_config())
            .unwrap_err()
            .to_string();
        assert!(err.contains("mock adapter is test/smoke-only"));
    }

    #[test]
    fn guarded_mock_adapter_is_allowed_for_smoke_runs() {
        let args = RunArgs {
            dataset: PathBuf::from("dataset.json"),
            config: PathBuf::from("config.toml"),
            out: PathBuf::from("out.jsonl"),
            summary_out: PathBuf::from("summary.json"),
            adapter: Some(AdapterKind::Mock),
            allow_mock_benchmark: true,
        };

        assert_eq!(args.selected_adapter(), AdapterKind::Mock);
        args.validate_adapter_selection(&synthetic_config())
            .unwrap();
    }

    #[test]
    fn bm25_mode_requires_guarded_mock_adapter_before_live_adapter_creation() {
        let args = RunArgs {
            dataset: PathBuf::from("dataset.json"),
            config: PathBuf::from("config.toml"),
            out: PathBuf::from("out.jsonl"),
            summary_out: PathBuf::from("summary.json"),
            adapter: None,
            allow_mock_benchmark: false,
        };
        let config = synthetic_config_with_mode(RetrievalMode::Bm25Only);

        let err = args
            .validate_adapter_selection(&config)
            .unwrap_err()
            .to_string();
        assert!(err.contains("retrieval.mode=bm25_only"));
        assert!(err.contains("refusing to create a live adapter"));
    }

    #[test]
    fn vector_only_mode_rejects_mock_adapter() {
        let args = RunArgs {
            dataset: PathBuf::from("dataset.json"),
            config: PathBuf::from("config.toml"),
            out: PathBuf::from("out.jsonl"),
            summary_out: PathBuf::from("summary.json"),
            adapter: Some(AdapterKind::Mock),
            allow_mock_benchmark: true,
        };
        let config = synthetic_config_with_mode(RetrievalMode::VectorOnly);

        let err = args
            .validate_adapter_selection(&config)
            .unwrap_err()
            .to_string();
        assert!(err.contains("retrieval.mode=vector_only"));
        assert!(err.contains("cannot run with `--adapter mock`"));
    }

    #[test]
    fn vector_only_mode_allows_default_real_adapter() {
        let args = RunArgs {
            dataset: PathBuf::from("dataset.json"),
            config: PathBuf::from("config.toml"),
            out: PathBuf::from("out.jsonl"),
            summary_out: PathBuf::from("summary.json"),
            adapter: None,
            allow_mock_benchmark: false,
        };
        let config = synthetic_config_with_mode(RetrievalMode::VectorOnly);

        assert_eq!(args.selected_adapter(), AdapterKind::Real);
        args.validate_adapter_selection(&config).unwrap();
    }

    #[test]
    fn checked_in_vector_configs_use_raw_candidate_ingestion_only() {
        for path in [
            "../../configs/synthetic_vector.toml",
            "../../configs/longmemeval_s_vector.toml",
            "../../configs/locomo_vector.toml",
        ] {
            let path = PathBuf::from(path);
            let config = read_config(&path).unwrap_or_else(|err| {
                panic!(
                    "read vector config policy fixture {}: {err}",
                    path.display()
                );
            });

            assert_eq!(
                config.retrieval.mode,
                RetrievalMode::VectorOnly,
                "{} must use vector_only mode",
                path.display()
            );
            assert!(
                !config.retrieval.include_derived_memories,
                "{}",
                path.display()
            );
            assert!(!config.retrieval.include_threads, "{}", path.display());
            assert!(!config.retrieval.include_entities, "{}", path.display());
            assert!(!config.ingest.create_threads, "{}", path.display());
            assert!(!config.ingest.index_session_summaries, "{}", path.display());
            assert!(
                !config.ingest.index_generated_observations,
                "{}",
                path.display()
            );
            assert!(
                config.ingest.enrichment_path.is_none(),
                "{} must not ingest enrichment in vector-only baseline configs",
                path.display()
            );
            config.validate().unwrap_or_else(|err| {
                panic!(
                    "validate vector config policy fixture {}: {err}",
                    path.display()
                );
            });
        }
    }

    #[tokio::test]
    #[cfg(not(feature = "real-character-memory"))]
    async fn feature_disabled_real_adapter_fails_actionably() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic"
        }))
        .unwrap();

        let err = match adapter(AdapterKind::Real, &config).await {
            Ok(_) => panic!("feature-disabled real adapter unexpectedly succeeded"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("real-character-memory"));
        assert!(err.contains("--adapter mock --allow-mock-benchmark"));
    }

    fn synthetic_config() -> BenchmarkRunConfig {
        synthetic_config_with_mode(RetrievalMode::Hybrid)
    }

    fn synthetic_config_with_mode(mode: RetrievalMode) -> BenchmarkRunConfig {
        BenchmarkRunConfig {
            run_id: "r".into(),
            dataset: "synthetic".into(),
            backend: Default::default(),
            retrieval: cmem_eval_core::RetrievalConfig {
                mode,
                ..Default::default()
            },
            ingest: cmem_eval_core::IngestConfig {
                index_observations: true,
                index_episode_summaries: true,
                ..Default::default()
            },
            metrics: Default::default(),
        }
    }
}
