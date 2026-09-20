# Plan: Situated-recall scenarios for the library's v0.2 phase

- status: in_progress
- generated: 2026-09-20
- last_updated: 2026-09-20
- work_type: code

## Goal
- Before the library implements situated recall, this repository can state each v0.2 catalog situation as a scenario a person can read and judge, with assertions that must hold and measures that are reported, and says of each scenario whether it passed, failed, or was not run because the pinned library cannot be asked yet. As the library's v0.2 slices land, the same scenarios start running with no re-authoring, and the pollution and context-size baselines are re-measured once.
- Decision each part informs: the assertions inform the library's acceptance of v0.2 (draft section 6) and the shape of its scene input; recall by reason under the loud-topic set informs route-floor calibration; bystander share and context tokens inform the cost of situated recall; the re-baseline informs whether pack-admission changes cost recall or context.

## Definition of Done
- Every scenario group in the library's v0.2 draft, section 4, exists as at least one scenario that names its catalog situation and states its retrieval-tier property as assertions on its probes and writes (there is no separate property field; the assertions are the property), checked with no language model.
- A run reports each scenario as passed, failed, or not run with the missing features named, and reports recall of what the moment calls for grouped by reason, bystander share, and context tokens. A first run against library `d0fe82d` is recorded in the Progress Log as the starting point.
- After the library's schema groundwork the workspace builds, the three validation commands pass, and the smoke recipe runs, with no register-cited byte changed.
- After the library's scene and routes land, no scenario is "not run", and the draft's section 6 retrieval-tier criteria can be read off one run report.
- The continuity baselines of ADR-I-0022 (pollution, context size, gap recall, the graph-only probe) are re-measured once at the library commit that closes v0.2 pack admission, with the comparability conditions from the census stated beside the numbers.
- README describes what ships; the plan closes in completed.

## Planner-added requirements
- A "not run" outcome beside passed and failed, decided for a whole scenario from the features it needs. Needed because: evaluation comes before the library work by ruling, so most scenarios cannot be asked of the pinned library on day one, and a scenario that runs with part of its input dropped can pass for the wrong reason.
- New scenarios use the controllable-similarity embedding provider. A scenario may set `embedding.own_concept = true`, and then a text with no assigned concept is its own concept; absent or false keeps the strict rule that every text is assigned exactly once, which is what protects generated fixtures. Needed because: their texts are in no frozen store, the frozen stores are register-cited bytes, cue properties do not depend on real embedding geometry, and hand-authored scenarios cannot reasonably assign a concept to every text. The loud-topic set uses the same provider, since it saturates the content cue by construction.
- A repeat comparison over scenario outcomes and assertion results. Needed because: the CLI diff compares retrieved identities and metrics only, so an assertion could flip between two runs while the diff reports zero differences.

## Scope / Non-goals
- Scope: `crates/cmem-eval-continuity` (scenario language, loader, generator, driver, metrics, report, new fixture files); `crates/cmem-eval-runner` (handoff of scenario outcomes into the run output, the repeat comparison, config tests); `crates/cmem-eval` (adapter and adapter contract) where the library's shapes pass through; `crates/cmem-eval-locomo/src/ingest.rs`, `crates/cmem-eval-benchmark-convert`, and `scripts/enrichment/build_snapshots.py` for the schema groundwork only; new configs; README.
- Non-goals: the behavioral tier (disclosure, posture, generated retelling; scheduled with the library's generation phase); a general assertion language; write-side scenes given by name or description (resolving them is consolidation's work in the library's v0.3, so v0.2 write-side scenes use keys, identity or setting); replaying experiences through reflection and comparing against the authored derived memories (the shape allows it; nothing is built for it); the LoCoMo and LongMemEval hybrid runs (decided 2026-09-20: once at the library's v0.2 closeout under their own authorization); any edit to a register-cited fixture, store, manifest, config or artifact; any library change; sealing any evidence.

## Design
- Chosen: a scenario is a readable story of experiences, the derived memories a caller authors from them, and probes. Narrative scenarios are hand-authored in one TOML file; scale scenarios (the loud-topic set) are generated as JSON in the existing pattern; both deserialize into the same types, chosen by file extension. The new shape is additive to the current one, so there is one loader and one shape, no version dispatch and no tolerance code, and the loader stays fail-closed as an input contract (unknown keys rejected, the `schema_version` equality check kept and unchanged while the change is additive), in the TOML form as in JSON, because a misspelled assertion key that is silently ignored passes vacuously: the existing checked fixtures keep loading only because nothing in them changed meaning, and the day a shape change invalidates them (Task_4 may be that day) they are regenerated as new files and the old ones remain as sealed bytes nothing reads. Structure: one loader, one driver, one report; the authored scene is mapped to the core adapter contract in one place in the continuity driver, with the supported feature set as a const beside it and a drift test between the two; the core crate carries and forwards fields and learns nothing about scenarios (ADR-I-0004). Evolution: when the library's scene shape is set, only that mapping and the feature set change; the same experiences and derived memories can later serve the generation phase. Verification: loading, feature derivation and assertion checking are unit-testable with no store; scenarios run service-free. Operation: no new runtime cost outside the new scenarios. Human: the decider can read a scenario and judge whether it is the catalog situation. Safety: gold (what a reference means, assertions, bystanders) never reaches the library; a participant given by name or description reaches it as that text only.
- Alternative: build the scenarios with the Rust generator and check in JSON, extending `Remember` and `Query` in place. Structure: no second file format. Evolution: same. Verification: byte-identity tests for free. Human: a scenario is builder calls, so judging it against the catalog means reading code; and `Remember` writes an episode, an observation and a generic reflection from one text, which cannot state a relationship state, a last interaction and obligations in both directions as distinct memories with distinct evidence (D4).
- Alternative: Rust integration tests against the library's new API once it exists. Verification: strongest typing, but nothing can be written before the library API exists, which inverts the evaluation-first ruling, and every library shape change edits every test.
- Alternative: a general predicate language over the native outcome. Evolution: couples fixtures to the library's serialized field names, which the groundwork is about to change; errors surface at run time, not at load.
- Why chosen: it can be written before the library work and survive it, it separates what happened from what the character holds about it (the library's own model, ADR-D-0028), and it keeps assertion and measurement apart so ADR-D-0019 is never violated by a measure. Fit: library v0.2 draft section 4; this repository's strictness rule in `docs/coding-agent/rules/common.md`; the compatibility policy (no dual paths) for the loader; ADR-I-0005 governs run artifacts only and itself warns against loosening input contracts.

### The scenario shape
The contract lives with the code: `crates/cmem-eval-continuity/README.md` documents the shape, each assertion and its identity, the measures, the cue vocabulary, the needed features and the not-run rule, and how to author and run a scenario. This plan keeps the intent that contract serves:
- A scenario is a readable story: experiences, the derived memories a caller authors from them, and probes, in named scenes given as perceived. Gold (what a reference means, assertions, bystanders) cannot reach the library.
- Assertions must hold; measures are only reported. There is no bare absence assertion and no memory is a bystander because of the scene it was formed in, because recall is never gated by default (ADR-D-0019). An omission always names its reason, and an old-but-current memory is never a negative merely for being old (ADR-D-0018).
- The carried reason is the author's catalog-level why and only groups recall. Where cues co-occur in the situation itself, `cued` and `not_cued` read the trace's cue kinds, one closed vocabulary; current state is not a cue kind.
- The loader is a fail-closed input contract in TOML as in JSON; computed gold (elapsed since the pair last met, staleness) comes from authored timestamps, never authored numbers.
- A result is a real answer from the library or an explicit not run, decided per scenario and statically from what the harness can forward end to end; nothing is dropped or coerced to make a scenario run, and a value the harness already forwards is a forwarding test, not a feature.
- Recall by reason is pooled over (probe, carried memory) pairs and is null, not zero, for a scenario that was not asked; cost is the context share of labelled near-miss distractors and context tokens.

### Scenario groups (from the v0.2 draft, section 4)
B1, B2, B3, D1 with D8, D4, D5, D7, D9, D11 with C6, D13, tasks and favors across long gaps, the loud-topic set (generated: cued items still carried while the content cue is saturated), and C4 as a retrieval proxy only (same surviving basis and current state across scenes and times, no stale-current leakage, the write warning; it does not establish consistent retelling). Each group has a default case and, where the draft names one, a control. Probe-side scenes exercise key, name and description, including one ambiguous and one unknown reference. B1 and B2 check that a memory from another scene is recalled with its scene reported; nothing asks for a memory to be withheld by the scene (library ADR-D-0038, Task_8).

### What the scenarios require of the library (handed to the library plan)
- The trace names every cue kind that admitted an item, not only the first and not only the route. Several cue kinds may share a route (due, date and own day are all time; pair and activity are both entity), and a D9 control cannot be read from a route-level trace because the memory legitimately arrives by the pair cue. Grounded in ADR-D-0022: each cue kind's admission is measured against starvation.
- No recall is gated by the scene, by default or by option, and the library computes no audience verdict (library ADR-D-0038, which replaced ADR-D-0019). The scenarios ask only that each admitted memory's scene is reported as recorded and the present scene as given. The earlier partition probes were removed by Task_8.
- The pair cue needs a counterpart other than the character's own entity. The self is a participant in every scene (ADR-D-0020), so if an admission through the self alone is labelled pair, every pair control fails.
- The trace's cue kinds include admission for being recent and salient. The draft calls importance and recency weights on activation, yet section 6 lists recent high-salience episodes among what a scene with no topic returns, so something admits them; D1 asserts `cued` for it and the assertion stays: the decider ruled on 2026-09-21 that this admission is the recency cue with salience as its weight. If the trace cannot expose it, Task_5 reports the missing capability and the scenario stays not run or failing; the fixture is not changed.
- The C4 churn example is six restatements within one minute, with the warning asserted on the final replacement only; the threshold is the library plan's to set.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: the continuity fixture shape (additive; a TOML form beside JSON), the adapter contract types in `memory_adapter.rs` (change with the library at Task_4), run output (gains scenario outcomes), the local untracked enrichment snapshots.
- stance: break
- justification: every consumer is in this workspace, the compatibility policy is to track the latest surface with no shims, and ADR-I-0005 guarantees sealed evidence as bytes by hash only. No code exists to keep an old fixture loadable; when a shape change invalidates the checked fixtures they are regenerated as new files (the frozen-store manifests bind to texts, not to a fixture hash, so the stores still apply) and the re-baseline names the changed input hash as an intentional difference.

## Context (workspace)
- Related files/areas: `.agent-work/orchestrator/v0-2-eval-census-report.md` (the census this plan rests on); `crates/cmem-eval-continuity/src/{fixture,generator,driver,metrics,report}.rs`; `crates/cmem-eval/src/{memory_adapter,adapter,controllable_similarity_embedding,metrics,results,outcome}.rs`; `crates/cmem-eval-runner/src/{pipeline,diff}.rs`; `configs/continuity_smoke.toml`.
- Existing patterns or references: the driver forces the trace on and retains the native `RetrieveOutcome`; `flatten_outcome` loses section membership, so section assertions read the native pack; the hub-scale scenario is the pattern for generated scale; the orchestrator rule "treat a forthcoming public API as the target contract and isolate current unavailability".
- Design record consulted and deviations from its acceptance: library ADR-D-0019, D-0022, D-0024, D-0028, D-0029, D-0030, D-0034, ADR-I-0020, ADR-I-0022; this repository's ADR-I-0004 and ADR-I-0005. No deviation.

### Protected assets (from the census, so the gates are reproducible from a fresh checkout)
SHA-256, all under `crates/cmem-eval-continuity/fixtures/`:
- `continuity_v3.json` `bf5e392eb3f0eb79f2f48fca6ead38a2e69109a7bebd257a47aea62f091f8eb3`
- `continuity_benchmarks_v1.json` `16c0eef36fa0bcde05a70b3a18b5c02d39d8169076bd3e60cf6dc4d47f7d9b49`
- `embeddings/task22_real_manifest.json` `dda314592900088234132503404ed6c4d3885f7e1c36edf4396473cb608cc38c`
- `embeddings/task22_real_store.json` `a2c5afaa96ce5e02d28058bd4c0951356d06695a975f4183de65281807a35130`
- `embeddings/continuity_benchmarks_v1_manifest.json` `a63b35f0ba2ef4deac06dcf8805822765a0bb5fdced8f43f91d0570a6d151714`
- `embeddings/continuity_benchmarks_v1_store.json` `c1f1eeaa45c1872e2284ec069fbd205ed3c1b1656ee89007accb3dd44657352b`

Comparability conditions for Task_6 (from the census, question 4): the same scenario subset and labels; the same provider, width and text hashes; the same section, root and fanout settings and the scored-versus-fallback regime, with floor settings reported beside them; unchanged recall and pollution denominators and sampled-negative semantics; the recurrence, temporal-contrast, thread and episode preservation checks kept; deterministic repeats compared by identity, rank and metric with timing recorded apart; every intentional difference (library commit, ingest shape, fixture file, floors) named beside the numbers. The 15-scenario canonical comparison stays distinct from ADR-I-0022's ten-scenario aggregate.

## Open Questions (max 3)
- Q1: If assumption A1 fails (the move to notions changes the embedded text of the canonical set), may a new store be frozen for it with one live provider call (new file, new hash; existing stores untouched), and is the before and after then accepted as a comparison with changed content? Recommended: yes to both, decided only if Task_4 reports the failure. No live call is planned otherwise.

## Assumptions
- A1: After the entity becomes a notion, the driver can author an entity's label as the text of a naming belief, so the frozen stores still cover the canonical and benchmark fixtures. Source: unverified (the library's belief shape and embedding text are not set); checked by Task_4, which stops and reports if it fails rather than touching a store.
- A2: The library's v0.2 trace exposes the facts the assertions read under public fields. Source: v0.2 draft section 3; checked by Task_5. A fact the library does not expose is raised to the library plan, never inferred here.
- A3: Library `main` moving breaks this repository's CI until Task_4 lands, because the dependency is a path and CI resolves library `main`. Source: census question 6. Order: library first, this repository immediately after (see Task Waves).

## Tasks

Reviewer evidence, for every task below: besides the diff review, the Reviewer produces the evidence `docs/coding-agent/rules/reviewer.md` requires for the files a task touches (an independent fixture regeneration with both hashes for generated fixtures and generator changes; for the hand-authored `situated_v1.toml`, which is its own source and cannot be regenerated, load and admission validation plus the catalog review of Task_2 instead, and no generator is written to satisfy the rule; a diff against the stored baseline for driver, report or metric changes; the embedded adapter suite with executed counts for adapter changes). "Unchanged smoke output" is shown by a diff against a smoke run taken at the task's base commit, not by two runs at the tip. Where a task asks for two runs at the tip, they show determinism only; the regression evidence for a driver, report or metric change is always the comparison with the task-base or stored baseline.

### Task_1: A situated scenario can be stated and loaded
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/fixture.rs
  - crates/cmem-eval-continuity/src/lib.rs
  - crates/cmem-eval-continuity/Cargo.toml
  - Cargo.lock (the mechanical update for the TOML dependency only)
  - crates/cmem-eval/src/controllable_similarity_embedding.rs
  - crates/cmem-eval-continuity/src/generator.rs (mechanical consumer migration only)
  - crates/cmem-eval-continuity/src/driver.rs (mechanical consumer migration only; new event kinds may be rejected as not yet runnable)
  - crates/cmem-eval-continuity/src/metrics.rs (mechanical consumer migration only)
  - crates/cmem-eval-benchmark-convert/src/lib.rs (mechanical consumer migration only)
- depends_on: []
- description: |
  The scenario shape in the Design section, additive to the current types, loadable from TOML and JSON, validated at load, with the computed gold (elapsed since the pair last met, staleness) and the per-scenario needed-feature set derived by the loader. `embedding.own_concept` is the explicit per-scenario opt-in described under Planner-added requirements; the strict default and the generator's rejection test stay unchanged. The shapes are as-perceived and library-neutral; the library's Rust types are not the model.
- acceptance:
  - The D4 example of the Design discussion (a keyed write-side scene, a commitment with direction and due date, a probe with a participant by name and no topic, one assertion of each kind Task_1 introduces (`cued` arrives with Task_7), bystanders) loads from TOML and validates, and its needed features and computed gold are what a reader would expect.
  - Gold never appears in what the loader hands the driver as library input: a participant given by name or description carries only that text.
  - Every memory an assertion or a `bystanders` list names is the external id of an `experience` or `derive` declared earlier in the scenario; load rejects an unknown or duplicate id in either, a memory that is both carried and a bystander on one probe, events out of time order, a duplicate declaration, and any other reference (a scene, the character's own entity, a participant key or its gold entity, a speaker, a derive's sources, subjects, superseded memory, actor or counterpart, a reference's candidates) that does not resolve to an earlier declaration of the permitted kind, an `omitted` without a reason, a write-side scene given by name or description, and an unknown key in either format.
  - The checked fixtures load unchanged and both existing generators reproduce them byte for byte, with no version dispatch or tolerance code added.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; the README smoke recipe"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; sha256 of the six protected assets against the census table"

### Task_2: The v0.2 situations exist as scenarios
- type: impl
- owns:
  - crates/cmem-eval-continuity/fixtures/situated_v1.toml
  - crates/cmem-eval-continuity/fixtures/situated_loud_topic_v1.json
  - crates/cmem-eval-continuity/src/generator.rs
  - crates/cmem-eval-continuity/src/bin/**
- depends_on: [Task_1]
- description: |
  Hand-author the narrative scenario groups listed in the Design section in the TOML file, each naming its catalog situation, with its default case and control, and comments where a reader needs the story. Generate the loud-topic set. Read the library's catalog sections B, C4, C6 and D and the v0.2 draft sections 1, 2 and 6 for what each property means; ADR-D-0019 governs B1 and B2.
- acceptance:
  - Every group has at least one scenario whose assertions state that group's property and nothing the behavioral tier owns.
  - Probe-side scenes appear by key, by name and by description, with one ambiguous and one unknown reference; B2 has the partition probe in each direction.
  - A memory is never a bystander or an omission merely because it is old: an old-but-current memory may still be omitted for a stated reason (partition, resolution, supersession, suppression), as the B2 partition control requires. No memory is a bystander because of the scene it was formed in; every `omitted` names its reason; no probe without a partition asserts an omission on scene grounds.
  - The file loads and validates; the generator reproduces the loud-topic file byte for byte.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier A altitude review of the scenarios against the catalog and the v0.2 draft (do they test the situation or a mechanism?), plus Tier D on the diff with the reviewer evidence clause"
  - kind: manual
    required: false
    owner: user
    detail: "The decider may read situated_v1.toml and judge the scenarios against the catalog before Wave 3"

### Task_3: A run says passed, failed, or not run for each scenario, and reports the measures
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/metrics.rs
  - crates/cmem-eval-continuity/src/report.rs
  - crates/cmem-eval/src/memory_adapter.rs
  - crates/cmem-eval/src/adapter.rs
  - crates/cmem-eval-runner/src/**
  - configs/continuity_situated.toml
  - README.md (the situated run recipe and the report's new parts)
- depends_on: [Task_1]
- description: |
  Run `experience`, `derive` and `probe` events through the adapter; the continuity driver maps them to the core adapter contract and declares the supported feature set beside that mapping; a scenario needing more is not run and is reported with what is missing. Check assertions against the native outcome and write outcomes; report the measures; carry scenario outcomes into the run output; add the repeat comparison. The run reports; it does not enforce thresholds. `gap_days` and the gap-recall buckets stay untouched.
- acceptance:
  - Against library `d0fe82d`, a small situated fixture in the task's own tests runs service-free: a scenario the library can be asked today (experience and derive writes with keyed participants and no direction, due date or trigger, retrieved by a legacy Query; every probe needs a scene and a reference time, so no probe is askable at this pin) runs on real results, and a scenario needing an unforwardable feature (one on the probe side, one on the write side) is not run, with no adapter-side operation of any kind for it (no namespace opened, nothing written, no retrieval issued).
  - The CLI loads a fixture by its extension, shown by an end-to-end run of a TOML fixture; every selected scenario appears in the report whether or not it ran, a run whose scenarios are all not run succeeds, and a mixed run omits none.
  - Section assertions read the native pack, not flattened items.
  - The repeat comparison covers scenario outcomes, each assertion's identity, result and reason, and the run-wide invariant, and a test shows one changed assertion is detected; the existing CLI diff keeps measuring only what it measures today.
  - The smoke recipe's output is unchanged, and the README gives the situated run recipe and describes the new report parts.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe"
  - kind: command
    required: true
    owner: orchestrator
    detail: "After Wave 2 integration: two runs of configs/continuity_situated.toml over Task_2's fixtures at library d0fe82d, the repeat comparison, and the result recorded in the Progress Log"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; confirm run stores were cleaned up"

### Task_7: A scenario can say an experience was salient, and that a cue admitted a memory
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/fixture.rs
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/metrics.rs
  - crates/cmem-eval-continuity/src/report.rs (null recall and `cued` results only)
  - crates/cmem-eval-runner/src/** (repeat-comparison and report tests for those only)
  - crates/cmem-eval/src/memory_adapter.rs (an optional salience on the staged write input only)
  - crates/cmem-eval/src/adapter.rs (applying that salience to the staged episode and observation only)
  - crates/cmem-eval-continuity/fixtures/situated_v1.toml
  - crates/cmem-eval-continuity/README.md (new: the scenario contract)
  - README.md (the link to it only)
  - crates/cmem-eval-continuity/fixtures/situated_loud_topic_v1.json (regenerated)
  - crates/cmem-eval-continuity/src/generator.rs (the loud-topic generation only)
- depends_on: [Task_2, Task_3]
- description: |
  The draft's section 6 lists "recent high-salience episodes" among what a scene with no topic returns, and an `experience` cannot say it was salient. Add an optional salience to `experience` (the legacy `Remember` already has one and the library already takes it), forward it, and give D1 a recent salient experience carried with the reason "recent and salient" beside a recent unremarkable one that is not asserted either way. Second, add a `cued` assertion, the positive twin of `not_cued`: a memory is admitted by the named cue kind, possibly among others, read from the same trace fact. In the catalog's own situations cues co-occur (D7's intention is about the counterpart who appears; D9's date match surfaces when the person is present), so in a small namespace `carried` alone passes on the pair cue even if the trigger or date cue does not exist. Add `cued` to the positive cases of D7 (trigger), D9 (date), D1 with D8 (due) and D5 (activity), keeping the scenes the catalog describes.
- acceptance:
  - An `experience` with a salience loads in TOML and JSON and reaches the library as authored; load rejects a salience that is not finite or is outside 0 to 1, the rule the legacy `Remember` salience already follows; an absent salience keeps today's behavior and bytes.
  - The positive cases of D7 (trigger), D9 (date), D1 with D8 (due) and D5 (activity) in `situated_v1.toml` each carry a `cued` assertion for their cue; in the generated loud-topic set the loud probe's due and pair targets each carry `cued` for their cue and its current-state target carries `cued` pair, since under a saturated content cue `carried` alone would credit a target the content cue admitted, and the generator reproduces the regenerated file byte for byte; the checker has a passing and a failing test for `cued` on an injected outcome.
  - A sentinel test plants distinctive gold strings (a carried reason, a reference's gold entity used nowhere else, an expected warning, a distinctive assertion section) in a scenario and shows none of them appears in any mapped write or retrieval input handed to the adapter; bystander ids are left out of the string check because a bystander's id is also its memory's id, and the input types have no field a classification could travel in. A forwarding test shows a speaker on an experience and a supersession on a derive reach the core input.
  - `crates/cmem-eval-continuity/README.md` documents the scenario contract from the code as built (the shape, each assertion and its identity, the measures, the cue vocabulary, the needed features and the not-run rule, how to author and run a scenario), so that the Orchestrator can cut the same detail out of this plan in the same wave; the top-level README links to it.
  - `cued` loads and validates like `not_cued`, needs the same trace feature, has a position-free identity, and a memory cannot be both `cued` and `not_cued` for one cue on one probe.
  - Load rejects a scene assertion on a derive whose sources come from different scenes; recall by reason is pooled: expected and admitted (probe, carried memory) pairs are accumulated per reason across the probes that ran, per scenario and for the run, and the ratio is reported beside the per-probe diagnostics (probes expecting 1 and 9 memories report 9 of 10, not only 0 of 1 and 9 of 9); it is null, not zero, for a not-run scenario and for a reason with no carried targets; covered in report and repeat-comparison tests.
  - A regression case with both an episode outcome and an observation outcome shows assertions and measures aggregate every native outcome.
  - The D1 scenario carries the recent salient experience with that reason and asserts `cued` recent and salient on it, and the probe-cue audit of Task_2 still holds for the scenario.
  - The checked generated fixtures are still reproduced byte for byte.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe; both situated fixtures load"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; Tier A review that the changed D1, D5, D7 and D9 cases are still the catalog situations and the draft's section 6, not a mechanism"

### Task_8: The scenarios stop asking for a partition
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/**
  - crates/cmem-eval-continuity/fixtures/situated_v1.toml
  - crates/cmem-eval-continuity/README.md
  - crates/cmem-eval-runner/src/** (only where a partition is named)
  - README.md (only where a partition is named)
- depends_on: [Task_7]
- description: |
  The library's ADR-D-0038 (accepted 2026-09-21) withdraws the query-time partition: no recall is gated by the scene, by default or by option, and the library computes no verdict about who may hear a memory; it reports each memory's scene as recorded and the present scene as given, including when it is partial. Remove the partition from the scenario language (the probe field, its needed features, the omission reason, the implicit applied-policy assertion) and from B1 and B2, which keep their default probes: the memory from the other scene is carried with its scene reported. Add nothing in its place; there is no audience assertion.
- acceptance:
  - No partition field, feature, omission reason or assertion remains in the language, the fixtures, the crate README or the root README, and a fixture that names one is rejected as an unknown key.
  - B1 and B2 still state their situations: cross-scene recall with the scene reported, in both directions for B2, with the probe-cue audit still holding.
  - The generated fixtures are still reproduced byte for byte, the six protected hashes match, and the situated fixtures still load and report every scenario not run at the pinned library with zero namespaces.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe against the task base; both situated fixtures load"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; Tier A check of the revised B1 and B2 against the catalog and ADR-D-0038"

### Task_4: The harness follows the library's schema groundwork
- type: impl
- owns:
  - crates/cmem-eval/src/**
  - crates/cmem-eval-continuity/src/** (as the library's actual deletions require; the census question 3 list)
  - crates/cmem-eval-continuity/fixtures/** (new files only, and only if a shape change invalidates the checked fixtures)
  - crates/cmem-eval-runner/src/**
  - crates/cmem-eval-locomo/src/ingest.rs
  - crates/cmem-eval-benchmark-convert/src/lib.rs
  - datasets/enriched/** (untracked local snapshot, manifest and builder report outputs; the preserved bare-id pair under .agent-work is kept)
  - scripts/enrichment/build_snapshots.py
  - scripts/enrichment/README.md
  - configs/locomo_*.toml and configs/longmemeval_s_*.toml (the snapshot path only; none of the benchmark configs is register-cited, only continuity configs are, and if one turns out to be cited it is left alone and a new config is added beside it)
  - crates/cmem-eval-continuity/fixtures/embeddings/** (new files only, and only under Open Question 1)
  - .github/workflows/ci.yml (the fixture path of the smoke step only, and only if the checked fixture is replaced)
  - README.md
- depends_on: [Task_3, Task_7, Task_8]
- description: |
  External dependency: the library's schema-groundwork branch. The exact library commit validated against is recorded in the Decision Log and in every run header, the sibling checkout is pinned to it, and all of this task's validation runs at that commit, never at a moving branch name. Track the library's new shapes with no shims: names and kinds become beliefs about a notion, interpreted-memory confidence goes, and whatever the library's value audit deletes goes with it. Link confidence follows the library's ruling, not this plan. If a deletion invalidates the checked fixtures, regenerate them as new files with the same texts and point the smoke, the tests and the README at them; the old files stay as bytes. Regenerate the local snapshots with their builder, into fresh paths, never in place: the current snapshot, manifest and builder-report files are untracked, expensive, and cited by hash in the 2026-09-16 record, so their hashes are recorded first, they are kept until the regenerated files are verified (builder self-test, manifest, Rust admission) and the configs point at the new paths, and the report states the disposition of the old pair.
- acceptance:
  - The workspace builds against the groundwork branch and the three validation commands pass.
  - The smoke recipe runs and two runs diff to zero; any difference from the pre-groundwork smoke output is listed with its cause.
  - No register-cited byte changed; if assumption A1 fails, the task stops and reports instead of touching a store, and resumes only on the decider's authorization of Open Question 1, in which case this task freezes the new store (new files only) so that Task_5 and Task_6 are reachable.
  - Source speaker attribution survives the move to beliefs in the converter and the snapshot builder: behavioral text bytes are unchanged, and the speaker is carried by native references (entities and beliefs), not by metadata, shown for both outputs (`docs/coding-agent/rules/reviewer.md`).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe twice and a diff; sha256 of the six protected assets against the census table; the snapshot builder's self-test, a regeneration of both snapshots, and a service-free Rust admission check of the regenerated shape (no benchmark run)"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause"
  - kind: manual
    required: false
    owner: user
    detail: "Only if assumption A1 failed: authorization for the one live embedding call (Open Question 1) before it is made"

### Task_5: The scenarios are asked of the library's scene and routes
- type: impl
- owns:
  - crates/cmem-eval/src/memory_adapter.rs
  - crates/cmem-eval/src/adapter.rs
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/report.rs
- depends_on: [Task_4, Task_7, Task_8]
- description: |
  External dependency: the library slices that add the scene, the reference time, the routes and the trace facts. Each step records the exact library commit it validates against in the Decision Log and the run headers, with the sibling checkout pinned to it. Extend together, as each slice lands, the core adapter contract and its forwarding, the driver's one mapping function, and the supported feature set beside it; the reference time is forwarded (today `query_date` stops at the adapter). May land in steps; each step reduces the "not run" count and never re-authors a scenario to fit the library.
- acceptance:
  - A feature joins the supported set only together with a drift test showing a distinctive authored value reaches the library-facing input whole, so a dropped or coerced field fails in the harness and is never blamed on the library.
  - No scenario in the situated fixtures is "not run".
  - A scenario that fails is reported to the library plan's owner with the trace, not adjusted.
  - The report lets the v0.2 draft's section 6 retrieval-tier criteria be read off one run.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe; two situated runs and the repeat comparison"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; Tier A check that no scenario was weakened to pass"

### Task_6: The continuity baselines are re-measured once
- type: test
- owns:
  - crates/cmem-eval-continuity/fixtures/embeddings/** (new files only, and only under Open Question 1)
  - docs/coding-agent/plans/active/v0-2-situated-recall-scenarios-plan.md
- depends_on: [Task_5]
- description: |
  At the library commit that closes v0.2 pack admission: recall by reason and cost over the loud-topic set, and one re-measurement of pollution, context size, gap recall and the graph-only probe on the canonical 15-scenario set, under the comparability conditions in the census (question 4). The canonical set reuses its existing frozen store; existing stores are never touched. Lab-notebook grade: numbers, config and input hashes, and both commits in the Decision Log; nothing sealed.
- acceptance:
  - Before and after numbers sit side by side with every intentional difference (library commit, ingest shape, fixture file if regenerated, floors) named.
  - Deterministic: the run repeats with zero differences.
  - All run stores cleaned up.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the runs, the CLI diff and the repeat comparison named above; sha256 of the protected assets"
  - kind: review
    required: true
    owner: orchestrator
    detail: "Numbers recorded in the Decision Log and handed to the library plan"

## Task Waves (explicit parallel dispatch sets)

- Wave 1: [Task_1]
- Wave 2 (parallel): [Task_2, Task_3], then the Orchestrator's starting run over the integrated result
- Wave 2b: [Task_7] (needs nothing from the library; stacks on Wave 2)
- Wave 2c: [Task_8] (needs nothing from the library; stacks on Wave 2b)
- Wave 3: [Task_4] (starts when the library groundwork branch exists)
- Wave 4: [Task_5] (follows library slices; may be several steps)
- Wave 5: [Task_6]

Waves 1 and 2 need nothing from the library, start on approval, stack on each other and merge on their own. Waves 3 and later branch from main when their library dependency exists, so they never hold Waves 1 and 2 open. Order for Wave 3: the library groundwork merges first and Task_4 merges immediately after; CI resolves library main, so a short red window on this repository's main is accepted rather than worked around.

## Rollback / Safety
- Every change is additive to the fixture shape or follows a library change; reverting a PR restores the previous state for Waves 1, 2 and 2b. From Wave 3 on that is not enough: CI resolves library `main`, so once the library groundwork has merged, reverting this repository alone leaves it building against an incompatible library. Rolling back Task_4 or later means a coordinated library revert, or pinning CI and the sibling checkout to the library commit before the groundwork until it is sorted out; the Orchestrator owns that call. Protected bytes are verified by hash in every task that works near them.

## Progress Log (append-only)

- 2026-09-20 Wave 1 completed: [Task_1]
  - Summary: the situated scenario language, loadable from TOML and JSON, fail-closed, with gold-free library input, computed gold, needed features and position-free assertion identities. PR 54.
  - Validation evidence: Tier D approved at 323835b after one finding (a scene's activity key was validated as an entity; it now resolves to an earlier authored thread or open loop). Both generators reproduce the checked fixtures byte for byte; six protected hashes match; base-versus-tip smoke diff zero. The runner's symlink test fails on the development machine (Windows OS 1314, no symlink privilege) and passed in CI on PR 54, which closes the orchestrator-owned check.
  - Notes: reviewer and worker checkouts live under C:/w with a pinned library worktree.
- 2026-09-20 Wave 2 completed: [Task_2, Task_3]
  - Summary: Task_2, 15 narrative scenarios and a generated loud-topic scenario, 40 probes (PR 56). Task_3, the run: passed, failed or not run per scenario decided before any runtime exists, assertions against the native outcome, the measures, TOML loading in the CLI, `compare-continuity`, the config and the README recipe.
  - Validation evidence: Task_2 Tier D approved twice with independent byte-identical regeneration, Tier A approved after seven scenario changes (accidental anniversaries, a trigger probe equal to its trigger string, a C6 probe with the departed person present, no suppression scenario, due instants equal to probe instants, a vacuous activity control, sibling inconsistencies). Task_3 Tier D approved at eebf41e after one finding (fractional timestamps truncated on the way to the library). Ingestion bytes for LoCoMo and LongMemEval unchanged; six protected hashes match.
  - Starting run (orchestrator, library d0fe82d, harness e7063ce, two runs each, release build): narrative 15 of 15 not run, 125 assertions not run; loud-topic 1 of 1 not run, 9 assertions; zero namespaces opened; `compare-continuity` reports no differences for either pair. Every scenario lacks `probe_scene`, `reference_time` and `omission_reasons`; 14 lack `write_scene_where`; 11 `no_topic`; 6 `cue_trace`; 6 `staleness`; 4 `direction`; 3 `memory_scene_trace`; 3 `elapsed_since_met`; 2 each `partition`, `partition_trace`, `participant_name`, `participant_description`, `reference_trace`, `write_warnings`, `trigger`, `intention_memory`; 1 each `due_date`, `write_scene_what`, `thread_provenance`. This list is what Task_5 has to make forwardable.
  - Notes: known language gap, an `experience` has no salience, so "recent high-salience episodes" (draft section 6) cannot be authored yet; now Task_7, which Task_5 depends on.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-20 Decision: the scenario shape was redrawn before approval, after the decider asked for the dataset representation to be picked apart.
  - Trigger / new insight: `Remember` fuses an episode, an observation and a generic reflection, which cannot state D4; pass or fail alone does not give calibration numbers; per-assertion "unsupported" was over-built; builder code is not reviewable against the catalog; the dual-version loader contradicted the compatibility policy.
  - Plan delta: experiences, derived memories and probes as separate events; assertions kept apart from measures; support decided per scenario from a feature set; hand-authored TOML for narrative scenarios and generation only for scale; one additive shape with no version dispatch; two requirements handed to the library plan.
  - Tradeoffs considered: a second file format beside JSON, accepted for reviewability; regenerating the checked fixtures now, declined because nothing in them changed meaning.
  - User approval: shape approved 2026-09-20; the benchmark hybrid runs are deferred to the library's v0.2 closeout by the same ruling.
  - Record proposed: none

- 2026-09-20 Decision: plan approved by the decider after Tier D (APPROVED at ba34364) and two Tier A passes.
  - Trigger / new insight: none; approval.
  - Plan delta (what changed): status approved. The additive loader (no version bump, fixtures regenerated only if invalidated) stands as written. Open Question 1 stays conditional and needs its own authorization if Task_4 reports that assumption A1 failed.
  - Tradeoffs considered: none new.
  - User approval: yes, 2026-09-20.
  - Record proposed: none

- 2026-09-20 Decision: rulings made during Waves 1 and 2.
  - Trigger / new insight: worker design alerts and review findings.
  - Plan delta (what changed): (1) own-concept embedding is an explicit per-scenario opt-in, not a default, so the strict exact-coverage rule keeps protecting generated fixtures. (2) The one mapping place is the continuity driver, with the supported feature set as a const beside it and a drift test, because the continuity crate depends on the core crate and core must not learn a dataset concept (ADR-I-0004); this replaces the plan's wording that the adapter declares the features. (3) Needed features were split so each is forwarded whole or not at all: write-scene participants apart from where, what and custom; one feature per derived subtype with no native kind, preference included, since choosing between the library's user and assistant preference kinds is an open decider question; thread provenance. The Task_3 worker made that change in `fixture.rs` under a pre-ruling limited to the requirements derivation. (4) `created_at` sits on each derived-memory input, not on the batch. (5) No JSON duplicate-key visitor; no positive "admitted by cue" assertion; a due item is due on or after its due instant and probes avoid equality; loud-topic bystanders are near-misses authored as plain experiences.
  - Tradeoffs considered: recorded with each ruling in the agmsg history.
  - User approval: not needed; none is contract-shape for the library.
  - Record proposed: none

- 2026-09-20 Decision: a positive `cued` assertion is added, reversing the earlier ruling that declined it.
  - Trigger / new insight: the Tier A review and the external review independently found that D7 and D9 pass on the pair cue alone. The fix they suggest, removing the person from the scene, contradicts the catalog: D7 is the counterpart appearing and D9 is a date match while the person is present. The cues co-occur in the situation itself, so isolation cannot come from the scene, and the loud-topic set only presses due, pair and state.
  - Plan delta (what changed): Task_7 gains the `cued` assertion and its use in D7, D9, D1 with D8 and D5, plus the two core adapter files for salience forwarding.
  - Tradeoffs considered: `cued` names a cue kind, which is closer to mechanism than `carried`; accepted because `not_cued` already reads the same trace fact, the library draft itself records which cue admitted each item, and the alternative leaves four section 6 criteria satisfiable by a library without those cues.
  - User approval: not needed; no library contract implication beyond the trace requirement already listed.
  - Record proposed: none

- 2026-09-20 Decision: value test on the plan after the review rounds, at the decider's request.
  - Trigger / new insight: several additions made while answering external review did not earn their place. A needed feature for a value the harness already forwards end to end (speaker, supersession) is always supported and gates nothing. A structural sentinel for bystanders restates what the input types already guarantee.
  - Plan delta (what changed): speaker and supersession are forwarding tests, not features; the sentinel is the string check only. The feature inventory is frozen at its current granularity: a new feature is added only for something the pinned library cannot take. The plan stops absorbing contract detail; the scenario contract moves to the crate's documentation in Task_7 (not at closeout, by the decider's instruction to clean up now), and this plan is cut back to intent in the same wave. Done 2026-09-20 with PR 58: the shape, assertion, measure and support paragraphs were replaced by the intent list above and a pointer to `crates/cmem-eval-continuity/README.md`.
  - Tradeoffs considered: the not-run gate is scaffolding for the period before a library slice lands; it recurs each library phase that is evaluated first, so it stays, but it is not grown beyond what a landing order needs.
  - User approval: requested by the decider 2026-09-20.
  - Record proposed: none

- 2026-09-21 Decision: the partition leaves the scenarios, following the library's ADR-D-0038.
  - Trigger / new insight: the decider judged the recall-side partition by character behavior rather than application compliance: a character from whom a confidence is withheld is ignorant, not discreet, and what it perceives of the room is often partial, so neither an omission policy nor a computed "shared with everyone present" verdict holds up. ADR-D-0038 replaces ADR-D-0019.
  - Plan delta (what changed): Task_8 removes the partition from the language and from B1 and B2, and adds nothing in its place. This is the reviewed fixture revision the plan allowed for a library ruling on partitions. The requirement "a partition over participants has a stated meaning" is withdrawn from the list handed to the library.
  - Tradeoffs considered: none kept; the scene assertion on a carried memory already states what B1 and B2 measure.
  - User approval: yes, 2026-09-21.
  - Record proposed: none here; the record is the library's.

## Notes
- Risks: the library's scene shape may want something the as-perceived fixture scene cannot say; that is a finding for the library plan, and the fixture follows the catalog, not the API. The assertion list may grow; each addition names the scenario that needs it. The `section` on a `carried` assertion couples a scenario to the library's pack section names, so it is used only where the draft itself names a section.
- Edge cases: tasks in different waves share `generator.rs`, `driver.rs` and the adapter files; tasks within Wave 2 do not overlap.
