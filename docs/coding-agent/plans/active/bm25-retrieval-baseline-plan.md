# Plan: BM25 Retrieval Baseline

- status: draft
- generated: 2026-05-03
- last_updated: 2026-05-03
- work_type: code

## Goal
- Add BM25-based retrieval as a selectable, service-free benchmark mode so eval runs can compare Character Memory hybrid retrieval against a deterministic lexical baseline.

## Definition of Done
- `retrieval.mode = "bm25_only"` can be selected from benchmark TOML without changing existing configs.
- Existing configs continue to default to the current hybrid behavior.
- BM25 retrieval ranks ingested episodes and observations deterministically without Qdrant, Oxigraph, OpenAI, or Character Memory live retrieval.
- BM25 outputs use the existing `RetrievedItem`, JSONL, summary, metrics, and official export paths.
- Baseline runs use distinct run IDs/output paths and cannot cleanup or overwrite active live benchmark resources.

## Scope / Non-goals
- Scope:
  - Retrieval mode config schema and propagation through `RetrieveInput`.
  - Eval-owned BM25 implementation over already-ingested benchmark episodes and observations.
  - Runner behavior for synthetic, LongMemEval-S, and LoCoMo.
  - BM25-specific tests, sample configs, and README/run guidance.
  - Safety notes for running BM25 while live benchmarks are active.
- Non-goals:
  - Vector-only Qdrant baseline.
  - Character Memory public API changes.
  - LLM reader metrics or judge integration.
  - Using live Qdrant/Oxigraph/OpenAI services for BM25.
  - Reusing active benchmark output files or mutating active live benchmark collections.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `configs/*_retrieval.toml`
  - `README.md`
- Existing patterns or references:
  - `RetrievalConfig` currently has top-k and inclusion flags, but no retrieval mode.
  - `RetrieveInput` is the adapter-level contract consumed by mock and real adapters.
  - `MockMemoryAdapter` already stores ingested episodes/observations in memory, but ranks with simple lexical overlap rather than BM25.
  - Metrics and summaries are retrieval-method agnostic because they score `RetrievedItem` external IDs.
- Repo reference docs consulted:
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`
  - `docs/coding-agent/lessons.md`
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`

## Open Questions (max 3)
- None blocking.

## Assumptions
- A1: BM25 can run during active live benchmark work because it does not require live test DBs, provided output paths and run IDs are separate.
- A2: Default mode remains `hybrid`, preserving current benchmark behavior for existing configs.
- A3: BM25 scoring should be deterministic across platforms, with stable tie-breaking by kind/rank/internal ID.
- A4: Gold evidence labels remain scorer-only and are not included in BM25 index fields.
- A5: BM25 context text can be assembled from the selected retrieved item texts using the existing `RetrievedContextPack` shape.
- A6: Initial BM25 ranking covers episodes and observations only; derived-memory BM25 can be added later as a separate explicit expansion.
- A7: Retrieval mode selection is TOML-only for the first pass to preserve benchmark reproducibility.
- A8: BM25 sample configs should be added for synthetic, LongMemEval-S, and LoCoMo.

## Tasks

### Task_1: Add Retrieval Mode Schema
- type: impl
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `configs/*_retrieval.toml`
- depends_on: []
- description: |
  Add a retrieval mode enum to shared config with backward-compatible defaults.
- acceptance:
  - `RetrievalMode` supports `hybrid` and `bm25_only`.
  - `RetrievalConfig::default()` selects `hybrid`.
  - Existing TOML files parse unchanged.
  - Invalid retrieval modes fail with a clear config error.
  - BM25 sample config files for synthetic, LongMemEval-S, and LoCoMo use distinct run IDs.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core config`
  - kind: review
    required: true
    owner: orchestrator
    detail: "Verify existing configs remain semantically hybrid and BM25 configs use distinct run IDs."

### Task_2: Propagate Retrieval Mode Through Runner Inputs
- type: impl
- owns:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_1]
- description: |
  Add mode to `RetrieveInput` and pass it from each dataset runner path.
- acceptance:
  - Synthetic, LongMemEval-S, and LoCoMo construct `RetrieveInput` with `config.retrieval.mode`.
  - Existing `hybrid` mode continues to call the current adapter retrieval behavior.
  - Per-question rows and summaries continue to include full config metadata.
  - Adapter selection (`real` vs `mock`) remains separate from retrieval mode.
  - Retrieval mode is not exposed as a CLI override in this first pass.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner`
  - kind: review
    required: true
    owner: orchestrator
    detail: "Confirm adapter mode and retrieval mode are not conflated."

### Task_3: Implement Eval-Owned BM25 Index And Ranking
- type: impl
- owns:
  - `crates/cmem-eval-core/src/bm25.rs`
  - `crates/cmem-eval-core/src/lib.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
- depends_on: [Task_2]
- description: |
  Implement a small deterministic BM25 scorer and use it for `bm25_only` retrieval over adapter-held benchmark inputs.
- acceptance:
  - BM25 tokenization is deterministic and documented in tests.
  - BM25 computes IDF, term frequency normalization, and length normalization across the namespace corpus.
  - Episodes use `EpisodeInput.summary` as document text.
  - Observations use `ObservationInput.text` as document text.
  - Derived memories are intentionally excluded from initial BM25 ranking even if `include_derived_memories = true`.
  - Results preserve external IDs, parent episode IDs, text, score, and stable ranks.
  - Empty or no-hit queries return deterministic low/zero-score ordering without panics.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core bm25`
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core mock_adapter`

### Task_4: Keep BM25 Service-Free And Active-Run Safe
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `README.md`
- depends_on: [Task_3]
- description: |
  Add guardrails and documentation so BM25 baselines can be run without touching live benchmark services or outputs.
- acceptance:
  - BM25 mode can run with `--adapter mock --allow-mock-benchmark` for service-free smoke validation.
  - BM25 docs instruct users to use separate `run_id`, `--out`, and `--summary-out` paths.
  - BM25 docs state that active live benchmark Qdrant/Oxigraph collections are not used.
  - Cleanup guidance warns against shared active prefixes.
  - Mock/smoke output remains visibly marked through existing adapter metadata.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_bm25.toml --out ./runs/synthetic_bm25.jsonl --summary-out ./runs/synthetic_bm25_summary.json --adapter mock --allow-mock-benchmark`
  - kind: review
    required: true
    owner: orchestrator
    detail: "Verify command paths are BM25-specific and do not overlap existing active benchmark outputs."

### Task_5: Whole-Workspace Validation And Review
- type: review
- owns: []
- depends_on: [Task_1, Task_2, Task_3, Task_4]
- description: |
  Run required validation and review the final diff against the isolation and compatibility goals.
- acceptance:
  - Required repo checks pass or failures are documented with actionable cause.
  - Existing hybrid synthetic smoke command still works.
  - BM25 synthetic smoke command works without live services.
  - Review confirms no active benchmark configs or outputs were modified.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo fmt --all --check`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo clippy --workspace --all-targets -- -D warnings`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test --workspace`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`
  - kind: review
    required: true
    owner: reviewer
    detail: "Reviewer verifies compatibility, BM25 correctness, and active-run isolation."

## Task Waves (explicit parallel dispatch sets)

Interpretation:
- Tasks listed in the same wave are intended to be dispatched in parallel by default,
  when `owns` are disjoint and dependencies are met.
- Waves are executed sequentially.

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]
- Wave 5 (parallel): [Task_5]

## E2E / Visual Validation Spec

- Not applicable. This plan has no UI or browser-facing changes.

## Rollback / Safety
- BM25 is gated by `retrieval.mode`; reverting to or omitting `hybrid` restores current behavior.
- Do not modify active benchmark output files.
- Do not use cleanup against prefixes used by active live benchmark runs.
- Prefer service-free BM25 validation with mock adapter before any long dataset run.
- Keep BM25 run IDs and output filenames method-specific.

## Progress Log (append-only)

- 2026-05-03 Draft created: [Task_1, Task_2, Task_3, Task_4, Task_5]
  - Summary: Initial BM25 baseline implementation plan.
  - Validation evidence: Not run; planning only.
  - Notes: Branch created for planning work: `codex/bm25-retrieval-baseline-plan`.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-05-03 Decision: Start with BM25 before vector-only.
  - Trigger / new insight: BM25 is service-free and should not touch active live benchmark DBs.
  - Plan delta (what changed): Scope excludes vector-only and live DB integration.
  - Tradeoffs considered: BM25 can be implemented fully in the eval harness; vector-only requires Qdrant collection access and embedding parity work.
  - User approval: yes.

- 2026-05-03 Decision: Resolve BM25 baseline open questions.
  - Trigger / new insight: User accepted recommendations on BM25 baseline scope and configuration style.
  - Plan delta (what changed): Derived memories are excluded from initial BM25; retrieval mode stays TOML-only; BM25 configs will be added for all runnable datasets.
  - Tradeoffs considered: Keeping the first BM25 baseline narrow improves comparability and reproducibility; broader derived-memory or CLI override support can be added later.
  - User approval: yes.

## Notes
- Risks:
  - BM25 may look deceptively comparable if run IDs or config metadata do not clearly identify the retrieval method.
  - If derived memories are added to BM25 later, scoring semantics may become less direct because current dataset gold IDs primarily target episodes/observations.
  - Long dataset BM25 runs can still consume CPU and write output files, even though they do not use live DBs.
- Edge cases:
  - Empty query.
  - Very short documents.
  - All-zero scores.
  - Duplicate terms and case/punctuation normalization.
