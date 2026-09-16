# Plan: Harness safety and identity contracts with no observer get one

- status: completed (approved by the decider 2026-09-16; Tier D APPROVED 2026-09-17 at 9083893; merged 2026-09-17 by atomic stack squash merge, evals main 97c020e)
- generated: 2026-09-16
- last_updated: 2026-09-17
- work_type: code

## Goal
- The harness's own standing rules and identity contracts each have a test that fails when they break: gold labels and speaker attribution never reach adapter metadata or stores, dataset ids that are unsafe in paths and collection names are rejected, operation ids cannot collide by concatenation, LoCoMo scoring produces the hand-computed value, sealing rejects a run directory without exactly one row artifact, fixture admission tests stand on a hand-authored scenario, and persist-with-retry exhaustion is observed.

## Definition of Done
- Each gap in the audit's F7 evals list has one test at the boundary that observes it, named in the report with the defect it would catch. Two of the gaps are missing tests over behaviour that is already implemented (`DatasetId::new` already rejects the named inputs; `deterministic_operation_id` already frames by length), so those tests are expected to pass on main.
- Any new test that fails on current main is committed as `#[ignore = "pending ruling: <observed behaviour>"]`, reported as a defect with the observed behaviour, and the item pauses for the decider's ruling; workspace validation passes with that test ignored and the ignored count stated; this PR joins the stack only with zero pending-ruling ignores.
- Workspace validation passes on the integrated branch with executed counts.

## Planner-added requirements
- None

## Scope / Non-goals
- Scope: new tests in `crates/cmem-eval/src/{adapter,runtime,outcome,fs_util}.rs`, `crates/cmem-eval-locomo/src/scoring.rs` or its tests, `crates/cmem-eval-runner/src/seal.rs`, `crates/cmem-eval-continuity/src/fixture.rs` tests; no production change in this plan (a ruled fix becomes a separate plan).
- Non-goals: re-anchoring existing tests (earlier plans); new metrics or datasets; production fixes.

## Design
- Chosen: one test per gap, sentinel-based where the contract is an absence (no gold bytes in stores), hand-computed where the contract is a value, hand-authored minimal input where the contract is admission. Structure: no new fixtures beyond a small scenario builder in the continuity test module; the `fs_util` test module created by the adapter-ownership plan receives the new cases. Evolution: the fixture builder decouples admission tests from generator ordering. Verification: each failure names one contract. Operation: unchanged. Human: names predict contracts. Safety: the sentinel test directly guards the repo's gold-label rule (common.md "Repo Safety / Boundaries").
- Alternative: assert the gold-label rule by code review only, as today. Verification: the leak path is a silently dropped field nobody would notice being wired; review does not observe it.
- Why chosen: the rule is the repository's own standing safety boundary and deserves a failing observer.

## Compatibility stance
- surface: none; tests only.
- stance: preserve
- justification: no production change in this plan; a ruled fix gets its own plan and stance.

## Context (workspace)
- Related files/areas: `crates/cmem-eval/src/adapter.rs` (EpisodeInput.metadata, ObservationInput.speaker, DerivedMemoryInput.metadata accepted and dropped); `crates/cmem-eval/src/runtime.rs:10` (`DatasetId::new`, lowercase ASCII, digit, underscore and hyphen whitelist already implemented; the gap is the missing test); `crates/cmem-eval/src/outcome.rs:28` (`deterministic_operation_id`, length-prefixed framing already implemented; the gap is the missing test); `crates/cmem-eval/src/fs_util.rs:43` (`persist_with_retry`, `PERSIST_ATTEMPTS`; test module created by the adapter-ownership plan); `crates/cmem-eval-locomo/src/scoring.rs:49` (`uses_configured_metric_ks`, key presence only; `pub fn score` exists) vs `crates/cmem-eval-longmemeval/tests/repeated_sessions.rs:100` (hand-computed model); `crates/cmem-eval-runner/src/seal.rs:56-61` (exactly one `.jsonl`, matching README.md:170); `crates/cmem-eval-continuity/src/fixture.rs:1024-1885` (admission tests index generated events positionally).
- Existing patterns or references: `file_contains` helper in the adapter tests for store-bytes assertions; rules common.md "Gold evidence labels must be used only for scoring and must not be copied into EpisodeInput, ObservationInput, or adapter metadata".
- Design record consulted and deviations from its acceptance: ADR-I-0005 (sealed evidence is bytes-by-hash and live readers read only the current shape; supersedes ADR-I-0002) for the seal test's expectations. No deviation.
- Audit source: `../CharacterMemory/.agent-work/reviewer/test-suite-audit-2026-09-16.md` section F7, evals list.

## Open Questions (max 3)
- Q1: If the sentinel test finds speaker or metadata bytes persisted anywhere, is the ruling to strip at the adapter boundary or to reject the input? Task_1 reports the observed path; the decider rules.

## Assumptions
- A1: This plan is sequenced after the typed-admission plan lands on the stack, so Task_4's rebased admission tests match the typed error, never `is_err()` — source: stack order (Task Waves); there is no fallback, since an `is_err()` anchor would reintroduce what the typed-admission plan removes.
- A2: LoCoMo scoring exposes `score` taking retrieved ids and gold and returning the metric map — source: `crates/cmem-eval-locomo/src/scoring.rs` (`pub fn score`, verified at plan review).

## Tasks

### Task_1: Shared core identity and safety gaps
- type: test
- owns:
  - crates/cmem-eval/**
- depends_on: []
- description: |
  Add tests that: a remembered episode, observation and derived memory carrying sentinel metadata and speaker bytes leave no sentinel in any namespace store file or retrieved context; `DatasetId::new` rejects path-unsafe, upper-case, empty and slash-bearing ids and admits the two shipped ids (expected to pass); `deterministic_operation_id` distinguishes `("a","bc")` from `("ab","c")` and is stable (expected to pass); `persist_with_retry` fails after exhausting attempts and does not retry a non-permission error kind, added to the `fs_util` test module the adapter-ownership plan created. A test that fails on main is committed ignored with the pending-ruling reason and reported with the observed path.
- acceptance:
  - Four tests exist, each naming its contract; the sentinel test asserts absence over every durable store the adapter creates plus the retrieved context.
  - Any failing test is committed `#[ignore = "pending ruling: ..."]`, reported with observed behaviour, and marked pending ruling.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval (executed count reported; ignored tests listed with reasons)"

### Task_2: LoCoMo hand-computed scoring value
- type: test
- owns:
  - crates/cmem-eval-locomo/**
- depends_on: []
- description: |
  Add one scoring test with gold at rank 1 versus missed, asserting hand-computed recall and nDCG values through the dialog-to-session projection, modelled on the LongMemEval scoring test.
- acceptance:
  - The test asserts numeric values computed by hand in the test body, not key presence.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo (executed count reported)"

### Task_3: Sealing rejects the wrong artifact count
- type: test
- owns:
  - crates/cmem-eval-runner/src/seal.rs
- depends_on: []
- description: |
  Add tests that `seal()` rejects a run directory with zero row artifacts and one with two, without writing a manifest.
- acceptance:
  - Two tests assert the rejection (an error result; a typed seal error is declined in the Decision Log) and that no manifest file was created.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner seal (executed count reported)"

### Task_4: Fixture admission tests stand on a hand-authored scenario
- type: test
- owns:
  - crates/cmem-eval-continuity/**
- depends_on: []
- description: |
  Add a minimal scenario builder in the fixture test module and rebase the admission tests on it so a generator reorder cannot make them panic on setup; the builder produces one valid scenario that every admission test mutates in one field; every rebased test matches the typed admission error introduced by the typed-admission plan.
- acceptance:
  - No admission test indexes into `generate_fixture_set(SEED)`; each mutates the builder's output.
  - The set of rejection cases is unchanged (census before and after) and every case asserts the typed variant, location and field.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-continuity (executed count reported; rejection-case census before and after)"

### Task_5: Rulings on revealed defects
- type: design
- owns:
  - docs/coding-agent/plans/active/harness-contract-gaps-plan.md
- depends_on: [Task_1, Task_2, Task_3]
- description: |
  For each test committed pending ruling, the orchestrator records the design intent, the alternative rejected and the ruling in the Decision Log, and names the follow-up plan that will carry any ruled-in production fix with its own `owns` and compatibility stance. Task_6 then applies the ruling to the ignored test.
- acceptance:
  - Every pending-ruling test has a Decision Log ruling with blast radius and, for each ruled-in fix, the follow-up plan path.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Decision Log carries one ruling per revealed defect; no production change in this plan"

### Task_6: Pending-ruling ignores resolved per the rulings
- type: test
- owns:
  - crates/cmem-eval/**
  - crates/cmem-eval-locomo/**
  - crates/cmem-eval-runner/src/seal.rs
- depends_on: [Task_5]
- description: |
  For each test committed `#[ignore = "pending ruling: ..."]`, apply the recorded ruling: delete the test when the follow-up plan will own the contract with its fix, or reshape it to assert the accepted behaviour. No production code changes.
- acceptance:
  - Zero `pending ruling` ignores remain in the workspace; each removal or reshape cites its Decision Log ruling.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test --workspace (executed and ignored counts reported); rg -n 'pending ruling' crates (zero hits)"

### Task_7: Independent review
- type: review
- owns: []
- depends_on: [Task_4, Task_6]
- description: |
  The orchestrator integrates the waves and runs the workspace validation commands on the integrated branch. The reviewer, in a pinned worktree, confirms each gap has an observer (suggested technique: plant a defect, watch the test fail, restore with a pre-edit hash check, for the sentinel and operation-id tests), the rejection-case census is unchanged, the ignored count is as stated, and the workspace suite passes.
- acceptance:
  - Reviewer status is APPROVED with observer evidence for the two named tests and the census attached.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (executed and ignored counts) on the integrated branch"
  - kind: review
    required: true
    owner: reviewer
    detail: "Pinned worktree; diff review against acceptance; observer evidence on two tests; census comparison; cargo test --workspace"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1, Task_2, Task_3, Task_4]
- Wave 2 (parallel): [Task_5]
- Wave 3 (parallel): [Task_6]
- Wave 4 (parallel): [Task_7]

Sequencing: this plan is dispatched after the typed-admission plan and the adapter-ownership plan land on the stack (A1 and the `fs_util` module origin). Parallel tasks run in separate worktrees so Cargo commands never share a target directory mid-edit; per-task validation is crate-scoped and the orchestrator runs the workspace-wide commands on the integrated branch after each wave.

## Rollback / Safety
- Single branch `feature/2026-09-16/harness-contract-gaps` stacked on the cost-cleanup branch; tests only. This PR is linked into the stack only after every revealed defect has a ruling and no pending-ruling ignore remains (2026-09-15 stack-merge incident).

## Progress Log (append-only)

- 2026-09-16 Plan drafted from the test-suite audit (F7, evals list).
- 2026-09-16 Confirmation pass: the ignore removal moved out of the ruling task into a worker-owned Task_6 with crate `owns`; review renumbered to Task_7.
- 2026-09-16 Reviewer pass (Tier A): ADR-I-0005 cited instead of its superseded predecessor; pending-ruling tests committed ignored with the reason so the stacked branch stays green; ruled fixes become a separate plan; A1 made a sequencing rule with no `is_err()` fallback; `DatasetId` and operation-id gaps stated as missing tests over implemented behaviour; `outcome.rs:28`; `fs_util` module origin named; workspace-wide validation hoisted to the orchestrator.

- 2026-09-17 Wave 1 completed: [Task_1 fde1b03, Task_2 570888c, Task_3 f1a5101, Task_4 f94a18c] (integrated tip f94a18c, rebased onto the cost-cleanup tip a7dd8f5)
  - Summary: sentinel gold-label and speaker leak test over every namespace store file and the retrieved context; DatasetId::new rejection and admission; deterministic_operation_id framing and stability; persist_with_retry exhaustion and non-retryable kind (fs_util test module); LoCoMo hand-computed dialog and projected-session recall and nDCG for a rank-one hit versus a miss; seal() refuses zero and two row artifacts without writing either manifest; all 31 fixture admission test functions stand on one hand-authored scenario with named event lookups (rejection census 53 to 53, 38 Admission and 15 Shape observers retained). Every new test passes on main: zero pending-ruling ignores.
  - Validation evidence: worker gates per report (cmem-eval 108 passed; locomo 47; runner seal filter 5; continuity suite before and after equal); orchestrator workspace run at f94a18c recorded in the Wave 4 entry.
- 2026-09-17 Wave 2 completed: [Task_5] no test failed on main, so no ruling was needed and Task_6 (ignore removal) is a no-op.

- 2026-09-17 Wave 4 completed: [Task_7] (APPROVED at 9083893; code identical after the rebase onto the cost-cleanup log commit bb59a58)
  - Validation evidence (reviewer, pinned worktree): fmt and clippy clean; workspace 325 passed plus the known symlink exception, ignored 0; runner integration 1 passed; smoke diff all zero; all seven gaps have observers; fixture census 53 to 53 (38 admission, 15 shape); plant-fail-restore evidence on the sentinel and operation-id tests in a scratch copy with pre and post hashes. Orchestrator at f94a18c: fmt and clippy clean, workspace green except the known exception, ignored 0.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-17 Decision: branch cut from the cost-cleanup branch after three of its four Wave 1 tasks integrated (Task_4, dataset crates and converter, still running); this branch rebases onto the cost-cleanup reviewed tip before its own review. The typed-admission and adapter-ownership plans are both Tier D approved beneath, satisfying A1 and the fs_util module origin.

- 2026-09-17 Decision (Task_3 alert): seal() has no typed cardinality error (anyhow ensure); the two tests assert refusal plus the absence of both manifests and the evidence root, without message text, and a typed seal error is declined for this plan: seal is the on-demand sealing command, not a library-facing validator, and the protected contract is refusal without side effect.
- 2026-09-17 Note (Task_1): DatasetId::new already rejected the named inputs and deterministic_operation_id already framed by length, as the plan expected; the gap was the missing observer in both cases.
- 2026-09-16 Decision: a new test that fails on main is committed ignored with the pending-ruling reason and pauses for a ruling rather than being fixed by the worker; production fixes get their own plan. Trigger: orchestrator design-altitude rule; 2026-09-15 stack-merge incident. Decider approval: plan accepted 2026-09-16; merge approval pending.

## Notes
- Risks: the sentinel test is the one most likely to fail on main; Q1 prepares the ruling.
- Edge cases: `persist_with_retry` timing on Windows permission errors; the test injects the error kind rather than relying on the filesystem.
