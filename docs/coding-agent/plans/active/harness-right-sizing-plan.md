# Plan: Harness Right-Sizing (strictness follows the claim)

- status: approved
- generated: 2026-09-02
- last_updated: 2026-09-02
- work_type: mixed

## Goal
- Shrink the evaluation harness to a lab-notebook core sized to the decisions it informs: a deterministic runner, rows built on the library's public types, a diff tool, continuity metrics, and an on-demand sealing step, under the principle ruled 2026-09-02 that strictness follows the claim, not the code.

## Definition of Done
- Tiers 1 and 2 (inner loop; tuning and regression decisions) are served by `run` plus `diff` with no sealing, no schema versioning, and no two-run ritual.
- Tier 3 (durable claims) is served by one `seal` command that hashes a run directory, records the harness and library commits, and copies the traces and report into a tracked evidence directory; the findings register cites seal hashes from then on.
- Every retained feature names the decision it informs; the deletions listed in this plan are gone with zero-hit censuses.
- Process rules that encoded tier-3 rigor as a default are retired or narrowed as listed.
- The existing hash-cited evidence stays byte-identical and readable by hash; the current reference evidence is promoted into tracked storage.

## Scope / Non-goals
- Scope: the nine library-independent steps below now; the library-dependent steps after the library's embedded vector-store phase lands (tracked here, sequenced against that phase's merges).
- Non-goals: any change to what the library measures or decides (that is recorded in the library repository); changes to sealed bytes; new benchmark features.

## Context (workspace)
- Basis: the design-value audit at `docs/audits/2026-09-02-harness-design-value-audit.md`; verdict tally 14 delete, 18 oversized, 5 demote to diagnostic, 3 seal-on-demand, 12 earn their place, over ~21,300 production and ~13,000 test lines across seven crates.
- Prerequisite: the evidence-integrity fixes (PR #22) merged, since they already remove the shared graph-path fallback and the unhonored enrichment knobs and fix batch-outcome duplication.
- Sealed bytes that never change: every existing line of `reports/v0-1-5-findings-register.md` (new information is only ever appended as a dated addendum, the precedent set on 2026-07-29); the continuity fixtures and committed embedding manifests and stores it cites by hash; the seven hash-cited configs. Deleting a config key that makes a cited config unparseable is acceptable (old artifacts are old); re-running a cited config is new evidence under a new hash.
- Constraint: the frozen-store file shape is kept as plain data while its guard logic is deleted, because the committed stores cost money to regenerate.
- Constraint: no run artifact is tracked today; the 885 MB of hash-cited evidence exists on one machine, so evidence promotion is the first sealing act.

## Open Questions (max 3)
- Q1: whether the conventional-benchmark pipelines (two dataset loaders and the converter) are re-audited at the default-flip decision that names them or earlier; default per the audit: at that decision.

## Assumptions
- A1: The library's outcome and telemetry types keep deriving serde, so rows can embed them directly once the harness is pinned to a library commit.
- A2: Mock deletion waits until the library runs service-free in CI (its embedded vector-store phase), or until CI gains a vector-service container.

## Tasks

### Task_1: Retire the rules that encode tier-3 rigor as a default
- type: docs
- owns:
  - docs/coding-agent/rules/common.md
  - docs/coding-agent/rules/reviewer.md
  - docs/coding-agent/rules/worker.md
  - docs/coding-agent/rules/orchestrator.md
  - docs/coding-agent/lessons.md
  - docs/coding-agent/plans/active/*.md
  - docs/decisions/**
- depends_on: []
- description: |
  Apply section 5 of the audit: retire the strict-reader and admission-strictness clauses and the ADR whose premise they rest on (with the archive move and reciprocal frontmatter the decisions README requires); retire the two-run, bijection, re-derivation, and reader-test clauses; narrow the field-trace, structured-error, canonical-hash, and design-consult clauses as listed; archive the lessons that teach two-run and canonicalization-literal discipline; close the stale active plans with a closeout note. Keep the gold-label, live-default, loud-mock, and fixture-field-consumer rules.
- acceptance:
  - Every clause quoted in section 5 of the audit is retired or narrowed exactly as listed; retained rules are unchanged.
  - No active plan remains that this plan supersedes.
- validation:
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Clause-by-clause check against audit section 5; decisions README lifecycle followed for the retired ADR"

### Task_2: Delete unclaimed features
- type: impl
- owns:
  - crates/cmem-eval-runner/**
  - crates/cmem-eval-core/**
  - crates/cmem-eval-adapter-cmem/src/lib.rs
  - configs/**
  - fixtures/synthetic_small.json
  - README.md
- depends_on: [Task_1]
- description: |
  Delete the official-export command, the summarize command with its identity invariants, the synthetic dataset and its smoke, the BM25 baseline, the reserved and assertion-only config knobs, the repair and attempt counters as reported metrics (the library's telemetry stays on rows as diagnostic data), and the 29 sweep configs no decision cites. Nothing depends on them; each deletion ships with a zero-hit census.
- acceptance:
  - Zero-hit census per deleted feature; README updated; workspace builds and tests green.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; census commands recorded"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; confirm no cited config or sealed byte changed"

### Task_3: The diff subcommand
- type: impl
- owns:
  - crates/cmem-eval-runner/**
  - README.md
- depends_on: [Task_1]
- description: |
  Add `diff <run-a> <run-b>`: set every `latency_ms` to zero and replace every `run_id` with a placeholder (the run-level fields the README recipe normalises today; returned object identities are never touched, they are what the diff compares), then print per-query differences in returned identities, ranks, metrics, and degradation flags, plus a summary; retire the shell hashing recipe from the README. This restores tier-2 regression detection before any reader is removed.
- acceptance:
  - Two identical runs diff empty; a run pair with an intended rank change reports exactly that change.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "diff on the reviewer's two candidate runs is empty; diff candidate vs parent baseline shows rank-only movement"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; README recipe retired"

### Task_4: Derived readers, run header, no schema versioning
- type: impl
- owns:
  - crates/cmem-eval-core/**
  - crates/cmem-eval-continuity/**
  - crates/cmem-eval-runner/**
  - reports/v0-1-5-findings-register.md (append a dated addendum only; existing lines byte-identical)
- depends_on: [Task_2, Task_3]
- description: |
  Replace the strict fail-closed readers with derived serde, delete the schema-version constants and dispatch, delete summary and report normalization metadata, and add the run header (harness commit, library commit, config as read plus hash, fixture hash, store hash, embedding source, adapter mode, generation time). Record the resurrection pointer for the strict readers in the register addendum as the retired ADR's replacement note requires.
- acceptance:
  - No `required_option`-style strictness and no schema-version dispatch remain; run header present on every run; register addendum carries the pointer.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "workspace gates; a run produces the header; diff of a run before and after this task shows only the header and dropped metadata"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; sealed bytes untouched"

### Task_5: Rows into traces, report shrink, frozen store simplified
- type: impl
- owns:
  - crates/cmem-eval-core/**
  - crates/cmem-eval-continuity/**
  - crates/cmem-eval-runner/**
  - crates/cmem-eval-adapter-cmem/src/lib.rs
  - README.md
- depends_on: [Task_4]
- description: |
  Merge the rows file into the trace file and shrink the report; delete the payload-congruence validation that existed only because two artifacts carried one payload; simplify the frozen embedding store to cache plus ordering validator, deleting the bijection guard, reuse merge, dimension-policy logic, and live-run provenance rejection while keeping the persisted file shape.
- acceptance:
  - One artifact per run plus header and report; committed stores load unchanged; diff tool works on the merged shape.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "workspace gates; committed stores load byte-unchanged; continuity run + diff green"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; store file shape byte-unchanged"

### Task_6: The seal subcommand and evidence promotion
- type: impl
- owns:
  - crates/cmem-eval-runner/**
  - evidence/**
  - reports/**
  - README.md
- depends_on: [Task_5]
- description: |
  Add `seal <run-dir>`: hash every artifact, write the hashes with the run header into a seal file, and copy the traces and report into a tracked evidence directory. Promote the current reference evidence (the register's reference pair, about 2.6 MB per run) into that directory, leaving every existing register line byte-identical and appending one dated addendum that links seal hashes to the register's cited hashes.
- acceptance:
  - A stranger can re-derive a cited number from tracked bytes; every existing register line is byte-identical and exactly one dated addendum is appended.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "seal on a fresh run; verify command recomputes hashes; promoted evidence hashes match the register's cited values"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Hash reconciliation against the register"

### Task_7: Honest CI for live tests
- type: test
- owns:
  - crates/cmem-eval-adapter-cmem/src/lib.rs
  - .github/workflows/ci.yml
- depends_on: [Task_2]
- description: |
  Mark the live adapter tests ignored so CI reports them honestly instead of passing by skip; keep the forced-live switch for local and service-backed runs.
- acceptance:
  - CI shows the live tests as ignored, not passed.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "cargo test --workspace output lists the live tests as ignored; forced-live run still executes them"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review"

### Task_8: Library-dependent shrink (after the library's embedded vector-store phase)
- type: impl
- owns:
  - crates/**
  - Cargo.toml
  - README.md
- depends_on: [Task_6, Task_7]
- description: |
  Sequenced against the library's phase merges: delete the mock adapter and its guard flags once the library runs service-free; embed the library's outcome and telemetry types on rows and delete the mirror vocabulary, the from-live projections, and the telemetry projection; move the vector-only baseline onto the retrieval trace as the library's record specifies (one singleton-scoped traced retrieval per kind with the multiplied limit and object-level deduplication) and drop the second vector-service client and payload constants; replace the string-keyed settings bridge with the library's typed construction; replace collection naming and the prefix cleanup guard with per-run directories; replace hand-built write plans with the library planner; delete the skip macros and run the adapter suite unconditionally in embedded mode; collapse the crates into one. Re-audit the conventional-benchmark loaders at the default-flip decision.
- acceptance:
  - One crate; rows carry library types; zero-hit census for the mirror, the mock, the direct search, and the skip macros; size within the estimate band in section 3 of the audit.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "workspace gates in embedded mode with no service; continuity run + diff against the last sealed reference"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review per step; size and census evidence"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4, Task_7]
- Wave 4 (parallel): [Task_5]
- Wave 5 (parallel): [Task_6]
- Wave 6 (parallel): [Task_8] (gated on the library phase's merges; split into per-step PRs)

## Rollback / Safety
- Every wave is a separately revertible PR; sealed bytes are never edited; evidence promotion adds files only.
- The strict readers' resurrection pointer is recorded before they are deleted.

## Progress Log (append-only)

Append-only editing rule (applies to both logs below): when appending an entry, anchor the edit on the previous entry and reproduce it (or anchor on the section's tail marker) so the edit inserts rather than replaces, and verify afterward that the log grew.

- 2026-09-02 Plan authored from the design-value audit; direction approved by the decider with one correction: harness work is tracked here, not in the library repository.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-02 Decision: adopt "strictness follows the claim, not the code" as the harness's standard.
  - Trigger / new insight: the harness reached the library's size while serving a development aid's purpose; tier-3 machinery sat on every run and produced accounting defects in plumbing rather than measurements of characters.
  - Plan delta: notebook core with `diff` and `seal`; rules retired; unclaimed features deleted; mirror and mock deleted after the library is service-free.
  - Tradeoffs considered: section 6 of the audit (two-run gate replaced by a two-minute diff; strict readers replaced by seal hashes; compile-time breakage on library change is the desired signal; evidence credibility restored by promotion).
  - User approval: yes, 2026-09-02.
- 2026-09-02 Decision: harness work is planned and tracked in this repository; the library's records state only what these measurements allow it to decide and when.
  - Trigger / new insight: eval-side tasks had been mixed into the library's phase plan.
  - Plan delta: the two eval-side tasks were removed from the library plan and live here as Task_8's steps.
  - User approval: yes, 2026-09-02.

## Notes
- Risks and mitigations: section 6 of the audit.
- Edge cases: cited configs may become unparseable after key deletions; that is accepted, and re-runs are new evidence.
