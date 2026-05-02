# Plan: Metric Registry Runtime Readiness

- status: draft
- generated: 2026-05-03
- last_updated: 2026-05-03
- work_type: mixed

## Goal
- Make the harness fully ready to collect the required metric registry for real LongMemEval-S and LoCoMo runs by adding structured per-question telemetry, context efficiency metrics, context composition metrics, trace-backed route/integrity metrics, QA placeholder/export readiness, and truthful unsupported-metric reporting.

## Definition of Done
- Internal JSONL rows preserve existing compatibility while adding structured fields needed for the metric registry.
- Summary JSON aggregates numeric metrics and reports support/coverage for null or unsupported registry metrics.
- LongMemEval-S and LoCoMo retrieval runs emit all available evidence metrics at the recommended `k` values.
- Context efficiency metrics include estimated retrieved context tokens, full source context estimates, and compression ratio.
- Real adapter captures available Character Memory `RetrieveOutcome` rationale/trace telemetry and exposes it to metrics.
- Integrity metrics are numeric when supported by returned context or trace telemetry and `null` only when the current public API cannot substantiate them.
- Official export flows remain compatible and QA metrics are represented as unavailable until predictions/judgments are supplied.
- Documentation clearly distinguishes always-available, estimate-only, trace-dependent, QA-dependent, and library-blocked metrics.

## Scope / Non-goals
- Scope:
  - Core result schema extension.
  - Metric registry and support/coverage helpers.
  - Real adapter telemetry extraction from Character Memory `RetrieveOutcome`.
  - Runner insertion of context, composition, route, integrity, and QA placeholder metrics.
  - Config updates for recommended `k` values and enrichment paths.
  - README docs for metric availability and real-run commands.
- Non-goals:
  - Running a reader LLM or judge model inside the Rust harness.
  - Implementing official QA scoring scripts in Rust.
  - Modifying the Character Memory library API from this repository.
  - Adding an exact tokenizer dependency unless explicitly selected later.
  - Claiming numeric integrity guarantees that cannot be proven from current public API output.

## Recommended Defaults / Resolved Questions
- Compression denominator: use the full source transcript text that the harness ingests for the current question/sample, estimated with the existing token estimator. For LongMemEval this is the instance haystack sessions. For LoCoMo this is the full sample conversation source used for the run.
- Token counting: use the existing heuristic token estimator initially and mark fields as `estimated_*`; avoid adding tokenizer dependencies before first real runs.
- Output placement: keep scalar registry metrics in `metrics` for aggregation, and add structured `context`, `telemetry`, `composition`, and `reader` fields for detailed analysis.
- QA handling: reserve internal `reader` fields with `null` prediction/score values; official exports continue to require external prediction JSONL and must not fabricate QA metrics.
- Trace-dependent metrics: for real metric runs, configs should default `retrieval.include_debug_rationale = true`; if trace is absent, trace-backed route/integrity metrics must be `null` and reflected in `metric_support`.
- Enrichment: configs should include the sanitized regenerated enrichment paths for real enriched runs while keeping comments that raw-only runs are possible for ablations.

## Metrics Coverage Targets

### Must Be Numeric In Retrieval-Only Runs
- LongMemEval-S:
  - `session_recall_any@1/3/5/10`
  - `session_recall_all@1/3/5/10`
  - `session_recall_fraction@1/3/5/10`
  - `session_mrr@1/3/5/10`
  - `session_ndcg@1/3/5/10`
  - `turn_recall_any@5/10/20/50`
  - `turn_recall_all@5/10/20/50`
  - `turn_recall_fraction@5/10/20/50`
  - `turn_mrr@5/10/20/50`
  - `turn_ndcg@5/10/20/50`
- LoCoMo:
  - `dialog_recall_any@5/10/20/50`
  - `dialog_recall_all@5/10/20/50`
  - `dialog_recall_fraction@5/10/20/50`
  - `dialog_mrr@5/10/20/50`
  - `dialog_ndcg@5/10/20/50`
  - `session_recall_any@1/3/5/10`
  - `session_recall_all@1/3/5/10`
  - `session_recall_fraction@1/3/5/10`
  - `session_mrr@1/3/5/10`
  - `session_ndcg@1/3/5/10`
- Shared:
  - `retrieved_context_estimated_tokens`
  - `full_history_estimated_tokens`
  - `context_compression_ratio`
  - `context_reduction_rate`
  - `retrieval_latency_ms`
  - context composition counts by returned kind.
  - rationale coverage for returned items.
  - missing external/provenance counts.

### Must Be Present But May Be Null Until API Support Exists
- `context_validation_pass_rate`
- `suppressed_memory_leakage_rate`
- `orphan_vector_leakage_rate`
- `superseded_current_leakage_rate`
- `cross_store_id_validation_pass_rate`
- route contribution rates that cannot be inferred from available trace fields.
- QA Accuracy/F1/EM/abstention/unsupported-answer metrics when no external reader/judge file is supplied.

## Character Memory Library-Side Needs
- If strict non-null integrity metrics are required, the library should expose per-returned-item validation status including authoritative object existence, retention state, current/superseded status, graph/domain validation, and vector-to-graph ID match.
- If route contribution must be precise, the library should expose per-returned-item route labels, not only candidate/trace-level events.
- If temporal-reasoning retrieval should use `query_date`, the public retrieval input should accept and apply the query timestamp.
- If exact token accounting is required, the library or caller should standardize a tokenizer/model accounting contract.

## Tasks

### Task_1: Extend Core Result Schema
- type: impl
- owns:
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-core/src/token_estimate.rs`
- depends_on: []
- description: |
  Add backward-compatible structured result fields for context efficiency, retrieval telemetry, composition, and reader/QA placeholders.
- acceptance:
  - `PerQuestionResult` can deserialize old rows with defaults for new fields.
  - Per-row schema can store structured `context`, `telemetry`, `composition`, `integrity`, and `reader` details.
  - `RetrievedContextPack` can carry optional adapter telemetry without changing existing mock behavior.
  - Existing result JSONL and summary tests still pass.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-core`

### Task_2: Add Metric Registry And Support Reporting
- type: impl
- owns:
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-core/src/results.rs`
- depends_on: [Task_1]
- description: |
  Define registry keys for required retrieval, context, runtime, integrity, composition, route, and QA metrics. Ensure every required metric is numeric or explicitly null with support metadata.
- acceptance:
  - Registry helpers can initialize required metric keys to `null`.
  - Numeric aggregation continues to work for available values.
  - Summary includes metric support/coverage for null, numeric, and missing values.
  - Tests cover unsupported metrics remaining visible in summary output.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-core`

### Task_3: Capture Real Adapter Trace Telemetry
- type: impl
- owns:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_1]
- description: |
  Map available Character Memory `RetrieveOutcome.rationale` and `RetrievalTrace` data into the eval adapter telemetry contract.
- acceptance:
  - Telemetry includes vector candidate count, graph verified count, stale omission count/reasons, lifecycle omission count/reasons, graph relation count, section assignment counts, and trace availability.
  - Returned item rationale remains populated.
  - Trace-dependent telemetry is absent/null when debug trace is disabled.
  - Feature-gated real adapter tests cover telemetry extraction from a constructed or fixture-like outcome where feasible.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_4: Compute Context Efficiency And Composition Metrics
- type: impl
- owns:
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-longmemeval/src/ingest.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_1, Task_2]
- description: |
  Compute full-history estimates from source data and retrieved-context estimates from returned context, then emit composition metrics from returned item kinds.
- acceptance:
  - LongMemEval rows include full-history estimated tokens/words/chars for each instance.
  - LoCoMo rows include full-history estimated tokens/words/chars for each sample.
  - Rows emit retrieved context estimated tokens, compression ratio, and reduction rate.
  - Rows emit counts for episodes, observations, derived memories, threads, entities, open loops, commitments, and character signals when available; unsupported section-specific counts are null rather than guessed.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-core && cargo test -p cmem-eval-runner`

### Task_5: Compute Trace-Backed Integrity And Route Metrics
- type: impl
- owns:
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_2, Task_3]
- description: |
  Replace placeholder integrity metrics with numeric values where current returned context or trace telemetry can substantiate them; leave library-blocked metrics as null with explicit support metadata.
- acceptance:
  - Provenance coverage for returned derived memories is numeric.
  - Context validation pass rate uses graph-verified/returned counts as a documented proxy when trace supports it; otherwise null.
  - Suppressed/deleted and superseded leakage use lifecycle/stale omission/returned telemetry where available; otherwise null.
  - Vector-only orphan and cross-store ID validation remain null unless a defensible trace-backed calculation exists.
  - Route contribution counts/rates are emitted for vector candidates, graph relations, and section assignments where trace exists.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-core && cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_6: Add QA Placeholder And External Prediction Metric Path
- type: impl
- owns:
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `README.md`
- depends_on: [Task_1, Task_2]
- description: |
  Make QA metrics explicitly absent in retrieval-only runs and preserve official export pathways for later external reader/judge results.
- acceptance:
  - Internal rows include `reader` placeholders with null answer/model/score fields for retrieval-only runs.
  - QA metric registry keys are present as null unless an external prediction/judgment path supplies values.
  - LongMemEval official QA export still requires explicit predictions and never fabricates hypotheses.
  - LoCoMo export continues to preserve prediction/context fields and documents that answer text is not stored in internal rows.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner official_exports`

### Task_7: Update Configs For Recommended Metric Collection
- type: docs
- owns:
  - `configs/longmemeval_s_retrieval.toml`
  - `configs/locomo_retrieval.toml`
  - `configs/synthetic_retrieval.toml`
  - `README.md`
- depends_on: [Task_4, Task_5, Task_6]
- description: |
  Align example configs and docs with the metric registry and real enriched runs.
- acceptance:
  - LongMemEval config uses `ks_session = [1, 3, 5, 10]` and `ks_turn = [5, 10, 20, 50]`.
  - LoCoMo config uses `ks_dialog = [5, 10, 20, 50]` and `ks_session = [1, 3, 5, 10]`.
  - Real enriched configs point to sanitized regenerated enrichment artifacts or document the exact flag needed to enable them.
  - `include_debug_rationale = true` is documented as required for trace-backed metrics.
  - README lists which metrics are numeric, null, trace-dependent, QA-dependent, and library-blocked.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test --workspace`

### Task_8: Smoke Runs And Official Export Validation
- type: test
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `README.md`
- depends_on: [Task_7]
- description: |
  Validate that the run/export path produces the expected metric-bearing artifacts without requiring live backend access.
- acceptance:
  - Mock synthetic smoke run succeeds.
  - Internal JSONL rows include new structured fields and registry metrics.
  - Summary JSON includes p50/p95 latency, metric support, and registry coverage.
  - Official LongMemEval retrieval export and LoCoMo export tests remain compatible.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner official_exports`

### Task_9: Full Review And Validation
- type: review
- owns: []
- depends_on: [Task_8]
- description: |
  Gate the metric readiness work with full validation and a reviewer pass against the handoff registry.
- acceptance:
  - Full workspace tests pass.
  - Real-adapter feature tests pass.
  - Clippy passes with real adapter feature enabled.
  - Reviewer confirms no false-zero unsupported metrics, no gold-label ingestion, and registry coverage is truthful.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo fmt --all --check`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test --workspace`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo clippy --workspace --all-targets --features real-character-memory -- -D warnings`
  - kind: review
    required: true
    owner: reviewer
    detail: "Review implementation against metric registry, official export compatibility, and gold-label isolation."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4, Task_6]
- Wave 4 (parallel): [Task_5]
- Wave 5 (parallel): [Task_7]
- Wave 6 (parallel): [Task_8]
- Wave 7 (parallel): [Task_9]

## Rollback / Safety
- Keep all new result fields backward-compatible with `#[serde(default)]`.
- Never move QA answers/gold evidence into adapter metadata or enrichment.
- Emit `null` for unsupported metrics rather than `0.0`.
- Keep official exports as post-processing artifacts.
- Keep mock benchmark runs explicitly guarded.

## Progress Log
- 2026-05-03 plan drafted.
  - Summary: Planned work required to make retrieval-only real runs collect the required metric registry honestly, while documenting QA/library-blocked metrics.
  - Validation evidence: Researcher mapped current code gaps and available Character Memory telemetry.

## Decision Log
- 2026-05-03 Decision:
  - Trigger / new insight: Current harness can run real retrieval evals but cannot yet collect the full metric registry.
  - Plan delta (what changed): Add metric registry readiness work across core result schema, adapter telemetry, runner metrics, configs, docs, and review.
  - Tradeoffs considered: Use heuristic token estimates first; reserve exact tokenization and reader/judge integration for later to avoid delaying retrieval metric runs.
  - User approval: pending.
