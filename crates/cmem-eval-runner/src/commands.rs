use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use cmem_eval_core::{
    BenchmarkRunConfig, EpisodeInput, MemoryAdapter, MockMemoryAdapter, ObservationInput,
    PerQuestionResult, RetrieveInput, Timer, insert_retrieval_metrics, read_jsonl, summarize_rows,
    write_jsonl, write_summary,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

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
            Command::Summarize(args) => summarize(args),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunCommand),
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
    #[arg(long, value_enum, default_value_t = AdapterKind::Mock)]
    adapter: AdapterKind,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
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
    let fixture: SyntheticFixture = read_json(&args.dataset)?;
    let adapter = adapter(args.adapter)?;
    let mut rows = Vec::new();

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
                namespace,
                query: question.question.clone(),
                query_date: None,
                top_k_episodes: config.retrieval.top_k_episodes,
                top_k_observations: config.retrieval.top_k_observations,
                include_derived_memories: config.retrieval.include_derived_memories,
            })
            .await?;
        let metrics = score_basic(
            &pack.items,
            &question.gold_episode_ids,
            &question.gold_observation_ids,
        );
        rows.push(PerQuestionResult {
            run_id: config.run_id.clone(),
            dataset: config.dataset.clone(),
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
    }
    write_outputs(args, config, rows)
}

async fn run_longmemeval(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    let instances = cmem_eval_longmemeval::load_path(&args.dataset)?;
    let adapter = adapter(args.adapter)?;
    let mut rows = Vec::new();
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
                namespace,
                query: instance.question.clone(),
                query_date: instance.question_date.clone(),
                top_k_episodes: config.retrieval.top_k_episodes,
                top_k_observations: config.retrieval.top_k_observations,
                include_derived_memories: config.retrieval.include_derived_memories,
            })
            .await?;
        let metrics = cmem_eval_longmemeval::scoring::score(&instance, &pack.items);
        let gold_observation_ids = instance.gold_turn_ids();
        rows.push(PerQuestionResult {
            run_id: config.run_id.clone(),
            dataset: config.dataset.clone(),
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
    }
    write_outputs(args, config, rows)
}

async fn run_locomo(args: RunArgs) -> Result<()> {
    let config = read_config(&args.config)?;
    let samples = cmem_eval_locomo::load_path(&args.dataset)?;
    let adapter = adapter(args.adapter)?;
    let mut rows = Vec::new();
    for sample in samples {
        let namespace = sample.namespace();
        let mapped = cmem_eval_locomo::ingest::to_memory_inputs(&sample, false);
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
                })
                .await?;
            let metrics = cmem_eval_locomo::scoring::score(&sample, qa, &pack.items);
            rows.push(PerQuestionResult {
                run_id: config.run_id.clone(),
                dataset: config.dataset.clone(),
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
    }
    write_outputs(args, config, rows)
}

fn summarize(args: SummarizeArgs) -> Result<()> {
    let rows = read_jsonl(&args.input)?;
    let Some(first) = rows.first() else {
        bail!("cannot summarize empty JSONL: {}", args.input.display());
    };
    let summary = summarize_rows(
        first.run_id.clone(),
        first.dataset.clone(),
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
        serde_json::to_value(&config)?,
        &rows,
    );
    write_jsonl(&args.out, &rows)?;
    write_summary(&args.summary_out, &summary)
}

fn adapter(kind: AdapterKind) -> Result<Box<dyn MemoryAdapter>> {
    match kind {
        AdapterKind::Mock => Ok(Box::<MockMemoryAdapter>::default()),
        AdapterKind::Real => bail!(
            "real Character Memory adapter target is reserved for the forthcoming public API; use --adapter mock for deterministic local runs"
        ),
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
            adapter: AdapterKind::Mock,
        })
        .await
        .unwrap();
        assert!(out.exists());
        assert!(summary.exists());
    }
}
