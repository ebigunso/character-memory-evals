# Plan: Dataset Admission (strict on identity and structure, lenient on annotations)

- status: draft
- generated: 2026-09-14
- last_updated: 2026-09-14
- work_type: code

## Goal
- A malformed, wrong-version or partially unreadable official dataset file fails before any embedding call or store mutation, with a typed error naming the item and the field, while every valid official record (including records that legitimately omit optional annotations) is admitted unchanged.

## Definition of Done
- Both dataset loaders (LoCoMo, LongMemEval) reject: an unrecognized top-level shape; an item without an id; duplicate ids within one file; an item without a question; an item with zero sessions. Rejection happens at load, before the pipeline creates a run root or calls a provider.
- Optional annotations (question type, answer, question date, session dates, speaker labels) stay optional; the tested key aliases stay admitted.
- The full official LoCoMo and LongMemEval-S files are admitted with the same item counts, ids and typed fields as before this plan (byte-level comparison of the parsed items), so no measurement changes.
- The decider ruling is recorded in the reviewer rules (dataset loaders join the fail-closed input surfaces) and the plan closes with censuses and the comparison evidence.

## Scope / Non-goals
- Scope: the two dataset loader crates, their typed errors, the runner's handling of a load failure (exit non-zero before admission of any output), tests, README dataset section, reviewer rule line.
- Non-goals: any change to scoring, ingest mapping, the converter, the continuity fixtures (already fail-closed), or the artifact contract; no schema for the datasets beyond what the loaders read today; no new dependency.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: dataset loader admission (input contract) and the loaders' public parse functions returning typed errors
- stance: break
- justification: no external consumer of the loader crates exists (the runner and the converter are the only callers, both in this workspace); official dataset files that are valid today remain admitted by the definition of done.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-longmemeval/src/loader.rs`, `crates/cmem-eval-locomo/src/loader.rs` (tolerant defaults: "unknown" ids, empty questions, empty sessions, synthetic session ids); `crates/cmem-eval-runner/src/pipeline.rs` (no post-load admission; only the empty-run guard); `crates/cmem-eval-benchmark-convert` (second caller of the loaders).
- Existing patterns or references: fixture and config admission fail closed (`crates/cmem-eval-continuity/src/fixture.rs`, `crates/cmem-eval/src/config.rs`); reviewer hotspot `admission_before_side_effect` in `docs/coding-agent/rules/reviewer.md`; ADR-I-0005 scopes derived-serde leniency to artifact readers only.
- Design record consulted and deviations from its acceptance: ADR-I-0004 (dataset crates own their loaders; no shared-crate edit in a dataset change) and ADR-I-0005 (input contracts are not artifact readers); no deviation.

## Open Questions (max 3)
- Q1: Whether an item with a question but an empty answer and no answer-session ids is a valid abstention record in LongMemEval (expected yes; Task_1 confirms from the official files) or a structural defect.
- Q2: Whether the converter needs the same strict admission or admits through the loaders unchanged (expected unchanged, since it calls the same parse functions).

## Assumptions
- A1: Both official files parse today with zero "unknown" ids and zero zero-session items — source: unverified, checked by Task_1.
- A2: The runner calls the loaders before creating the run root — source: `crates/cmem-eval-runner/src/pipeline.rs` load order, checked by Task_2.

## Tasks

### Task_1: Dataset field census
- type: research
- owns:
  - docs/coding-agent/plans/active/dataset-admission-plan.md
- depends_on: []
- description: |
  For each official file (LoCoMo, LongMemEval-S) tabulate, per field the loaders read: present in every item, present in some, absent; and whether the loader defaults it today. Confirm A1 and answer Q1 and Q2 from the files, not from memory. Record the table in this plan's Decision Log.
- acceptance:
  - The table names every field each loader reads with its presence class and today's default.
  - A1, Q1 and Q2 are answered with file evidence (item counts and example ids).
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Plan Decision Log carries the census and the answers; the strict set in Task_2 follows from it"

### Task_2: Strict admission in both loaders
- type: impl
- owns:
  - crates/cmem-eval-longmemeval/**
  - crates/cmem-eval-locomo/**
  - crates/cmem-eval-runner/src/pipeline.rs
  - README.md
- depends_on: [Task_1]
- description: |
  Each loader returns a typed admission error (item index or id, field, reason) for the strict set: unrecognized top-level shape, missing id, duplicate id, missing question, zero sessions; optional annotations and the tested key aliases stay as they are. The runner surfaces a load failure before creating the run root or calling any provider. README's dataset section states the contract.
- acceptance:
  - Every strict-set case has a rejection test with the named field; every optional-annotation case has an admission test.
  - The full official files are admitted with parsed items byte-identical to the pre-plan parse (comparison script retained as evidence).
  - A malformed file fails before any run root exists (census of the output directory after the failure).
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "fmt; clippy --workspace --all-targets -D warnings; embedded workspace suite; the pre/post parse comparison on both official files; the maintained continuity smoke pair"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review against the strict set; independent parse comparison; rejection-before-side-effect probe with a malformed file"

### Task_3: Rule and closeout
- type: docs
- owns:
  - docs/coding-agent/rules/reviewer.md
  - docs/coding-agent/plans/active/dataset-admission-plan.md
- depends_on: [Task_2]
- description: |
  Add dataset loaders to the fail-closed input surfaces in the reviewer hotspot; close the plan with the censuses and move it to completed.
- acceptance:
  - The reviewer rule names dataset loaders beside fixtures, configs and frozen stores.
  - The plan is in completed with its final progress entry.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Rule wording and closeout entry"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]

## Rollback / Safety
- Loader changes are additive rejections; reverting the branch restores tolerant parsing. No persisted format changes.

## Progress Log (append-only)

- 2026-09-14 Plan drafted from the decider's ruling (option 3 of the dataset-handling question raised during the loader cleanup): strict on identity and structure, lenient on annotations, done now rather than folded into v0.2 planning.

## Decision Log (append-only; re-plans and major discoveries)

## Notes
- The datasets are external official files cited by hash in the run header; admission protects paid provider calls and citable evidence from silently degraded inputs.
