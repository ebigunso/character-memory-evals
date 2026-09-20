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
A scenario has an id, the catalog situations it serves, the character's own entity, entities (id, label and kind as perceived), named scenes, and events in time order. Every event has an id unique in its scenario; the id of an `experience` or a `derive` is that memory's external id. An assertion's identity is its event's id, its kind and its whole subject (the memory, and with it the scene for a scene assertion or the cue kind for `cued` and `not_cued`), never its position; load rejects two assertions with the same complete identity, so reordering unchanged assertions is not a difference.
- A scene: who (each participant given by identity key, by name, or by description, with the entity the author means kept beside it as gold), where, what, custom. Declared once per scenario and referenced, so "the same scene" and "a different scene" are statements by reference (B1, B2). A probe may also give a scene inline.
- `experience`: what happened, in a scene, at a time, with text, an optional speaker and an optional salience (finite, 0 to 1). Writes an episode and its observations.
- `derive`: a derived memory a caller authors, with its own timestamp (events are chronological, and the timestamp is forwarded per memory as its creation time) and no scene of its own, since its scene follows from the experiences it rests on (a scene assertion may name an experience, or a derive whose source experiences share one scene; on a derive with sources from different scenes it is rejected at load, because what the library reports for such a memory is the library's scope-key question, not something the fixture can compute): subtype, text, the experiences it rests on, the entities it is about, what it supersedes, and where the situation needs them an actor and a counterpart, a due date, a trigger, and a write warning the author expects (near-verbatim restatement, churning chain; the C4 proxy).
- `probe`: a time, a scene, an optional topic, an optional partition; then assertions and measures.
- `correct`, `forget`, `link`, `restart` are unchanged. `Remember` and `Query` stay for the generated canonical set, so its baselines stay comparable.

Assertions on a probe (must hold):
- `carried`: a memory is admitted, with the author's catalog-level reason (pair, due, date, trigger, activity, own day, recent and salient, topic) and, only where the draft itself names one, the pack section. The reason is gold for grouping recall and is never checked against the trace; only `cued` and `not_cued` read the trace's cue kinds.
- `in_order`: the relative order of some carried memories (D5, D13).
- `omitted`: a memory is not admitted, always with a reason (partition, resolution, supersession, suppression). There is no bare absence assertion: recall is never gated by default (ADR-D-0019).
- `cued`: a memory is admitted by a named cue kind, possibly among others. The catalog's own situations have co-occurring cues (D7's intention is about the counterpart who appears; D9's date match surfaces while the person is present), so `carried` alone cannot show that the named cue exists.
- `not_cued`: a control; a memory is not admitted by a named cue (a non-matching date, an absent counterpart). It may still arrive by another cue.
- `references`: a participant reference resolved, was ambiguous (with the candidates), or was unknown.
- a probe that carries a partition asserts, with no further authoring, that the trace records the applied policy (draft section 6).
- the scene reported on a carried memory; elapsed time since the pair last met and staleness as age, both checked against values the loader computes from the authored timestamps, never authored numbers.
- one run-wide invariant: no omission on lifecycle or currency grounds without a reason (draft section 6).

Measures on a probe (reported, never asserted): recall of `carried` grouped by reason, null for a scenario that is not run and for a reason with no carried targets among the probes that ran, so an unasked scenario never reads as zero recall; `bystanders`, distractors on cue grounds only (nothing present, due, dated, triggered, in progress, recent and salient, or on topic calls for them), reported as a context share: admitted bystanders over all admitted memories in the pack, counted per authored memory: one count per `experience` or `derive` external id, an admitted episode or observation crediting its experience once, entities and threads not counted; null when a scenario is not run or nothing is admitted. It is a cost measure, not distractor recall, and an unlabelled memory is never counted as irrelevant; in the generated loud-topic set every distractor is labelled, so the share is exact there. Context tokens are the existing `count_tokens` over the pack's context text, zero for an empty pack and null for a scenario that is not run, so a scenario that was never asked does not look free. A memory is never a bystander because of the scene it was formed in, since that share would measure that a cross-scene memory failed to surface (ADR-D-0019). The temporal rationale share is already a metric and is read off the report.

Support is decided per scenario and statically: the continuity driver declares the features it can forward to the pinned library through the core adapter contract (write-scene participants by key, a speaker, and separately write-scene where, what and custom; a scene on a probe; no topic; a reference time; a participant, a place and an activity each by name and by description; a partition; one feature per derived subtype with no native library kind; thread provenance; direction, due date, trigger, salience; each trace fact and write warnings). The same inventory drives feature derivation, the driver's supported set and the drift and not-run tests, a scenario's needed features follow from its content, and a scenario needing a feature outside that set is not run and is reported with the missing features. Nothing is dropped or coerced to make a scenario run. Once a scenario runs, an absent fact is a failure.

### Scenario groups (from the v0.2 draft, section 4)
B1, B2, B3, D1 with D8, D4, D5, D7, D9, D11 with C6, D13, tasks and favors across long gaps, the loud-topic set (generated: cued items still carried while the content cue is saturated), and C4 as a retrieval proxy only (same surviving basis and current state across scenes and times, no stale-current leakage, the write warning; it does not establish consistent retelling). Each group has a default case and, where the draft names one, a control. Probe-side scenes exercise key, name and description, including one ambiguous and one unknown reference. B2 carries a partition probe in each direction: a group memory probed from a one-on-one with a member, and a one-on-one memory probed from the group.

### What the scenarios require of the library (handed to the library plan)
- The trace names every cue kind that admitted an item, not only the first and not only the route. Several cue kinds may share a route (due, date and own day are all time; pair and activity are both entity), and a D9 control cannot be read from a route-level trace because the memory legitimately arrives by the pair cue. Grounded in ADR-D-0022: each cue kind's admission is measured against starvation.
- A partition over participants needs a stated meaning. The B2 scenarios are authored to "everyone present now was present then" (a group memory may surface in a one-on-one with a member; a one-on-one memory is omitted in the group). The library plan confirms or rules otherwise, and the scenarios follow the ruling. A memory with no recorded participants is left unasserted under a partition until the library rules.
- The pair cue needs a counterpart other than the character's own entity. The self is a participant in every scene (ADR-D-0020), so if an admission through the self alone is labelled pair, every pair control fails.
- The C4 churn example is six restatements within one minute, with the warning asserted on the final replacement only; the threshold is the library plan's to set.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: the continuity fixture shape (additive; a TOML form beside JSON), the adapter contract types in `memory_adapter.rs` (change with the library at Task_4), run output (gains scenario outcomes), the local untracked enrichment snapshots.
- stance: break
- justification: every consumer is in this workspace, the compatibility policy is to track the latest surface with no shims, and ADR-I-0005 guarantees sealed evidence as bytes by hash only. No code exists to keep an old fixture loadable; when a shape change invalidates the checked fixtures they are regenerated as new files (the frozen-store manifests bind to texts, not to a fixture hash, so the stores still apply) and the re-baseline names the changed input hash as an intentional difference.

## Context (workspace)
- Related files/areas: `.agent-work/orchestrator/v0-2-eval-census-report.md` (the census this plan rests on); `crates/cmem-eval-continuity/src/{fixture,generator,driver,metrics,report}.rs`; `crates/cmem-eval/src/{memory_adapter,adapter,controllable_similarity_embedding,metrics,results,outcome}.rs`; `crates/cmem-eval-runner/src/{pipeline,diff}.rs`; `configs/continuity_smoke.toml`.
- Existing patterns or references: the driver forces the trace on and retains the native `RetrieveOutcome`; `flatten_outcome` loses section membership, so section assertions read the native pack; the hub-scale scenario is the pattern for generated scale; the orchestrator rule "treat a forthcoming public API as the target contract and isolate current unavailability".
- Design record consulted and deviations from its acceptance: library ADR-D-0019, D-0022, D-0024, D-0028, D-0029, D-0030, D-0034, ADR-I-0020, ADR-I-0022; this repository's ADR-I-0004 and ADR-I-0005. No deviation.

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
  - crates/cmem-eval-continuity/fixtures/situated_loud_topic_v1.json (regenerated)
  - crates/cmem-eval-continuity/src/generator.rs (the loud-topic generation only)
- depends_on: [Task_2, Task_3]
- description: |
  The draft's section 6 lists "recent high-salience episodes" among what a scene with no topic returns, and an `experience` cannot say it was salient. Add an optional salience to `experience` (the legacy `Remember` already has one and the library already takes it), forward it, and give D1 a recent salient experience carried with the reason "recent and salient" beside a recent unremarkable one that is not asserted either way. Second, add a `cued` assertion, the positive twin of `not_cued`: a memory is admitted by the named cue kind, possibly among others, read from the same trace fact. In the catalog's own situations cues co-occur (D7's intention is about the counterpart who appears; D9's date match surfaces when the person is present), so in a small namespace `carried` alone passes on the pair cue even if the trigger or date cue does not exist. Add `cued` to the positive cases of D7 (trigger), D9 (date), D1 with D8 (due) and D5 (activity), keeping the scenes the catalog describes.
- acceptance:
  - An `experience` with a salience loads in TOML and JSON and reaches the library as authored; load rejects a salience that is not finite or is outside 0 to 1, the rule the legacy `Remember` salience already follows; an absent salience keeps today's behavior and bytes.
  - The positive cases of D7 (trigger), D9 (date), D1 with D8 (due) and D5 (activity) in `situated_v1.toml` each carry a `cued` assertion for their cue; in the generated loud-topic set the loud probe's due, pair and state targets each carry `cued` for their cue, since under a saturated content cue `carried` alone would credit a target the content cue admitted, and the generator reproduces the regenerated file byte for byte; the checker has a passing and a failing test for `cued` on an injected outcome.
  - A sentinel test plants distinctive gold strings (a carried reason, a reference's gold entity used nowhere else, an expected warning, a distinctive assertion section) in a scenario and shows none of them appears in any mapped write or retrieval input handed to the adapter; a bystander's id is also its memory's id and legitimately appears in the write, so for bystanders the test shows structurally that no mapped input has a field the classification could travel in; a speaker on an experience is a derived needed feature like any other forwarded field.
  - `cued` loads and validates like `not_cued`, needs the same trace feature, has a position-free identity, and a memory cannot be both `cued` and `not_cued` for one cue on one probe.
  - Load rejects a scene assertion on a derive whose sources come from different scenes; recall by reason is null, not zero, for a not-run scenario and for a reason with no carried targets, covered in report and repeat-comparison tests.
  - The D1 scenario carries the recent salient experience with that reason, and the probe-cue audit of Task_2 still holds for the scenario.
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
  - .github/workflows/ci.yml (the fixture path of the smoke step only, and only if the checked fixture is replaced)
  - README.md
- depends_on: [Task_3, Task_7]
- description: |
  External dependency: the library's schema-groundwork branch (named in the Decision Log when it exists). Track the library's new shapes with no shims: names and kinds become beliefs about a notion, interpreted-memory confidence goes, and whatever the library's value audit deletes goes with it. Link confidence follows the library's ruling, not this plan. If a deletion invalidates the checked fixtures, regenerate them as new files with the same texts and point the smoke, the tests and the README at them; the old files stay as bytes. Regenerate the local snapshots with their builder.
- acceptance:
  - The workspace builds against the groundwork branch and the three validation commands pass.
  - The smoke recipe runs and two runs diff to zero; any difference from the pre-groundwork smoke output is listed with its cause.
  - No register-cited byte changed; if assumption A1 fails, the task stops and reports instead of touching a store.
  - Source speaker attribution survives the move to beliefs in the converter and the snapshot builder.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the README smoke recipe twice and a diff; sha256 of the six protected assets against the census table; the snapshot builder's self-test, a regeneration of both snapshots, and a service-free Rust admission check of the regenerated shape (no benchmark run)"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause"

### Task_5: The scenarios are asked of the library's scene and routes
- type: impl
- owns:
  - crates/cmem-eval/src/memory_adapter.rs
  - crates/cmem-eval/src/adapter.rs
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/report.rs
- depends_on: [Task_4, Task_7]
- description: |
  External dependency: the library slices that add the scene, the reference time, the partition, the routes and the trace facts. Extend together, as each slice lands, the core adapter contract and its forwarding, the driver's one mapping function, and the supported feature set beside it; the reference time is forwarded (today `query_date` stops at the adapter). May land in steps; each step reduces the "not run" count and never re-authors a scenario to fit the library.
- acceptance:
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
  - kind: manual
    required: false
    owner: user
    detail: "Only if assumption A1 failed: authorization for the one live embedding call (Open Question 1) before it is made"

## Task Waves (explicit parallel dispatch sets)

- Wave 1: [Task_1]
- Wave 2 (parallel): [Task_2, Task_3], then the Orchestrator's starting run over the integrated result
- Wave 2b: [Task_7] (needs nothing from the library; stacks on Wave 2)
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

## Notes
- Risks: the library's scene shape may want something the as-perceived fixture scene cannot say; that is a finding for the library plan, and the fixture follows the catalog, not the API. The assertion list may grow; each addition names the scenario that needs it. The `section` on a `carried` assertion couples a scenario to the library's pack section names, so it is used only where the draft itself names a section.
- Edge cases: tasks in different waves share `generator.rs`, `driver.rs` and the adapter files; tasks within Wave 2 do not overlap.
