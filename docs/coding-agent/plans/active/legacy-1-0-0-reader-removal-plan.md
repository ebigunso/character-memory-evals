# Plan: Legacy 1.0.0 Reader Removal (Option C)

- status: in_progress
- generated: 2026-07-28
- last_updated: 2026-07-29
- work_type: code

## Goal
- Remove all legacy 1.0.0 artifact-reading capability from the live code paths so every reader tracks schema 2.0.0 only, per ebigunso's Option C ruling (2026-07-28).

## Definition of Done
- Zero 1.0.0/V1/Legacy knowledge in production code: no legacy DTOs, no version dispatch, no legacy constants, no legacy projection helpers, no V1 re-exports.
- `read_jsonl` returns `Vec<PerQuestionResult>` and `read_continuity_traces` returns `Vec<ContinuityQueryTrace>`, both strict 2.0.0 fail-closed.
- Sealed artifacts and the findings register are byte-identical to before this change (zero artifact mutations, zero register hash effects).
- Resurrection pointer recorded: the last commit at which the V1 readers existed.
- rules/common.md legacy-dispatch clause rewritten to the new strict-only contract.
- Internal Tier D review APPROVED; PR merged to main after Copilot review.

## Scope / Non-goals
- Scope: crates/cmem-eval-core (results.rs, lib.rs), crates/cmem-eval-continuity (driver.rs, lib.rs), crates/cmem-eval-runner (commands.rs, official_exports.rs, pipeline test helpers), README.md legacy mention, rules/common.md clause, register resurrection pointer line.
- Non-goals: no sealed-artifact edits or migration of any kind; no changes to emitted 2.0.0 artifact bytes or writers; no strictness weakening of any 2.0.0 reader; the contract/strictness ADR pair is drafted only after remediation lands (Task_4) and persists only on decider approval.

## Context (workspace)
- Census: evals-researcher forensic census at HEAD 9997ccd (agmsg 2026-07-28T11:00Z + 4 continuation parts) — full V1 site map, sealed corpus inventory (203 register-associated paths), blast radius.
- Design consult: Claude Tier-A consult (2026-07-28) recommending Option C; verified census claims in code.
- Ruling: ebigunso 2026-07-28 — Option C (deletion), after likelihood analysis of genuine readout scenarios; export-official archival V1 support dropped; resurrection pointer recorded.
- Provenance: docs/coding-agent/HANDOFF.md (2026-07-24 "EXCEPTIONS SUCK" calibration ruling).

## Open Questions (max 3)
- None; all decider forks resolved 2026-07-28.

## Assumptions
- A1: The resurrection pointer references the parent commit of the deletion commit on main (readers last present there); recorded as prose, no hash-cited evidence touched.
- A2: A read-side deletion cannot alter successfully emitted artifact bytes, so the reviewer live two-run reproducibility gate does not fire (reviewer.md trigger refinement, 2026-07-23); reviewer dispatch states this explicitly.

## Tasks

### Task_1: ADR pair proposal and persistence (decider-gated, before implementation)
- type: design
- owns:
  - docs/decisions/
- depends_on: []
- description: |
  Draft the decomposed ADR pair per durable-docs-authoring and the handoff obligation, recording the Option C ruling before any code changes: (1) the clean 2.0.0 artifact-contract ADR — strict fail-closed readers track the current schema only, sealed pre-2.0.0 evidence is hash-verified bytes with no live reading capability, readers move with the schema on future bumps (no dual dispatch), resurrection pointer mechanism recorded in the findings register — with warranted_by counterfactual (without the record, a future agent hitting unreadable sealed artifacts would plausibly re-add legacy dispatch or migrate sealed bytes; both occurred/were proposed in this repo's history); (2) the reader-strictness scope ADR (strictness is a trust-boundary property scoped to hash-cited evidence readers).
  Present both drafts to ebigunso; persist only on approval, as a docs-only commit per git-workflow, before Wave 2 dispatch.
- acceptance:
  - Both drafts follow docs/decisions/ template and numbering; consulted records model names only.
  - Decider approval obtained before any ADR file is committed.
  - Approved ADRs committed before implementation dispatch.
- validation:
  - kind: review
    required: true
    owner: user
    detail: "ebigunso approves or amends the ADR pair before persistence"

### Task_2: Delete V1 reading capability across all live surfaces
- type: impl
- owns:
  - crates/cmem-eval-core/src/results.rs
  - crates/cmem-eval-core/src/lib.rs
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/lib.rs
  - crates/cmem-eval-runner/src/commands.rs
  - crates/cmem-eval-runner/src/official_exports.rs
  - crates/cmem-eval-runner/src/pipeline.rs
  - README.md
- depends_on: [Task_1]
- description: |
  Remove every 1.0.0 site from the census map: LEGACY_RESULT_SCHEMA_VERSION, LegacyPerQuestionResultV1, LegacyRetrievedItemV1, the legacy rendered-context reconstruction, VersionedPerQuestionResult and into_v2, the RowSchema V1 branch, and the V1 arm of read_jsonl dispatch (results.rs:16-17,49-140,672-734); the legacy trace constant, LegacyContinuityQueryTraceV1, VersionedContinuityQueryTrace, and the V1 reader arm (driver.rs:28-67,163-225); the V1/V2 match arms and legacy projection helpers in official_exports.rs; dual-reader consumption in commands.rs export-official and summarize (both call the strict readers directly); wildcard re-export hygiene in both lib.rs files.
  read_jsonl and read_continuity_traces become strict 2.0.0-only returning current DTOs; missing/other/mixed schema versions remain rejected fail-closed with the existing error quality.
  Delete V1-specific tests (tolerant V1 decode, mixed-version, legacy renderer contract); retain and adapt all strict-V2 tests; simplify pipeline test helpers that unwrap version enums.
  Update the README legacy-reader mention (README.md:329) to the strict-only contract.
  Do not touch any file under runs/ or reports/. Commit locally on the task branch; do not push.
- acceptance:
  - `git grep -iE "1\.0\.0|LegacyPerQuestionResultV1|LegacyRetrievedItemV1|LegacyContinuityQueryTraceV1|Versioned(PerQuestionResult|ContinuityQueryTrace)|into_v2"` over crates/ returns only 2.0.0-era hits (schema constants/tests asserting rejection), zero legacy symbols.
  - Strict readers reject a synthesized 1.0.0 row and a synthesized 1.0.0 trace with a controlled schema error (regression tests included).
  - No file under runs/ or reports/ is modified (clean `git status` for those paths).
  - Emitted-artifact writers and 2.0.0 reader admission behavior are unchanged.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings"
  - kind: command
    required: true
    owner: worker
    detail: "cargo test --workspace (service-free: explicitly skip every live Qdrant test — three at 2026-07-29: live_frozen_write_surface_matches_continuity_runtime_normalization, live_adapter_reattaches_with_external_ids, live_reset_preserves_sibling_namespace_durable_stores; state the exclusions in the report)"
  - kind: command
    required: true
    owner: worker
    detail: "Synthetic smoke per rules/common.md: cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark (then delete the two smoke outputs)"
  - kind: review
    required: true
    owner: reviewer
    detail: "Task_4 diff review vs acceptance criteria"

### Task_3: Rule clause rewrite and resurrection pointer (orchestrator-owned)
- type: docs
- owns:
  - docs/coding-agent/rules/common.md
  - reports/v0-1-5-findings-register.md
- depends_on: [Task_2]
- description: |
  Rewrite the rules/common.md Repo Naming / Structure clause (line 51): readers are strict fail-closed 2.0.0-only; sealed pre-2.0.0 evidence remains hash-verified bytes with no live reading capability; readers move with the schema on future bumps (no dual dispatch).
  Append the resurrection pointer to the findings register as a clearly dated addendum line: V1 readers last present at the recorded parent commit of the deletion commit; do not modify any existing register line or cited hash.
- acceptance:
  - common.md no longer documents a legacy dispatch; new clause states the strict-only contract and the bytes-by-hash sealed-evidence meaning.
  - Register gains exactly one appended addendum line; all existing lines byte-identical.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Confirm register diff is append-only and the rule clause matches the ruling"

### Task_4: Internal Tier D review
- type: review
- owns: []
- depends_on: [Task_2, Task_3]
- description: |
  evals-reviewer reviews the full diff from an isolated worktree pinned at the local task-branch commit. Dispatch states explicitly: read-side deletion, cannot alter successfully emitted artifact bytes, so the live two-run reproducibility gate does not fire (reviewer.md 2026-07-23 trigger refinement); offline review with the service-free validation evidence from Task_1.
- acceptance:
  - Reviewer status APPROVED.
  - Reviewer confirms zero remaining V1 knowledge, unchanged 2.0.0 admission behavior, and untouched sealed paths.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Independent verification of Task_2 acceptance greps and test evidence from the pinned worktree"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]

## Rollback / Safety
- All code changes land on a task branch; main is untouched until PR merge. Revert = drop the branch.
- Sealed artifacts under runs/ and the register's existing lines are out of scope for every task except the register's single append; any unexpected diff there is a stop-and-alert tripwire event.
- The deleted readers remain recoverable at the recorded resurrection commit.

## Progress Log (append-only)

- 2026-07-29 Wave 1 completed: [Task_1]
  - Summary: ADR pair drafted (ADR-I-0002 single-schema artifact contract / bytes-by-hash sealing; ADR-I-0003 reader admission strictness scope), presented to ebigunso, amended per wording corrections (version-agnostic wording; no team-local tier jargon), approved 2026-07-29 and committed on the phase branch.
  - Validation evidence: decider approval in-session 2026-07-29; template/numbering conformance vs docs/decisions/README.md and template.md; grep sweep confirms zero version literals and zero tier jargon in both records.
  - Notes: plan approved with ADR-first reordering (see Decision Log).

- 2026-07-29 Wave 2 completed: [Task_2]
  - Summary: evals-worker removed all live 1.0.0 reader capability (6 files, +78/-468); strict readers return current DTOs; rejection regressions added; README updated. Local commit e8707cd, not pushed.
  - Validation evidence: fmt + strict clippy pass; service-free workspace suite pass with the three live Qdrant tests excluded by name; synthetic mock smoke pass (schema 2.0.0, mock-marked, outputs deleted per disposition); acceptance grep shows only rejection-test 1.0.0 literals; runs/ and reports/ byte-untouched.
  - Notes: three in-flight rulings (owns path fix to src/pipeline.rs; three live-test exclusions; smoke output redirect to .agent-work). Two lesson candidates carried to closeout (canonical-writer round-trip expectations; zero-match test-filter evidence).

- 2026-07-29 Wave 3 completed: [Task_3]
  - Summary: rules/common.md schema clause rewritten to the single-schema/bytes-by-hash contract citing ADR-I-0002; findings register gained a 4-line append-only addendum recording the resurrection pointer (readers last on main at 9997ccd — main-reachable, chosen over branch SHAs because squash merges drop branch lineage).
  - Validation evidence: register diff is append-only (+4/-0); clause matches the ruling; reviewer independently re-verifies in Task_4.
  - Notes: orchestrator-owned edits per rule-file governance.

- 2026-07-29 Wave 4 completed: [Task_4]
  - Summary: evals-reviewer APPROVED with zero findings from the pinned worktree at 52c6e5d (range 9997ccd..52c6e5d).
  - Validation evidence: independent re-verification of all acceptance criteria — zero legacy knowledge (grep + rg --no-ignore), strict reader contract incl. summarize rejecting at schema detection, writer/export conservation, sealed paths untouched with the register delta exactly +4/-0 append-only, governance docs matching ADR-I-0002; full service-free gate suite reproduced (core 109, continuity 80, runner 42, adapter 40/3 filtered, converter 11, LoCoMo 12, LongMemEval 8, zero failures). Live two-run gate NOT_TRIGGERED per read-side-deletion rationale.
  - Notes: branch pushed after internal approval per push-sequencing rule; PR #20 opened, Copilot review + monitor armed.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-07-28 Decision: Option C (full deletion) selected by ebigunso over migration (non-lossless, breaks hash citations, sealed-policy conflict) and containment (dead code behind a fence, fails EARNS-ITS-PLACE, ratifies the rejected exception shape).
  - Trigger / new insight: evals-researcher census + Tier-A design consult + ebigunso's likelihood analysis of genuine readout scenarios (hash duties never need typed readers; semantic-readout scenarios are unlikely and better served by rerun or git-history resurrection).
  - Plan delta: initial plan authored under this ruling.
  - Tradeoffs considered: recorded in the ruling packet (agmsg 2026-07-28) and the consult verdict.
  - User approval: Option C ruled yes (2026-07-28, in-session); plan approval pending.

- 2026-07-28 Decision: ADR authoring moved ahead of implementation (Task_4 -> Task_1) per ebigunso: decisions set in stone before implementation begins.
  - Trigger / new insight: user direction on plan review.
  - Plan delta: ADR pair is now Task_1 with persistence gating Wave 2; impl/rules/review renumbered to Task_2/Task_3/Task_4 with dependencies updated.
  - Tradeoffs considered: the harness exception check ("remediate first, then record") is satisfied in spirit because the ADR records the clean target contract already ruled, not the live exception; the exception's removal is committed work in the same plan.
  - User approval: yes (2026-07-28, in-session).

- 2026-07-29 Decision: Task_2 owns path corrected from crates/cmem-eval-runner/tests/pipeline.rs to crates/cmem-eval-runner/src/pipeline.rs.
  - Trigger / new insight: evals-worker pre-edit check found tests/pipeline.rs does not exist; the version-enum helpers are at src/pipeline.rs:1757-1772 (matching the original census message; the tests/ variant was a continuation-part transcription slip carried into the plan).
  - Plan delta: owns entry replaced; no scope change in substance.
  - Tradeoffs considered: none — mechanical correction.
  - User approval: not required (path fix within ruled scope).

## Notes
- Risks: pipeline test helpers may have deeper coupling to the version enums than the census surface shows; worker reports any owns-expansion need instead of improvising.
- Edge cases: summarize currently parses V1 before rejecting via into_v2 — after deletion it must reject at schema detection with a controlled error, not a panic; regression required (covered by Task_1 acceptance).
