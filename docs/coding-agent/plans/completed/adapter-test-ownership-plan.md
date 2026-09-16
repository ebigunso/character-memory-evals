# Plan: Harness tests assert harness contracts; library semantics stay upstream

- status: completed (approved by the decider 2026-09-16; Tier D APPROVED 2026-09-16 at b0b4c37; merged 2026-09-17 by atomic stack squash merge, evals main 97c020e)
- generated: 2026-09-16
- last_updated: 2026-09-16
- work_type: code

## Goal
- The shared-core adapter suite asserts only what the harness owns (namespace naming, durable-store ownership, external-ID reattach, explicit lifecycle, admission before I/O), each contract in its own test and in both the embedded and the service-gated invocation; assertions that restate library behaviour or borrow library defaults are removed or re-anchored, and misplaced tests move to the module whose code they exercise.

## Definition of Done
- `reattach_with_external_ids` is split into named tests for external-ID round trip, missing-store reattach rejection per durable store, and fresh-reset isolation; each split test keeps both its embedded invocation and its `service-tests`-gated invocation; the correction-retry and suppression-check blocks are deleted with the upstream library tests that cover them cited and shown executing.
- No harness test asserts the library's vector tie-break order, its recall completeness telemetry, or its literal selectivity defaults; the override test asserts "absence preserves the library default" by comparing two constructions, never by a literal.
- The frozen write-surface drift guard lives in the continuity crate and `cmem-eval` no longer dev-depends on `cmem-eval-continuity`.
- The `fs_util` test module hosts the persist-retry and atomic-replace tests from both former locations (the adapter suite and the runner); the integrity-details test lives beside its siblings in `metrics.rs`; the duplicate deterministic-embedding test and the runner's Entity-root selectivity assertion are deleted.
- The retention-without-reason assertion stays at both the config-admission and the live-construction boundary (ruled 2026-09-16 under reviewer.md `coupled_config_invariants`).
- Embedded adapter suite and the `service-tests` suite pass with Qdrant up; workspace validation passes on the integrated branch with executed counts.

## Planner-added requirements
- None

## Scope / Non-goals
- Scope: `crates/cmem-eval/src/{adapter,metrics,fs_util}.rs` tests and `crates/cmem-eval/Cargo.toml`; `crates/cmem-eval-continuity` tests (receiving one test); `crates/cmem-eval-runner/src/{pipeline,frozen_embeddings}.rs` tests.
- Non-goals: production changes; cost and census cleanup (next plan); new gap tests (last plan).

## Design
- Chosen: split by contract, delete what the library already observes in a default run, move each test to the crate and module that owns the code it exercises. Structure: the inverted dev-dependency edge disappears; one owner per test. Evolution: a library retune of defaults, tie-break or telemetry no longer reds the harness (fits the pinned-harness stance in `docs/audits/2026-09-02-harness-design-value-audit.md`). Verification: a failure names one contract; service-mode coverage is preserved by construction. Operation: unchanged. Human: test location predicts the code under test. Safety: unchanged.
- Alternative: keep the mega-test and only delete the two upstream-covered blocks. Structure: unchanged. Verification: a failure anywhere in 300 lines still names nothing; the borrowed library anchors still red on retune.
- Why chosen: the split is the same edit set with a durable payoff.

## Compatibility stance
- surface: none; test code and a dev-dependency only.
- stance: preserve
- justification: no production surface changes.

## Context (workspace)
- Related files/areas: `crates/cmem-eval/src/adapter.rs:2787,2871,3295,3322,3520,3648,4421,4601,4688,3043-3044`; the reattach helper `:3743-4187` with its two callers, the embedded test at `:3697` and `#[cfg(feature = "service-tests")] service_mode_reattaches_with_external_ids` at `:3737-3740`; upstream blocks `:3892-3965` and `:4060-4073`; the service-gated reset test `service_mode_reset_preserves_sibling_namespace_durable_stores` at `:4195-4197`; `crates/cmem-eval/Cargo.toml:28` (dev-dependency on the continuity crate); `crates/cmem-eval-runner/src/pipeline.rs:2485`; `crates/cmem-eval-runner/src/frozen_embeddings.rs:177,205`.
- Existing patterns or references: upstream observers `../CharacterMemory/src/usecases/correct_forget.rs` retry tests (`:1455,1485,1564,1638,1694`, unconditional lib tests), `../CharacterMemory/src/memory.rs:493` (unconditional), `../CharacterMemory/tests/retrieval_guardrails_tests.rs:117,254` (skip-gated on the current library pin; run unconditionally once the library's integration-suite plan lands); reviewer.md "Adapter lifecycle changes" evidence row; worker.md `service-tests` requirement.
- Design record consulted and deviations from its acceptance: ADR-I-0004 (crate ownership) for the moves; the 2026-09-02 design-value audit for the pinned-harness stance. No deviation.
- Audit source: `../CharacterMemory/.agent-work/reviewer/test-suite-audit-2026-09-16.md` section F5; partition report D.

## Open Questions (max 3)
- None (Q1 on the retention duplicate was ruled 2026-09-16: keep both, see Decision Log).

## Assumptions
- A1: The library's integration-suite plan (`../CharacterMemory/docs/coding-agent/plans/active/integration-suite-embedded-default-plan.md`) has landed and the companion pin has moved to it, so `retrieval_guardrails_tests.rs:117,254` execute in a default run — source: this plan's sequencing rule (Task Waves); if the pin has not moved, Task_1 and Task_3 defer the two deletions that cite those tests (keeping the harness assertions) and record the deferral in the Decision Log.
- A2: The library exports the selectivity defaults or a settings constructor exposes them — source: unverified, checked by Task_1; the acceptance already requires the two-construction comparison, so the assumption only affects how the comparison is written.

## Tasks

### Task_1: Shared-core adapter suite split and re-anchored
- type: test
- owns:
  - crates/cmem-eval/**
- depends_on: []
- description: |
  Split the reattach helper into named tests parameterized over the embedded and service-gated invocations, delete its two upstream-covered blocks citing the upstream tests, re-anchor the singleton-budget, restart and override tests on harness contracts (override: two constructions compared, no literal), move the persist-retry test to `fs_util` and receive the two `fs_util` tests the runner deletes in Task_3 (`frozen_embeddings.rs:177,205`), move the integrity-details test to `metrics.rs`, delete the duplicate deterministic-embedding test and the cache-miss prose duplicates, remove the drift-guard test from this crate together with the continuity dev-dependency, and keep the retention duplicate per the ruling.
- acceptance:
  - `reattach_with_external_ids` no longer exists; its contracts are covered by at least three named tests, each under 120 lines, each with an embedded invocation and a `service-tests`-gated invocation named `service_mode_*` so the required service-suite filter keeps selecting it.
  - No test in the crate asserts a literal library default, a library tie-break order, or library completeness telemetry; the override test compares two constructions.
  - The `fs_util` test module hosts the persist-retry and atomic-replace tests from both former locations.
  - `Cargo.toml` has no dev-dependency on `cmem-eval-continuity`.
  - Each deleted or moved test is mapped to its destination or upstream observer by file and line.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval (executed count reported before and after); cargo test -p cmem-eval --features service-tests service_mode_ with Qdrant up (executed count reported; an unavailable service is a failure); rg -n 'cmem-eval-continuity' crates/cmem-eval/Cargo.toml (zero hits)"

### Task_2: Continuity crate receives the drift guard
- type: test
- owns:
  - crates/cmem-eval-continuity/**
- depends_on: []
- description: |
  Host the frozen write-surface drift guard where the runtime normalization it compares against lives, exercising the live adapter through the crate's existing dependency direction.
- acceptance:
  - One test asserts the frozen write surface matches the continuity runtime normalization; it compiles without any new dependency.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-continuity (executed count reported)"

### Task_3: Runner drops upstream and misplaced assertions
- type: test
- owns:
  - crates/cmem-eval-runner/**
- depends_on: []
- description: |
  Remove the Entity-root selectivity assertion from the vector-only restart test citing the upstream observer (subject to A1); delete the two `fs_util` tests from `frozen_embeddings.rs`, handing them over verbatim in the report for Task_1 to host.
- acceptance:
  - The runner has no assertion about which root types the library scores (or the deferral is recorded per A1).
  - `frozen_embeddings.rs` tests exercise only runner code; the two removed tests are handed over verbatim in the report.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner (executed count reported; Windows symlink exception noted)"

### Task_4: Independent review
- type: review
- owns: []
- depends_on: [Task_1, Task_2, Task_3]
- description: |
  The orchestrator integrates Wave 1, runs the workspace validation commands on the integrated branch and states the library pin. The reviewer, in a pinned worktree with the library pinned at that commit, executes (not merely locates) each cited upstream test with the evidence attached, confirms the reviewer.md adapter-lifecycle evidence row is satisfied by the split tests (fresh open, intended reattach, fresh-instance reset across every durable store, phase-local isolation, surviving sibling) in both embedded and service mode, confirms the dev-dependency is gone and the `fs_util` handoff landed, and runs the workspace suite plus the `service-tests` suite with Qdrant up.
- acceptance:
  - Reviewer status is APPROVED with the upstream execution evidence, the lifecycle-evidence mapping and the service-suite census attached.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (executed counts) on the integrated branch; library pin stated"
  - kind: review
    required: true
    owner: reviewer
    detail: "Pinned worktree; upstream tests executed at the library pin (REQUIRE_QDRANT_TESTS=1 with Qdrant up if the pin predates the library's integration-suite plan); lifecycle-evidence mapping; cargo test -p cmem-eval --features service-tests service_mode_ with Qdrant up and endpoint stated; cargo test --workspace"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1, Task_2, Task_3]
- Wave 2 (parallel): [Task_4]

Sequencing: this plan is dispatched after the library's integration-suite plan merges and the companion pin moves to it (A1); if it must go earlier, the two deletions citing `retrieval_guardrails_tests.rs` are deferred and recorded. Parallel tasks run in separate worktrees so Cargo commands never share a target directory mid-edit; per-task validation is crate-scoped and the orchestrator runs the workspace-wide commands on the integrated branch after each wave.

## Rollback / Safety
- Single branch `feature/2026-09-16/adapter-test-ownership` stacked on the typed-admission branch; test code and a dev-dependency removal; revertible as one commit.

## Progress Log (append-only)

- 2026-09-16 Plan drafted from the test-suite audit (F5; partition report D).
- 2026-09-16 Confirmation pass: service-gated invocations must keep the `service_mode_` prefix the required filter selects; A1 fallback wording aligned with the Task Waves deferral rule.
- 2026-09-16 Reviewer pass (Tier A): the service-gated caller of the reattach helper and the service-gated reset test added to Context and acceptance; `service-tests` suite made a required check for worker and reviewer; upstream citations that skip by default on the current pin made an execution check and the plan sequenced after the library's integration-suite plan; `fs_util` handoff made explicit on both sides; two-construction comparison promoted into acceptance; workspace-wide validation hoisted to the orchestrator.

- 2026-09-16 Wave 1 completed: [Task_1, Task_2, Task_3] (e280b2a, 9d01955, b0b4c37 integrated; tip b0b4c37)
  - Summary: reattach helper split into six named lifecycle tests with embedded and service_mode_ invocations; correction-retry block split per assertion (harness-owned outputs kept); re-anchors on budget, completeness and two-construction defaults; fs_util, metrics and drift-guard moves; continuity dev-dependency removed; suppression block and runner selectivity assertion deferred per the sequencing clause.
  - Validation evidence (orchestrator at b0b4c37): fmt clean (run from the short alias), clippy clean, workspace tests cmem-eval 120, benchmark-convert 10, continuity 73, locomo 46 (1 ignored), longmemeval 34 (1 ignored), runner 50 passed plus the known symlink exception; worker service suite 7 passed with Qdrant up.
- 2026-09-16 Wave 2 completed: [Task_4] (APPROVED at b0b4c37, no findings)
  - Validation evidence (reviewer, pinned worktree): workspace 333 passed, embedded 120, upstream retry lib tests executed on library 4a00303, service suite 7 passed against Qdrant v1.19.0 at http://127.0.0.1:6334 (collections empty before and after), fmt and clippy clean, smoke diff zero, fixture SHA unchanged.
  - Notes: from a shell that resolves the short-path junction to the .worktrees path, cargo metadata rejects the library crate as a stray workspace member; run cargo from the C:/w alias.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-16 Decision: dispatched before the library's integration-suite pull request (character-memory PR 96) merged, under the plan's own deferral clause: the two deletions that cite tests/retrieval_guardrails_tests.rs (the adapter suppression-check block and the runner's Entity-root selectivity assertion) stay until the companion pin moves to a library commit where that file runs unconditionally; a follow-up removes them then. Trigger: decider directive to progress all eight plans stacked; the library pull request awaits merge approval.

- 2026-09-16 Decision: library semantics covered upstream are deleted here rather than kept in parity, but only once the upstream observer executes in a default run. Trigger: audit F5; reviewer finding that two cited upstream tests skip with Qdrant down on the current pin. Decider approval: plan accepted 2026-09-16; merge approval pending.
- 2026-09-16 Decision: the retention-without-reason assertion is kept at both boundaries. Trigger: reviewer.md `coupled_config_invariants` requires validation at configuration admission and at the production-reachable live consumer. Decider approval: plan accepted 2026-09-16; merge approval pending.
- 2026-09-17 Closeout: the companion pin is library main 30ebdb6, where `tests/retrieval_guardrails_tests.rs` runs unconditionally, so the two deferred deletions land: the adapter's `suppression_survives_reattach` test pair (its exclusion assertion is `restart_safe_retrieval_excludes_suppressed_and_superseded_memories` upstream; the reattach path stays covered by the lifecycle tests) and the runner's Entity-root selectivity assertion (`selectivity_telemetry_and_fanout_override_bound_entity_root_expansion` upstream).

## Notes
- Risks: Task_1 and Task_3 move tests across crates in one wave; the orchestrator runs the workspace suite after the wave, not per task, and checks the handoff against both reports.
- Edge cases: the `service-tests` suite needs Qdrant up on the worker's machine; an unavailable service is a failure, not a skip (worker.md).
