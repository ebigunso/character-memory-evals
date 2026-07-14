# Plan: PR #9 Copilot Round 6

- status: in_progress
- generated: 2026-07-14
- last_updated: 2026-07-14
- work_type: mixed

## Goal
- Remove the final dead caller-owned collection identity from the unreleased continuity fixture v2 contract and correct registry-path documentation without changing live runtime identity or benchmark semantics.

## Definition of Done
- `collection_name` is absent from the Rust fixture schema, generator, checked artifact, validation, and test literals; schema-v2 inputs containing it fail closed.
- The canonical v2 artifact is regenerated deterministically and its new hash is recorded.
- README accurately distinguishes validation-required Oxigraph/stat paths from the deterministic identity-registry fallback.
- Strict Rust gates, full mock semantic comparison, one targeted live restart run, and independent targeted review pass.

## Scope / Non-goals
- Scope: continuity fixture/generator/test surface, canonical v2 artifact, README wording, review lesson, validation evidence, and this plan.
- Non-goals: changing adapter collection/registry derivation, changing schema version beyond unreleased v2, changing report/trace/result schemas, or rerunning two full live suites for a field with no runtime reader.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-continuity/src/{fixture,generator,metrics}.rs`, `crates/cmem-eval-continuity/fixtures/continuity_v2.json`, `README.md`, `docs/coding-agent/lessons.md`.
- Existing patterns or references: `ContinuityScenario` denies unknown fields; adapter derives collection and registry paths from `(namespace_prefix, run_id, namespace)`.
- Repo reference docs consulted: repository common/worker/orchestrator rules, recent round-five lessons, Rust/schema/public-contract/state quality gates.

## Open Questions (max 3)
- None. The orchestrator authorized the v2 amendment and allowed a reasoned targeted-live evidence route.

## Assumptions
- Fixture schema v2 remains unreleased outside `feature/continuity-driver`; branch/tag containment is checked before commit.
- Qdrant remains reachable at `http://172.29.24.25:6334` for the targeted restart scenario.

## Tasks

### Task_1: Amend the unreleased fixture v2 contract
- type: impl
- owns:
  - `crates/cmem-eval-continuity/src/fixture.rs`
  - `crates/cmem-eval-continuity/src/generator.rs`
  - `crates/cmem-eval-continuity/src/metrics.rs`
  - `crates/cmem-eval-continuity/fixtures/continuity_v2.json`
- depends_on: []
- description: |
  Remove the unused fixture collection name and its synthetic uniqueness policy while preserving namespace ownership of runtime identity.
- acceptance:
  - `ContinuityScenario` no longer exposes `collection_name`.
  - Fixture validation no longer validates or deduplicates a dead collection value.
  - Schema v2 rejects retired `collection_name` through `deny_unknown_fields`.
  - Generated and checked v2 artifacts are canonical and deterministic.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Continuity parser/generator tests, rooted field audit, canonical artifact equality, and strict Clippy"
  - kind: review
    required: true
    owner: reviewer
    detail: "Confirm collection identity remains adapter-owned and v2 amendment policy is justified"

### Task_2: Correct persistence-path documentation and record prevention
- type: docs
- owns:
  - `README.md`
  - `docs/coding-agent/lessons.md`
- depends_on: [Task_1]
- description: |
  Document the optional identity-registry root and deterministic fallback, update v2 retirement wording, and capture the missed dead-field ownership sweep.
- acceptance:
  - README names only Oxigraph and retrieval-stat paths as validation-required.
  - README explains the `runs/<run_id>` registry fallback and committed override.
  - Schema-v2 wording lists `collection_name` among retired caller-owned identity fields.
  - Lesson provides an actionable end-to-end ownership-audit guardrail.
- validation:
  - kind: manual
    required: true
    owner: worker
    detail: "Cross-check wording against core validation and adapter fallback code"
  - kind: review
    required: true
    owner: reviewer
    detail: "Verify documentation accurately distinguishes required paths, fallback persistence, and committed override"

### Task_3: Validate behavior neutrality and live restart
- type: test
- owns:
  - `runs/continuity/round6-*/**`
- depends_on: [Task_1, Task_2]
- description: |
  Prove the removed field does not change benchmark semantics with a full mock comparison and one scoped live restart exercise, then run repository gates.
- acceptance:
  - Full mock run covers all eight scenarios and matches round-five trace, normalized-row, and report-content hashes.
  - Targeted `cross-store-stress` live run completes restart/reattach and cleanup successfully through the VM-IP endpoint.
  - Formatting, strict workspace Clippy, targeted packages, and workspace tests pass.
  - Evidence records new fixture hash, exact scope, endpoint, and CharacterMemory provenance.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace"
  - kind: e2e
    required: true
    owner: worker
    detail: "Full eight-scenario mock comparison plus one live cross-store-stress restart run"

### Task_4: Independent targeted review
- type: review
- owns: []
- depends_on: [Task_3]
- description: |
  Reviewer verifies the schema amendment, fallback documentation, regenerated artifact, and proportional evidence decision.
- acceptance:
  - Reviewer returns APPROVED with no blocking findings.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Targeted review of the round-six commit range and evidence packet"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]

## Rollback / Safety
- Revert the coherent round-six commit to restore the branch-only fixture field. No adapter identity or persistent-store migration is introduced.

## Progress Log (append-only)

- 2026-07-14 Wave 0 completed: forensic research integrated.
  - Summary: `collection_name` has no runtime reader; registry fallback and required-path wording were traced to their owners.
  - Validation evidence: exhaustive read-only inventory with file/line evidence; no workspace mutations.
  - Notes: research confirms a v2 amendment and targeted-live route are proportionate.
- 2026-07-14 Waves 1-2 completed: schema v2 and documentation amended.
  - Summary: retired `collection_name`, regenerated the canonical fixture, corrected registry fallback wording, and added parser/lesson coverage.
  - Validation evidence: continuity 55/55, core fallback validation, adapter collection-ownership test, runner 23/23, canonical regeneration, and rooted field audit.
  - Notes: the new canonical fixture SHA-256 is `7960A3A740A47C88C21361A7B585868E482E35B9C1A3858DBC051B6DCBFE1455`.
- 2026-07-14 Wave 3 completed under an explicit strict-gate waiver: semantic and live evidence are stable.
  - Summary: all eight mock scenarios matched round five semantically; targeted `cross-store-stress` live restart/reattach passed with zero leaf diffs after latency normalization.
  - Validation evidence: mock trace `FDD13D62B7839482EFC7B19F2631646152516C7D5D4F3131956B3189A2CE223E`, rows `E2E7254E26DF757C27F0C2B8F73A0AA5108A10A464C8D3120359A6D13E32927F`, report content `04B491B95A4F0012546F131D9361C9FFD79AB5A47F6F5B0D6B47C3FDC3FBE988`; targeted live result/trace/report comparisons had zero semantic diffs; format and strict Clippy passed.
  - Notes: `cargo test --workspace` twice failed only in existing Qdrant post-success collection deletion; readiness was `200`, the isolated reattach test passed, and the workspace suite passed with the two external-service live tests filtered. The orchestrator explicitly waived those two aggregate invocations as an environment teardown flake; acceptance evidence rests on the full mock comparison and targeted live run, not the waived aggregate.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-07-14 Decision: amend schema v2 rather than introduce v3.
  - Trigger / new insight: v2 exists only on this feature branch and has not shipped or been tagged.
  - Plan delta (what changed): remove `collection_name` in place, regenerate `continuity_v2.json`, and add retired-field rejection.
  - Tradeoffs considered: v3 would preserve compatibility for a contract that has no released consumer and would perpetuate an unnecessary intermediate schema.
  - User approval: yes, via orchestrator dispatch.
- 2026-07-14 Decision: use full mock comparison plus one targeted live restart run instead of two full live suites.
  - Trigger / new insight: runner and driver never read `collection_name`; runtime collection and registry identities derive solely from config and namespace.
  - Plan delta (what changed): refresh the fixture hash, compare all deterministic mock outputs, and exercise the highest-risk live restart/reattach scenario once.
  - Tradeoffs considered: two full live runs would repeat unchanged adapter inputs across sixteen scenarios without testing an additional path.
  - User approval: yes, the dispatch explicitly permits a reasoned targeted-live route.

## Notes
- Risks: accidental schema-version drift, retained validation-only surface, stale checked artifact, or README fallback ambiguity.
- Edge cases: retired field rejection, duplicate namespaces remaining enforced, registry fallback when the optional root is absent.
