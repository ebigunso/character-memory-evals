# CharacterMemoryEvals

Evaluation harness for the Character Memory memory substrate. This repository measures retrieval quality, continuity-context construction, context-size reduction, latency, and provenance-oriented integrity signals.

Benchmark CLI runs default to the live Character Memory adapter. Mock runs are available only as explicit smoke/test runs so benchmark output is not accidentally generated from the mock adapter.

## Commands

```bash
cargo test --workspace
```

The repository pins Rust 1.97.0 with the `rustfmt` and `clippy` components. Each GitHub Actions validation job installs and verifies that toolchain explicitly and checks out this repository beside the public `ebigunso/character-memory` repository so the `../CharacterMemory` path dependency resolves. The sibling checkout requires no deploy key, PAT, or repository secret.

- The Resolve Character Memory revision job captures the public sibling's current `main` commit once so every gate validates the same snapshot.
- The Formatting job checks `cargo fmt --all --check` without compiling the workspace.
- The Clippy job enforces warnings-as-errors across the workspace and all targets.
- The Tests job runs the complete workspace test suite.
- The Mock smoke job runs the guarded service-free synthetic CLI and verifies that both output artifacts are non-empty.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_retrieval.toml \
  --out ./runs/synthetic.jsonl \
  --summary-out ./runs/synthetic_summary.json \
  --adapter mock \
  --allow-mock-benchmark
```

Benchmark commands default to the live Character Memory adapter. Provide backend settings:

```bash
export QDRANT_CONNECTION_STRING=http://localhost:6334
export OPENAI_API_KEY=...
```

For live runs that use deterministic embeddings instead of OpenAI, set
`[backend.embedding] provider = "deterministic"` in the run config.

For service-free smoke validation, opt into mock output explicitly:

```bash
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_retrieval.toml \
  --out ./runs/synthetic.jsonl \
  --summary-out ./runs/synthetic_summary.json \
  --adapter mock \
  --allow-mock-benchmark
```

Live synthetic runs use the default adapter:

```bash
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_retrieval.toml \
  --out ./runs/synthetic.jsonl \
  --summary-out ./runs/synthetic_summary.json
```

BM25 retrieval is a service-free lexical baseline selected in TOML with
`[retrieval] mode = "bm25_only"`. It ranks ingested episodes and observations
inside the eval harness and does not connect to Qdrant, Oxigraph, OpenAI, or
live Character Memory retrieval. Use BM25-specific run IDs and output paths so
active benchmark artifacts are not overwritten:

```bash
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_bm25.toml \
  --out ./runs/synthetic_bm25.jsonl \
  --summary-out ./runs/synthetic_bm25_summary.json \
  --adapter mock \
  --allow-mock-benchmark
```

BM25 configs are available for synthetic, LongMemEval-S, and LoCoMo:
`configs/synthetic_bm25.toml`, `configs/longmemeval_s_bm25.toml`, and
`configs/locomo_bm25.toml`. Keep cleanup disabled or use a BM25-specific
namespace prefix; never reuse active live benchmark output paths for baseline
runs.

Vector-only retrieval is a live baseline selected in TOML with
`[retrieval] mode = "vector_only"`. It ingests through Character Memory, then
the eval runner bypasses Character Memory retrieval and searches the namespace
Qdrant collection directly with basic vector similarity over benchmark-provided
raw candidates: episodes and observations. It uses the configured embedding
provider for query embeddings and cannot run with `--adapter mock`.

```bash
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_vector.toml \
  --out ./runs/synthetic_vector.jsonl \
  --summary-out ./runs/synthetic_vector_summary.json
```

Vector-only configs are available for synthetic, LongMemEval-S, and LoCoMo:
`configs/synthetic_vector.toml`, `configs/longmemeval_s_vector.toml`, and
`configs/locomo_vector.toml`. Use vector-specific run IDs, namespace prefixes,
and output paths. Do not point vector-only runs at active benchmark Qdrant or
OpenAI resources unless sharing that load is intentional.

LongMemEval-S and LoCoMo expect local dataset files:

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s \
  --dataset ./datasets/longmemeval_s_cleaned.json \
  --config ./configs/longmemeval_s_retrieval.toml \
  --out ./runs/longmemeval_s_v0_1.jsonl \
  --summary-out ./runs/longmemeval_s_v0_1_summary.json

cargo run -p cmem-eval-runner -- run locomo \
  --dataset ./datasets/locomo10.json \
  --config ./configs/locomo_retrieval.toml \
  --out ./runs/locomo_v0_1.jsonl \
  --summary-out ./runs/locomo_v0_1_summary.json
```

Gold evidence labels are used only for scoring. They are not copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.

## Architecture

The workspace separates shared evaluation contracts, dataset-specific behavior, live Character Memory integration, and CLI orchestration:

- `crates/cmem-eval-core` owns backend-neutral configuration, the `MemoryAdapter` contract and DTOs, deterministic metric primitives, runtime metric-family composition, and versioned result/summary types. Core contains no dataset-name dispatch.
- `crates/cmem-eval-adapter-cmem` is the reusable live Character Memory adapter. It maps the core contract to the sibling library, derives deterministic collection names from the configured prefix, run ID, and namespace, and persists a BTreeMap-backed external-ID registry so a new adapter process can reattach to existing stores without losing benchmark IDs.
- `crates/cmem-eval-longmemeval` and `crates/cmem-eval-locomo` own their loaders, ingest mapping, scorers, full-history construction, config-name validation, and retrieval metric-family declarations.
- `crates/cmem-eval-runner` owns the CLI and static dataset selection. Its `DatasetSpec` seam feeds per-dataset loader/mapper/scorer/full-history/metric-family behavior into one generic ingest → enrich → retrieve → score → result pipeline.

Adding a dataset requires a dataset crate plus a runner `DatasetSpec` implementation, but no `cmem-eval-core` change. The future continuity benchmark belongs in `crates/cmem-eval-continuity`, with its loader, mapping, scoring, full-history logic, and metric-family declaration kept inside that crate.

JSONL rows and summaries use report schema version `1.0.0`; readers reject missing or different versions rather than entering a compatibility mode. The runtime required-metric set combines the core base family with the selected dataset family, and unsupported required metrics remain explicit `null` values reflected by `metric_support` and `registry_coverage`. Retrieval latency remains first-class as per-row `latency_ms` and summary `latency.latency_ms` mean/median/p50/p95 values, but it is excluded from deterministic `metrics`; summaries also record the embedding provider.

Live namespace lifecycle is explicit: `open_namespace` creates fresh run state, while `reattach_namespace` restores the persisted identity registry and reconnects to deterministic collections. Cleanup remains guarded by the configured eval prefix.

## Precomputed Graph Enrichment

The runner can inject graph-shaped memory objects after raw episodes and
observations are stored. This is meant for enrichment prepared by a separate
LLM/Codex pass over source conversation text, avoiding runtime LLM calls inside
the eval harness.

Enable an artifact with `[ingest] enrichment_path = "..."`. The file is JSONL;
each row is scoped to one eval namespace, such as `lme:<question_id>` or
`locomo:<sample_id>`:

```json
{"namespace":"lme:example","entities":[{"external_id":"user","entity_type":"user","name":"User"}],"threads":[{"external_id":"thread:travel","title":"Travel plans","summary":"The user is planning travel.","status":"active"}],"derived_memories":[{"external_id":"dm:travel:1","derived_type":"claim","text":"The user is considering a May trip.","source_episode_external_ids":["session_1"],"thread_external_ids":["thread:travel"],"entity_external_ids":["user"]}],"links":[{"external_id":"link:dm-thread","from":{"object_type":"derived_memory","external_id":"dm:travel:1"},"relation":"part_of_thread","to":{"object_type":"memory_thread","external_id":"thread:travel"}}]}
```

Supported object types are `episode`, `observation`, `entity`,
`memory_thread`, and `derived_memory`. Supported enum values follow the public
Character Memory API snake-case names, for example `user_preference`,
`relationship_note`, `open_loop`, `character_signal`, and `project_note` for
derived memories.

Enrichment must be generated only from haystack/source conversation data. The
loader rejects common gold-label keys such as `answer`, `evidence`,
`answer_session_ids`, `has_answer`, `gold_*`, and `label` anywhere in the JSON.
Derived memories must include source episode or observation external IDs so
provenance survives round trip.

LoCoMo also has benchmark-provided session summaries and generated observations.
The default LoCoMo config indexes those as provenanced derived memories with
`index_session_summaries = true` and `index_generated_observations = true`.
LongMemEval-S does not include equivalent generated memory fields, so additional
entities, threads, links, and derived memories should come from an enrichment
JSONL artifact.

## Official Exports

Internal benchmark runs write eval JSONL first. Official-compatible artifacts are
post-processing outputs so runs, logs, summaries, and exports can be preserved
independently.

LongMemEval retrieval export writes JSONL rows with `question_id` and
`retrieval_results.ranked_items`:

```bash
cargo run -p cmem-eval-runner -- export-official longmemeval retrieval \
  --input ./runs/longmemeval_s_v0_1.jsonl \
  --out ./runs/longmemeval_s_v0_1_retrieval_official.jsonl
```

LongMemEval QA export requires explicit predictions and never fabricates empty
hypotheses. Prediction JSONL must contain `question_id` plus `hypothesis`
or `prediction`:

```bash
cargo run -p cmem-eval-runner -- export-official longmemeval qa \
  --input ./runs/longmemeval_s_v0_1.jsonl \
  --predictions ./runs/longmemeval_s_predictions.jsonl \
  --out ./runs/longmemeval_s_v0_1_qa_official.jsonl
```

LoCoMo export preserves sample/QA identity recovered from stable internal IDs
like `<sample_id>:qa:<index>`, category, question, optional prediction/context
fields, and retrieved dialog/session IDs:

```bash
cargo run -p cmem-eval-runner -- export-official locomo \
  --input ./runs/locomo_v0_1.jsonl \
  --predictions ./runs/locomo_predictions.jsonl \
  --out ./runs/locomo_v0_1_official.jsonl
```

LoCoMo official answer text is not stored in internal run JSONL, so the export
sets `answer` to `null` unless a later dataset-join export path is added.

## Metric Registry

Internal run JSONL records retrieval-only metrics directly and reserves
reader/QA fields for later external prediction and judge outputs. Scalar
metrics live under `metrics` so summaries can aggregate them; structured
details live under `context`, `composition`, `telemetry`, `integrity`, and
`reader`.

Always-available retrieval metrics:

```text
recall_any@k
recall_all@k
recall_fraction@k
mrr@k
ndcg@k
```

LongMemEval-S uses `session_*` and `turn_*` prefixes. LoCoMo uses `dialog_*`
and `session_*` prefixes.

Context efficiency metrics use exact `tiktoken` counts with the `o200k_base`
encoding:

```text
retrieved_context_tokens
full_history_tokens
context_compression_ratio
context_reduction_rate
```

`full_history_tokens` is counted from the source transcript that the harness
ingests for the current question/sample. Token counts are literal text counts
over the retrieved context and source history; they do not include chat-message
framing overhead.

Trace-dependent metrics require:

```toml
[retrieval]
include_debug_rationale = true
```

When trace data is available, rows can also include route and integrity evidence
such as `vector_candidate_count`, `graph_relations_count`,
`graph_verified_count`, `section_assignment_count`,
`suppressed_or_deleted_returned_count`, and
`superseded_current_returned_count`. When trace data is unavailable, affected
route/integrity metrics are emitted as `null` and are reflected in
`metric_support` / `registry_coverage`; the harness does not write false zeroes
for unsupported checks.

QA metrics such as accuracy, F1, exact match, abstention accuracy, and
unsupported-answer rate remain `null` in retrieval-only runs. Use the official
export commands with external predictions/judgments when evaluating answer
quality.

## Timestamp And Cleanup Policy

Official benchmark timestamps are normalized before live ingestion. LongMemEval-S
dates such as `2023/05/30 (Tue) 23:40` and LoCoMo dates such as
`1:56 pm on 8 May, 2023` are treated as benchmark-local naive timestamps and
serialized as UTC RFC3339 values, for example `2023-05-30T23:40:00Z`. The raw
timestamp remains in eval metadata for debugging, but the live adapter stays
strict: any non-RFC3339 timestamp that reaches it fails with context instead of
being guessed at the backend boundary.

Backend post-run cleanup is disabled by default. When `[backend.cleanup] enabled = true`, the runner deletes only live Qdrant collections it created for completed eval namespaces, and only when `require_collection_prefix` matches the configured `namespace_prefix` after Qdrant-name sanitization. Post-run cleanup never deletes files under `runs/`, `reports/`, `datasets/`, or other result artifacts.

Fresh runs always remove any prior deterministic collection and identity registry for the same `(namespace_prefix, run_id, namespace)` before ingest, using `namespace_prefix` as the deletion safety guard. Disabling post-run cleanup therefore preserves a completed collection for inspection only until the next fresh run with the same identity. Use the explicit reattach lifecycle when state must be preserved intentionally across adapter instances or runs.

## Character Memory API

The eval-side adapter contract is in `cmem-eval-core::memory_adapter`. It is written as the target public API boundary for Character Memory: external IDs, namespaces, ranks, scores, rationale, and context text must survive round trip. Live runs require backend settings; the initial embedding default is OpenAI `text-embedding-3-large`.

Omitted `--adapter` and explicit `--adapter real` both select the live adapter.
Mock output is for unit/integration smoke checks only and is marked with `adapter.mode =
"mock_smoke"` in result artifacts.
