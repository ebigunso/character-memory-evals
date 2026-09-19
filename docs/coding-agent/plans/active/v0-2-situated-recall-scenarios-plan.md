# Plan: Situated-recall scenarios for the library's v0.2 phase

- status: draft
- generated: 2026-09-20
- last_updated: 2026-09-20
- work_type: code

## Goal
- Before the library implements situated recall, this repository can state each v0.2 catalog situation as a scenario with a checkable retrieval-tier property, and reports per property whether it passed, failed, or could not be asked of the library yet. As the library's v0.2 slices land, the same scenarios move from "cannot be asked" to pass or fail with no re-authoring, and the pollution and context-size baselines are re-measured once.
- Decision each part informs: the scenarios inform the library's acceptance of v0.2 (draft section 6) and the shape of its scene input; the loud-topic probe informs route-floor calibration; the re-baseline informs whether pack-admission changes cost recall or context.

## Definition of Done
- Every scenario group in the library's v0.2 draft, section 4, exists as at least one scenario that names its catalog situation and carries its retrieval-tier property as an expectation, checked with no language model.
- A continuity run reports, per expectation: pass, fail, or unsupported (the pinned library cannot be asked). A first run against library `d0fe82d` is recorded in this plan's Progress Log as the starting point.
- After the library's schema groundwork (entity as a notion, no interpreted-memory confidence, the value-audit deletions) the workspace builds, the three validation commands pass, and the smoke recipe runs, with no register-cited byte changed.
- After the library's scene and routes land, every scenario is asked (none unsupported), and the draft's section 6 retrieval-tier criteria can be read off one run report.
- The continuity baselines of ADR-I-0022 (pollution, context size, gap recall, the graph-only probe) are re-measured once at the library commit that closes v0.2 pack admission, with the comparability conditions from the census stated beside the numbers.
- README describes what ships; the plan closes in completed.

## Planner-added requirements
- An "unsupported" outcome beside pass and fail. Needed because: evaluation comes before the library work by ruling, so most properties cannot be asked of the pinned library on day one, and a scenario that silently does not run is indistinguishable from one that passes.
- A pair-gap measure (time since these participants last met) replacing the use of `gap_days` for D11. Needed because: `gap_days` is the age of gold-labelled events, which cannot check "elapsed time since the pair last met" (census, question 1).
- New scenarios use the controllable-similarity embedding provider. Needed because: their texts are in no frozen store, the frozen stores are register-cited bytes, and route properties do not depend on real embedding geometry. The loud-topic probe and the re-baseline are the exceptions (Open Question 1).

## Scope / Non-goals
- Scope: `crates/cmem-eval-continuity` (scenario language, generator, driver, metrics, report, a new fixture file); `crates/cmem-eval` (adapter and adapter contract, result keys) where the library's changed shapes pass through; `crates/cmem-eval-runner/src/enrichment.rs`, `crates/cmem-eval-locomo/src/ingest.rs`, `crates/cmem-eval-benchmark-convert`, and `scripts/enrichment/build_snapshots.py` for the schema groundwork only; one new config; README.
- Non-goals: the behavioral tier (disclosure, posture, generated retelling; scheduled with the library's generation phase); a general assertion language; any edit to `continuity_v3.json`, `continuity_benchmarks_v1.json`, the frozen stores, their manifests, or any register-cited config or artifact; LoCoMo and LongMemEval scoring changes; any library change (the library plan owns those); sealing any evidence.

## Design
- Chosen: one scenario language, extended. The continuity fixture schema gains, as optional fields under a new version, an authored scene on remember and on query (given as perceived: descriptions, names, keys; ADR-D-0029), an optional topic, an optional partition, typed derived memories with direction, due date and trigger, beliefs about a notion, and a closed list of expectation kinds. The existing v3 files stay loadable unmodified by the same loader. Expectations are checked against the library's native outcome (pack sections, trace, write outcomes), which the driver already retains. Structure: one loader, one driver, one report; the scene is authored in the fixture and mapped to the library's input in one place in the driver. Evolution: when the library's scene shape is set, only that mapping changes; a new expectation kind is one enum arm. Verification: fixture validation and expectation checking are unit-testable with no store; scenarios run service-free. Operation: no new runtime cost outside the new scenarios. Human: a scenario reads as the catalog situation it names. Safety: gold labels stay in expectations and never reach ingestion (common rule).
- Alternative: author the scenarios as Rust integration tests against the library's new API once it exists. Structure: no fixture change, but a second way to state a continuity scenario. Evolution: every library shape change edits every test. Verification: strongest typing, but nothing can be written until the library API exists, which inverts the evaluation-first ruling. Operation, Human, Safety: same.
- Alternative: a general predicate language over the native outcome (JSON paths and comparators). Structure: smaller Rust surface, larger fixture surface. Evolution: couples fixtures to the library's serialized field names, which the groundwork is about to change. Verification: errors surface at run time, not at fixture load.
- Why chosen: it is the only one that can be written before the library work and survive it, and the closed expectation list is what the census says is needed and no more (the census found no need for a general language). Fit: library v0.2 draft section 4 ("evaluation first", "scenarios are named by catalog situation and carry their retrieval-tier property"); this repository's strictness rule in `docs/coding-agent/rules/common.md`.

### The closed list of expectation kinds
Each is observed on the native outcome. A kind the pinned library cannot produce reports unsupported.
- section membership: a memory is, or is not, in a named pack section; optionally in a stated order within it (D5, D13).
- the scene reported on an admitted memory: participants, setting, when (B1, B2).
- trace facts: the scene was partial; a partition was applied; elapsed time since the pair last met; an omission with its reason (resolution, supersession, suppression, partition); which route admitted an item; a reference was ambiguous or unknown.
- staleness reported as age on a current item (D11, C6).
- a write warning was raised (near-verbatim restatement, churning chain; the C4 proxy).

### Scenario groups and the property each carries (from the v0.2 draft, section 4)
B1, B2, B3, D1 with D8, D4, D5, D7, D9, D11 with C6, D13, the graph-only probe under a loud topic, tasks and favors across long gaps, and C4 as a retrieval proxy only (same surviving basis and current state across scenes and times, no stale-current leakage, the write warning; it does not establish consistent retelling). Each group has a default case and, where the draft names one, a control (a non-matching date, an absent counterpart, the partition off and on).

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: the continuity fixture schema (new version, additive), the adapter contract types in `memory_adapter.rs` (entity, derived-memory and link inputs change with the library), result metric keys, the local untracked enrichment snapshots.
- stance: break
- justification: every consumer is in this workspace and the repository's compatibility policy is to track the library's latest surface with no shims. The exception is register-cited bytes, which are untouched by construction: v3 files stay loadable because they are inputs of live tests and the smoke, not for old-artifact parseability. Local snapshots are regenerated by their builder, and the 2026-09-16 record keeps the old hashes.

## Context (workspace)
- Related files/areas: `.agent-work/orchestrator/v0-2-eval-census-report.md` (the census this plan rests on; promote nothing from it); `crates/cmem-eval-continuity/src/{fixture,generator,driver,metrics,report}.rs`; `crates/cmem-eval/src/{memory_adapter,adapter,metrics,results,outcome}.rs`; `configs/continuity_smoke.toml`.
- Existing patterns or references: the driver forces the trace on and retains the native `RetrieveOutcome`; `flatten_outcome` loses section membership, so section expectations read the native pack; the orchestrator rule "treat a forthcoming public API as the target contract and isolate current unavailability".
- Design record consulted and deviations from its acceptance: library ADR-D-0019, D-0022, D-0024, D-0029, D-0030, D-0034, ADR-I-0020, ADR-I-0022; this repository's ADR-I-0004 and ADR-I-0005. No deviation.

## Open Questions (max 3)
- Q1: The loud-topic probe and the re-baseline need real embedding geometry. Extending coverage to new texts means one live embedding call to freeze a new store (new file, new hash; existing stores untouched). Recommended: yes, once, in Task_6. Needs the decider's authorization because it spends on the provider.
- Q2: Whether the LoCoMo and LongMemEval hybrid runs (live provider, eight runs last time) are repeated for v0.2. Recommended: not in this plan; once at the library's v0.2 closeout under its own authorization, since this plan's re-baseline is the continuity one the draft names.

## Assumptions
- A1: After the entity becomes a notion, the driver can author a v3 entity's label as the text of a naming belief, so the frozen stores still cover the v3 and benchmark fixtures. Source: unverified (the library's belief shape and embedding text are not set); checked by Task_4, which stops and reports if it fails rather than touching a store.
- A2: The library's v0.2 trace exposes the facts in the expectation list under public fields. Source: v0.2 draft section 3; checked by Task_5. A fact the library does not expose is raised to the library plan, never inferred here.
- A3: Library `main` moving breaks this repository's CI until Task_4 lands, because the dependency is a path and CI resolves library `main`. Source: census question 6. The Orchestrator sequences Task_4 against the library's groundwork branch so the two merge together.

## Tasks

### Task_1: The scenario language can state a situated scenario
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/fixture.rs
  - crates/cmem-eval-continuity/src/lib.rs
- depends_on: []
- description: |
  Extend the fixture schema under a new version with the authored scene, optional topic, optional partition, typed derived memories (subtype, actor and counterpart, due date, trigger), beliefs about a notion, the catalog situation a scenario serves, and the closed expectation list in the Design section. Validation at load. The shapes are as-perceived and library-neutral; the library's Rust types are not the model.
- acceptance:
  - A scenario with a scene and no topic, a partition, a typed commitment with direction and due date, and one expectation of each kind loads and validates.
  - `continuity_v3.json` and `continuity_benchmarks_v1.json` load unmodified through the same loader, and their byte-identity tests still pass.
  - A field introduced by the new version is rejected in a version 3 file; an expectation that references an undeclared memory is rejected at load.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review against acceptance; confirm no register-cited byte changed (hashes in the census report)"

### Task_2: The v0.2 situations exist as scenarios
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/generator.rs
  - crates/cmem-eval-continuity/fixtures/continuity_situated_v1.json
- depends_on: [Task_1]
- description: |
  Author the scenario groups listed in the Design section, each naming its catalog situation, with its default case and control. Use the controllable-similarity provider. Read the library's catalog sections B, C4, C6 and D and the v0.2 draft sections 1, 2 and 6 for what each property means; ADR-D-0019 governs B1 and B2 (the default is recall across scenes with the scene reported; omission only under an explicit partition).
- acceptance:
  - Every group in the list has at least one scenario, and each scenario's expectations state that group's property and nothing the behavioral tier owns.
  - Scenes are given as perceived in at least three forms across the set: by key, by name, and by description, including one ambiguous and one unknown reference.
  - Old-but-current memories are never labelled as negatives; an expected omission always names its reason.
  - The generator reproduces the fixture file byte for byte.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier A altitude review of the scenarios against the catalog and the v0.2 draft (do they test the situation or a mechanism?), plus Tier D on the diff"

### Task_3: A run reports pass, fail, or unsupported per expectation
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/metrics.rs
  - crates/cmem-eval-continuity/src/report.rs
  - configs/continuity_situated.toml
- depends_on: [Task_1]
- description: |
  Check expectations against the native outcome and write outcomes; report per expectation and per scenario group. Whatever the pinned library cannot be asked (a scene, a reference time, a partition, a typed memory it cannot take) is unsupported with the reason, never a silent skip and never a fail. Add the pair-gap measure. The run reports; it does not enforce thresholds.
- acceptance:
  - Against library `d0fe82d` the situated fixture runs to completion service-free, every expectation has one of the three outcomes, and each unsupported outcome names what the library lacks.
  - Section expectations read the native pack, not flattened items.
  - Two runs of the situated config diff to zero differences.
  - The v3 smoke recipe is unchanged in output.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe; two runs of configs/continuity_situated.toml and a diff"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review; confirm run stores were cleaned up"

### Task_4: The harness follows the library's schema groundwork
- type: impl
- owns:
  - crates/cmem-eval/src/**
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/generator.rs
  - crates/cmem-eval-runner/src/enrichment.rs
  - crates/cmem-eval-locomo/src/ingest.rs
  - crates/cmem-eval-benchmark-convert/src/lib.rs
  - scripts/enrichment/build_snapshots.py
  - scripts/enrichment/README.md
  - README.md
- depends_on: [Task_3]
- description: |
  External dependency: the library's schema-groundwork branch (named in the Decision Log when it exists). Track the library's new shapes with no shims: names and kinds become beliefs about a notion, interpreted-memory confidence goes, and whatever the library's value audit deletes goes with it. Link confidence follows the library's ruling, not this plan. Regenerate the local snapshots with their builder. The census (question 3) is the site list.
- acceptance:
  - The workspace builds against the groundwork branch and the three validation commands pass.
  - The smoke recipe runs and two runs diff to zero; any difference from the pre-groundwork smoke output is listed with its cause.
  - No register-cited byte changed; if assumption A1 fails, the task stops and reports instead of touching a fixture or store.
  - Source speaker attribution survives the move to beliefs in the converter and the snapshot builder.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe twice and a diff; sha256 of the six protected assets against the census table"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review"

### Task_5: The scenarios are asked of the library's scene and routes
- type: impl
- owns:
  - crates/cmem-eval/src/memory_adapter.rs
  - crates/cmem-eval/src/adapter.rs
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/report.rs
- depends_on: [Task_4]
- description: |
  External dependency: the library slices that add the scene, the reference time, the partition, the routes and the trace facts. Map the authored scene to the library's input in one place; forward the reference time (today `query_date` stops at the adapter). May land in steps as library slices merge; each step reduces the unsupported count and never re-authors a scenario to fit the library.
- acceptance:
  - No expectation in the situated fixture is unsupported.
  - A scenario that fails is reported to the library plan's owner with the trace, not adjusted.
  - The report lets the v0.2 draft's section 6 retrieval-tier criteria be read off one run.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; two situated runs and a diff"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review; Tier A check that no scenario was weakened to pass"

### Task_6: The continuity baselines are re-measured once
- type: test
- owns:
  - configs/continuity_situated_loud_topic.toml
  - crates/cmem-eval-continuity/fixtures/embeddings/**
  - docs/coding-agent/plans/active/v0-2-situated-recall-scenarios-plan.md
- depends_on: [Task_5]
- description: |
  At the library commit that closes v0.2 pack admission: the loud-topic probe (state and time floors hold while the content route is saturated), and one re-measurement of pollution, context size, gap recall and the graph-only probe on the canonical 15-scenario set, under the comparability conditions in the census (question 4). New frozen files only; existing stores untouched. Lab-notebook grade: numbers, config and input hashes, and both commits in the Decision Log; nothing sealed.
- acceptance:
  - Before and after numbers sit side by side with every intentional difference (library commit, ingest shape, floors) named.
  - Deterministic: the run repeats with zero differences.
  - All run stores cleaned up.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the runs and diffs named above; sha256 of the protected assets"
  - kind: review
    required: true
    owner: orchestrator
    detail: "Numbers recorded in the Decision Log and handed to the library plan"
  - kind: manual
    required: true
    owner: user
    detail: "Authorization for the one live embedding call (Open Question 1) before it is made"

## Task Waves (explicit parallel dispatch sets)

- Wave 1: [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3: [Task_4] (starts when the library groundwork branch exists)
- Wave 4: [Task_5] (follows library slices; may be several steps)
- Wave 5: [Task_6]

Waves 1 and 2 need nothing from the library and start on approval. Each wave's PR stacks on the previous one.

## Rollback / Safety
- Every change is additive to the fixture schema or follows a library change; reverting a PR restores the previous state. Protected bytes are verified by hash in every task that could touch their neighbourhood.

## Progress Log (append-only)

- (none yet)

## Decision Log (append-only; re-plans and major discoveries)

- (none yet)

## Notes
- Risks: the library's scene shape may want something the as-perceived fixture scene cannot say; that is a finding for the library plan, and the fixture follows the catalog, not the API. The expectation list may grow; each addition names the scenario that needs it.
- Edge cases: Task_2 and Task_4 both own `generator.rs` and Task_3, Task_4 and Task_5 own `driver.rs`; they are in different waves.
