# Plan: Timestamp Normalization And Cleanup Safety

- status: draft
- generated: 2026-05-02
- last_updated: 2026-05-02
- work_type: code

## Goal
- Prevent benchmark date formats from breaking live ingestion and make live backend cleanup possible without risking non-eval resources.

## Definition of Done
- LongMemEval-S and LoCoMo timestamp strings are normalized or safely tolerated before real adapter ingestion.
- Invalid timestamps fail with useful context or are intentionally dropped according to documented policy.
- Cleanup is fail-closed and restricted to configured eval-owned collection prefixes.
- Namespace reset behavior is documented for no-cleanup and cleanup-enabled modes.
- Tests cover timestamp normalization and cleanup target validation without requiring live services.

## Scope / Non-goals
- Scope:
  - Shared timestamp parsing/normalization policy.
  - Dataset date normalization where dataset-specific formats are known.
  - Live adapter cleanup target validation.
  - Documentation of cleanup behavior.
- Non-goals:
  - Native cleanup/reset API in Character Memory.
  - Broad Qdrant administration tooling.
  - Official benchmark export formats.
  - Raw dataset download or service orchestration.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-longmemeval/src/loader.rs`
  - `crates/cmem-eval-longmemeval/src/ingest.rs`
  - `crates/cmem-eval-locomo/src/loader.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
  - `configs/*.toml`
  - `README.md`
- Existing patterns or references:
  - Dataset-specific quirks belong in dataset crates.
  - Cleanup must be scoped to eval-owned benchmark collections.
  - Default validation must remain service-free.
  - Official LongMemEval cleaned files use date strings such as `2023/05/30 (Tue) 23:40`.
  - Official LoCoMo uses date strings such as `1:56 pm on 8 May, 2023`.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `https://github.com/xiaowu0162/LongMemEval`
  - `https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned`
  - `https://github.com/snap-research/locomo`
  - `https://raw.githubusercontent.com/snap-research/locomo/main/data/locomo10.json`

## Open Questions
- None.

## Assumptions
- A1: Timestamp normalization can be tested with local unit tests and does not require live services.
- A2: Cleanup must never delete a collection unless it matches an eval-owned prefix.
- A3: `reset_namespace` should keep isolation even when physical cleanup is disabled.
- A4: Cleanup should be eval-side for v0.1, using a direct backend cleanup path where the Character Memory public API lacks reset/delete.
- A5: Cleanup applies only to live backend resources created for benchmark namespaces and must never delete `runs/`, `reports/`, datasets, logs, or summaries.
- A6: Each benchmark run should use a run-scoped namespace/collection prefix so LongMemEval-S and LoCoMo can be run, cleaned, and rerun independently.

## Tasks

### Task_1: Define Timestamp Normalization Policy
- type: design
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `README.md`
  - `docs/coding-agent/plans/active/timestamp-normalization-cleanup-safety-plan.md`
- depends_on: []
- description: |
  Decide where timestamp tolerance lives and document the policy for unparsable benchmark dates.
- acceptance:
  - Policy states whether unparsable timestamps are dropped or fatal.
  - Policy separates dataset normalization from adapter parsing.
  - Policy includes enough context for error messages.
  - Plan is updated if the policy changes implementation ownership.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Policy recorded before implementation dispatch."

### Task_2: Normalize Benchmark Dates
- type: impl
- owns:
  - `crates/cmem-eval-longmemeval/src/loader.rs`
  - `crates/cmem-eval-longmemeval/src/ingest.rs`
  - `crates/cmem-eval-locomo/src/loader.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
- depends_on: [Task_1]
- description: |
  Convert known benchmark date formats into RFC3339 or `None` before building adapter inputs.
- acceptance:
  - LongMemEval `YYYY/MM/DD (Ddd) HH:MM` dates are covered by tests.
  - LoCoMo `h:mm am/pm on D Month, YYYY` dates are covered by tests.
  - Normalized dates produce RFC3339 strings with an explicit configured/default timezone policy.
  - Original raw JSON remains available for debugging.
  - Gold labels remain absent from adapter inputs.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-longmemeval && cargo test -p cmem-eval-locomo`

### Task_3: Harden Real Adapter Timestamp Handling
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_1, Task_2]
- description: |
  Make adapter timestamp parsing match the chosen policy and provide useful context on failures.
- acceptance:
  - Empty timestamps remain `None`.
  - RFC3339 timestamps still parse correctly.
  - Unparsable timestamps follow the documented policy.
  - Tests cover accepted and rejected/dropped timestamp values.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_4: Implement Cleanup Target Safety
- type: impl
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-runner/Cargo.toml`
- depends_on: []
- description: |
  Implement or explicitly gate cleanup for live benchmark-owned backend resources with prefix validation.
- acceptance:
  - Cleanup cannot target collections outside configured eval-owned prefix.
  - Cleanup disabled remains the safe default.
  - Cleanup can be run independently for one benchmark run without deleting other runs' backend state.
  - Cleanup preserves all evaluation result artifacts under `runs/` and `reports/`.
  - Cleanup behavior is testable without deleting live resources.
  - If direct Qdrant deletion is not implemented for the selected backend, config validation rejects `cleanup.enabled = true` with an actionable message.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core && cargo test -p cmem-eval-runner --features real-character-memory real_adapter`
  - kind: review
    required: true
    owner: reviewer
    detail: "Safety review for cleanup prefix checks and fail-closed behavior."

### Task_5: Cleanup And Timestamp Docs
- type: docs
- owns:
  - `README.md`
  - `configs/*.toml`
- depends_on: [Task_2, Task_3, Task_4]
- description: |
  Document timestamp behavior and cleanup modes for real benchmark runs.
- acceptance:
  - README explains timestamp tolerance/failure behavior.
  - README explains cleanup disabled vs enabled behavior.
  - Config examples include safe cleanup defaults.
  - Docs do not promise library-native reset/delete APIs.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Docs review for cleanup safety and timestamp behavior."

### Task_6: Timestamp And Cleanup Review
- type: review
- owns: []
- depends_on: [Task_5]
- description: |
  Review readiness against timestamp and cleanup findings.
- acceptance:
  - Reviewer confirms official dataset date formats will not break live ingestion unexpectedly.
  - Reviewer confirms cleanup cannot delete non-eval resources.
  - Required validations are evidenced or live-service parts are explicitly skipped.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-longmemeval && cargo test -p cmem-eval-locomo`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs timestamp/cleanup acceptance criteria."

## Task Waves
- Wave 1 (parallel): [Task_1, Task_4]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_5]
- Wave 5 (parallel): [Task_6]

## Rollback / Safety
- Cleanup must fail closed if prefix ownership cannot be proven.
- Keep cleanup disabled by default.
- Do not add native Character Memory cleanup APIs in this eval repo plan.

## Progress Log
- 2026-05-02 draft created.
  - Summary: Split timestamp tolerance and cleanup safety into a live-runtime safety plan.
  - Validation evidence: Review findings 5 and 6.
  - Notes: No UI scope.

## Decision Log
- 2026-05-02 Decision:
  - Trigger / new insight: Timestamp parsing and cleanup are both live-runtime safety issues but separate from adapter metadata mapping.
  - Plan delta (what changed): Created a focused timestamp/cleanup plan.
  - Tradeoffs considered: Cleanup may require either direct Qdrant client implementation or explicit rejection of enabled cleanup until supported.
  - User approval: pending.
- 2026-05-02 Decision:
  - Trigger / new insight: User needs to run LongMemEval-S and LoCoMo separately, preserve logs/results, and clean backend state for another run.
  - Plan delta (what changed): Resolved cleanup default to eval-side backend cleanup for v0.1, scoped by run-owned prefixes and explicitly excluding result artifacts.
  - Tradeoffs considered: Direct backend cleanup adds operational risk, so it must be fail-closed and prefix-restricted; waiting for a library-native cleanup API would block repeatable eval runs.
  - User approval: yes.

## Notes
- Risks:
  - Direct Qdrant cleanup adds dependency and operational risk.
  - Dataset timestamp formats may vary beyond local fixtures.
- Edge cases:
  - `query_date` may remain eval-side metadata if Character Memory has no temporal retrieval field.
  - LongMemEval oracle sessions may not be sorted and should preserve source ordering unless a task explicitly sorts by normalized timestamp.
  - LoCoMo session date keys are stored separately from session turn arrays as `session_N_date_time`.
