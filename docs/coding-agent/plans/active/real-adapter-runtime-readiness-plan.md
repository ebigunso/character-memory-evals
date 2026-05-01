# Plan: Real Adapter Runtime Readiness

- status: draft
- generated: 2026-05-02
- last_updated: 2026-05-02
- work_type: code

## Goal
- Make the live Character Memory adapter compile-ready, actionably gated, and safe as the default benchmark runtime.

## Definition of Done
- `--adapter real` and omitted adapter both use the live Character Memory adapter when `real-character-memory` is enabled.
- Feature-disabled live runs fail with a clear rebuild message.
- Mock benchmark runs still require `--allow-mock-benchmark` and mark output as mock/smoke.
- Real adapter retrieval maps external IDs, ranks, scores, rationale, text, and context counts into eval rows.
- Live-feature compile and runner tests complete without lockfile churn.

## Scope / Non-goals
- Scope:
  - Runner CLI adapter selection and validation.
  - Feature-gated live adapter construction.
  - Real adapter retrieval flattening and metadata preservation.
  - Documentation for live/default and mock/smoke commands.
- Non-goals:
  - Qdrant cleanup implementation; see `timestamp-normalization-cleanup-safety-plan.md`.
  - Official benchmark export formats; see `official-benchmark-export-formats-plan.md`.
  - Changing Character Memory library APIs.
  - Making default validation depend on live services.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-runner/Cargo.toml`
  - `crates/cmem-eval-core/src/results.rs`
  - `README.md`
- Existing patterns or references:
  - Benchmark CLI runs default live.
  - Mock benchmark runs require explicit opt-in and output metadata.
  - `MockMemoryAdapter` remains for deterministic tests and smoke validation.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`
  - `docs/coding-agent/lessons.md`

## Open Questions
- None blocking.

## Assumptions
- A1: The Character Memory public facade remains the target API boundary.
- A2: Live adapter compile may require the sibling `CharacterMemory` checkout to be visible outside the sandbox.
- A3: Service-dependent live smoke tests must be ignored or skipped unless explicitly enabled.

## Tasks

### Task_1: Stabilize Live Adapter Build Surface
- type: impl
- owns:
  - `crates/cmem-eval-runner/Cargo.toml`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: []
- description: |
  Confirm and fix the feature-gated real adapter build path so live-default commands compile when the feature is enabled and fail actionably when it is not.
- acceptance:
  - `real-character-memory` feature pulls in all needed dependencies without causing unexpected `Cargo.lock` updates during normal checks.
  - Feature-disabled live default returns the existing actionable error.
  - `--adapter mock` still requires `--allow-mock-benchmark`.
  - Tests cover live/default selection and mock guard behavior.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner`
  - kind: command
    required: true
    owner: worker
    detail: `cargo check -p cmem-eval-runner --features real-character-memory`

### Task_2: Verify Real Retrieval Mapping
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-core/src/results.rs`
- depends_on: [Task_1]
- description: |
  Tighten real retrieval flattening so eval outputs preserve the metadata required by scoring and downstream exports.
- acceptance:
  - Retrieved episodes expose eval external IDs when available.
  - Retrieved observations expose eval external IDs and parent episode external IDs.
  - Ranks and scores are deterministic when trace data is missing.
  - Context text counts match the rendered context text.
  - Unsupported derived-memory provenance remains explicit rather than silently claimed.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`
  - kind: review
    required: true
    owner: reviewer
    detail: "Review real adapter mapping against external-ID round-trip requirements."

### Task_3: Document Live Runtime Requirements
- type: docs
- owns:
  - `README.md`
  - `scripts/README.md`
- depends_on: [Task_1, Task_2]
- description: |
  Document the exact live run prerequisites and keep mock smoke commands visibly separate from real benchmark commands.
- acceptance:
  - README shows `--features real-character-memory` for live LongMemEval-S and LoCoMo commands.
  - README lists required Qdrant/OpenAI environment variables and deterministic embedding alternative.
  - README warns that mock output is smoke/test-only.
  - Docs do not imply live services are required for default CI validation.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Docs review for live/default vs mock/smoke clarity."

### Task_4: Runtime Readiness Review
- type: review
- owns: []
- depends_on: [Task_1, Task_2, Task_3]
- description: |
  Review the finished runtime changes against the prior P1 real-adapter finding and live-default rule.
- acceptance:
  - Reviewer confirms the hard-stub finding is obsolete.
  - Reviewer confirms live/default and mock/smoke behavior cannot be confused.
  - Required validations are evidenced or explicitly blocked by local service availability.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo check -p cmem-eval-runner --features real-character-memory`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs plan acceptance criteria."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]

## Rollback / Safety
- Disable the `real-character-memory` feature to return to service-free checks.
- Keep mock adapter behavior available only through the explicit guard.
- Do not modify the sibling Character Memory library in this plan.

## Progress Log
- 2026-05-02 draft created.
  - Summary: Split live adapter/runtime readiness from cleanup, export, config, and LoCoMo fidelity work.
  - Validation evidence: Review findings 1 and related readiness review.
  - Notes: No UI scope.

## Decision Log
- 2026-05-02 Decision:
  - Trigger / new insight: Runtime readiness is distinct from backend cleanup and official export work.
  - Plan delta (what changed): Created a focused live adapter/runtime plan.
  - Tradeoffs considered: Keeping this narrow avoids mixing live compilation with export schema work.
  - User approval: pending.

## Notes
- Risks:
  - Local `cargo check --features real-character-memory` may be slow due to sibling Character Memory dependencies.
  - Lockfile churn from optional dependency resolution should be resolved, not ignored.
- Edge cases:
  - Feature-disabled default live run should fail before any mock fallback can occur.
