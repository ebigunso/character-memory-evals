# CharacterMemoryEvals

Evaluation harness for the Character Memory memory substrate. This repository measures retrieval quality, continuity-context construction, context-size reduction, latency, and provenance-oriented integrity signals.

CharacterMemoryEvals is the public companion evaluation repository for the public [`ebigunso/character-memory`](https://github.com/ebigunso/character-memory) library.

Benchmark CLI runs use Character Memory with its embedded vector store by default. BM25 is a separate baseline over ingested conversation text.

## Commands

```bash
cargo test --workspace
```

The repository pins Rust 1.97.0 with the `rustfmt` and `clippy` components. Each GitHub Actions validation job installs and verifies that toolchain explicitly and checks out this repository beside the public `ebigunso/character-memory` repository so the `../CharacterMemory` path dependency resolves. The sibling checkout requires no deploy key, PAT, or repository secret.

- The Resolve Character Memory revision job captures the public sibling's current `main` commit once so every gate validates the same snapshot.
- The Formatting job checks `cargo fmt --all --check` without compiling the workspace.
- The Clippy job enforces warnings-as-errors across the workspace and all targets.
- The Tests job runs the complete workspace suite with embedded stores and no external service.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

OpenAI embeddings require `OPENAI_API_KEY`; Qdrant service mode additionally requires `QDRANT_CONNECTION_STRING`:

```bash
export QDRANT_CONNECTION_STRING=http://127.0.0.1:6334
export OPENAI_API_KEY=...
```

For live runs that use deterministic embeddings instead of OpenAI, set
`[backend.embedding] provider = "deterministic"` in the run config.

### Vector store mode

`[backend] vector_store_mode = "embedded"` is the default and needs no Qdrant service or connection string. Each run creates exactly `OUT_DIR/stores`, where `OUT_DIR` is the parent of `--out`. Each namespace has a child directory containing its `vectors`, `graph`, `stats.sqlite` and `identities.json`. The runner owns this root and deletes it when the run ends, including on error, unless stores are explicitly retained. An existing root fails admission with its path; retained or interrupted runs are never overwritten.

Set `[backend] vector_store_mode = "service"` to use Qdrant at `backend.qdrant_connection_string` or `QDRANT_CONNECTION_STRING`. Hybrid and vector-only retrieval support both modes. The vector-only baseline uses the library retrieval trace, with one singleton-scoped retrieval per measured object kind and object-level deduplication within each section budget. Its context text comes from the evaluation ingest registry. Rows retain each library `RetrieveOutcome` under `retrieval_outcomes`. Each outcome carries its native completeness verdict in `rationale.telemetry.vector_recall_completeness` and its object scope in `rationale.telemetry.configured_object_types`. BM25 has no library retrieval outcomes. Completed conventional items detach their namespace handles while retaining durable stores until run-end cleanup.

## Continuity Evaluation

Continuity fixtures run an ordered, fixture-scripted lifecycle through remember, staged prepare/validate/commit, retrieve, correct, forget, link, and restart operations. The harness observes and reports retrieval and lifecycle measurements; it does not enforce metric thresholds as CI pass/fail gates.

### Configuration and prerequisites

Archived continuity inputs are `continuity_retrieval.toml`, `continuity_baseline_*.toml`, `continuity_binding_*.toml`, `continuity_task9_*.toml` and `continuity_task9b_*.toml`. They preserve historical regimes and are not current CLI inputs after config-schema removals; register-cited bytes remain immutable. Use `continuity_smoke.toml` or `continuity_crossmode_*.toml` for current runs, and record a new config/hash when rerunning a historical regime. The conventional LoCoMo and LongMemEval retrieval, vector, BM25 and cross-mode configs remain maintained. Continuity uses each fixture scenario's embedding provider: the checked schema-v3 fixture has two semantic scenarios using the committed `text-embedding-3-large` frozen store and thirteen structural scenarios using controllable similarity padded to the store width. Frozen configs require `backend.embedding.store_path`. Persistent graph, SQLite statistics and the identity registry are created inside each run namespace automatically for restart support.

### Run a service-free continuity smoke

`configs/continuity_smoke.toml` is the maintained, unsealed inner-loop config and may change with the current CLI. Run its frozen `graded-similarity` scenario with the embedded adapter, then compare its merged traces with a previous run produced by the current artifact shape:

```bash
mkdir -p .agent-work/continuity-smoke/candidate
cargo run -p cmem-eval-runner -- run continuity --dataset ./crates/cmem-eval-continuity/fixtures/continuity_v3.json --config ./configs/continuity_smoke.toml --scenario graded-similarity --out ./.agent-work/continuity-smoke/candidate/traces.jsonl
cargo run -p cmem-eval-runner -- diff ./.agent-work/continuity-smoke/baseline/traces.jsonl ./.agent-work/continuity-smoke/candidate/traces.jsonl
```

The v3 catalog adds five purpose-built scenarios. `graded-similarity` uses frozen real-model geometry to require target > near miss > background. `combined-life` uses the same frozen store for a 62-event, year-spanning life history with two interleaved threads, correction chains, links, hubs, and varied salience. `temporal-patterns`, `entrenched-correction`, and `autobiographical` use controllable similarity so temporal structure, repeated-misinformation correction, and ordinary-person provider judgment remain deterministic and independently interpretable.

The optional `[backend.character_memory]` table overrides Character Memory's selectivity controls for a run. `selectivity_smoothing_alpha` and `selectivity_gamma` are individually optional and must be finite positive numbers when present. The nested `retrieval.fanout` tables support exactly three relation/object paths: `about_entity.derived_memory`, `participant_entity.episode`, and `part_of_thread.derived_memory`; each leaf budget table is atomic, so a present table must contain both `min` and `max`, and its minimum must not exceed its maximum. The committed values pin the shipped Character Memory defaults (`1.0`, `1.0`, `0/20`, `0/5`, and `0/15`) so baseline reports are self-describing. Omitting `[backend.character_memory]`, either selectivity key, or an entire leaf budget table delegates those settings to the installed Character Memory defaults without adding an eval-side fallback. The exact TOML, including that table, is recorded once in `header.json` under `config`.

```toml
[backend.character_memory]
selectivity_smoothing_alpha = 1.0
selectivity_gamma = 1.0

[backend.character_memory.retrieval.fanout.about_entity.derived_memory]
min = 0
max = 20

[backend.character_memory.retrieval.fanout.participant_entity.episode]
min = 0
max = 5

[backend.character_memory.retrieval.fanout.part_of_thread.derived_memory]
min = 0
max = 15
```

Continuity runs require Rust 1.97.0, the sibling `../CharacterMemory` checkout, the checked fixture and writable paths for the configured stores. The embedded adapter is the service-free default. Qdrant is optional: select `backend.vector_store_mode = "service"` and provide a gRPC endpoint such as `http://127.0.0.1:6334` to use it. Controllable-similarity and frozen runtime providers require no `OPENAI_API_KEY`; generating a new OpenAI frozen store requires the key.

### Generate and validate frozen real embeddings

A frozen store is a JSON cache keyed by model and the SHA-256 of each exact UTF-8 text. The existing schema-v2 file shape stays unchanged: `source` and `dimension_policy` are descriptive strings, and historical labels load verbatim without admission policy. Entries retain exact text beside each `f32` vector. Loading checks schema, model, configured vector width, sorted unique hashes, exact text bytes and finite components. Extra cache entries are allowed. A missing text fails before continuity creates namespace resources and names the `cmem-eval embeddings generate` command; runtime never fills the cache through a network request.

The generation manifest has stable text IDs plus optional `similarity_orderings`. Continuity manifests enumerate exact frozen-provider lookup text: CharacterMemory-normalized content for write events after the adapter removes the object-surface prefix, and byte-exact fixture text for queries. Fixture event text remains source-exact; normalization belongs only to the runtime embedding contract. Each ordering names an anchor and candidate IDs from most to least similar, with a non-negative minimum adjacent margin. This is the authoring gate for real-embedding scenarios: a target, same-domain near miss, and unrelated background can be declared in descending order, and generation fails before writing the store unless measured cosine similarities satisfy that order. Revise the embedded texts when the intended geometry fails; do not weaken the ordering to preserve placeholder prose.

Set `OPENAI_API_KEY`, then run the one explicit network step. The command deduplicates exact texts by SHA-256 and sends one batched [OpenAI embeddings request](https://developers.openai.com/api/reference/resources/embeddings/methods/create), with one returned vector per unique text and no automatic retry after an ambiguous network failure:

```bash
cargo run -p cmem-eval-runner -- embeddings generate \
  --manifest ./crates/cmem-eval-continuity/fixtures/embeddings/task22_real_manifest.json \
  --model text-embedding-3-large \
  --out ./crates/cmem-eval-continuity/fixtures/embeddings/task22_real_store.json
```

Generation requests every unique manifest text in one batch. Optional `--dimensions` is sent directly to the provider; otherwise the provider chooses its default width. New stores describe this as `requested_dimensions=<width>` or `provider_default`. The cache width must match the configured index width; provenance labels do not restrict runtime use.

Recheck store integrity, coverage, and semantic orderings without a key or network:

```bash
cargo run -p cmem-eval-runner -- embeddings validate \
  --manifest ./crates/cmem-eval-continuity/fixtures/embeddings/task22_real_manifest.json \
  --store ./crates/cmem-eval-continuity/fixtures/embeddings/task22_real_store.json
```

Use the resulting store with a schema-v3 frozen-only config. Every manifest or runtime lookup must be cached; unrelated cached texts may remain:

```toml
[backend.embedding]
provider = "frozen"
model = "text-embedding-3-large"
vector_size = 3072
store_path = "crates/cmem-eval-continuity/fixtures/embeddings/task22_real_store.json"
```

Use `provider = "frozen"` when selected schema-v3 scenarios contain both explicit `controllable_similarity` and `frozen` embedding blocks; the runtime binds each scenario to its declared provider. The committed `task21_smoke_manifest.json` and `task21_smoke_store.json` exercise format and validation machinery with a store that declares `source = "test_fixture"`. Frozen CLI runs preflight cache coverage and model/width consistency. The source label describes how vectors were obtained; it does not decide admission. OpenAI generation records `source = "open_ai_api"` and the requested model.

Set the live endpoint in the current shell before a live run:

```bash
export QDRANT_CONNECTION_STRING=http://127.0.0.1:6334
```

PowerShell uses `$env:QDRANT_CONNECTION_STRING = "http://127.0.0.1:6334"` for the same setting.

### Generate a fixture candidate

The checked fixture seed is `20260712`. Generate into `runs/` for inspection instead of overwriting the checked fixture before reviewing the diff:

```bash
cargo run -p cmem-eval-continuity --bin generate_continuity_fixtures -- \
  ./runs/continuity/generated/continuity_v3.json 20260712
```

Schema v3 keeps backend persistence identities derived from config, stable namespaces, and external IDs and continues to reject the retired caller-supplied `collection_name`, `memory_id`, and `replacement_memory_id` fields. It requires every scenario to declare `provider = controllable_similarity` or `provider = frozen`; older fixture schema versions are rejected with the found and expected versions. Parse the candidate, inspect its semantic diff against `crates/cmem-eval-continuity/fixtures/continuity_v3.json`, validate the frozen store, and run the generator determinism tests before replacing the checked fixture.

### Read run artifacts

Every run writes one JSONL artifact named by `--out`, plus adjacent `header.json` and `report.json`. The output filename must end in `.jsonl`. Outputs are always new files: if any of these three names already exists as a file, link or directory, admission fails before writing artifacts. Each writer also creates its file atomically and fails if a destination appears after admission. Choose a new output directory or deliberately remove the existing outputs before running again. Outputs are admitted outside that directory's disposable `stores` root before artifact writes.

- Continuity `traces.jsonl` carries each query's result payload once at the top level: IDs, question/type, gold labels, retrieved items, context, metrics, measured latency and native outcomes. It also records fixture/namespace/event identity, timestamp, expected labels, history text and restart observations belonging to that probe query. There is no separate rows or summary file.
- Conventional datasets keep one result row per query in their JSONL artifact.
- `header.json` owns run identity, adapter, exact config and hashes, input hash, commits, storage root/retention, and scenario or dataset embedding bindings. Reports carry no second header.
- Continuity `report.json` contains aggregate metrics, support/coverage, degradation, latency and restart count; per-scenario metrics/support/coverage; and measured tuning observations. Full rationale, fanout, health and restart payloads are read from traces. Conventional `report.json` contains the row aggregates, support/coverage, latency and degradation.

These artifacts use ordinary derived serde without schema dispatch. Measured latency and native timestamps can vary; use `diff` to compare runs.

### Compare runs

`diff` reads conventional rows or merged continuity traces through the same derived result-row serde and compares by question after normalizing only `run_id` and `latency_ms`. A row whose required fields are missing or mistyped fails to deserialize; an artifact from a superseded shape is old and is compared with an offline tool resurrected from the commit the findings register names, never by the live command. It reports returned-identity, rank, metric, and degradation-flag changes plus a summary:

```bash
cargo run -p cmem-eval-runner -- diff \
  ./runs/continuity/baseline/traces.jsonl \
  ./runs/continuity/candidate/traces.jsonl
```

Required registry keys are initialized to JSON `null` when a row cannot measure them. In `metric_support`, `numeric_rows` counts measured values, `null_rows` counts explicitly unsupported rows, and `unsupported = true` means every present row was null. A null is not zero and does not mean the evaluation failed. `registry_coverage.missing_required_metrics` instead identifies required keys that were absent entirely.

Fixture `irrelevant_external_ids` are sampled negatives, not an exhaustive complement of the relevant set. `sampled_context_pollution_rate` and its rationale attribution classify only explicitly relevant IDs and explicitly sampled-negative IDs; unlabeled retrieved items are not silently treated as negative.

### Extend the scenario library

1. Add or update a deterministic scenario constructor in `crates/cmem-eval-continuity/src/generator.rs`; add a `ScenarioPattern` variant in `fixture.rs` only when the scenario represents a new pattern.
2. Give every event, query, and created object a stable unique external ID. Do not add backend memory IDs to the fixture schema: the driver derives persistence identities from external IDs. Events must be chronological, and correction, forget, link, and relevance references must target supported object kinds admitted earlier in that scenario.
3. Declare non-empty, unique `relevant_external_ids` for every non-abstention query. A scenario with pattern `abstention` instead requires every query to have an empty relevant set; no other pattern may use one. Sampled `irrelevant_external_ids` may be empty when no defensible negative exists; when present, they must be unique and disjoint from the relevant IDs. Keep these labels in fixture/scoring paths only; do not copy them into adapter inputs or metadata.
4. Assign every text that reaches the controllable-similarity provider to exactly one embedding concept. Entity labels are embedding inputs as well as display text, so every entity label must also appear exactly once in `embedding.concepts`; the generator assigns referenced entity labels to the first referencing concept and unreferenced labels to `entity_background`. For a frozen scenario, put every exact runtime text in the generation manifest and declare the semantic orderings that scenario needs.
5. Use schema v3, the only accepted fixture schema version. Tag every controllable-similarity embedding block with `"provider": "controllable_similarity"` and every frozen block with `"provider": "frozen"`.
6. Use the dedicated v3 patterns rather than overloading earlier measurements: `graded_similarity` covers target/near-miss/background discrimination; `combined_life` covers interleaved patterns in one namespace; `temporal_patterns` covers interval, recurrence, and one-off-versus-repeated structure; `entrenched_correction` covers late correction after reinforcement; `autobiographical` covers self-history continuity; `multi_evidence_assembly` covers answers requiring several evidence items; and `abstention` covers pollution-only no-answer queries. Existing patterns remain for their established semantics.
7. Regenerate a candidate with the checked seed, inspect the semantic and byte diff, run offline frozen-store validation when applicable, and run the fixture parser, generator determinism, embedded driver, and workspace tests before replacing checked JSON.

### Add a continuity metric

1. Implement the measurement in `crates/cmem-eval-continuity/src/metrics.rs` using only fixture labels and backend-neutral trace telemetry. Keep entity handling type-neutral and preserve deterministic ordering.
2. Register every required key in `continuity_metric_family`; add dynamic keys from the selected scenarios when the metric varies by fixture vocabulary.
3. Initialize unsupported values as `null`, never a fabricated zero. Add hand-computed tests for measured values and an explicit missing-telemetry test for null support.
4. Confirm the metric appears in the merged trace and aggregate and per-scenario report sections with matching `metric_support` and `registry_coverage`.

Continuity metrics are measurements for comparison and tuning. Adding a metric does not create a CI threshold or a pass/fail policy.

BM25 retrieval is the service-free lexical hurdle that Character Memory recall must beat. Select it with `[retrieval] mode = "bm25_only"`; the baseline ranks ingested episodes and observations without Qdrant, Oxigraph, OpenAI, or live Character Memory retrieval:

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s \
  --dataset ./datasets/longmemeval_s_cleaned.json \
  --config ./configs/longmemeval_s_bm25.toml \
  --out ./runs/longmemeval_s_bm25/results.jsonl
```

BM25 configs are available for LongMemEval-S and LoCoMo: `configs/longmemeval_s_bm25.toml` and `configs/locomo_bm25.toml`. Use baseline-specific run IDs and output paths so active benchmark artifacts are not overwritten.

Vector-only retrieval uses `[retrieval] mode = "vector_only"`. It ingests through Character Memory and reads the library retrieval trace for one object kind at a time, then ranks raw episodes and observations within their configured budgets. It supports embedded and Qdrant service stores and uses the configured embedding provider. Vector-only and BM25 policies accept only episodes and observations and require a nonzero budget for every selected kind. Continuity rejects BM25 before fixture or embedding setup.

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s \
  --dataset ./datasets/longmemeval_s_cleaned.json \
  --config ./configs/longmemeval_s_vector.toml \
  --out ./runs/longmemeval_s_vector/results.jsonl
```

Vector-only configs are available for LongMemEval-S and LoCoMo:
`configs/longmemeval_s_vector.toml` and `configs/locomo_vector.toml`. Use vector-specific run IDs, namespace prefixes,
and output paths. Do not point vector-only runs at active benchmark Qdrant or
OpenAI resources unless sharing that load is intentional.

LongMemEval-S and LoCoMo expect local dataset files:

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s \
  --dataset ./datasets/longmemeval_s_cleaned.json \
  --config ./configs/longmemeval_s_retrieval.toml \
  --out ./runs/longmemeval_s_v0_1/results.jsonl

cargo run -p cmem-eval-runner -- run locomo \
  --dataset ./datasets/locomo10.json \
  --config ./configs/locomo_retrieval.toml \
  --out ./runs/locomo_v0_1/results.jsonl
```

Gold evidence labels are used only for scoring. They are not copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.

## Architecture

The workspace separates shared evaluation contracts, dataset-specific behavior, live Character Memory integration, and CLI orchestration:

- `crates/cmem-eval` owns the Character Memory adapter, configuration, external-ID mappings, context rendering, shared metrics, and result/summary types. It derives namespace store names and persists external IDs for reattach. Dataset-name dispatch stays in the runner.
- `crates/cmem-eval-longmemeval`, `crates/cmem-eval-locomo`, and `crates/cmem-eval-continuity` own their loaders, ingest or event mapping, scorers or trace contracts, full-history construction, config-name validation, and metric-family declarations.
- `crates/cmem-eval-runner` owns the CLI and static dataset selection. Its `DatasetSpec` seam feeds conventional datasets into the generic ingest → enrich → retrieve → score → result pipeline and routes continuity fixtures through their ordered scripted lifecycle driver.

Adding a dataset requires a dataset crate plus a runner `DatasetSpec` implementation, but no `cmem-eval` change. Continuity-specific fixture parsing, ordered event execution, and query trace serialization remain in `crates/cmem-eval-continuity`.

JSONL rows or continuity traces, headers, and reports deserialize through ordinary derived serde. Output artifacts have no schema version or compatibility dispatch: unknown fields are ignored, optional fields may be absent, and required fields and field types follow their serde definitions. Sealed evidence remains immutable bytes verified by hash. Use `cmem-eval diff` to compare query results; reports contain measurements without normalization metadata or cross-artifact congruence checks.

The runtime required-metric set combines the core base family with the selected dataset family, and unsupported required metrics remain explicit `null` values reflected by `metric_support` and `registry_coverage`. Retrieval latency remains per-row `latency_ms` and report mean/median/p50/p95 values (`aggregate.latency.latency_ms` for continuity, `latency.latency_ms` for conventional datasets), separate from deterministic `metrics`. Rows carry a run ID with their query and result data; run provenance lives in the header.

Rows embed native `RetrieveOutcome` values and `RecordedOutcome<T> { operation_id, outcome }` envelopes for `RememberOutcome`, `LinkOutcome`, and `LifecycleMutationOutcome`. The harness operation ID is a deterministic idempotency identity shared by retries. Writers sort outcome arrays by this ID for canonical serialization; that order does not represent execution order. The library outcome is serialized without field projection. The degradation summary reads native vector failures, stats-update failures, and repair markers.

Native outcomes retain retrieval diagnostics directly. Character-facing recall, rationale, pollution, and lifecycle-safety metrics remain measurements.

Live namespace lifecycle is explicit: `open_namespace` creates fresh state, while `reattach_namespace` requires the complete durable identity: external-ID registry, vector store or deterministic Qdrant collection, graph store and SQLite statistics. Conventional items open once per dataset item (a LongMemEval question or LoCoMo sample) and detach handles after completion. Service collection names derive from `(run_id, namespace)` plus the first 16 hex characters of the canonical run-root SHA-256, so separate output directories own separate collections; a collection already present fails admission by name and survives cleanup. Restart reattaches only the current run's owned state.

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

## Metric Registry

Internal run JSONL records retrieval-only metrics directly. Scalar metrics live under `metrics` so summaries can aggregate them; structured details live under `context`, `composition`, `telemetry`, and `integrity`.

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
[retrieval.surface_policy]
include_debug_rationale = true
```

Native retrieval outcomes retain vector, graph, lifecycle, and section diagnostics. Character-facing integrity metrics are computed directly from these traces; unsupported metrics remain `null` and are reflected in `metric_support` and `registry_coverage`.

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

Runs delete their owned namespace stores and service collections on success or failure, then remove `OUT_DIR/stores`. Retention requires `[backend] retain_stores = true` and a nonblank `retain_reason`; without retention, a reason is rejected.

`header.json` carries run provenance once: run and dataset identity, harness and library checkout commits, generation time, exact config TOML and its SHA-256, adapter mode, input fixture or dataset SHA-256, storage root and its full SHA-256, and one optional `retain_reason` (stores are retained exactly when a reason is present). `embedding_bindings` maps each continuity scenario ID to its runtime binding; conventional runs have one entry keyed by dataset ID. Bindings retain the controllable fixture hash or frozen store hash and source, plus the model, provider and dimension policy where applicable. Reports aggregate metrics, coverage, latency and degradation without repeating header provenance.

A run deletes only what it created. Retained roots and collections from interrupted runs must be inspected and deliberately removed before reusing that output directory/run identity, or use a fresh output directory and run ID. Cleanup failures are reported and preserve the root for inspection. The retired `namespace_prefix`, `identity_registry_dir`, `oxigraph_persistence_path`, `retrieval_stats_path` and `[backend.cleanup]` keys are rejected. On Windows, use a short checkout such as `C:/w/cme` with `C:/w/CharacterMemory` pointing to the pinned library checkout; RocksDB rejects long and verbatim paths.

## Character Memory API

The adapter inputs and rendered context types live in `cmem-eval::memory_adapter`; `cmem-eval::adapter::CharacterMemoryAdapter` calls the sibling library directly. External IDs, namespaces, ranks, scores, rationale, and context text remain evaluation-owned mappings. Library outcome and enum types are reused directly. The embedding default is OpenAI `text-embedding-3-large`.

The workspace test suite runs with embedded stores, including restart, reattach, and cleanup checks. Service-specific collection administration is gated by the explicit `service-tests` feature and fails if Qdrant is unavailable:

```bash
QDRANT_CONNECTION_STRING=http://127.0.0.1:6334 cargo test -p cmem-eval --features service-tests service_mode_ -- --nocapture
```
