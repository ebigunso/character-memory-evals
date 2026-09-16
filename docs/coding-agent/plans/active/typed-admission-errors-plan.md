# Plan: Admission failures are typed everywhere tests assert them

- status: in_progress (approved by the decider 2026-09-16)
- generated: 2026-09-16
- last_updated: 2026-09-16
- work_type: code

## Goal
- Every admission and validation failure a test asserts in this workspace is a typed error with a location and field, matching the shape the dataset loaders already use, so no test passes on the wrong rejection because a sentence contains a substring and no message rewording edits a test.

## Definition of Done
- The continuity fixture parser returns a typed admission error naming the scenario (or file root), the field and a kind; the error implements `std::error::Error`, so the runner's existing `?` conversions into `anyhow` at its two call sites stay source-compatible; the parser's admission tests match on that error and no longer on message substrings, and the three serde-behaviour tests are deleted.
- The runner's output-path admission has a typed already-exists error beside the existing in-stores error, and the four admission tests match on it.
- The OpenAI embedding response validation in the shared core returns a typed error and its five rejection tests match variants.
- The dataset crates' own-name validation tests match the typed config error; the generator's cluster-count test asserts the two numbers, not the sentence; the results, diff and converter reader tests assert the rejection property (corrupt and truncated input are refused) without serde prose.
- The README continuity smoke recipe runs clean; workspace validation passes on the integrated branch with executed counts.

## Planner-added requirements
- None

## Scope / Non-goals
- Scope: `crates/cmem-eval-continuity/src/fixture.rs` and its tests, `crates/cmem-eval-runner/src/{pipeline,diff}.rs`, `crates/cmem-eval/src/{openai_embedding,results}.rs`, `crates/cmem-eval-{locomo,longmemeval}/src/lib.rs` tests, `crates/cmem-eval-continuity/src/generator.rs:2037`, `crates/cmem-eval-benchmark-convert/src/lib.rs:992-1000`.
- Non-goals: changing which inputs are admitted; artifact readers (derived serde per ADR-I-0005 stay untyped); test deletions and cost work (later plans).

## Design
- Chosen: one owned admission error per public input parser, shaped like `LoadError::Admission { location, field, reason }` in the dataset crates, with the location carrying the scenario or root, and implementing `std::error::Error` so callers that already convert into `anyhow` need no edit. Structure: each crate owns its error; no shared error crate; no cross-crate caller edits in the same wave. Evolution: follows worker.md ("new validators on library-facing surfaces classify failures with an owned structured error type at introduction") and reviewer.md `admission_before_side_effect`. Verification: tests match variants and fields; message text is free to change. Operation: unchanged. Human: the error names where and what. Safety: unchanged.
- Alternative: keep `anyhow` and assert on a stable error code prefix in the message. Structure: unchanged. Evolution: a second convention beside the typed loaders. Verification: still string matching; a typo in a prefix passes silently.
- Why chosen: the typed shape exists two crates over and is the repo's stated rule; the alternative adds a convention.

## Compatibility stance
- surface: the fixture parser's public functions (`parse_fixture_bytes` at `crates/cmem-eval-continuity/src/fixture.rs:829` and its siblings) and the runner's output admission function change their error type. The core's `ordered_embeddings` is module-private (`crates/cmem-eval/src/openai_embedding.rs:122`) and has no compatibility surface.
- stance: break
- justification: the fixture parser's out-of-crate callers are the runner at `crates/cmem-eval-runner/src/pipeline.rs:86` and `:568`, both `?` into `anyhow`, which the `std::error::Error` requirement keeps compiling unchanged; the runner admission function's callers are in the runner. No artifact format changes.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-continuity/src/fixture.rs:1024-1885` (25 admission tests; up to 90 `contains(` call sites of which the error-string subset is the target; `:1741/:1753/:1761` serde-behaviour tests; 33 `bail!` sites in the parser); `crates/cmem-eval-runner/src/pipeline.rs:1209-1268` (`OutputPathInStores`, the single "already exists" branch at `:1257`) and tests `:1912,1936,1965,1993`; `crates/cmem-eval/src/openai_embedding.rs:122` (`ordered_embeddings`) and tests `:221-280`; `crates/cmem-eval-locomo/src/lib.rs:171`, `crates/cmem-eval-longmemeval/src/lib.rs:113`; `crates/cmem-eval-continuity/src/generator.rs:2037`; `crates/cmem-eval/src/results.rs:514`; `crates/cmem-eval-runner/src/diff.rs:348`.
- Existing patterns or references: `crates/cmem-eval-locomo/src/error.rs` (`AdmissionLocation`, `LoadError::Admission`); dataset-admission plan (completed 2026-09-14) for the alias-destination assertion rule.
- Design record consulted and deviations from its acceptance: ADR-I-0005 (artifact readers stay derived serde). No deviation: only input parsers gain typed errors; reader tests only lose prose assertions.
- Audit source: `../CharacterMemory/.agent-work/reviewer/test-suite-audit-2026-09-16.md` section F3 (evals prose list); partition reports D and E.

## Open Questions (max 3)
- None.

## Assumptions
- A1: The fixture parser's rejections originate in its 33 `bail!` sites plus serde-originated shape failures, and can be enumerated into a closed kind set — source: `rg -c 'bail!' crates/cmem-eval-continuity/src/fixture.rs` (33); Task_1 reports how serde-originated failures are wrapped with their field name.
- A2: The runner's already-exists rejections share one producing branch — source: `pipeline.rs:1257` (verified at plan review).

## Tasks

### Task_1: Continuity fixture admission error and test re-anchoring
- type: impl
- owns:
  - crates/cmem-eval-continuity/**
- depends_on: []
- description: |
  Introduce the typed admission error for the fixture parser (implementing `std::error::Error` and `Display`), route every parser rejection through it, update in-crate callers, re-anchor the 25 admission tests onto variant, location and field, delete the two serde-behaviour reader tests and the redundant public-shape round-trip, and make the generator's cluster-count test assert its two numbers. Serde-originated failures are wrapped into the typed error with the field name carried, not asserted by serde's prose. Out-of-crate callers are not edited: the error type must convert into `anyhow` through `?` unchanged.
- acceptance:
  - No `contains(` on an error string remains in the crate's tests; every admission test asserts the variant, the location and the field.
  - Every `bail!` in the parser is replaced or wrapped; a census of remaining `anyhow` returns in the parser is reported (expected zero).
  - The new error type implements `std::error::Error`; the runner compiles without edits (checked by `cargo check --workspace`).
  - The checked fixture still parses and the byte-identity test passes unchanged.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo check --workspace; cargo test -p cmem-eval-continuity (executed count reported); rg -n 'contains\\(' crates/cmem-eval-continuity/src/fixture.rs (must list no error-string matches)"

### Task_2: Runner output admission typed error and diff assertion
- type: impl
- owns:
  - crates/cmem-eval-runner/**
- depends_on: []
- description: |
  Add a typed already-exists error beside the in-stores error, produce it from the single branch, and re-anchor the four admission tests on it; assert the diff summary's counters instead of its rendered line.
- acceptance:
  - The four output-admission tests downcast or match the typed error and assert its name field.
  - The diff test asserts `differing_queries == 0` and its sibling counters, not a rendered string.
  - The README continuity smoke recipe passes (two runs plus diff, all counts zero).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner (executed count reported; the symlink test's Windows exception noted if it fails); README continuity smoke recipe on configs/continuity_smoke.toml"

### Task_3: Shared core response validation typed error and results reader assertion
- type: impl
- owns:
  - crates/cmem-eval/src/openai_embedding.rs
  - crates/cmem-eval/src/results.rs
- depends_on: []
- description: |
  Give the file-local `ordered_embeddings` an owned error enum covering its five rejection branches, convert at the caller boundary within the file, and match variants in the five tests; make the results reader test assert that corrupt and truncated input are refused without quoting serde or std prose.
- acceptance:
  - Five variants, each produced by exactly one branch and asserted by exactly one test.
  - The results reader test asserts refusal of both inputs and no serde or std message text.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval (executed count reported)"

### Task_4: Dataset crates and converter reader assertions
- type: test
- owns:
  - crates/cmem-eval-locomo/src/lib.rs
  - crates/cmem-eval-longmemeval/src/lib.rs
  - crates/cmem-eval-benchmark-convert/src/lib.rs
- depends_on: []
- description: |
  Match the typed config error variant in the two own-name validation tests; merge the converter's two selection-reader rejection tests into one that asserts corrupt and partial input are refused, without serde prose.
- acceptance:
  - No message-substring assertion remains in the three files.
  - The converter has one reader rejection test covering corrupt and partial input and asserting refusal of each.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo; cargo test -p cmem-eval-longmemeval; cargo test -p cmem-eval-benchmark-convert (executed counts reported)"

### Task_5: Independent review
- type: review
- owns: []
- depends_on: [Task_1, Task_2, Task_3, Task_4]
- description: |
  The orchestrator integrates Wave 1 and runs the workspace validation commands on the integrated branch. The reviewer, in a pinned worktree with the library pin stated, runs a workspace-wide census of error-string `contains(` and `starts_with(` assertions and confirms every remaining one is on a non-error surface; verifies each new error type has per-branch negative tests (one per variant) and that admission still precedes side effects in the runner (output directory census after a rejected run); runs the workspace suite and the smoke recipe.
- acceptance:
  - Reviewer status is APPROVED with the census and the per-variant coverage table attached.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (executed counts) on the integrated branch"
  - kind: review
    required: true
    owner: reviewer
    detail: "Pinned worktree with the library pin stated; rg census; per-variant negative-test table; cargo test --workspace; smoke recipe; output-directory census after a rejected run"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1, Task_2, Task_3, Task_4]
- Wave 2 (parallel): [Task_5]

Parallel tasks run in separate worktrees so Cargo commands never share a target directory mid-edit; per-task validation is crate-scoped and the orchestrator runs the workspace-wide fmt, clippy and test commands on the integrated branch after each wave.

## Rollback / Safety
- Single branch `feature/2026-09-16/typed-admission-errors`; error types are additive on input surfaces; no artifact format changes; revertible as one commit.

## Progress Log (append-only)

- 2026-09-16 Plan drafted from the test-suite audit (F3, evals prose list).
- 2026-09-16 Reviewer pass (Tier A): fixture error must implement `std::error::Error` so the runner's two `?` call sites need no edit in the same wave; stray Task_4 text removed from Task_2; `ordered_embeddings` dropped from the compatibility surface (module-private); counts corrected (33 `bail!`, up to 90 `contains(`); reader tests state the surviving property; workspace-wide validation hoisted to the orchestrator.

- 2026-09-16 Decider accepted all eight plans; execution starts, PRs stacked.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-16 Decision: the fixture parser adopts the dataset loaders' typed admission shape rather than a message-prefix convention. Trigger: about 90 prose assertions in one file. User approval: pending.

## Notes
- Risks: serde-originated rejections in the fixture parser may not carry the field name cleanly; Task_1 reports how they are wrapped.
- Edge cases: the runner symlink test fails on this Windows machine (OS error 1314); Linux CI is authoritative for it.
