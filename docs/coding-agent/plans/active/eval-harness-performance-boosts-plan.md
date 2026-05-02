# Plan: Eval Harness Performance Boosts

- status: approved
- generated: 2026-05-03
- last_updated: 2026-05-03
- work_type: code

## Goal
- Reduce avoidable eval-harness overhead for LongMemEval-S and LoCoMo live runs while preserving benchmark semantics, gold-label isolation, and result artifact compatibility.
- Keep LongMemEval-S as per-question isolated retrieval tasks, but avoid paying one Character Memory write call per episode or observation.

## Definition of Done
- Live ingest uses batched harness adapter calls where Character Memory's public `RememberDraft` API can persist multiple objects at once.
- LongMemEval-S runner no longer calls the live adapter once per session and once per turn.
- LoCoMo runner no longer recomputes full-history context metrics or evidence session lookups for every QA row in a sample.
- Existing JSONL result rows, summary files, official exports, and metric key semantics remain compatible.
- Gold evidence labels remain scorer/result-only and are not copied into memory inputs, enrichment metadata, or adapter metadata.
- Required validation commands pass, including service-free synthetic smoke and real-adapter feature compilation/tests.

## Scope / Non-goals
- Scope:
  - `MemoryAdapter` batch ingest contract and mock compatibility defaults.
  - Real Character Memory adapter batch writes for episodes and observations.
  - Runner wiring for LongMemEval-S and LoCoMo batch ingest.
  - LoCoMo per-sample context/evidence caches.
  - Regression tests that verify output shape and gold-label isolation remain intact.
- Non-goals:
  - Changing official benchmark scoring semantics or output formats.
  - Runtime LLM enrichment.
  - Parallel question/sample execution.
  - Collapsing LongMemEval-S questions into a shared memory store.
  - Reworking Character Memory storage, namespace filtering, or reset semantics.
  - Fixing Character Memory's OpenAI embedding provider batching in this repository.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-locomo/src/types.rs`
  - `crates/cmem-eval-locomo/src/scoring.rs`
  - `crates/cmem-eval-longmemeval/src/scoring.rs`
  - `fixtures/`
- Existing patterns or references:
  - The real adapter already batches enrichment through one `RememberDraft::new(objects).with_links(links)` call.
  - Character Memory's remember pipeline batches graph upserts, embedding generation requests at the embedder trait boundary, and Qdrant upserts within a single `RememberDraft`.
  - LongMemEval-S local dataset has separate haystacks per question, so per-question namespace isolation is correct.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`

## Open Questions (max 3)
- Resolved: use the recommended defaults recorded in the Decision Log.

## Assumptions
- A1: LongMemEval-S questions must remain isolated by namespace because each question has its own haystack/reference set.
- A2: Observations must still be remembered after episodes so observation drafts can resolve episode external IDs.
- A3: Batch write success can update external ID maps after the Character Memory call returns; no output artifacts rely on intermediate per-object internal IDs during ingest.
- A4: Upstream OpenAI embedding batching remains library-side work and is tracked outside this harness plan.

## Tasks

### Task_1: Add Batch Ingest Adapter Contract
- type: impl
- owns:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
- depends_on: []
- description: |
  Extend the shared eval adapter boundary with batch ingest methods for episodes and observations.
  Provide default implementations that preserve existing single-item behavior, so mock tests and any future adapters remain compatible.
- acceptance:
  - `MemoryAdapter` exposes `remember_episodes(Vec<EpisodeInput>)` and `remember_observations(Vec<ObservationInput>)` or equivalent borrowed-slice methods.
  - Default implementations call the existing single-item methods in order.
  - `MockMemoryAdapter` remains behaviorally equivalent for single-item and batch ingest.
  - Existing retrieval results and mock context text ordering remain stable.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-core"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review adapter contract for backward compatibility and no gold-label handling changes."

### Task_2: Implement Real Adapter Batch Writes
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_1]
- description: |
  Override the batch adapter methods in `CharacterMemoryAdapter`.
  Build one multi-object `RememberDraft` for episode batches and one multi-object `RememberDraft` for observation batches per namespace.
- acceptance:
  - Episode batch creates namespace state once, builds deterministic IDs for all episodes, sends one Character Memory remember call, and updates forward/reverse episode maps after success.
  - Observation batch resolves all episode external IDs before writing, builds deterministic IDs for all observations, sends one Character Memory remember call, and updates forward/reverse observation maps after success.
  - Single-item methods either delegate to batch methods or remain consistent with batch mapping.
  - Error messages for unknown episode references remain actionable.
  - Existing enrichment batching remains unchanged.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner --features real-character-memory"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review real adapter map updates, failure behavior, and namespace isolation."

### Task_3: Wire Runner Batch Ingest
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_1, Task_2]
- description: |
  Replace per-episode and per-observation remember loops in synthetic, LongMemEval-S, and LoCoMo runner paths with batch adapter calls while keeping progress output and row generation unchanged.
- acceptance:
  - LongMemEval-S calls the adapter once for all mapped episodes and once for all mapped observations per question.
  - LoCoMo calls the adapter once for all mapped episodes and once for all mapped observations per sample.
  - Synthetic path uses batch methods too, preserving mock smoke coverage.
  - Progress output still reports ingest phase counts clearly, without pretending individual writes are still occurring.
  - Result JSONL and summary schemas are unchanged.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"
  - kind: command
    required: true
    owner: worker
    detail: "cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review runner diff for unchanged scoring inputs and gold-label isolation."

### Task_4: Cache LoCoMo Per-Sample Work
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-locomo/src/types.rs`
  - `crates/cmem-eval-locomo/src/scoring.rs`
- depends_on: []
- description: |
  Remove repeated per-QA work in LoCoMo by computing full-history context metrics and evidence session mappings once per sample.
  Keep QA evidence use limited to scoring and result gold fields.
- acceptance:
  - `locomo_full_history_text` and its derived context baseline metrics are computed once per sample, not once per QA.
  - Dialog ID to session ID mapping is computed once per sample and reused for scoring/result gold session IDs.
  - LoCoMo scoring produces the same metric values as before for existing tests.
  - No QA answer/evidence content is copied into memory inputs or adapter metadata.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo"
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review LoCoMo cache code for scorer-only gold label usage."

### Task_5: Add Artifact Compatibility Regression Coverage
- type: test
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `fixtures/`
- depends_on: [Task_3, Task_4]
- description: |
  Add or strengthen service-free regression checks proving that batch ingest and LoCoMo caching do not change expected output structure, adapter metadata, metric keys, or gold-label boundaries.
- acceptance:
  - Synthetic command test still verifies output and summary creation.
  - Tests cover batch path behavior rather than only single-item adapter calls.
  - Regression assertions check representative row fields: `retrieved`, `metrics`, `context`, `telemetry`, `composition`, `integrity`, and `reader`.
  - Gold-label exclusion remains covered for LongMemEval-S and LoCoMo ingest paths.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-longmemeval"
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review regression coverage against artifact compatibility criteria."

### Task_6: Full Validation And Review
- type: review
- owns: []
- depends_on: [Task_2, Task_3, Task_4, Task_5]
- description: |
  Run the full validation gate and review the complete performance branch.
  Confirm that remaining performance issues are either upstream-library-side or intentionally deferred.
- acceptance:
  - Required repo validation commands pass or any failure is explicitly resolved before completion.
  - Reviewer reports no correctness, artifact compatibility, or gold-label leakage findings.
  - Remaining non-harness bottlenecks are documented in the final report.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo fmt --all --check"
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo clippy --workspace --all-targets -- -D warnings"
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo test --workspace"
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo clippy --workspace --all-targets --features real-character-memory -- -D warnings"
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo test --workspace --features real-character-memory"
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark"
  - kind: review
    required: true
    owner: reviewer
    detail: "Full branch review focused on benchmark correctness, performance intent, artifact compatibility, and gold-label boundaries."

## Task Waves (explicit parallel dispatch sets)

Interpretation:
- Tasks listed in the same wave are intended to be dispatched in parallel by default when `owns` are disjoint and dependencies are met.
- Waves are executed sequentially.

- Wave 1 (parallel) - contract and independent LoCoMo cache prep: [Task_1, Task_4]
- Wave 2 (parallel) - real adapter batching: [Task_2]
- Wave 3 (parallel) - runner wiring: [Task_3]
- Wave 4 (parallel) - regression coverage: [Task_5]
- Wave 5 (parallel) - final validation/review: [Task_6]

## E2E / Visual Validation Spec

- Not applicable. This plan does not touch UI or browser-facing flows.

## Rollback / Safety
- Batch ingest can be rolled back by reverting `MemoryAdapter` batch methods and restoring runner calls to single-item `remember_episode` / `remember_observation`.
- Real adapter batch methods should keep single-item methods working, so a partial rollback can make runner use single-item calls again without changing external APIs.
- No dataset files, result artifacts, or official export schemas should be modified by this plan.

## Progress Log (append-only)

- 2026-05-03 Plan drafted: [Task_1, Task_2, Task_3, Task_4, Task_5, Task_6]
  - Summary: Harness-side batching and LoCoMo caching plan created from performance scan.
  - Validation evidence: Plan integrity checked for owns/dependencies/validation ownership.
  - Notes: Upstream OpenAI embedding batching and namespace storage changes are non-goals.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-05-03 Decision: Keep LongMemEval-S per-question isolated.
  - Trigger / new insight: Local dataset check showed each question has its own haystack session set; the first two questions had zero session overlap.
  - Plan delta (what changed): Optimize within each namespace via batch ingest instead of attempting shared corpus reuse.
  - Tradeoffs considered: Shared global indexing would be faster but would violate benchmark semantics without reliable namespace filtering.
  - User approval: yes; user accepted the per-question dataset shape finding.

- 2026-05-03 Decision: Treat upstream OpenAI batch embedding as library-side work.
  - Trigger / new insight: Character Memory's OpenAI provider `bulk_generate_embeddings` loops one request per text.
  - Plan delta (what changed): Harness plan batches Character Memory `RememberDraft`s but does not modify the sibling library provider.
  - Tradeoffs considered: Editing the library could improve throughput further but belongs in a separate library PR.
  - User approval: pending if this plan is selected for execution.

- 2026-05-03 Decision: Resolve plan defaults for implementation.
  - Trigger / new insight: User accepted the recommended defaults for the open questions.
  - Plan delta (what changed): Plan status moved to approved; open questions closed.
  - Tradeoffs considered:
    - Batch write failure mode: fail the whole batch for benchmark correctness and implementation simplicity; no per-object fallback in this plan.
    - Batch sizing: use per-namespace phase batches now: all episodes in one batch, all observations in one batch; add chunk sizing only if backend limits force it.
    - Performance timings: keep phase/progress timing on stderr; do not add new structured result fields in this plan.
  - User approval: yes.

## Notes
- Risks:
  - Large all-observation batches may expose backend payload or timeout limits. If that happens, replan to chunk batches by a conservative fixed size while still avoiding per-object calls.
  - Batch failure loses per-object failure granularity. This is acceptable for initial benchmark viability but may need a diagnostic fallback later.
  - Existing real adapter namespace mutexes still block future concurrency; this plan avoids introducing parallel execution.
- Edge cases:
  - Empty episode or observation batches should be accepted as no-ops.
  - Observation batches must fail clearly if any observation references an unknown episode external ID.
  - Batch map updates should occur only after successful Character Memory persistence.
