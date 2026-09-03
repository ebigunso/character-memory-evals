# Plan: PR #9 Copilot Round 5

- status: completed
- generated: 2026-07-14
- last_updated: 2026-09-02
- work_type: mixed

## Goal
- Make continuity fixture identity, object-kind admission, entity-kind admission, checked artifacts, and reproducibility documentation honest and fail-closed before any backend side effects.

## Definition of Done
- Fixture schema v2 removes dead caller-supplied memory IDs and the v1 artifact is replaced by deterministic `continuity_v2.json`.
- Parser admission tracks persisted object kinds, rejects unsupported operation/endpoint shapes, admits Remember-created observation/derived/thread identities, and rejects cross-kind collisions.
- Entity kinds are a closed continuity-owned enum with an exhaustive driver mapping.
- The README hashing recipe preserves array shape for one or many rows and reports scenario scope.
- Strict Rust gates, scoped/full recipe checks, two full live runs, and independent Reviewer approval pass.

## Scope / Non-goals
- Scope: continuity fixture/generator/driver/metrics/report call sites, checked artifact, runner fixture path, README reproducibility recipe, tests, lessons, and evidence outputs.
- Non-goals: changing CharacterMemory object identity derivation, adapter production identity/registry behavior, report/result/trace schema versions, metric semantics, or adding CharacterMemory as a continuity dependency.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-continuity/**`, `crates/cmem-eval-runner/src/pipeline.rs`, `README.md`, `docs/coding-agent/lessons.md`.
- Existing patterns or references: adapter derives persisted IDs from `(namespace, kind, external_id)`; fixture/report schemas are independently versioned; round-four exhaustive-vocabulary parity lesson.
- Repo reference docs consulted: `README.md`, repo rules, completed architecture/continuity plans, recent lessons.

## Open Questions (max 3)
- None. The orchestrator approved route (b), fixture schema v2, and fresh live evidence requirements.

## Assumptions
- The sibling CharacterMemory provenance remains commit `7bc4e06be2f02b63991d164cb527b73b4f0ad32e` for evidence labeling.
- Qdrant remains reachable through the VM-IP endpoint `http://172.29.24.25:6334`; readiness will be checked before live runs.

## Tasks

### Task_1: Migrate the fixture contract to schema v2
- type: impl
- owns:
  - `crates/cmem-eval-continuity/src/fixture.rs`
  - `crates/cmem-eval-continuity/src/generator.rs`
  - `crates/cmem-eval-continuity/src/bin/generate_continuity_fixtures.rs`
  - `crates/cmem-eval-continuity/fixtures/continuity_v1.json`
  - `crates/cmem-eval-continuity/fixtures/continuity_v2.json`
  - `crates/cmem-eval-continuity/Cargo.toml`
- depends_on: []
- description: |
  Remove misleading caller-supplied object IDs, bump the fixture schema, make retired fields fail closed, close the entity-kind vocabulary, and regenerate the canonical artifact.
- acceptance:
  - All four dead ID fields and generator UUIDv5 machinery are removed.
  - Schema v1 and schema-v2 inputs containing retired fields are rejected.
  - `continuity_v2.json` is canonical and deterministic; `continuity_v1.json` is removed.
  - Duplicate-entity-memory-ID finding is explicitly documented as moot under route (b), while external-ID uniqueness remains enforced.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Continuity parser/generator tests, rooted retired-field audit, and cross-process canonical artifact equality"
  - kind: review
    required: true
    owner: reviewer
    detail: "Confirm schema-version policy, dead-surface removal, and unchanged adapter identity ownership"

### Task_2: Enforce typed object and entity admission
- type: impl
- owns:
  - `crates/cmem-eval-continuity/src/fixture.rs`
  - `crates/cmem-eval-continuity/src/driver.rs`
  - `crates/cmem-eval-continuity/src/metrics.rs`
- depends_on: [Task_1]
- description: |
  Track every persisted external identity by kind before execution, share generated-ID helpers, and reject operation shapes the driver cannot execute.
- acceptance:
  - Remember admits episode, observation, derived-memory, and optional thread IDs with cross-kind collision detection.
  - Correct, forget, and link endpoints accept exactly their supported object kinds and reject invalid shapes at fixture parsing.
  - Entity kinds deserialize through a closed continuity enum and map exhaustively to facade names.
  - Regressions cover every rejected shape named in the dispatch plus valid implicit observation/derived/thread references.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Targeted continuity and runner tests plus strict Clippy exhaustive-match enforcement"
  - kind: review
    required: true
    owner: reviewer
    detail: "Verify parser/driver kind-set parity and pre-side-effect rejection"

### Task_3: Repair reproducibility documentation and fixture references
- type: docs
- owns:
  - `README.md`
  - `crates/cmem-eval-runner/src/pipeline.rs`
  - `docs/coding-agent/lessons.md`
- depends_on: [Task_1, Task_2]
- description: |
  Update fixture paths/schema wording, make the PowerShell hash recipe cardinality-stable with `-InputObject @($rows)`, report scope, and record the review lesson.
- acceptance:
  - All live/current fixture references name `continuity_v2.json`.
  - One-row and multi-row normalized serialization are JSON arrays.
  - Recipe output includes fixture scope and evidence labels distinguish scoped from full-suite runs.
  - Historical plan paths are preserved and excluded explicitly from removal audits.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Run recipe against scoped one-scenario and full mock outputs; parse normalized bytes as arrays and verify printed scope"
  - kind: manual
    required: true
    owner: worker
    detail: "Rooted audit for stale live fixture paths and retired schema fields, excluding documented historical plans"

### Task_4: Validate service-free and live reproducibility
- type: test
- owns:
  - `runs/continuity/round5/**`
- depends_on: [Task_1, Task_2, Task_3]
- description: |
  Run strict repository gates and two serialized full eight-scenario live runs using the committed v2 fixture/config and VM-IP endpoint.
- acceptance:
  - Formatting, strict Clippy, targeted packages, and workspace tests pass.
  - Both live runs cover all eight scenarios with no `--scenario` filter.
  - Trace, latency-normalized row, and report-content hashes match between runs.
  - Evidence records scope, endpoint, fixture/config hashes, and CharacterMemory provenance.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace"
  - kind: e2e
    required: true
    owner: worker
    detail: "Two full live continuity runs via VM-IP with reproducibility hashes and provenance labels"

### Task_5: Independent review
- type: review
- owns: []
- depends_on: [Task_4]
- description: |
  Reviewer verifies the complete round-five delta, artifact migration, admission parity, documented hash recipe, and live evidence.
- acceptance:
  - Reviewer status is APPROVED with no blocking findings.
  - Reviewer independently checks the live artifact hashes and eight-scenario scope.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Independent defect/contract review and evidence verification"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]
- Wave 5 (parallel): [Task_5]

## Rollback / Safety
- The migration is isolated to fixture schema v2 and its consumers. Reverting the coherent round-five commit restores v1; no adapter production identity or external store migration is introduced.

## Progress Log (append-only)

- 2026-07-14 Wave 0 completed: forensic research integrated.
  - Summary: route (b), schema v2, typed admission, enum mapping, recipe repair, and live evidence surface mapped.
  - Validation evidence: read-only file/symbol inventory from Researcher; no workspace mutations.
  - Notes: orchestrator approval is explicit in the round-five dispatch.
- 2026-07-14 Waves 1-3 completed: fixture schema v2, typed admission, driver parity, checked artifact, runner path, README recipe, and review lesson implemented.
  - Summary: caller-supplied memory IDs removed; entity/object kinds fail closed; implicit observation/derived/thread IDs are admitted and exercised.
  - Validation evidence: 55 continuity tests, 23 runner tests, and 26 adapter tests pass; scoped and full mock recipe probes preserve JSON-array shape and report exact fixture scope.
  - Notes: duplicate entity memory-ID handling is moot because schema v2 rejects that retired field; generated and cross-kind external-ID collisions remain covered.
- 2026-07-14 Wave 4 completed: strict and live reproducibility gates passed.
  - Summary: two unfiltered eight-scenario live runs completed through `http://172.29.24.25:6334` with CharacterMemory `7bc4e06be2f02b63991d164cb527b73b4f0ad32e`.
  - Validation evidence: `cargo fmt --all --check`, strict workspace Clippy, and `cargo test --workspace` pass. Both runs produced trace `8C18B1A4A1D7CB4667D475FD038DB54FAB6B08049006F624C40D72AC6CA34B98`, normalized rows `2B68F0FEB090A6E205929284271B01FA4105A7759635118BD4862C21586CBB07`, and report content `10EB0EE6EA81A62EA0574C8518B7D92018C01EFE0BDBD86256606C191AAAEE55`.
  - Notes: fixture SHA-256 is `EC7591F4BF1BFEC0783DA716ED10FC89072CAC261A4FF9C701F1A6691FCACFA6`; config SHA-256 is `B431393E3CE528C9284E604742678859B8AC40F31D1D9105BECC2A017B361449`.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-07-14 Decision: remove dead caller-supplied memory IDs and bump the continuity fixture schema to v2.
  - Trigger / new insight: fixture IDs never become persisted identities; adapter identity is derived independently from namespace/kind/external ID.
  - Plan delta (what changed): replace v1 checked artifact, remove dead fields, preserve adapter identity ownership, and require fresh live evidence because fixture bytes and metadata change.
  - Tradeoffs considered: threading IDs through the adapter would exercise a new live identity path and invalidate prior identity behavior; removal produces the smaller honest contract.
  - User approval: yes, via orchestrator dispatch.

## Notes
- Risks: schema incompatibility, incomplete implicit-ID admission, scoped/full hash cardinality drift, and live evidence invalidation.
- Edge cases: repeated threads, generated-ID collisions, invalid operation target kinds, memory-link endpoints, one-row JSON serialization.

## Closeout

- 2026-09-02 Plan closeout
  - Summary: Closed the stale active record after its logged waves completed and archived it during the harness right-sizing audit.
  - Validation evidence: The completed progress log above records the scoped, strict, mock, and live evidence used at delivery.
  - Notes: Moved to `plans/completed/`; no implementation or evidence artifact changed.
