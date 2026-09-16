# Plan: Benchmark-Derived Content (LoCoMo dataset summaries and observations; LongMemEval repeated sessions as distinct memories)

- status: completed
- generated: 2026-09-14
- last_updated: 2026-09-16
- work_type: code

## Goal
- LoCoMo runs ingest the summaries and observations the official dataset provides, as provenanced memories that reach the library ingest and the precomputed-enrichment path, so the harness measures Character Memory over the full benchmark content while leaving the door open for library-generated reflections later; the two baselines (lexical and vector-only) stay a crude search over the chat log, indexing each episode as its generic descriptive sentence plus the observations that carry the turns, the hurdle every Character Memory retrieval mechanism is compared against; LongMemEval-S ingests the 13 repeated sessions as distinct memories with their own dates, because the date changes how the graph retrieves a memory and must not be overridden (decider rulings 2026-09-14).

## Definition of Done
- LoCoMo content: every official session parses with its dataset summary and its typed generated observations (speaker, statement, evidence dialog ids); the loader reads the official `session_<N>_summary` and `session_<N>_observation` maps beside the documented lookups; under the two existing ingest flags the summary and each observation become derived memories with provenance to the session episode and to the evidence observations that exist in the sample; the flags keep their meaning (dataset-provided content), a library-generated source is the v0.2 comparison and is not built.
- LoCoMo consumers: with an enrichment snapshot configured, dataset-derived memories and the snapshot graph are ingested together instead of the snapshot discarding them; the hybrid retrieval config and the three cross-mode configs index summaries and observations; the two baseline configs do not.
- Baselines: the lexical and vector-only modes index episodes and observations only; for LoCoMo their episode text stays the generic descriptive sentence used today (never the dataset summary; the observations carry the chat log, and whether episode text should instead be the session's turns is a measurement question for the v0.2 value audit, not decided here), and LoCoMo config admission rejects any derived-content flag or enrichment configuration in a baseline mode while LongMemEval's baseline admission is unchanged (its lexical results move only for the 13 repeated sessions, which the baseline now sees as distinct candidates, the expected diff); the guarantee is enforced at admission, at the runner boundary and by the exact configuration the run header already records.
- LongMemEval-S identities: every episode is unique within its item without narrowing admission (bare id for a single occurrence, collision-safe `#<ordinal>` suffix for later copies), observation ids follow their episode, every copy keeps its own date, and the assigned identity serves identities only while text keeps the raw session id; one crate helper derives the assignment from the item for both ingest and scoring; scoring maps retrieved ids back to benchmark ids for both metric families and credits a session retrieved as two copies once.
- LongMemEval-S snapshot: the deterministic, provider-free builder represents every copy with its own date and provenance; the LongMemEval snapshot is regenerated locally at its configured path (the datasets directory stays untracked), the bare-id pair is preserved before replacement for the base run, and the runner admits a snapshot only after verifying its manifest (workflow id, artifact hash, and the official dataset hash carried forward from the source-only builder) before any run state exists.
- Re-baseline: the lexical and hybrid configs of each dataset run at the base commit and at the tip under the same library commit; headline metrics, diff counts, changed item sets, config hashes, harness and library commits and the two snapshot hashes are recorded in this plan's Decision Log; content-attributable LongMemEval differences are confined to the 13 repeated-session items, with provider jitter recorded separately and its observed magnitude stated; all run state is cleaned up.
- README states both behaviours, the baseline exception, the result-row identity domains and the speaker limitation; the plan closes in completed.

## Scope / Non-goals
- Scope: `crates/cmem-eval-locomo` (types, loader, ingest, rustdoc, tests); `crates/cmem-eval-runner/src/pipeline.rs` and `crates/cmem-eval-runner/src/enrichment.rs` (LoCoMo snapshot merge, baseline projection and admission, snapshot manifest admission); `crates/cmem-eval-longmemeval` (ingest, scoring, types, rustdoc, tests); the enrichment builders (`scripts/enrichment/build_source_only.py`, `scripts/enrichment/build_snapshots.py`, their README); one shared metric defect fix in `crates/cmem-eval/src/metrics.rs` (duplicate retrieved ids credited once in DCG), delivered as its own change beneath Task_2 so no dataset change carries a shared-crate diff (ADR-I-0004); the LoCoMo benchmark configs; README; the re-baseline runs and their record.
- Non-goals: any change to the baseline engines, their selected object types or derived-memory retrieval; any LoCoMo scoring change (evidence recall stays measured on retrieved observations; whether a retrieved derived memory with evidence provenance should count is a v0.2 value-audit question); any library change; a library-generated summary or observation source; entity plumbing (the LoCoMo speaker is preserved on the runner-side input only, since the adapter does not persist derived-memory metadata; speaker entities are v0.2 work); changing the LoCoMo snapshot artifact; edits to sealed evidence or hash-cited configs; the loaders' admission rules.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: parsed LoCoMo items (summaries and observations populated; the generated-observation type becomes structured), LoCoMo derived-memory identities, LongMemEval episode and observation identities for repeated sessions, the snapshot manifest contract (it gains the official dataset hash and the LongMemEval workflow id changes; the v1 LoCoMo manifest without the field stays admitted; the bare-id LongMemEval manifest does not) and the source-only provenance sidecar (new), result-row identity domains (assigned ids on retrieved items, raw ids on gold lists), benchmark measurement baselines
- stance: break
- justification: the harness is pinned to a library version and old artifacts are old, not a compatibility surface (harness strictness follows the claim); every consumer of these types is in this workspace; sealed evidence is untouched by construction (new runs write new files); run artifact schemas do not change, values inside new artifacts do; the snapshot manifest schema gains one field and one workflow id value; the only older manifest still admitted is the v1 LoCoMo one without the dataset hash, while LongMemEval admission requires the v2 workflow id and the hash.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-locomo/src/loader.rs` (`apply_benchmark_derived_fields` looks up keys the official file never uses; `generated_observations_from_value` flattens a `[statement, evidence]` pair into one string), `crates/cmem-eval-locomo/src/ingest.rs` (summary and observation derived memories behind the flags; the episode summary is the session summary when present), `crates/cmem-eval-runner/src/pipeline.rs` (`LoCoMoSpec::enrichment` returns the snapshot graph and drops the mapped derived memories; `Bm25Baseline::new` receives episodes and observations, whose summary text it indexes; the ingest progress event sits inside the live-adapter branch; the lexical mode never loads enrichment), `crates/cmem-eval-runner/src/enrichment.rs` (`merge_enrichment`; snapshot loading reads only the artifact path), `crates/cmem-eval/src/results.rs` (`RunHeader` records the exact configuration text), `crates/cmem-eval-longmemeval/src/ingest.rs` and `scoring.rs` (episode and observation ids from `session_id`; the scorer receives the instance), `scripts/enrichment/build_snapshots.py` (deterministic source replay; keeps the last visible occurrence of a repeated id; hard-coded canonical counts; `WORKFLOW_ID`), `scripts/enrichment/build_source_only.py` (prints the official input hash and output hash), `configs/locomo_*.toml`, `datasets/README.md` (datasets are local and untracked; the LongMemEval snapshot is about 447 MiB).
- Existing patterns or references: derived memories carry source episode or observation external ids (README enrichment rule); LoCoMo turn ids (`dia_id`) are the observation external ids at ingest, so evidence ids resolve to provenance directly; the dataset admission plan committed to the bare-id and numeric annotation lookups with tests and recorded the census that found the key mismatch and the 13 repeated sessions.
- Design record consulted and deviations from its acceptance: ADR-I-0004 (dataset crates own their loaders and ingest; the runner changes only where the consumer paths live) and ADR-I-0005 (artifact readers unaffected). No deviation.

## Open Questions (max 3)
- Q1 (settled 2026-09-14, Decision Log): generated observations use `DerivedType::Claim`; the summary keeps Reflection.
- Q2 (settled 2026-09-14, Decision Log): the decider authorized the re-baseline runs of this plan, including the hybrid provider runs.

## Assumptions
- A1: Official LoCoMo carries 272 summaries and 2541 observation pairs (evidence a string in 2531 and a list in 10; after splitting comma-separated ids, 2561 references with zero unresolved) — source: censuses 2026-09-14; re-checked by Task_1's opt-in official-file census.
- A2: The 13 LongMemEval repeats (13 items, each carrying one repeated pair, so 26 session occurrences and 13 later copies) have identical turn arrays (gold labels included) and differ only in their `haystack_dates` entry, are never answer sessions, and no official session id contains `#` — source: plan review census 2026-09-14; re-checked by Task_2's opt-in census.
- A3: No benchmark config is hash-cited in `reports/v0-1-5-findings-register.md` (only continuity configs are) — source: register grep 2026-09-14, confirmed at plan review.

## Tasks

### Task_1: LoCoMo dataset summaries and observations reach every consumer
- type: impl
- owns:
  - crates/cmem-eval-locomo/**
  - crates/cmem-eval-runner/src/pipeline.rs
  - crates/cmem-eval-runner/src/enrichment.rs
  - configs/locomo_retrieval.toml
  - configs/locomo_vector.toml
  - configs/locomo_bm25.toml
  - configs/locomo_crossmode_embedded.toml
  - configs/locomo_crossmode_service.toml
  - configs/locomo_crossmode_service_repeat.toml
- depends_on: []
- description: |
  Read the official key forms; type the generated observations; ingest them as provenanced derived memories; merge dataset-derived memories with a configured snapshot; give the baselines their chat-log projection and admission guards; add snapshot manifest admission in the runner; set the configs; report the chosen derived type with its rationale. The design notes below are the reviewed shape; a simpler implementation that meets the acceptance is welcome and is reported as a deviation.
- acceptance:
  - Opt-in official-file census (an ignored test run with the dataset path in an environment variable): 272 summaries, 2541 typed observations, 2561 evidence references, zero unresolved, zero dropped entries; fixture tests cover comma-separated evidence, the legacy bare-string observation, malformed map shapes staying non-fatal with counted drops, and the key precedence.
  - Ingest and consumer tests: one derived memory per observation with the statement as text and provenance to the episode and resolved evidence observations, the speaker in metadata; the summary as episode summary and Reflection derived memory under its flag; with a snapshot configured the merged enrichment input handed to the adapter contains both the dataset-derived memories and the snapshot graph, the existing enrichment-file branch still merges its configured graph the same way (regression), and an external-id collision fails with a typed error.
  - Baseline tests for the lexical and vector-only modes: derived memories are not indexed, a distinctive dataset-summary token is absent from the indexed and retrievable text, a baseline config with a derived-content flag or enrichment configured is rejected at admission with a typed error, and the regime derived from a written run header's configuration equals the projection used; the existing runner test that expects a lexical run to succeed with a missing enrichment path is split so LoCoMo asserts the admission error while LongMemEval keeps its behaviour.
  - Snapshot admission tests: a snapshot without its sibling manifest, wrong workflow id, artifact bytes not matching the manifest hash, and (for the v2 LongMemEval manifest) a missing or mismatched dataset hash are rejected with a typed error before any run root or store exists; a v1 LoCoMo manifest without the dataset hash is admitted, and a v1 manifest carrying a mismatched hash is rejected.
  - The dataset admission strict set is unchanged (its tests pass); the retrieval and cross-mode configs index summaries and observations, the two baseline configs do not; no other config changes.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (service-free, fixture-based); the official-file census as an ignored test run explicitly with the dataset path"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review against the acceptance; independent official-file census; provenance check on a sample of derived memories; baseline projection and admission checks in both baseline modes; snapshot admission ordering check"
- design notes (reviewed shape, not a contract): |
  Loader: keep the documented exact-id and numeric lookups; add the official suffixed key derived from the canonical decimal session number parsed from the record id, in the precedence exact record id, official key, numeric fallback. The official `session_<N>_observation` value is an object keyed by speaker whose values are lists of entries; the loader walks each speaker key and attaches it to every observation emitted from that list. An entry is either a `[statement, evidence]` pair (evidence a dialog id, a comma-separated string of ids, or a list of such; a string is first resolved as one exact dialog id of the sample and only split on commas when that fails, since dialog ids are opaque strings) or a legacy bare statement string (which may also appear directly at session level, with no speaker) with no evidence; any other shape is dropped and counted; a non-string summary (an array included) is dropped. Counters `unresolved_evidence_references` and `dropped_observation_entries` ride the runner's memory batch so the ingest progress detail reports them in every mode (the event moves out of the live-adapter branch). Ingest: statement as text, provenance to the episode plus the resolved evidence observations, speaker in metadata. Runner: `LoCoMoSpec::enrichment` merges dataset-derived memories with the snapshot graph (ids disjoint by construction; collision is a typed error from the enrichment module); baseline modes get the generic episode sentence as the projection at the memory-inputs boundary; the enrichment step returns nothing for vector-only (the lexical path never reaches it); an owned structured error in the LoCoMo crate names the illegal baseline combinations. Snapshot admission in the enrichment module: manifest located as the artifact path with `.jsonl` replaced by `_manifest.json`; checks workflow id (`deterministic-exact-source-replay-v1` for LoCoMo, `deterministic-exact-source-replay-v2` for LongMemEval), artifact SHA-256, and the dataset hash against the run's input hash (required for v2, when present for v1); runs before `create_run_root` and adapter construction, only for modes that consume snapshots.

### Task_2: LongMemEval repeated sessions as distinct memories
- type: impl
- owns:
  - crates/cmem-eval-longmemeval/**
  - crates/cmem-eval-runner/tests/longmemeval_repeated_sessions.rs
  - scripts/enrichment/build_snapshots.py
  - scripts/enrichment/build_source_only.py
  - scripts/enrichment/README.md
  - crates/cmem-eval/src/metrics.rs
- depends_on: []
- description: |
  One crate helper derives the identity assignment from the item; ingest and scoring use it; the snapshot builders adopt it and bind the artifact to its dataset; the LongMemEval snapshot is regenerated locally after the bare-id pair is preserved. The design notes below are the reviewed shape; a simpler implementation that meets the acceptance is welcome and is reported as a deviation.
- acceptance:
  - Loader, ingest and identity tests: the official parallel-array duplicate shape still admits both dated copies; two copies yield two episodes with distinct ids and their own dates and observation ids that follow; a three-copy fixture with a raw suffixed id present yields unique ids without changing admission; the episode summary text keeps the raw session id; a dump of episode ids before and after differs only in the 13 copies (opt-in census).
  - Shared metric fix, as its own change beneath this task: a retrieved list with a repeated gold id credits it once in DCG, NDCG stays within bounds and recall is unchanged (unit test in the shared crate).
  - Scoring and row tests: a later copy counts for its answer session and a turn of a later copy for its bare gold turn id through the helper's tables (no id parsing; a session id containing the turn delimiter is a case); a session retrieved as two copies earns credit once with every metric within bounds; the runner integration test writes lexical-mode rows for a repeated-session item showing assigned ids on retrieved items and raw ids on gold lists.
  - Builders: identity assignment over the full session list before the visibility cutoff, thread, canonical, memory and link ids from the assigned identities (distinct per copy, and every derived-memory and link id unique across the artifact), titles and summaries interpolating the raw session id and every memory text equal to its source turn text, canonical counts recalculated, the source-only builder emitting the provenance sidecar and the snapshot builder verifying the sanitized bytes against it before forwarding the dataset hash; both self-tests pass, including a handoff-mismatch case and an identical-turn repeat.
  - Artifacts: the bare-id LongMemEval snapshot and manifest are copied and hash-verified to the work area before regeneration and their paths and hashes reported; the LongMemEval and LoCoMo source-only files are regenerated to emit their sidecars, each with bytes hashing to the value its current snapshot manifest records, so the no-sidecar rule needs no exception; the LongMemEval snapshot is regenerated at its configured path with workflow id `deterministic-exact-source-replay-v2`, every reference resolving to the copy it was built from with both copies of a repeated session carrying their own dates; the LoCoMo snapshot regenerated into a temporary path is byte-identical to the configured one (both hashes reported).
  - The crate rustdoc distinguishes the raw benchmark session id from the assigned ingest identity and states that scoring maps back through the helper; `scripts/enrichment/README.md` documents the provenance sidecar and the per-dataset workflow ids.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (service-free, fixture-based); the official-file census and identity dump as ignored tests run with the dataset path; both builder self-tests; both local regenerations with the manifest values and the LoCoMo SHA-256 pair"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review; independent census; identity stability check; metric boundedness probe with a two-copy retrieval; independent regeneration of both snapshots with snapshot-to-ingest identity and date checks on the repeated items and the LoCoMo byte-identity with both SHA-256 values"
- design notes (reviewed shape, not a contract): |
  Identity: bare id for a single occurrence; for later occurrences the bare id plus `#<ordinal>` with the ordinal raised past any id already present in the item; the helper returns the episode mapping and an exact assigned-observation-id to raw-turn-id table; scoring collapses repeated mapped ids to their first rank. Snapshot builder: the same assignment computed before the question-date cutoff and used by its own validation path; identical-turn repeats (on the sanitized fields it sees) retained as distinct copies, differing repeats reject the source; per-dataset workflow id; the provenance sidecar beside the source-only output is named by replacing `.json` with `_provenance.json` and carries the official input path and hash, the output hash and the workflow id; the snapshot builder refuses to run without it and carries the official hash into the snapshot manifest. Preserved bare-id pair: kept under the worker's work area with the retention reason (rollback source; the worker rule forbids deleting local snapshots during validation).

### Task_3: Re-baseline, README, closeout
- type: docs
- owns:
  - README.md
  - docs/coding-agent/plans/active/benchmark-derived-content-plan.md
  - docs/coding-agent/plans/completed/benchmark-derived-content-plan.md
  - docs/coding-agent/lessons.md
- depends_on: [Task_1, Task_2]
- description: |
  Record the eight base/tip runs and four diffs executed by the orchestrator under Q2, independently verify their headers, report values, changed-item sets and asset hashes, state both dataset behaviours in README, and move the plan to completed with the lessons link retargeted. Run execution and cleanup remain orchestrator-owned under the 2026-09-16 closeout dispatch.
- acceptance:
  - Four before-and-after pairs recorded with config hashes (including the temporary base config), the harness commit of each run, one library commit and one dataset input hash (from the run headers) shared by base and tip of every pair, both LongMemEval snapshot hashes, the LoCoMo snapshot and manifest hashes used by both LoCoMo hybrid runs, the regenerated manifest's source and output hashes and workflow id, diff counts and changed item sets; content-attributable LongMemEval differences are confined to the 13 repeated-session items, with provider jitter recorded separately and its observed magnitude stated.
  - README states that LoCoMo ingests dataset-provided summaries and observations as provenanced derived memories under the two flags and that they are merged alongside a configured enrichment snapshot at runtime (the snapshot artifact itself stays source-only); that both baselines search the chat log only; that a library-generated source is a later comparison; that repeated LongMemEval sessions are distinct memories with their own dates; that result rows carry assigned identities on retrieved items and raw ids on gold lists; that the LoCoMo speaker is not persisted by the adapter; and that a configured enrichment snapshot needs its sibling manifest with the pinned workflow id, artifact hash and (for LongMemEval) dataset hash, admitted before any run state exists.
  - No run store or run output remains outside the recorded evidence; the preserved bare-id pair stays under the work area with its retention reason; original dataset assets are unchanged and the regenerated LongMemEval snapshot and its manifest are at their configured paths with the recorded hashes (verified after cleanup); the plan is in completed with its final progress entry and the lessons entry links to its completed path.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "the eight runs and four diffs; store and run-output cleanup census; original dataset assets and configured snapshot hashes after cleanup"
  - kind: manual
    required: true
    owner: worker
    detail: "verify all eight header bindings, config hashes and differences, headline metrics, changed-item sets and repeated-session census against read-only evidence"
  - kind: command
    required: true
    owner: worker
    detail: "plan validator in balanced mode, markdown links for the moved plan and lessons entry, and cargo fmt --all --check"
  - kind: review
    required: true
    owner: orchestrator
    detail: "Decision Log record, README wording, closeout entry"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1, Task_2]
- Wave 2 (parallel): [Task_3]

## Rollback / Safety
- Reverting the branch restores today's parsing, identities and baseline inputs in code. It does not restore the replaced gitignored LongMemEval snapshot: that artifact is restored from the preserved bare-id pair while it exists, or regenerated by checking out the base commit's builder and running it against the recorded source, with the hash verified before any reverted hybrid run; that replacement of an ignored artifact is separate from the Git revert. Run artifact schemas do not change and the snapshot manifest gains one optional field; the values inside new artifacts do change, which is the declared break; sealed evidence and hash-cited configs are untouched; new runs write new files only.

## Progress Log (append-only)

- 2026-09-14 Plan drafted from two decider rulings: LoCoMo uses the dataset's summaries and observations (with a library-generated source kept open for later); the 13 repeated LongMemEval sessions are distinct memories because the date changes graph retrieval and must not be overridden.
- 2026-09-14 Plan review (evals-reviewer and Copilot) applied: the consumer paths are in scope (the enrichment snapshot path discarded mapped derived memories and the lexical baseline never indexed them, so the flags alone delivered nothing in the maintained configs); evidence strings with comma-separated dialog ids are normalized (2561 references, zero unresolved); the LongMemEval identity policy is collision-safe and reversible through ingest data rather than suffix parsing, covers both scoring families, and collapses duplicate credit; the derived-type rationale flows through the worker report; three cross-mode configs; task types within the allowed set.
- 2026-09-14 Decider ruling: the lexical baseline stays a crude search over the chat log; derived-memory indexing is removed from the plan (Decision Log).
- 2026-09-14 Copilot round 2: the identity mapping is recomputed from the item by one crate helper shared by ingest and scoring, since the dataset seam passes the item and the retrieved items to scoring and carries no ingest output; LongMemEval snapshots are not regenerated and bind to the first copy of a repeated session (Decision Log).
- 2026-09-14 Plan review round 2 (evals-reviewer): the baseline indexes episode summary text, so populated dataset summaries would have leaked into the lexical corpus with the flags off; the LoCoMo lexical path now hands the baseline a chat-log projection of each episode and a token test proves the absence.
- 2026-09-14 Plan review round 3 (evals-reviewer and Copilot): the snapshot decision is corrected, since the builder is deterministic and provider-free and keeps the last visible occurrence; the LongMemEval snapshots are regenerated with the shared identity assignment so every copy keeps its own provenance and date; `configs/locomo_vector.toml` stays a source-only baseline like the lexical one.
- 2026-09-14 Copilot round 3: the documented bare-id and numeric summary lookups stay (the dataset admission plan committed to them with tests) and the official key forms are added beside them; unresolved evidence references get a destination (a count on the memory inputs surfaced through the ingest progress detail); the baseline non-goal names what stays fixed (engines, object types, derived retrieval) while the episode projection is in scope.
- 2026-09-14 Plan review round 4 (evals-reviewer): the chat-log episode projection and its token test apply to both baseline modes (the vector-only mode reads the same memory inputs through the live adapter); the LongMemEval snapshot is an untracked 447 MiB artifact, so it is regenerated locally by the reproducible builder with its manifest values recorded rather than committed; Task_2 validation now names the builder self-test, both regenerations and the reviewer's independent identity, date and byte-identity checks.
- 2026-09-14 Copilot round 4: the snapshot Decision Log clause now says regenerated locally and untracked; the three LoCoMo cross-mode configs leave this plan (no cross-mode run is part of it; they align with the retrieval config when a parity run is next needed); the LongMemEval hybrid base run uses the preserved pre-change snapshot and the tip run the regenerated one, with both hashes recorded.
- 2026-09-14 Copilot round 5: the snapshot-merge acceptance asserts on the merged enrichment input the adapter receives; local LoCoMo regeneration is validation while changing that artifact stays a non-goal; malformed annotation maps stay non-fatal; LoCoMo config validation rejects a baseline mode with a derived-content flag on; a loader regression pins the parallel duplicate shape; the LongMemEval rustdoc distinguishes raw from assigned identity.
- 2026-09-14 Plan review round 5 (evals-reviewer): the persisted episode-text regime needs no new header field, because the regime is a pure function of the retrieval mode in the header's exact configuration text; a test derives it from a written header and compares it with the projection used.
- 2026-09-14 Copilot round 6: legacy scalar observation entries stay admitted as evidence-less observations; unresolved evidence references and dropped malformed entries are separate counters; the baseline projection is enforced at the runner boundary as well as at config admission; scoring uses an exact assigned-to-raw observation id table from the helper rather than delimiter parsing; the diff command's expected nonzero exit is captured; the cross-mode configs stay flags-off until a parity task updates them.
- 2026-09-14 Copilot round 7: the builder computes the identity assignment over the full session list before the visibility cutoff, derives thread, link and canonical identities from the assigned identity, and carries a per-dataset workflow id so the regenerated LongMemEval artifact is distinguishable while LoCoMo stays byte-identical; the LoCoMo snapshot-precedence entry is qualified with the LongMemEval exception; closeout cleanup is limited to run outputs and temporary copies with the original gitignored assets verified in place.
- 2026-09-14 Copilot round 8: annotation key precedence is fixed and tested; the ingest progress event is emitted in every retrieval mode so the counters reach lexical runs; the run header records the effective episode-text regime; the assigned identity is used only for identities while episode text keeps the raw session id; the tip hybrid run uses a temporary snapshot and config copy so original assets stay untouched; rollback wording distinguishes schema stability from the declared value changes.
- 2026-09-14 Plan review round 6 (evals-reviewer): the row-level regression gets an executable home, a new service-free runner integration test file owned by Task_2, disjoint from the runner source Task_1 owns.
- 2026-09-14 Copilot round 9: baseline modes reject enrichment configuration at admission and the runner's enrichment step returns nothing for them; the two admitted observation shapes are named against every malformed shape; a three-copy fixture proves unique assigned ids; the builder derives derived-memory and link ids from the assigned identities with a uniqueness check.
- 2026-09-14 Copilot round 10: the new LoCoMo validation failures use an owned structured error type asserted by variant; result rows keep two identity domains by contract (assigned for retrieved items, raw for gold lists) with a row-level test; an array-valued summary is dropped by the string-only branch; the builder's own validation path uses the same identity assignment; legacy session-level entries carry no speaker (optional); the LongMemEval workflow id is pinned to deterministic-exact-source-replay-v2.
- 2026-09-14 Plan review round 7 (evals-reviewer): Task_2 preserves and hash-verifies the bare-id LongMemEval snapshot pair before replacing it and hands the paths and hashes to Task_3; rollback states how the replaced ignored artifact is restored (preserved pair, or regeneration at the base recipe with hash verification), separately from the Git revert.
- 2026-09-14 Copilot round 11: the regenerated LongMemEval snapshot persists at its configured path so every later hybrid run resolves per-copy provenance, with the old artifact preserved in a temporary directory for the base run; the builder's canonical object counts are recalculated and its self-test fixture mirrors admission (identical repeats retained, differing repeats rejected); the runner's enrichment merge module joins Task_1 with an owned typed collision error; the lexical-corpus note is qualified.
- 2026-09-14 Copilot round 12: the LoCoMo byte-identity regeneration targets a temporary output path so the builder never replaces the configured LoCoMo artifact; the builder keeps raw ids in every text field with a test; Task_3's README acceptance covers the two identity domains of result rows; the enrichment-module ownership and preservation-before-replacement points were already applied.
- 2026-09-14 Copilot round 13: the speaker travels in derived-memory metadata since ingest emits no entities; the runner admits a snapshot only when its manifest carries the pinned workflow id the dataset expects (Task_1, typed error), so a stale bare-id artifact cannot silently rebind provenance; the existing lexical-run runner test is split per dataset; each re-baseline run records its harness commit.
- 2026-09-14 Plan review round 8 (evals-reviewer): snapshot admission binds the manifest's version claim to the loaded bytes by comparing the artifact hash as well as the workflow id, with a regression pairing a valid manifest with stale bytes.
- 2026-09-14 Copilot round 14: snapshot text assertions are field by field; the manifest is located by the builder's sibling-file convention and admission runs before the run root and adapter exist; the counters travel on the runner's memory batch; each re-baseline pair must carry the same library commit; the LongMemEval scorer already receives the instance so no call-site change is needed; the builder's admission mirror is scoped to the sanitized fields with an official census.
- 2026-09-14 Copilot round 15: the speaker is preserved on the runner-side input only, since the adapter does not persist derived-memory metadata (recorded limitation, entities are v0.2 work); the pre-run snapshot admission is scoped to modes that consume snapshots; the official key lookup canonicalizes the parsed session number.
- 2026-09-14 Copilot round 16: the cross-mode configs turn the flags on so hybrid content is identical across modes (no parity run here); official-file censuses are opt-in ignored tests since the datasets are untracked; the preserved bare-id snapshot pair is retained under the work area with a reason; the snapshot manifest binds the artifact to the official dataset hash handed forward by the source-only builder, checked against the run input hash (required for v2, when present for the v1 LoCoMo manifest).
- 2026-09-14 Plan review round 9 (evals-reviewer): the dataset-hash handoff is bound to the sanitized source bytes (the source-only builder persists both hashes, the snapshot builder verifies the bytes before forwarding), with a handoff-mismatch regression in Task_2 and the four dataset-admission cases in Task_1; both builder self-tests are named in the Task_2 gate.
- 2026-09-14 Copilot round 17: the source-only provenance sidecar has a defined path convention and schema (input path and hash, output hash, workflow id) and the snapshot builder locates it from its source argument and refuses to run without it.
- 2026-09-14 Copilot round 18: the lexical path never reaches the enrichment step, so the consumer-side enrichment guard is stated for the vector-only mode and lexical coverage is the admission rejection plus the input-boundary test; Task_2 regenerates the LongMemEval source-only file to emit the provenance sidecar (bytes asserted unchanged) and documents the sidecar in the scripts README.
- 2026-09-14 Restructured on the decider's direction: the Definition of Done is reduced to contract level (content, consumers, baselines, identities, snapshot, re-baseline, README) and the validated implementation detail from the review rounds moves into per-task design notes that the workers may follow or improve on; no requirement is dropped. Further implementation-level review findings go into the worker briefs and the implementation review, not into this plan.
- 2026-09-14 Preservation check after the restructure (evals-reviewer): three existing output and evidence requirements that the regrouping had dropped are restored (exact source text and artifact-wide id uniqueness in the snapshot test; the pre-run recheck of the preserved pair and the final manifest check; the sidecar documented in the scripts README and the regenerated manifest values recorded in the Decision Log).
- 2026-09-14 Two Copilot points on the restructured text applied as scope and accuracy corrections: the observation design note names the speaker-keyed object traversal, and the LoCoMo source-only file is regenerated to emit its sidecar too (bytes unchanged) so the builder's no-sidecar rule holds for both datasets. Further shape-only findings go to the briefs.
- 2026-09-14 Three further Copilot points applied as contract corrections: the compatibility stance names the snapshot manifest change (new dataset-hash field, new LongMemEval workflow id, older manifest still admitted); the baseline admission rule is scoped to LoCoMo with LongMemEval's lexical behaviour unchanged; the baseline episode text is named as the generic sentence used today rather than a chat-log projection, with turn-based episode text recorded as a v0.2 value-audit question.
- 2026-09-14 Five small accuracy and evidence corrections (Copilot): A2 states the date exception for the 13 repeats; the older-manifest admission is qualified to the v1 LoCoMo manifest; the LoCoMo snapshot and manifest hashes are recorded with the re-baseline; the cross-mode ruling is explicit in the Decision Log; Task_3 owns the lessons file to retarget the link when the plan moves.
- 2026-09-14 Two more from Copilot: README documents the snapshot manifest admission (a new operator-facing requirement); the evidence design note resolves an exact dialog id before splitting on commas.
- 2026-09-14 Three more from Copilot: the LongMemEval lexical baseline is stated as unchanged in admission with results moving only for the 13 repeats; a v1 manifest with a present but mismatched dataset hash joins the admission tests; the PR description is aligned with the generic-sentence baseline episode text.
- 2026-09-14 From the latest Copilot pass, three items kept as done-criteria or measurement validity (a regression for the existing enrichment-file branch; README wording that the memories are merged alongside a snapshot at runtime; equal dataset input hash per re-baseline pair); the three implementation-shape items (structured error for the snapshot validator, exact manifest field naming, sidecar workflow-id equality) are routed to the worker briefs.
- 2026-09-14 Plan approved by the decider at 63f9aff; Task_1 dispatched to evals-worker on task/bdc-locomo and Task_2 to evals-worker2 on task/bdc-longmemeval, both stacked on the plan branch (PR #40 stays open until the stack merges).

- 2026-09-14 Task_1 and Task_2 delivered: independent review approved shared metric #41 at 55cbaa5, LongMemEval #42 at 6038087 and LoCoMo #43 at 86a5922. Copilot approved #41; one mirror-image cross-PR comment on each of #42 and #43 was answered.
- 2026-09-15 Task_1 merged through #43 into the plan branch as 2278d80. The refreshed stack then merged shared DCG #41 as 399bf93 and LongMemEval #42 as 77c5796; the linked stack also carried plan PR #40 into main as 807fee3 before closeout. That unintended merge is recorded in the lessons log.
- 2026-09-15 Follow-up #45 added typed WrongDataset admission for snapshot manifest names; independent reviewer and Copilot approved it, and it merged as 9661ec6.
- 2026-09-15/16 The orchestrator ran all four configs at base c1b3637 and tip 9661ec6 with library 4a00303 and cleanup enabled. The base lexical pair began at 02:22 UTC; base LoCoMo hybrid took 87 minutes and LongMemEval hybrid took 7.1 hours against the preserved bare-id snapshot. The tip lexical pair was followed by LoCoMo hybrid (97 minutes) and LongMemEval hybrid (7 hours) against the regenerated snapshot. Four diffs and all header/report evidence were retained for this record.
- 2026-09-16 Task_3 records the four comparisons below, verifies the headers and changed-query sets independently, updates the consumer documentation and two lessons, and moves this plan to completed. The balanced plan validator, local Markdown target/anchor checks and cargo fmt --all --check pass. The closeout review and recorded-evidence cleanup are owned by the orchestrator; the preserved bare-id pair remains retained for rollback until the decider releases it.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-14 Recorded for the v0.2 value audit, not decided here: whether a retrieved LoCoMo derived memory carrying evidence provenance should count toward evidence recall; today only retrieved observations count.
- 2026-09-14 Snapshot precedence (LoCoMo): a configured enrichment snapshot adds graph on top of the item's own content; dataset-derived memories are the item's own content and are ingested with it. Merging is the runner's job; regenerating the LoCoMo snapshot is not required because its ids and the dataset-derived ids are disjoint. The LongMemEval snapshot is the exception recorded below, regenerated because bare ids no longer preserve repeated-session provenance.
- 2026-09-14 LongMemEval snapshot binding: precomputed snapshots reference bare session and turn ids and their builder collapsed repeats to one copy; regenerating them costs provider calls for 13 noise sessions whose copies differ only by date. Snapshot provenance binds to the bare id (first copy); later copies carry their raw turns only. Revisit if LongMemEval snapshots are regenerated for another reason.
- 2026-09-14 Correction replacing the entry above (plan review): the builder is the deterministic, provider-free source-replay builder, so regeneration is cheap, and binding unchanged bare provenance to the first copy would silently move the temporal source occurrence (the builder used the last visible one). Ruling: the builder adopts the identity assignment, every copy is represented with its own date and ids, and the LongMemEval snapshot is regenerated locally and kept untracked (its manifest values recorded here); provenance and dates are preserved, not rebound.
- 2026-09-14 Vector-only baseline: `configs/locomo_vector.toml` keeps the derived-content flags off. Its surface policy admits episodes and observations only, and like the lexical baseline it is a search over the chat log that Character Memory is compared against.
- 2026-09-14 Lexical baseline scope: the hurdle indexes every text item Character Memory ingests, derived memories included, so the comparison stays like for like; scoring for LoCoMo still counts retrieved observations only.
- 2026-09-14 Decider ruling replacing the entry above: the lexical baseline searches the chat log only (episodes and observations), never derived summaries or observations. It is the crude basic search over the logs that every Character Memory retrieval mechanism, including the dataset-provided derived content, is compared against. The lexical baseline config keeps both derived-content flags off.
- 2026-09-14 Cross-mode configs: the three LoCoMo cross-mode configs are in scope and turn the derived-content flags on (superseding the earlier round that had removed them), because they are hybrid configs whose purpose is identical content across vector-store modes; no cross-mode run is part of this plan and the next parity run measures them as changed.
- 2026-09-14 Baseline episode text: the lexical and vector-only baselines keep the generic descriptive sentence as LoCoMo episode text, as today, so the baselines do not move with this plan; making episode text the session's turns would change what the hurdle measures and is left to the v0.2 value audit.
- 2026-09-14 Plan-time versus implementation-time findings (decider direction after 29 review rounds): a plan fixes scope, contracts, ownership and how done is judged; findings that only choose an implementation shape (error placement, file naming, counter plumbing, fixture contents, wording) are input to the worker briefs and the implementation review. The reviewed shapes gathered so far are kept as per-task design notes so nothing validated is lost.
- 2026-09-14 Decider approved the plan at 63f9aff. Q1: LoCoMo generated observations are ingested as `DerivedType::Claim` (a factual statement about a person or event derived from dialog evidence; Reflection stays the type of the session summary, and none of the preference, commitment or note types fits a third-party factual statement). Q2: the re-baseline runs of Task_3, including the hybrid provider runs, are authorized by the decider for this plan.

- 2026-09-14 Duplicate credit belongs in the shared metric: the shared retrieval metrics deduplicated recall hits but counted a repeated gold id in DCG at every rank, so mapping two copies onto one session id would have double-counted. Ruling (worker alert during Task_2): the shared DCG gains a seen-id set so a repeated retrieved id never earns credit twice for any dataset, and the LongMemEval scorer maps exact identities only; Task_2 owns that one shared change and its unit test, delivered as its own change beneath the Task_2 branch so the dataset change carries no shared-crate diff and ADR-I-0004 holds. No existing measurement moves, since retrieved lists carried unique ids before this plan.

### 2026-09-16 Re-baseline record and measurement amendment

The four pairs below compare base harness `c1b36378232070dab541f078b26eb22663346668` with tip harness `9661ec647bdb5ac767424b1a94a30e0f08b49408`. Every header records library `4a003038853e6236281689f0588d467e4ff1019a`. Both sides of each LoCoMo pair record input SHA-256 `79fa87e90f04081343b8c8debecb80a9a6842b76a7aa537dc9fdf651ea698ff4`; both sides of each LongMemEval-S pair record `d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442`. These are the full run-header values, independently checked against the original dataset files.

All eight configurations have `retain_stores` off. Hybrid runs use live OpenAI `text-embedding-3-large` with 3072 dimensions; this comparison measures live runs, including their observed score variation.

| Pair / config | Base config SHA-256 | Tip config SHA-256 |
|---|---|---|
| `locomo_bm25.toml` | `73cda9ee1e119ce922020e54b18629a30dedb24db1bf3cec55dc021107a676ab` | `8bf1ad452f748ec6ca0b48d168b3ca056adf793324bced798412447a3f535462` |
| `longmemeval_s_bm25.toml` | `010855490cb89f9be37b76cd5b68b65a9bb41d136269c9ad1787f0b7320b08cd` | `010855490cb89f9be37b76cd5b68b65a9bb41d136269c9ad1787f0b7320b08cd` |
| `locomo_retrieval.toml` | `05eea3bad6be4a5b0063232e7fe413ddec15cd873c3997b6353e95e8559db958` | `d6ad0b58bcfa998e5ac35f99e36ec3ed0b23ee496d9999212dc36e8d8ebd3d7c` |
| `longmemeval_s_retrieval.toml` | `a12d822d169f032d0fb56edb276d15d2d981487fbcb4bc661a4f8edc4dcbe055` | `279cf7dfdb43c2bae20fb16f1a51c84da65f7d1a431b5d50181d146adb397986` |

Each config hash was recomputed from the header's exact TOML and matched its `config.sha256` sidecar. LoCoMo lexical changes only the two derived-content flags from true to false; LoCoMo hybrid changes them from false to true. LongMemEval lexical is byte-identical. The LongMemEval hybrid base hash covers the temporary `longmemeval_s_retrieval_base.toml`, whose only difference from the tip config is `enrichment_snapshot_path` pointing at the preserved bare-id pair. Thus only the LongMemEval lexical pair has an equal config hash; the other differences are intentional and recorded.

| Run | Base header generated_at (UTC) | Tip header generated_at (UTC) |
|---|---|---|
| `locomo_bm25` | `2026-09-15T02:22:25.749300100Z` | `2026-09-15T02:57:59.582068700Z` |
| `longmemeval_s_bm25` | `2026-09-15T02:23:02.400489400Z` | `2026-09-15T02:58:21.998302600Z` |
| `locomo_hybrid` | `2026-09-15T02:23:39.683370500Z` | `2026-09-15T06:21:26.273931900Z` |
| `longmemeval_s_hybrid` | `2026-09-15T03:50:43.151858Z` | `2026-09-15T16:06:21.784151800Z` |

| Pair | Queries | Differing queries | Identity changes | Rank changes | Metric changes | Degradation changes |
|---|---:|---:|---:|---:|---:|---:|
| LoCoMo lexical | 1986 | 0 | 0 | 0 | 0 | 0 |
| LongMemEval-S lexical | 500 | 13 | 6 | 13 | 6 | 0 |
| LoCoMo hybrid | 1986 | 1983 | 1983 | 1983 | 1983 | 0 |
| LongMemEval-S hybrid | 500 | 18 | 6 | 18 | 7 | 0 |

No pair has a query missing from either side. The diff command returns nonzero for the three differing pairs; that result is recorded as a comparison outcome.

- LoCoMo lexical changed-query set: empty; the official-file corpus is unchanged, as expected.
- LongMemEval-S lexical changed-query set: `001be529`, `078150f1`, `18bc8abd`, `1d4da289`, `1e043500`, `58bf7951`, `91b15a6e`, `c7dc5443`, `caf03d32`, `d23cf73b`, `gpt4_4929293b`, `gpt4_76048e76`, `gpt4_c27434e8_abs`. This is exactly the set of 13 official items with repeated haystack session IDs.
- LoCoMo hybrid changed-query set: all official questions except `conv-26:qa:157`, `conv-30:qa:44`, `conv-42:qa:227`; the differences span all ten conversations.
- LongMemEval-S hybrid changed-query set: `0862e8bf`, `1cea1afa`, `1de5cff2`, `2788b940`, `4388e9dd`, `4f54b7c9`, `561fabcd`, `6222b6eb`, `76d63226`, `852ce960`, `95228167`, `a3838d2b`, `dcfa8644`, `e831120c`, `gpt4_4fc4f797`, `gpt4_68e94288`, `gpt4_7ddcf75f`, `gpt4_d6585ce8`. None is one of the 13 repeated-session items.

Report means below are rounded to four decimal places; counts above and hash values are exact.

| Pair | Metric | Base mean | Tip mean |
|---|---|---:|---:|
| `locomo_bm25` | `session_recall_any@10` | 0.4031 | 0.4031 |
| `locomo_bm25` | `session_ndcg@10` | 0.1734 | 0.1734 |
| `locomo_bm25` | `dialog_recall_any@10` | 0.5580 | 0.5580 |
| `locomo_bm25` | `dialog_ndcg@10` | 0.3806 | 0.3806 |
| `locomo_bm25` | `retrieved_context_tokens` | 704.4693 | 704.4693 |
| `longmemeval_s_bm25` | `session_mrr@10` | 0.0036 | 0.0036 |
| `longmemeval_s_bm25` | `turn_mrr@10` | 0.6583 | 0.6583 |
| `longmemeval_s_bm25` | `turn_ndcg@10` | 0.6429 | 0.6429 |
| `longmemeval_s_bm25` | `retrieved_context_tokens` | 11343.0260 | 11341.3320 |
| `locomo_hybrid` | `session_recall_any@10` | 0.0339 | 0.3768 |
| `locomo_hybrid` | `session_ndcg@10` | 0.0166 | 0.3227 |
| `locomo_hybrid` | `dialog_recall_any@10` | 0.5156 | 0.2871 |
| `locomo_hybrid` | `dialog_ndcg@10` | 0.3547 | 0.2366 |
| `locomo_hybrid` | `num_derived_memories` | 5.6299 | 8.8646 |
| `locomo_hybrid` | `retrieved_context_tokens` | 552.0851 | 733.6908 |
| `longmemeval_s_hybrid` | `session_mrr@10` | 0.0032 | 0.0022 |
| `longmemeval_s_hybrid` | `turn_mrr@10` | 0.7614 | 0.7614 |
| `longmemeval_s_hybrid` | `turn_ndcg@10` | 0.7474 | 0.7474 |
| `longmemeval_s_hybrid` | `retrieved_context_tokens` | 2292.3500 | 2291.3580 |

LoCoMo hybrid provenance coverage is 1.0 on both sides; every measured leakage rate (`orphan_vector_leakage_rate`, `superseded_current_leakage_rate` and `suppressed_memory_leakage_rate`) is zero on both sides. Dataset summaries and observations make session-level retrieval work where it barely did. Those derived memories also occupy pack slots previously held by dialog turns, so dialog-level recall falls within the same pack size. The 2026-09-16 ruling treats this as the measured character with its new content, not a defect; it is planning input for the v0.2 pack-admission and ranking work. LoCoMo scoring still credits retrieved dialog observations rather than evidence references carried by a retrieved derived memory.

Measurement amendment, orchestrator ruling 2026-09-16: the acceptance condition is content-attributable LongMemEval differences confined to the 13 repeated-session items, with provider jitter recorded separately and its observed magnitude stated. All 13 repeated-session items have identical ordered retrieved identity lists in the two hybrid runs, and none retrieves an episode, whether an original or a later copy. The tip retrieves 43 episodes among 5982 items overall. Of the 18 differing hybrid queries outside that set, 12 reorder the same members and six replace one retrieved item each; 482 queries retain the same ordered identity list. The largest absolute score difference for matched items is `0.013519287109375`. This independently reproduces `jitter_longmemeval_s_hybrid.json`. Under the ruling these differences are recorded as provider embedding jitter, with zero observed content-attributable LongMemEval hybrid changes. The observed LongMemEval jitter scale provides context for the much larger LoCoMo content effect; it is not an independently measured upper bound for LoCoMo.

| Snapshot pair | Artifact SHA-256 | Manifest SHA-256 |
|---|---|---|
| LoCoMo, both hybrid runs | `85c5c7ef964d2554155ec2a9a0c797ec54c1d17f9cd81542f72d9dd4be7c149f` | `c66a0a1c75163ee9691976de8641ff30b9b1476701f1638bcb5511648edac5f6` |
| LongMemEval-S base, preserved bare-id pair | `dee954d719d354f38874f6f58c01e51c6f5438df5426cb67080d392ac9986145` | `2ca55bdd9e743dcd40174c52f814f713fc1f56a128ac21e37ab94fceb4602129` |
| LongMemEval-S tip, regenerated pair | `c4c3537021e68caaa6e52d002dbc97264d89cf757b5761528649df7b9f6f46e0` | `11a1776dcb8ca315bd5ecefcdb2d6fa36f23201f4d3efafeb07a59b40ae5bd58` |

The regenerated manifest at `datasets/enriched/longmemeval_s_online_snapshots_manifest.json` records workflow `deterministic-exact-source-replay-v2`, dataset name `longmemeval-s`, official `dataset.sha256` `d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442`, sanitized `source.sha256` `add932a4fea279a96fe1e85430133cd9a316126f55e01473a3fa54cad9c2e6d1`, 22392 threads and 231565 derived memories. Its artifact hash matches the configured snapshot `datasets/enriched/longmemeval_s_online_snapshots.jsonl`. The original dataset files, unchanged LoCoMo snapshot/manifest and regenerated LongMemEval pair were hash-verified again during closeout; the preserved pair was verified before the base run and again during closeout.

Evidence and disposition: the eight `header.json`, `report.json` and `config.sha256` files live under each base/tip worktree's `.agent-work/bdc-rebaseline/<base-or-tip>/<run>/`; the tip worktree also holds `diff_<run>.txt`, the two hybrid `deltas_<run>.json`, `record_lexical.json` and the jitter record. The orchestrator verified zero stores after each run, and closeout independently found every header's storage root absent. Run outputs remain within those recorded evidence directories until the orchestrator removes `results.jsonl` after this record is written; headers, reports, diffs and deltas stay until the closeout PR merges, then are removed. The bare-id snapshot and manifest remain under `.agent-work/evals-worker2/bdc-task2/preserved/`, with `preservation.json` retaining the reason `rollback source and Task_3 base-run input`; their deletion requires the decider's instruction. Original dataset assets and the configured regenerated snapshot are retained.

Closeout ownership amendment: the orchestrator owns run execution, the four diffs, cleanup and final review; Task_3 owns evidence verification and these documentation changes. No new decision record is proposed: the measurements and their interpretation belong to this experiment record, the two workflow/admission lessons belong to the lessons log, and the shared-metric packaging decision already follows ADR-I-0004.

## Notes
- The README's enrichment section already describes the LoCoMo default as indexing summaries and observations; this plan makes that true end to end. The BM25 baseline config had both flags on and silently ran without the content; by the decider's ruling the baseline searches the chat log only, so its flags go off; for the official file the base commit never loaded the top-level summaries and the tip projects them away, so the lexical corpus is expected to be unchanged; a file carrying record-level summary annotations would already have them in the base corpus, so the re-baseline compares the projection change and records any difference with its cause rather than assuming none.
