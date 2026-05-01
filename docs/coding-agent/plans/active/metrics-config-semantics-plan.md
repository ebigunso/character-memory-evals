# Plan: Metrics And Config Semantics

- status: draft
- generated: 2026-05-02
- last_updated: 2026-05-02
- work_type: code

## Goal
- Make metrics and config behavior truthful: every committed config knob is either honored, rejected, or documented as intentionally unsupported.

## Definition of Done
- Integrity metrics are emitted consistently, including explicit `null` for unsupported states.
- Metric `k` values come from config or unsupported config values fail clearly.
- `store_gold_labels` cannot enable ingestion of gold labels.
- Config flags that remain no-ops are removed, rejected, or documented in outputs.
- Config tests cover TOML compatibility and prohibited semantics.

## Scope / Non-goals
- Scope:
  - Shared config schema and validation.
  - Metrics aggregation and integrity metric semantics.
  - Benchmark config TOML alignment.
  - Runner use of config-owned metric `k` values.
- Non-goals:
  - LoCoMo caption parsing and text indexing; see `locomo-dataset-ingestion-fidelity-plan.md`.
  - Live adapter cleanup; see `timestamp-normalization-cleanup-safety-plan.md`.
  - Official export formats; see `official-benchmark-export-formats-plan.md`.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-longmemeval/src/scoring.rs`
  - `crates/cmem-eval-locomo/src/scoring.rs`
  - `configs/*.toml`
- Existing patterns or references:
  - Gold evidence labels are scorer-only.
  - Result summaries aggregate numeric metric keys from rows.
  - Unsupported integrity states should be `null`, not silently zero.
  - LongMemEval turn-level evidence comes from optional `has_answer: true` fields on turns.
  - LoCoMo evidence comes from QA `evidence` dialog IDs and official QA `category` values may be numeric.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `https://github.com/xiaowu0162/LongMemEval`
  - `https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned`
  - `https://github.com/snap-research/locomo`

## Open Questions
- None.

## Assumptions
- A1: It is acceptable to change scorer function signatures to accept typed metric settings.
- A2: The repo should prefer failing fast over running with misleading config.
- A3: Gold-label storage remains prohibited regardless of TOML value.
- A4: Unsupported config keys should fail hard by default before ingestion starts. Warnings are reserved only for explicitly documented, non-behavioral metadata.
- A5: Run outputs and summaries are evaluation artifacts and must be preserved across backend cleanup.

## Tasks

### Task_1: Type And Validate Metric Config
- type: impl
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `configs/*.toml`
- depends_on: []
- description: |
  Replace opaque metrics config with typed metric-k settings for LongMemEval-S and LoCoMo while preserving existing TOML intent.
- acceptance:
  - Config can express session, turn, and dialog `k` values.
  - Existing config files deserialize into typed metric settings.
  - Empty or invalid `k` arrays fail clearly.
  - Dataset-irrelevant metric settings are rejected or ignored with explicit output warnings.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core`

### Task_2: Wire Configured Metrics Into Scorers
- type: impl
- owns:
  - `crates/cmem-eval-longmemeval/src/scoring.rs`
  - `crates/cmem-eval-locomo/src/scoring.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_1]
- description: |
  Stop hardcoding metric `k` values in scorers and pass the typed config values from the runner.
- acceptance:
  - LongMemEval session and turn metrics use configured `ks_session` and `ks_turn`.
  - LoCoMo dialog and session metrics use configured `ks_dialog` and `ks_session`.
  - Empty-gold rows still skip retrieval metrics and keep integrity metrics.
  - Unit tests prove non-default `k` values affect emitted metric keys.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-longmemeval && cargo test -p cmem-eval-locomo`

### Task_3: Enforce Gold-Label And No-Op Config Policy
- type: impl
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `configs/*.toml`
- depends_on: [Task_1]
- description: |
  Make prohibited or unsupported config fields impossible to mistake for active benchmark behavior.
- acceptance:
  - `store_gold_labels = true` fails validation before ingestion.
  - Unsupported retrieval flags are either mapped into `RetrieveInput` or explicitly reported as unsupported.
  - Config validation runs once before dataset ingestion starts.
  - Output summary preserves config validation warnings when warnings are allowed.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core && cargo test -p cmem-eval-runner`

### Task_4: Integrity Metrics Semantics
- type: impl
- owns:
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_3]
- description: |
  Ensure every row and summary communicates integrity metric support accurately.
- acceptance:
  - Required integrity keys are present on every row.
  - Unsupported integrity states are `null` per row and are represented in summary metadata.
  - Observed integrity counts are numeric and aggregate normally.
  - Tests cover numeric and `null` integrity rows.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core`
  - kind: review
    required: true
    owner: reviewer
    detail: "Review integrity metric output against guide section 16."

### Task_5: Config And Metrics Review
- type: review
- owns: []
- depends_on: [Task_2, Task_4]
- description: |
  Review the shared config/metric changes against the prior review findings.
- acceptance:
  - Reviewer confirms metric config no longer silently lies.
  - Reviewer confirms gold labels remain scorer-only.
  - Reviewer confirms integrity metrics are emitted even when unsupported.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-core`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-longmemeval && cargo test -p cmem-eval-locomo`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs config/metrics acceptance criteria."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4]
- Wave 4 (parallel): [Task_5]

## Rollback / Safety
- Config validation should fail before any live backend writes occur.
- Do not introduce any pathway that stores gold labels in adapter inputs or metadata.

## Progress Log
- 2026-05-02 draft created.
  - Summary: Split shared metrics/config work from dataset-specific and live-runtime plans.
  - Validation evidence: Review findings 2, 3, and 8.
  - Notes: No UI scope.

## Decision Log
- 2026-05-02 Decision:
  - Trigger / new insight: Config no-ops and integrity metrics affect multiple crates and need a shared plan.
  - Plan delta (what changed): Created a dedicated config/metrics plan.
  - Tradeoffs considered: Failing unsupported config is more honest than silently preserving dead TOML knobs.
  - User approval: pending.
- 2026-05-02 Decision:
  - Trigger / new insight: User wants separate eval runs whose artifacts can be preserved and analyzed later.
  - Plan delta (what changed): Resolved unsupported config policy to fail hard by default; run artifacts remain preserved and are never part of cleanup.
  - Tradeoffs considered: Hard failures may require config updates sooner, but they prevent invalid benchmark runs from producing misleading artifacts.
  - User approval: yes.

## Notes
- Risks:
  - Typed metric config may require small scorer API changes across dataset crates.
  - Summary format for `null` integrity states must remain machine-readable.
- Edge cases:
  - Abstention/no-evidence rows should not get retrieval metrics, but should still get integrity metrics.
