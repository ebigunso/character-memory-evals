# Plan: Benchmark-Derived Content (LoCoMo dataset summaries and observations; LongMemEval repeated sessions as distinct memories)

- status: draft
- generated: 2026-09-14
- last_updated: 2026-09-14
- work_type: code

## Goal
- LoCoMo runs ingest the summaries and observations the official dataset provides, as provenanced memories that reach the library ingest and the precomputed-enrichment path, so the harness measures Character Memory over the full benchmark content while leaving the door open for library-generated reflections later; the lexical baseline stays a crude search over the chat log alone, the hurdle every Character Memory retrieval mechanism is compared against; LongMemEval-S ingests the 13 repeated sessions as distinct memories with their own dates, because the date changes how the graph retrieves a memory and must not be overridden (decider rulings 2026-09-14).

## Definition of Done
- LoCoMo loader: reads the official top-level `session_summary` map (keys `session_<N>_summary`, one string per session) and `observation` map (keys `session_<N>_observation`; per speaker, a list of `[statement, evidence]` pairs where evidence is a string of one or more comma-separated dialog ids, or a list of such strings); every official session parses with its summary and its typed generated observations (speaker, statement, evidence dialog ids normalized by splitting on commas and trimming). The numeric and bare-id lookups that never matched official data are removed, and the crate rustdoc states the admitted key forms.
- LoCoMo ingest: the session summary is the episode summary and, under `index_session_summaries`, a Reflection derived memory as today; under `index_generated_observations`, each generated observation becomes one derived memory whose text is the statement alone, with provenance to the session episode and to the observations named by its normalized evidence ids that exist in the sample (an evidence id naming no turn is dropped from provenance and counted), and, only if ingest already emits speaker entities, attached to that entity (no new entity plumbing). The two flags keep their names and meaning: they select dataset-provided content; a library-generated source is the v0.2 comparison and is not built here.
- LoCoMo consumers: with a precomputed enrichment snapshot configured, the run ingests the dataset-derived memories and the snapshot graph together (the runner merges them; external ids are disjoint by construction since dataset-derived ids carry `:derived:`; a collision is a typed failure), instead of the snapshot discarding them. The lexical baseline keeps indexing episodes and observations only (the chat log), never derived memories, by the decider's ruling: it is the crude search the Character Memory retrieval mechanisms are measured against. Because the baseline indexes an episode's summary text, the LoCoMo lexical path hands it a chat-log projection of each episode (the generic descriptive sentence used today, never the dataset summary) while Character Memory ingest keeps the dataset summary on the episode; the engine is unchanged. Consumer-level tests prove the snapshot path carries the observations, the lexical baseline indexes no derived memory, and a distinctive token from a dataset summary is absent from the lexical baseline's indexed and retrievable text with the flags off.
- LoCoMo configs: the maintained Character Memory configs (`configs/locomo_retrieval.toml`, `configs/locomo_crossmode_embedded.toml`, `configs/locomo_crossmode_service.toml`, `configs/locomo_crossmode_service_repeat.toml`) index summaries and observations; the two baseline configs, `configs/locomo_bm25.toml` and `configs/locomo_vector.toml`, turn both flags off, since a baseline searches the chat log only (the vector-only surface policy admits episodes and observations alone, so writing derived memories there would surface nothing); no hash-cited config is edited (only continuity configs are hash-cited in the register).
- LongMemEval-S identities: ingest assigns every episode an external id that is unique within the item without narrowing admission: a session id that occurs once keeps its bare id; a later occurrence gets the bare id plus a `#<ordinal>` suffix, and if that string is itself present among the item's session ids the ordinal is raised until the id is unused. Observation ids follow their episode id. Every copy keeps its own date. One helper in the dataset crate derives the identity assignment from the item's session id list; ingest and scoring both call it, so the mapping is recomputed from the item rather than carried through the runner seam (which passes the item and the retrieved items to scoring) and nothing parses a suffix back.
- LongMemEval-S scoring: both metric families use that helper's mapping. Retrieved episode ids map to benchmark session ids; retrieved observation ids map by replacing their episode-id prefix through the same mapping so a turn of any copy matches its bare gold turn id; after mapping, repeated ids are collapsed to their first rank so a session retrieved as two copies earns credit once and every metric stays within its bounds.
- LongMemEval-S enrichment snapshots: the precomputed snapshots are built by the deterministic, provider-free source-replay builder (`scripts/enrichment/build_snapshots.py`), which today keeps only the last visible occurrence of a repeated session id and references sessions and turns by bare id. The builder applies the same identity assignment as ingest (bare id for a single occurrence, `#<ordinal>` for later ones, ordinal raised past any present id), represents every copy with its own date, and references each copy's own episode and turn ids; the LongMemEval snapshot artifact and manifest are regenerated and committed. A test loads the regenerated snapshot against ingest identities and proves every reference resolves to the copy it was built from, so provenance and dates are preserved rather than rebound.
- Re-baseline: for each dataset, the lexical baseline config and the hybrid retrieval config run once at the base commit and once at the tip (same library pin, cleanup on), diffed with the runner's diff command; headline metrics, diff counts and the changed item sets are recorded in this plan's Decision Log. LongMemEval differences are confined to items among the 13.
- README's dataset and enrichment sections state both behaviours; the plan closes in completed.

## Scope / Non-goals
- Scope: `crates/cmem-eval-locomo` (types, loader, ingest, rustdoc, tests), `crates/cmem-eval-runner/src/pipeline.rs` (snapshot merge for LoCoMo), `crates/cmem-eval-longmemeval` (ingest, scoring, types, tests), the snapshot builder's identity assignment and the regenerated LongMemEval snapshot artifact with its manifest, the LoCoMo benchmark configs, README, the re-baseline runs and their record.
- Non-goals: any change to the lexical baseline (it indexes episodes and observations only); any LoCoMo scoring change (evidence recall stays measured on retrieved observations; whether a retrieved derived memory with evidence provenance should count is a v0.2 value-audit question, recorded below); any library change; a library-generated summary or observation source; new entity plumbing; regenerating the LoCoMo snapshots (their ids are unaffected); edits to sealed evidence or hash-cited configs; the loaders' admission rules.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: parsed LoCoMo items (summaries and observations now populated; the generated-observation type becomes structured), LoCoMo derived-memory identities, LongMemEval episode and observation identities for repeated sessions, benchmark measurement baselines
- stance: break
- justification: the harness is pinned to a library version and old artifacts are old, not a compatibility surface (harness strictness follows the claim); every consumer of these types is in this workspace; sealed evidence is untouched by construction (new runs write new files).

## Context (workspace)
- Related files/areas: `crates/cmem-eval-locomo/src/loader.rs` (`apply_benchmark_derived_fields`, `generated_observations_from_value`, which today joins a `[statement, evidence]` pair into one string), `crates/cmem-eval-locomo/src/types.rs` (`LoCoMoSession.generated_observations: Vec<String>`), `crates/cmem-eval-locomo/src/ingest.rs` (summary and observation derived memories, flags, participants), `crates/cmem-eval-runner/src/pipeline.rs` (`LoCoMoSpec::enrichment` returns the snapshot graph and drops the mapped derived memories; `Bm25Baseline::new` receives episodes and observations only, which stays), `crates/cmem-eval-longmemeval/src/ingest.rs` (episode and observation external ids from `session_id`), `crates/cmem-eval-longmemeval/src/scoring.rs` (session metrics on retrieved episode ids against `answer_session_ids`; turn metrics on retrieved observation ids against gold turn ids), `crates/cmem-eval/src/metrics.rs` (`retrieval_metrics`, `insert_retrieval_metrics`), `configs/locomo_*.toml`, README "Precomputed Graph Enrichment" (already claims the default LoCoMo config indexes summaries and observations; false today).
- Existing patterns or references: derived memories carry source episode or observation external ids (README enrichment rule); LoCoMo turn ids (`dia_id`) are the observation external ids at ingest, so evidence ids resolve to provenance directly; the dataset admission plan's Decision Log records the census that found the key mismatch and the 13 repeated sessions.
- Design record consulted and deviations from its acceptance: ADR-I-0004 (dataset crates own their loaders and ingest; the runner and the shared crate change only where the consumer paths live) and ADR-I-0005 (artifact readers unaffected). No deviation.

## Open Questions (max 3)
- Q1: Which `DerivedType` the generated observations use (library vocabulary: Reflection, UserPreference, AssistantPreference, Commitment, OpenLoop, CharacterSignal, RelationshipNote, ProjectNote, and the rest). Task_1 picks the closest neutral type and reports the rationale; the orchestrator records it in the Decision Log; the summary keeps Reflection as today.
- Q2: Whether the hybrid re-baseline runs (paid provider calls) are authorized; the decider authorizes provider runs per run. Task_3 asks before running; the lexical runs need no authorization.

## Assumptions
- A1: Official LoCoMo carries 272 summaries and 2541 observation pairs, evidence a string in 2531 and a list in 10, and after splitting on commas the evidence references number 2561 with zero unresolved — source: censuses 2026-09-14 (orchestration session and plan review); re-checked by Task_1's parser tests over the full file.
- A2: The 13 LongMemEval repeats are never answer sessions and no official session id contains `#` — source: plan review census 2026-09-14; re-checked by Task_2's census.
- A3: No benchmark config is hash-cited in `reports/v0-1-5-findings-register.md` (only continuity configs are) — source: register grep 2026-09-14, confirmed at plan review.

## Tasks

### Task_1: LoCoMo dataset summaries and observations reach every consumer
- type: impl
- owns:
  - crates/cmem-eval-locomo/**
  - crates/cmem-eval-runner/src/pipeline.rs
  - configs/locomo_retrieval.toml
  - configs/locomo_vector.toml
  - configs/locomo_bm25.toml
  - configs/locomo_crossmode_embedded.toml
  - configs/locomo_crossmode_service.toml
  - configs/locomo_crossmode_service_repeat.toml
- depends_on: []
- description: |
  Read the official key forms; type the generated observations (speaker, statement, normalized evidence dialog ids) instead of flattening them; remove the lookups that never matched official data and state the admitted forms in the crate rustdoc; ingest each observation as one derived memory with provenance to the episode and to the resolved evidence observations, attached to the speaker entity only if ingest already emits one; count and report unresolved evidence ids; merge dataset-derived memories with a configured enrichment snapshot in the runner instead of discarding them; leave the lexical baseline on episodes and observations only, handing it the chat-log projection of each episode (today's generic sentence, not the dataset summary) so its corpus does not change; keep the flags, turn them on in the owned Character Memory configs and off in both baseline configs; report the chosen derived type with its rationale.
- acceptance:
  - Parsing the official file yields 272 summaries and 2541 typed observations carrying 2561 evidence references with zero unresolved; a comma-separated evidence string is a regression case.
  - Ingest tests prove one derived memory per observation with the statement as text and provenance to the episode plus the resolved evidence observations, and the summary as episode summary and Reflection derived memory under its flag.
  - A runner-level test proves that with a snapshot configured the item's memory inputs contain both the dataset-derived memories and the snapshot graph, with a typed failure on an external-id collision; lexical-baseline tests prove derived memories are not indexed and that a distinctive dataset-summary token is absent from the indexed and retrievable text.
  - The dataset admission strict set is unchanged (its tests pass); optional-annotation leniency still holds for files without the maps.
  - The owned Character Memory configs index summaries and observations and the two baseline configs do not; no other config changes.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (service-free); the full-file census printed by a test or example"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review; independent census of summaries, observations and evidence references over the official file; provenance check on a sample of derived memories; snapshot-merge consumer check; confirmation that the lexical baseline indexes only episodes and observations with the chat-log episode projection and no dataset-summary token"

### Task_2: LongMemEval repeated sessions as distinct memories
- type: impl
- owns:
  - crates/cmem-eval-longmemeval/**
  - scripts/enrichment/build_snapshots.py
  - datasets/enriched/longmemeval_s_online_snapshots.jsonl
  - datasets/enriched/longmemeval_s_online_snapshots_manifest.json
  - datasets/enriched/longmemeval_s_online_snapshots_report.md
- depends_on: []
- description: |
  One crate helper derives the identity assignment from the item's session id list (bare id for a single occurrence; bare id plus `#<ordinal>` for later occurrences, ordinal raised past any id already present). Ingest uses it for episode and observation ids, each copy keeps its date; scoring calls the same helper on the item to map retrieved episode and observation ids for both metric families and collapses repeated mapped ids to their first rank. The snapshot builder applies the same identity assignment, represents every copy with its own date and ids, and the LongMemEval snapshot artifact, manifest and report are regenerated with it; the LoCoMo snapshot is byte-identical after the builder change. Tests cover repeated sessions (identities, dates, both scoring families, duplicate credit), a native id that already looks suffixed, and identity stability for non-repeated sessions.
- acceptance:
  - Ingest tests: two copies of one session id produce two episodes with distinct external ids and their own dates, observation ids follow the episode id, and an item whose ids already contain `s#2` beside two `s` copies yields three distinct ids without changing admission.
  - Scoring tests: a retrieved later copy counts for its answer session; a retrieved turn of a later copy counts for its bare gold turn id; a session retrieved as two copies earns credit once and every metric value stays within its bounds; the mapping used is the helper's output for the item, not a parsed suffix.
  - Snapshot test: the regenerated LongMemEval snapshot resolves every session and turn reference to the ingest identity of the copy it was built from, both copies of a repeated session carry their own dates, and no reference dangles; the LoCoMo snapshot artifact is unchanged.
  - Census over the official file: exactly 13 items produce a suffixed identity, none an answer session; a dump of episode external ids before and after is identical except those 13 copies.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (service-free); the census and the identity dump comparison"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review; independent census; identity stability check; metric boundedness probe with a two-copy retrieval"

### Task_3: Re-baseline, README, closeout
- type: impl
- owns:
  - README.md
  - docs/coding-agent/plans/active/benchmark-derived-content-plan.md
  - docs/coding-agent/plans/completed/benchmark-derived-content-plan.md
- depends_on: [Task_1, Task_2]
- description: |
  Run `configs/locomo_bm25.toml`, `configs/locomo_retrieval.toml`, `configs/longmemeval_s_bm25.toml` and `configs/longmemeval_s_retrieval.toml` once at the base commit and once at the tip (same library pin; cleanup on; provider runs only after the decider's per-run authorization), diff each pair with the runner's diff command, and record headline metrics, diff counts and the item sets that changed in the Decision Log; state both behaviours in README's dataset and enrichment sections; move the plan to completed.
- acceptance:
  - Four before-and-after pairs recorded with config hashes, library commit and diff counts; LongMemEval differences are confined to the 13 items.
  - README states that LoCoMo ingests dataset-provided summaries and observations as provenanced derived memories under the two flags, that they reach the snapshot path, that the lexical baseline searches the chat log only, that a library-generated source is a later comparison, and that repeated LongMemEval sessions are distinct memories with their own dates.
  - No run store or artifact remains outside the recorded evidence; the plan is in completed with its final progress entry.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the eight runs and four diffs; store cleanup census after each run"
  - kind: review
    required: true
    owner: orchestrator
    detail: "Decision Log record, README wording, closeout entry"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1, Task_2]
- Wave 2 (parallel): [Task_3]

## Rollback / Safety
- Reverting the branch restores today's parsing, identities and baseline inputs; no persisted format changes; sealed evidence and hash-cited configs are untouched; new runs write new files only.

## Progress Log (append-only)

- 2026-09-14 Plan drafted from two decider rulings: LoCoMo uses the dataset's summaries and observations (with a library-generated source kept open for later); the 13 repeated LongMemEval sessions are distinct memories because the date changes graph retrieval and must not be overridden.
- 2026-09-14 Plan review (evals-reviewer and Copilot) applied: the consumer paths are in scope (the enrichment snapshot path discarded mapped derived memories and the lexical baseline never indexed them, so the flags alone delivered nothing in the maintained configs); evidence strings with comma-separated dialog ids are normalized (2561 references, zero unresolved); the LongMemEval identity policy is collision-safe and reversible through ingest data rather than suffix parsing, covers both scoring families, and collapses duplicate credit; the derived-type rationale flows through the worker report; three cross-mode configs; task types within the allowed set.
- 2026-09-14 Decider ruling: the lexical baseline stays a crude search over the chat log; derived-memory indexing is removed from the plan (Decision Log).
- 2026-09-14 Copilot round 2: the identity mapping is recomputed from the item by one crate helper shared by ingest and scoring, since the dataset seam passes the item and the retrieved items to scoring and carries no ingest output; LongMemEval snapshots are not regenerated and bind to the first copy of a repeated session (Decision Log).
- 2026-09-14 Plan review round 3 (evals-reviewer and Copilot): the snapshot decision is corrected, since the builder is deterministic and provider-free and keeps the last visible occurrence; the LongMemEval snapshots are regenerated with the shared identity assignment so every copy keeps its own provenance and date; `configs/locomo_vector.toml` stays a source-only baseline like the lexical one.
- 2026-09-14 Plan review round 2 (evals-reviewer): the baseline indexes episode summary text, so populated dataset summaries would have leaked into the lexical corpus with the flags off; the LoCoMo lexical path now hands the baseline a chat-log projection of each episode and a token test proves the absence.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-14 Recorded for the v0.2 value audit, not decided here: whether a retrieved LoCoMo derived memory carrying evidence provenance should count toward evidence recall; today only retrieved observations count.
- 2026-09-14 Snapshot precedence: a configured enrichment snapshot adds graph on top of the item's own content; dataset-derived memories are the item's own content and are ingested with it. Merging is the runner's job; regenerating snapshots is not required because their ids and the dataset-derived ids are disjoint.
- 2026-09-14 LongMemEval snapshot binding: precomputed snapshots reference bare session and turn ids and their builder collapsed repeats to one copy; regenerating them costs provider calls for 13 noise sessions whose copies differ only by date. Snapshot provenance binds to the bare id (first copy); later copies carry their raw turns only. Revisit if LongMemEval snapshots are regenerated for another reason.
- 2026-09-14 Correction replacing the entry above (plan review): the builder is the deterministic, provider-free source-replay builder, so regeneration is cheap, and binding unchanged bare provenance to the first copy would silently move the temporal source occurrence (the builder used the last visible one). Ruling: the builder adopts the identity assignment, every copy is represented with its own date and ids, and the LongMemEval snapshot is regenerated and committed; provenance and dates are preserved, not rebound.
- 2026-09-14 Vector-only baseline: `configs/locomo_vector.toml` keeps the derived-content flags off. Its surface policy admits episodes and observations only, and like the lexical baseline it is a search over the chat log that Character Memory is compared against.
- 2026-09-14 Lexical baseline scope: the hurdle indexes every text item Character Memory ingests, derived memories included, so the comparison stays like for like; scoring for LoCoMo still counts retrieved observations only.
- 2026-09-14 Decider ruling replacing the entry above: the lexical baseline searches the chat log only (episodes and observations), never derived summaries or observations. It is the crude basic search over the logs that every Character Memory retrieval mechanism, including the dataset-provided derived content, is compared against. The lexical baseline config keeps both derived-content flags off.

## Notes
- The README's enrichment section already describes the LoCoMo default as indexing summaries and observations; this plan makes that true end to end. The BM25 baseline config had both flags on and silently ran without the content; by the decider's ruling the baseline searches the chat log only, so its flags go off and its numbers are expected to hold across this plan; the re-baseline confirms that.
