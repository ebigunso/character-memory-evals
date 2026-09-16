# Plan: Workspace tests pay for each contract once and stop pinning fixture statistics

- status: in_progress (approved by the decider 2026-09-16; Tier D APPROVED 2026-09-17 at a7dd8f5; pull request open, awaiting merge approval)
- generated: 2026-09-16
- last_updated: 2026-09-17
- work_type: code

## Goal
- `cargo test --workspace` runs the full continuity fixture once, classifies enrichment manifests at the loader rather than through full pipeline runs, spawns no subprocess for a determinism proof in the shared core (the env-isolation test keeps its child process), and pins no dataset or fixture count that the byte-identity test already guards; duplicate and framework-behaviour tests are gone, and one-shot dataset censuses live as runnable examples outside the test suite.

## Definition of Done
- The runner's continuity command test runs the fixture once and asserts the trace's invariants (artifact identity, header provenance, row-trace congruence) without a baseline artifact; the two-run determinism comparison remains the README smoke recipe the reviewer already runs; the driver's operation-coverage sweep is the only other full-fixture run; the command test's no-overwrite and serde assertions are covered at unit level.
- The manifest classification matrix lives in `enrichment.rs` against the snapshot loader; the pipeline keeps two cases proving admission precedes run-root and adapter creation.
- The four shared-core subprocess determinism tests and their probes are deleted; the continuity generator's byte-identity test (`generator.rs:2001`), its parent cross-process determinism test and the child probe the parent launches are unchanged (Decision Log 2026-09-17, Task_3 alert).
- Literal-count assertions over the checked fixture, the metric registry cardinality and the committed benchmark store are replaced by the property each protects or deleted; the converter's re-typed manifest becomes invariant assertions; the two ignored official-file censuses become `cargo run --example` binaries under their dataset crates with a thin `scripts/` wrapper each, and the workspace ignored-test count goes from 2 to 0.
- Duplicate and framework-behaviour tests named in the audit (config parse-only and retired-key triple coverage, results single-field serde, tiktoken behaviour, const-equals-literal, legacy-hash reimplementation, loader normalization duplicates, the byte-identical io/json twins, the alias cross-products, the enum delegation, the non-overlapping overlap test, the ungrouped grouping test, the command-test overlap, the diff derived-serde test) are removed or reshaped as the audit states.
- Wall time of `cargo test -p cmem-eval-runner` and `cargo test -p cmem-eval-continuity` is reported before and after; workspace validation passes on the integrated branch with executed counts.

## Planner-added requirements
- None

## Scope / Non-goals
- Scope: test code across `crates/cmem-eval-runner`, `crates/cmem-eval`, `crates/cmem-eval-continuity`, `crates/cmem-eval-locomo`, `crates/cmem-eval-longmemeval`, `crates/cmem-eval-benchmark-convert`; new `examples/` under the two dataset crates; `scripts/` receives two wrappers and README lines.
- Non-goals: production changes; typed errors (earlier plan); new gap tests (next plan); the continuity byte-identity and parent cross-process determinism tests (kept, see Decision Log); committing any new baseline artifact.

## Design
- Chosen: assert each property at the cheapest boundary that observes it, keep byte-identity as the single drift guard for the checked fixture, keep the two-run determinism check as the README smoke recipe rather than a test, and move statistics gathering into runnable examples. Structure: one owner per contract; no new committed artifact. Evolution: deliberate fixture regeneration edits one hash, not a dozen counts (fits "strictness follows the claim", rules common.md, and reviewer.md "run twice only when sealing"). Verification: the suite is faster and every failure names a property; examples compile under `clippy --all-targets`. Operation: fewer full-fixture runs per CI. Human: censuses are examples, read as such. Safety: unchanged.
- Alternative: commit a baseline trace and diff the single test run against it. Structure: a new committed artifact with a regeneration recipe. Evolution: every deliberate trace-shape change regenerates and re-reviews the baseline; the reviewer already runs the smoke recipe that gives the same determinism evidence. Verification: equivalent detection at higher maintenance.
- Why chosen: the smoke recipe already carries the two-run claim; a committed baseline would duplicate it and add an artifact the rules say to avoid outside sealing.

## Compatibility stance
- surface: none; test code, examples and scripts only.
- stance: preserve
- justification: no production surface or artifact format changes.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-runner/src/pipeline.rs:2074,2395,2563 (two full-fixture runs),2899,3203,3229 (15 cases x 2 datasets through full runs)`, `diff.rs:362`, `enrichment.rs:424`, `commands_tests.rs:5`; `crates/cmem-eval/src/{config.rs:884,993,1021,1049,1075,1113,1129; results.rs:329; frozen_embedding.rs:635,665; controllable_similarity_embedding.rs:330,355; openai_embedding.rs:190; token_count.rs:18,23,33,46; deterministic_embedding.rs:76}`; `crates/cmem-eval-continuity/src/{generator.rs:2001 (byte-identity),2136 (parent cross-process test),2162 (child probe),2172,2309-2330,2507,2597,2129; metrics.rs:775,819; driver.rs:1164 (operation-coverage sweep),1462; report.rs:334}`; `crates/cmem-eval-locomo/{tests/admission.rs:5,341; tests/benchmark_derived_content.rs:202 (ignored census calling load_path); src/loader.rs:598,629}`; `crates/cmem-eval-longmemeval/{tests/admission.rs:5,379; tests/repeated_sessions.rs:154 (ignored census); src/loader.rs:288}`; `crates/cmem-eval-benchmark-convert/src/lib.rs:1292`.
- Existing patterns or references: reviewer.md "run twice only when sealing; otherwise compare the run to the stored baseline with `diff`" and the README smoke recipe; `scripts/README.md` (shell and Python today; nothing there can call Rust loaders, hence examples).
- Design record consulted and deviations from its acceptance: the 2026-09-02 design-value audit (two-run gate is seal-on-demand only). No deviation.
- Audit source: `../CharacterMemory/.agent-work/reviewer/test-suite-audit-2026-09-16.md` sections F3 (censuses, constants) and F6; partition reports D and E.

## Open Questions (max 3)
- None.

## Assumptions
- A1: The continuity command test's non-determinism assertions (artifact identity, provenance header, row-trace congruence) can be asserted from a single run — source: the test body at `pipeline.rs:2563` already asserts them on run A before comparing to run B.
- A2: The two official-file censuses depend only on public loader functions and can run as `cargo run --example` binaries taking the dataset path from an environment variable — source: `tests/benchmark_derived_content.rs:202`, `tests/repeated_sessions.rs:154` (env-var paths, `load_path`).

## Tasks

### Task_1: Runner tests run the fixture once and classify manifests at the loader
- type: test
- owns:
  - crates/cmem-eval-runner/**
- depends_on: []
- description: |
  Reduce the continuity command test to one run asserting the trace's invariants, moving its no-overwrite and serde checks to unit level and leaving two-run determinism to the README smoke recipe; move the manifest classification cases to `enrichment.rs` against the snapshot loader, keeping two ordering cases in the pipeline; drop the 19-key census, the 646 count, the malformed-dataset matrix down to one case per dataset, the derived-serde diff test, the command-test flag overlap; make the enrichment loader test group across two namespaces.
- acceptance:
  - `pipeline.rs` invokes the full continuity fixture at most once per test run and no test reads a baseline artifact.
  - Manifest classification has one test per defect class in `enrichment.rs` and two ordering tests in `pipeline.rs`.
  - Wall time of the crate's tests is reported before and after.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner (executed count and wall time before and after; Windows symlink exception noted); README continuity smoke recipe (two runs plus diff, all counts zero)"

### Task_2: Shared core drops subprocess, framework and duplicate tests
- type: test
- owns:
  - crates/cmem-eval/**
- depends_on: []
- description: |
  Delete the frozen and controllable subprocess determinism tests with their probes, the const-equals-literal and tiktoken-behaviour tests, the legacy-hash reimplementation, the results single-field serde test; merge the config parse-only pairs and fold retired keys into the container-boundary case table.
- acceptance:
  - No test in the crate spawns a process for a determinism proof; the env-isolation test keeps its child process (Decision Log 2026-09-17, Task_2 alert).
  - Each deleted or merged test is mapped to its surviving observer.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval (executed count reported); rg -n 'Command::new' crates/cmem-eval/src (the only test-module hit is the env-isolation test's child process, per the 2026-09-17 Task_2 decision)"

### Task_3: Continuity tests assert properties, not fixture statistics
- type: test
- owns:
  - crates/cmem-eval-continuity/**
- depends_on: []
- description: |
  Replace literal count assertions in the generator tests with the property each protects, keep the child cross-process probe, the parent determinism test and the byte-identity test unchanged, delete the private chrono helper test and the enum-delegation driver test, fix or delete the non-overlapping overlap metric test, assert the metric key set instead of its cardinality, keep the report reader's no-overwrite half only, and keep the driver's operation-coverage sweep as the crate's one full-fixture run.
- acceptance:
  - No generator test asserts an integer count of events, queries, links or contrasts of the checked fixture.
  - The byte-identity test (`generator.rs:2001`), the parent cross-process determinism test and its child probe are unchanged (Decision Log 2026-09-17, Task_3 alert).
  - Wall time of the crate's tests is reported before and after.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-continuity (executed count and wall time before and after)"

### Task_4: Dataset crates and converter drop censuses and cross-products
- type: test
- owns:
  - crates/cmem-eval-locomo/**
  - crates/cmem-eval-longmemeval/**
  - crates/cmem-eval-benchmark-convert/**
  - scripts/**
- depends_on: []
- description: |
  Turn the two ignored official-file censuses into `examples/` binaries under their dataset crates, each taking the dataset path from the same environment variable as today, with a thin `scripts/` wrapper and a README line each; loop the alias admission tests per axis instead of as cross-products, still asserting the typed destination field; delete the byte-identical io/json twins (or keep one) and the loader tests duplicated by ingest and admission; replace the converter's re-typed manifest with invariant assertions (abstention implies empty gold, session count bound, fixture-id derivation).
- acceptance:
  - No `#[ignore]` test remains in the dataset crates; the workspace ignored-test count goes from 2 to 0; the examples build under `cargo clippy --workspace --all-targets -- -D warnings` and reproduce the censuses from the env-var paths.
  - The alias tests execute one case per axis value and assert admission in the typed destination field (reviewer.md alias rule).
  - The converter test asserts invariants over the committed manifest, not a transcription of it.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo; cargo test -p cmem-eval-longmemeval; cargo test -p cmem-eval-benchmark-convert (executed counts reported); cargo clippy -p cmem-eval-locomo -p cmem-eval-longmemeval --all-targets -- -D warnings; one example run per crate against the local official file, output attached"

### Task_5: Independent review
- type: review
- owns: []
- depends_on: [Task_1, Task_2, Task_3, Task_4]
- description: |
  The orchestrator integrates Wave 1 and runs the workspace validation commands on the integrated branch. The reviewer, in a pinned worktree, confirms each deleted test's surviving observer, counts full-fixture runs per workspace test run (expected two: driver sweep and runner single run), confirms no process spawn remains in shared-core tests and the ignored count is zero, compares wall times, and runs the workspace suite plus the smoke recipe.
- acceptance:
  - Reviewer status is APPROVED with the full-fixture run census, ignored-count census and wall-time comparison attached.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (executed counts, ignored count, wall time) on the integrated branch"
  - kind: review
    required: true
    owner: reviewer
    detail: "Pinned worktree; diff review against acceptance; full-fixture run census; process-spawn census; ignored-count census; wall-time comparison; smoke recipe"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1, Task_2, Task_3, Task_4]
- Wave 2 (parallel): [Task_5]

Parallel tasks run in separate worktrees so Cargo commands never share a target directory mid-edit; per-task validation is crate-scoped and the orchestrator runs the workspace-wide commands on the integrated branch after each wave.

## Rollback / Safety
- Single branch `feature/2026-09-16/test-cost-and-census-cleanup` stacked on the adapter-ownership branch; test code, examples and scripts only; revertible as one commit.

## Progress Log (append-only)

- 2026-09-16 Plan drafted from the test-suite audit (F3 censuses, F6).
- 2026-09-16 Reviewer pass (Tier A): baseline-artifact dependency removed (single run asserts invariants; two-run determinism stays in the smoke recipe); censuses become `cargo run --example` binaries because `scripts/` cannot call Rust loaders; Task_3 acceptance reconciled with its description; deviation from the audit on the continuity cross-process test recorded; ignored-count census added; workspace-wide validation hoisted to the orchestrator.

- 2026-09-17 Wave 1 completed: [Task_1 8659f04, Task_2 dcd52c9, Task_3 03b3084, Task_4 caa137d] (integrated tip caa137d)
  - Summary: runner command test runs the fixture once and asserts invariants (no baseline artifact); manifest classification moved to enrichment.rs with two ordering cases kept; shared-core subprocess determinism pairs, framework and duplicate tests removed; generator tests assert properties rather than fixture counts; the trace-file assertion block moved from the runner to driver.rs unit tests; the two official-file censuses are cargo run --example binaries with scripts/ wrappers; alias tests loop per axis; converter manifest test asserts invariants.
  - Validation evidence (orchestrator at caa137d, from the short alias): cargo fmt --all --check clean; cargo clippy --workspace --all-targets -D warnings clean (examples included); cargo test --workspace: cmem-eval 104, benchmark-convert 10, continuity 72, locomo 10+29+4, longmemeval 7+22+3, runner 57 passed plus the known symlink exception; ignored count 0 (was 2). Runner crate wall time 255 s to 76 s (Task_1 report); continuity 73 s to 97 s with the trace-file unit tests added (Task_3 report, no speedup claimed).

- 2026-09-17 Wave 2 completed: [Task_5] (APPROVED at a7dd8f5, no findings)
  - Validation evidence (reviewer, pinned worktree): fmt and clippy clean; workspace 318 passed plus the known symlink exception, ignored 0; runner integration 1 passed; smoke diff all zero; regenerated fixture bytes match; both official-file examples reproduce the known counts with source hashes unchanged; manifest probes leave no run root. Process census: the retained env-isolation test, the generator parent-probe pair, and a pre-existing Windows junction setup in the pipeline tests are the only child processes, none a determinism proof.
- 2026-09-17 Copilot follow-up (PR 50): the LongMemEval census example opened its dump path with an unconditional write, so a dump path aliasing the dataset would truncate the official file after reading it; it now uses `File::create_new` like the repository's other artifact writers and refuses an existing path (README updated).
- 2026-09-17 Copilot follow-up (PR 50): the LoCoMo vector baseline's raw-ingestion flag assertions are unconditional again (both baseline configs set them false; the conditional had been introduced by Task_1 without a recorded reason); the census move surfaced that no test observed the loader's official `YYYY/MM/DD (Day) HH:MM` normalization (the retired census only asserted propagation), so one loader unit test asserts the RFC3339 value.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-16 Decision: branch cut from the adapter-ownership branch tip (PR 48, Tier D approved) per the stack order. The typed-admission review's classified census of about 100 remaining error-string assertion lines (carried from that plan's Decision Log) is input here: any such line inside a test this plan deletes or reshapes goes with it; lines this plan does not touch stay out of scope and are reported in the Wave 1 reports as remaining.

- 2026-09-17 Decision (Task_2 alert): adapter::tests::oxigraph_env_cannot_redirect_graph_path and its child probe stay unchanged as an explicit exception; the test guards run-root ownership and needs a child process because environment mutation is process-global. Acceptance reads: no process spawn remains for determinism proofs.
- 2026-09-17 Decision (Task_3 alert): the generator parent determinism test and its child probe stay unchanged; the probe must remain a discoverable test for the parent to launch it with --exact, so the Definition of Done line about removing its test attribute is withdrawn (it would only move the probe from passed to ignored).
- 2026-09-17 Decision (Task_1 alert): the trace-file no-overwrite, canonical-order and additive-serde assertions moved from the runner command test to driver.rs unit tests in the continuity crate, which owns write_continuity_traces and read_continuity_traces.
- 2026-09-17 Decision (Task_4 alert): the converter manifest invariants are source-specific: LongMemEval entries assert gold_turn_ids_empty equals abstention; LoCoMo entries keep the absent-gold rule through validate and the evidence-image proof; session bounds and fixture-id derivation hold for every entry.
- 2026-09-16 Decision: byte-identity is the single drift guard for the checked fixture; counts over it are not tests. Trigger: audit F3 and F6. Decider approval: plan accepted 2026-09-16; merge approval pending.
- 2026-09-16 Decision: the continuity generator's parent cross-process determinism test is kept although the audit's F6 reasoning deletes the shared-core subprocess tests. Rationale: the fixture bytes back durable, hash-cited claims (sealed evidence, register citations), whereas the shared-core tests proved a JSON map lookup and pinned f32 bits, which in-process tests already cover. (The 2026-09-17 Task_3 decision below keeps the child probe as a discoverable test.) Decider approval: plan accepted 2026-09-16; merge approval pending.
- 2026-09-16 Decision: no baseline trace is committed; the single-run test asserts invariants and the README smoke recipe carries the two-run determinism claim. Trigger: reviewer finding that a gitignored baseline cannot run on a fresh clone or in CI.

## Notes
- Risks: examples add compile targets to `clippy --all-targets`; they must stay warning-clean.
- Edge cases: the alias per-axis loops must still assert the typed destination field, not just admission.
