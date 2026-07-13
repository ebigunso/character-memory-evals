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

## Continuity Evaluation

Continuity fixtures run an ordered, fixture-scripted lifecycle through remember, staged prepare/validate/commit, retrieve, correct, forget, link, and restart operations. The harness observes and reports retrieval and lifecycle measurements; it does not enforce metric thresholds as CI pass/fail gates.

### Configuration and prerequisites

`configs/continuity_retrieval.toml` is the single committed continuity config for both mock smoke runs and live evaluations. A separate mock config is unnecessary because mock selection is an explicit CLI adapter choice. Continuity validation requires the `controllable_similarity` deterministic provider, the fixture's small vector size of 8, and persistent Oxigraph, retrieval-stat SQLite, and identity-registry paths so restart scenarios can reconstruct every store. The config records `max_vector_candidates = 48` and `max_graph_roots = 48` so report tuning observations remain correlated with the measured candidate-limit regime.

Mock runs require Rust 1.97.0 and the checked fixture only; they do not connect to Qdrant, Oxigraph, SQLite, OpenAI, or another service. Live runs additionally require the sibling `../CharacterMemory` checkout, a local Qdrant gRPC endpoint such as `http://localhost:6334`, and writable paths under `runs/continuity/stores/`. The controllable-similarity provider does not require `OPENAI_API_KEY`.

Set the live endpoint in the current shell before a live run:

```bash
export QDRANT_CONNECTION_STRING=http://localhost:6334
```

PowerShell uses `$env:QDRANT_CONNECTION_STRING = "http://localhost:6334"` for the same setting.

### Run a service-free mock smoke

The guarded mock command runs all eight checked scenarios, writes visibly marked `mock_smoke` artifacts, and uses the same config and metric registry as the live path:

```bash
cargo run -p cmem-eval-runner -- run continuity \
  --dataset ./crates/cmem-eval-continuity/fixtures/continuity_v1.json \
  --config ./configs/continuity_retrieval.toml \
  --out ./runs/continuity/mock/results.jsonl \
  --summary-out ./runs/continuity/mock/summary.json \
  --trace-out ./runs/continuity/mock/traces.jsonl \
  --report-out ./runs/continuity/mock/report.json \
  --adapter mock \
  --allow-mock-benchmark
```

### Run a live restart scenario

This bounded live command exercises Qdrant plus the configured persistent stores and performs the mid-scenario drop/reconstruct path. Remove `--scenario cross-store-stress` to run the complete scenario set.

```bash
cargo run -p cmem-eval-runner -- run continuity \
  --dataset ./crates/cmem-eval-continuity/fixtures/continuity_v1.json \
  --config ./configs/continuity_retrieval.toml \
  --out ./runs/continuity/live/results.jsonl \
  --summary-out ./runs/continuity/live/summary.json \
  --trace-out ./runs/continuity/live/traces.jsonl \
  --report-out ./runs/continuity/live/report.json \
  --scenario cross-store-stress
```

Fresh runs reset only the deterministic namespace-scoped stores derived from the config's prefix, run ID, and fixture namespace. The restart event inside `cross-store-stress` drops the active adapter without deleting those stores, reconstructs Qdrant/Oxigraph/SQLite/identity state, and remeasures the next scripted query before and after reconstruction.

### Re-summarize existing results

Continuity metric families include fixture-derived entity-kind keys, so `summarize` must receive the original config and source fixture. Repeat `--scenario <fixture-id>` when the original run selected one scenario.

```bash
cargo run -p cmem-eval-runner -- summarize \
  --input ./runs/continuity/mock/results.jsonl \
  --config ./configs/continuity_retrieval.toml \
  --dataset ./crates/cmem-eval-continuity/fixtures/continuity_v1.json \
  --out ./runs/continuity/mock/resummary.json
```

### Generate a fixture candidate

The checked fixture seed is `20260712`. Generate into `runs/` for inspection instead of overwriting the checked fixture before reviewing the diff:

```bash
cargo run -p cmem-eval-continuity --bin generate_continuity_fixtures -- \
  ./runs/continuity/generated/continuity_v1.json 20260712
```

The generated file should match `crates/cmem-eval-continuity/fixtures/continuity_v1.json` byte-for-byte when the generator and seed are unchanged.

### Read the continuity artifacts

- `results.jsonl` contains one schema-versioned retrieval result per query. `summary.json` contains numeric aggregates, support counts, registry coverage, and latency.
- `traces.jsonl` contains the deterministic query, expected labels, history text, complete retrieved context pack, rationales, and backend-neutral telemetry used by continuity metrics.
- `report.json` has a top-level `metadata` block and deterministic `content`. `metadata` contains the generation timestamp, run, dataset, and adapter identity, fixture provenance (schema version, fixture seed, embedding seeds, and fixture IDs), the full config snapshot, schema versions, and normalization policy; it does not contain the fixture body. Compare repeat runs by removing `metadata`; correction/forget library mutation timestamps are excluded from deterministic content.
- `content.aggregate` reports metrics, `metric_support`, and registry coverage across the selected run. `content.scenarios` repeats those views per fixture and includes full query/context/rationale samples, fanout/selectivity decisions, stats-health observations, and any restart observations.
- A restart observation records the lifecycle restoration count, before/after returned object IDs and recall, graph/fanout/selectivity snapshots, signed deltas, and whether the returned object set stayed stable.
- `tuning_observations` records measured behavior together with the relevant config regime. These are tuning signals, not assertions that a Character Memory default passed or failed.

Required registry keys are initialized to JSON `null` when a row cannot measure them. In `metric_support`, `numeric_rows` counts measured values, `null_rows` counts explicitly unsupported rows, and `unsupported = true` means every present row was null. A null is not zero and does not mean the evaluation failed. `registry_coverage.missing_required_metrics` instead identifies required keys that were absent entirely.

Fixture `irrelevant_external_ids` are sampled negatives, not an exhaustive complement of the relevant set. `sampled_context_pollution_rate` and its rationale attribution classify only explicitly relevant IDs and explicitly sampled-negative IDs; unlabeled retrieved items are not silently treated as negative.

### Extend the scenario library

1. Add or update a deterministic scenario constructor in `crates/cmem-eval-continuity/src/generator.rs`; add a `ScenarioPattern` variant in `fixture.rs` only when the scenario represents a new pattern.
2. Give every event, query, created object, and memory object a stable unique identity. Events must be chronological, and correction, forget, link, and relevance references must target objects admitted earlier in that scenario.
3. Declare non-empty, unique, disjoint `relevant_external_ids` and sampled `irrelevant_external_ids` for every query. Keep these labels in fixture/scoring paths only; do not copy them into adapter inputs or metadata.
4. Assign every text that reaches the controllable-similarity provider to exactly one embedding concept. Entity labels are embedding inputs as well as display text, so every entity label must also appear exactly once in `embedding.concepts`; the generator assigns referenced entity labels to the first referencing concept and unreferenced labels to `entity_background`.
5. Regenerate a candidate with the checked seed, inspect the semantic and byte diff, and run the fixture parser, generator determinism, mock driver, and workspace tests before replacing the checked JSON.

### Add a continuity metric

1. Implement the measurement in `crates/cmem-eval-continuity/src/metrics.rs` using only fixture labels and backend-neutral trace telemetry. Keep entity handling type-neutral and preserve deterministic ordering.
2. Register every required key in `continuity_metric_family`; add dynamic keys from the selected scenarios when the metric varies by fixture vocabulary.
3. Initialize unsupported values as `null`, never a fabricated zero. Add hand-computed tests for measured values and an explicit missing-telemetry test for null support.
4. Confirm run and `summarize` produce identical config, `metric_support`, and `registry_coverage`, and confirm the metric appears in aggregate and per-scenario report sections.

Continuity metrics are measurements for comparison and tuning. Adding a metric does not create a CI threshold or a pass/fail policy.

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
- `crates/cmem-eval-longmemeval`, `crates/cmem-eval-locomo`, and `crates/cmem-eval-continuity` own their loaders, ingest or event mapping, scorers or trace contracts, full-history construction, config-name validation, and metric-family declarations.
- `crates/cmem-eval-runner` owns the CLI and static dataset selection. Its `DatasetSpec` seam feeds conventional datasets into the generic ingest → enrich → retrieve → score → result pipeline and routes continuity fixtures through their ordered scripted lifecycle driver.

Adding a dataset requires a dataset crate plus a runner `DatasetSpec` implementation, but no `cmem-eval-core` change. Continuity-specific fixture parsing, ordered event execution, and query trace serialization remain in `crates/cmem-eval-continuity`.

JSONL rows and summaries use report schema version `1.0.0`; readers reject missing or different versions rather than entering a compatibility mode. The runtime required-metric set combines the core base family with the selected dataset family, and unsupported required metrics remain explicit `null` values reflected by `metric_support` and `registry_coverage`. Retrieval latency remains first-class as per-row `latency_ms` and summary `latency.latency_ms` mean/median/p50/p95 values, but it is excluded from deterministic `metrics`; summaries also record the embedding provider.

Live namespace lifecycle is explicit: `open_namespace` creates fresh run state, while `reattach_namespace` requires and restores the complete durable identity consisting of the external-ID registry, deterministic Qdrant collection, and every configured namespace-scoped Oxigraph and retrieval-stat store. Configured `oxigraph_persistence_path` values are shared roots whose namespace child directories use the same prefix/run/namespace UUID identity as Qdrant; configured `retrieval_stats_path` values are filename templates whose derived sibling files use that identity while preserving the configured extension. Cleanup remains guarded by the configured eval prefix and never deletes a configured shared root.

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

Backend post-run cleanup is disabled by default. When `[backend.cleanup] enabled = true`, the runner deletes the deterministic Qdrant collection, external-ID registry, namespace-scoped Oxigraph directory, and namespace-scoped retrieval-stat database plus SQLite sidecars for each completed eval namespace, and only when `require_collection_prefix` matches the configured `namespace_prefix` after Qdrant-name sanitization. Post-run cleanup targets only those exact derived durable-store paths; it does not delete configured shared roots or unrelated files under `runs/`, `reports/`, `datasets/`, or other result locations.

Fresh runs always remove every prior namespace-scoped durable store for the same `(namespace_prefix, run_id, namespace)` before ingest, using `namespace_prefix` as the Qdrant deletion safety guard and derived namespace identities to avoid deleting sibling stores. Disabling post-run cleanup therefore preserves completed registry, Qdrant, Oxigraph, and retrieval-stat state for inspection only until the next fresh run with the same identity. Use the explicit reattach lifecycle when all of that state must be preserved intentionally across adapter instances or runs; reattach fails and names every missing configured store rather than admitting partial state.

## Character Memory API

The eval-side adapter contract is in `cmem-eval-core::memory_adapter`. It is written as the target public API boundary for Character Memory: external IDs, namespaces, ranks, scores, rationale, and context text must survive round trip. Live runs require backend settings; the initial embedding default is OpenAI `text-embedding-3-large`.

Omitted `--adapter` and explicit `--adapter real` both select the live adapter.
Mock output is for unit/integration smoke checks only and is marked with `adapter.mode =
"mock_smoke"` in result artifacts.
