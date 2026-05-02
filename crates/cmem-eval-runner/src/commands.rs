use crate::official_exports;
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use cmem_eval_core::{
    BenchmarkRunConfig, EpisodeInput, MemoryAdapter, MockMemoryAdapter, ObservationInput,
    PerQuestionResult, RetrieveInput, RunAdapterMetadata, Timer, insert_integrity_metrics,
    insert_retrieval_metrics, read_jsonl, summarize_rows, write_jsonl, write_summary,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

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
    args.validate_adapter_selection()?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let mut rows = Vec::new();
    let mut namespaces_to_cleanup = Vec::new();

    for question in fixture.questions {
        let namespace = format!("synthetic:{}", question.question_id);
        let timer = Timer::start();
        adapter.reset_namespace(&namespace).await?;
        for session in &question.sessions {
            adapter
                .remember_episode(EpisodeInput {
                    external_id: session.session_id.clone(),
                    namespace: namespace.clone(),
                    summary: episode_summary(&session.session_id, &session.turns),
                    started_at: session.date.clone(),
                    ended_at: session.date.clone(),
                    participants: participants(&session.turns),
                    metadata: serde_json::json!({"source": "synthetic"}),
                })
                .await?;
            for (idx, turn) in session.turns.iter().enumerate() {
                adapter
                    .remember_observation(ObservationInput {
                        external_id: format!("{}:turn:{}", session.session_id, idx + 1),
                        episode_external_id: session.session_id.clone(),
                        namespace: namespace.clone(),
                        speaker: turn.role.clone(),
                        text: turn.content.clone(),
                        observed_at: session.date.clone(),
                        metadata: serde_json::json!({"source": "synthetic"}),
                    })
                    .await?;
            }
        }
        let pack = adapter
            .retrieve(RetrieveInput {
                namespace: namespace.clone(),
                query: question.question.clone(),
                query_date: None,
                top_k_episodes: config.retrieval.top_k_episodes,
                top_k_observations: config.retrieval.top_k_observations,
                include_derived_memories: config.retrieval.include_derived_memories,
                include_debug_rationale: config.retrieval.include_debug_rationale,
            })
            .await?;
        let mut metrics = score_basic(
            &pack.items,
            &question.gold_episode_ids,
            &question.gold_observation_ids,
        );
        insert_integrity_metrics(&mut metrics, &pack.items);
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
            latency_ms: timer.elapsed_ms(),
            context_char_count: pack.context_char_count,
            context_word_count: pack.context_word_count,
        });
        namespaces_to_cleanup.push(namespace);
    }
    write_outputs(args, config.clone(), rows)?;
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
}

async fn run_longmemeval(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    config.validate()?;
    let instances = cmem_eval_longmemeval::load_path(&args.dataset)?;
    let selected = args.selected_adapter();
    args.validate_adapter_selection()?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let mut rows = Vec::new();
    let mut namespaces_to_cleanup = Vec::new();
    for instance in instances {
        let namespace = instance.namespace();
        let timer = Timer::start();
        adapter.reset_namespace(&namespace).await?;
        let mapped = cmem_eval_longmemeval::ingest::to_memory_inputs(&instance);
        for episode in mapped.episodes {
            adapter.remember_episode(episode).await?;
        }
        for observation in mapped.observations {
            adapter.remember_observation(observation).await?;
        }
        let pack = adapter
            .retrieve(RetrieveInput {
                namespace: namespace.clone(),
                query: instance.question.clone(),
                query_date: instance.question_date.clone(),
                top_k_episodes: config.retrieval.top_k_episodes,
                top_k_observations: config.retrieval.top_k_observations,
                include_derived_memories: config.retrieval.include_derived_memories,
                include_debug_rationale: config.retrieval.include_debug_rationale,
            })
            .await?;
        let mut metrics = cmem_eval_longmemeval::scoring::score(
            &instance,
            &pack.items,
            &config.metrics.ks_session,
            &config.metrics.ks_turn,
        );
        if let Some(metrics) = metrics.as_object_mut() {
            insert_integrity_metrics(metrics, &pack.items);
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
            latency_ms: timer.elapsed_ms(),
            context_char_count: pack.context_char_count,
            context_word_count: pack.context_word_count,
        });
        namespaces_to_cleanup.push(namespace);
    }
    write_outputs(args, config.clone(), rows)?;
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
}

async fn run_locomo(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    config.validate()?;
    let samples = cmem_eval_locomo::load_path(&args.dataset)?;
    let selected = args.selected_adapter();
    args.validate_adapter_selection()?;
    let adapter_metadata = selected.metadata();
    let adapter = adapter(selected, &config).await?;
    let mut rows = Vec::new();
    let mut namespaces_to_cleanup = Vec::new();
    for sample in samples {
        let namespace = sample.namespace();
        let mapped = cmem_eval_locomo::ingest::to_memory_inputs(
            &sample,
            config.ingest.include_image_captions,
        );
        adapter.reset_namespace(&namespace).await?;
        for episode in mapped.episodes {
            adapter.remember_episode(episode).await?;
        }
        for observation in mapped.observations {
            adapter.remember_observation(observation).await?;
        }
        for qa in &sample.qa {
            let timer = Timer::start();
            let pack = adapter
                .retrieve(RetrieveInput {
                    namespace: namespace.clone(),
                    query: qa.question.clone(),
                    query_date: None,
                    top_k_episodes: config.retrieval.top_k_episodes,
                    top_k_observations: config.retrieval.top_k_observations,
                    include_derived_memories: config.retrieval.include_derived_memories,
                    include_debug_rationale: config.retrieval.include_debug_rationale,
                })
                .await?;
            let mut metrics = cmem_eval_locomo::scoring::score(
                &sample,
                qa,
                &pack.items,
                &config.metrics.ks_dialog,
                &config.metrics.ks_session,
            );
            if let Some(metrics) = metrics.as_object_mut() {
                insert_integrity_metrics(metrics, &pack.items);
            }
            rows.push(PerQuestionResult {
                run_id: config.run_id.clone(),
                dataset: config.dataset.clone(),
                adapter: adapter_metadata.clone(),
                question_id: qa.question_id.clone(),
                question_type: qa.question_type.clone(),
                question: qa.question.clone(),
                gold_episode_ids: sample.evidence_sessions(qa),
                gold_observation_ids: qa.evidence_dialog_ids.clone(),
                retrieved: pack.items,
                metrics,
                latency_ms: timer.elapsed_ms(),
                context_char_count: pack.context_char_count,
                context_word_count: pack.context_word_count,
            });
        }
        namespaces_to_cleanup.push(namespace);
    }
    write_outputs(args, config.clone(), rows)?;
    cleanup_namespaces_after_artifacts(&*adapter, &config, &namespaces_to_cleanup).await
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

    fn validate_adapter_selection(&self) -> Result<()> {
        if self.selected_adapter() == AdapterKind::Mock && !self.allow_mock_benchmark {
            bail!(
                "mock adapter is test/smoke-only; pass `--allow-mock-benchmark` to make mock output explicit, or omit `--adapter` for the default live Character Memory run"
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
        args.validate_adapter_selection().unwrap();
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

        let err = args.validate_adapter_selection().unwrap_err().to_string();
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
        args.validate_adapter_selection().unwrap();
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
}
