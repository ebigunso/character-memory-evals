# Plan: Benchmark-Derived Content (LoCoMo dataset summaries and observations; LongMemEval repeated sessions as distinct memories)

- status: draft
- generated: 2026-09-14
- last_updated: 2026-09-14
- work_type: code

## Goal
- LoCoMo runs ingest the summaries and observations the official dataset provides, as provenanced memories, so the harness measures Character Memory over the full benchmark content while leaving the door open for library-generated reflections later; LongMemEval-S ingests the 13 repeated sessions as distinct memories with their own dates, because the date changes how the graph retrieves a memory and must not be overridden (decider rulings 2026-09-14).

## Definition of Done
- LoCoMo: the loader reads the official top-level `session_summary` map (keys `session_<N>_summary`, one string per session) and `observation` map (keys `session_<N>_observation`; per speaker, a list of `[statement, evidence]` pairs where evidence is a dialog id or a list of dialog ids); every official session parses with its summary and its typed generated observations (speaker, statement, evidence dialog ids). The numeric and bare-id lookups that never matched official data are removed.
- LoCoMo ingest: the session summary is the episode summary and, under `index_session_summaries`, a derived memory as today; each generated observation becomes one derived memory whose text is the statement alone, with provenance to the session episode and to the observations named by its evidence dialog ids that exist in the sample (an evidence id naming no turn is dropped from provenance and counted), attached to the speaker entity when ingest creates one. The two ingest flags keep their names and meaning: they select dataset-provided content; a library-generated source is the v0.2 comparison and is not built here.
- LoCoMo configs: the maintained benchmark configs (`configs/locomo_retrieval.toml`, `configs/locomo_vector.toml`, `configs/locomo_bm25.toml`, the four cross-mode configs) index summaries and observations; no hash-cited config is edited (only continuity configs are hash-cited in the register).
- LongMemEval-S ingest: a session id that repeats within an item keeps its bare id for the first occurrence and gets `<id>#<n>` (n from 2) for later occurrences, on the episode and its observations; every copy keeps its own date. Scoring maps a retrieved episode id back to the benchmark session id by stripping that suffix, so a hit on any copy of an answer session counts. The loader's admission exception for identical repeats is unchanged.
- Re-baseline: for each of the two datasets, the lexical baseline config and the hybrid retrieval config run once at the base commit and once at the tip (same library pin, cleanup on), diffed with the runner's diff command; headline metrics and the diff counts are recorded in this plan's Decision Log. LongMemEval differences are confined to items among the 13.
- README's dataset section states both behaviours; the plan closes in completed.

## Scope / Non-goals
- Scope: `crates/cmem-eval-locomo` (loader types, loader, ingest, tests), `crates/cmem-eval-longmemeval` (ingest, scoring, tests), the LoCoMo benchmark configs, README dataset section, the re-baseline runs and their record.
- Non-goals: any scoring change for LoCoMo (evidence recall stays measured on observations; whether a retrieved derived memory with evidence provenance should count is a v0.2 value-audit question, recorded below); any library change; a library-generated summary or observation source; edits to sealed evidence or hash-cited configs; the loader's admission rules.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: parsed LoCoMo items (summaries and observations now populated; the generated-observation type gains fields), LoCoMo derived-memory identities, LongMemEval episode identities for repeated sessions, benchmark measurement baselines
- stance: break
- justification: the harness is pinned to a library version and old artifacts are old, not a compatibility surface (harness strictness follows the claim); every consumer of these types is in this workspace; sealed evidence is untouched by construction (new runs write new files).

## Context (workspace)
- Related files/areas: `crates/cmem-eval-locomo/src/loader.rs` (`apply_benchmark_derived_fields`, `generated_observations_from_value`), `crates/cmem-eval-locomo/src/types.rs` (`LoCoMoSession.generated_observations: Vec<String>` today), `crates/cmem-eval-locomo/src/ingest.rs` (summary and observation derived memories, flags), `crates/cmem-eval-longmemeval/src/ingest.rs` (episode and observation external ids from `session_id`), `crates/cmem-eval-longmemeval/src/scoring.rs` (session metrics compare retrieved episode external ids with `answer_session_ids`), `configs/locomo_*.toml`, README "Precomputed Graph Enrichment" (which already claims the default LoCoMo config indexes summaries and observations; false today).
- Existing patterns or references: derived memories carry source episode or observation external ids (README enrichment rule); LoCoMo turn ids (`dia_id`) are the observation external ids at ingest, so observation evidence ids resolve to provenance directly; the dataset admission plan's Decision Log records the census that found the key mismatch and the 13 repeated sessions.
- Design record consulted and deviations from its acceptance: ADR-I-0004 (dataset crates own their loaders and ingest; no shared-crate edit), ADR-I-0005 (artifact readers unaffected). Library ADR-I-0029 for outcome shapes is not touched. No deviation.

## Open Questions (max 3)
- Q1: Which `DerivedType` the generated observations use (the library vocabulary: Reflection, UserPreference, AssistantPreference, Commitment, OpenLoop, CharacterSignal, RelationshipNote, ProjectNote, ...). Task_1 picks the closest neutral type with a one-line rationale in the Decision Log; the summary keeps Reflection as today.
- Q2: Whether the hybrid re-baseline runs (paid provider calls) are authorized; the decider authorizes provider runs per run. Task_3 asks before running; the lexical runs need no authorization.

## Assumptions
- A1: Official LoCoMo carries 272 summaries and 2541 observation entries, all `[statement, evidence]` pairs, evidence a string in 2531 and a list in 10 — source: census 2026-09-14 (orchestration session), re-checked by Task_1's parser tests over the full file.
- A2: The 13 LongMemEval repeats are never answer sessions — source: dataset admission plan Decision Log; re-checked by Task_2's census.
- A3: No benchmark config is hash-cited in `reports/v0-1-5-findings-register.md` (only continuity configs are) — source: register grep 2026-09-14.

## Tasks

### Task_1: LoCoMo dataset summaries and observations as provenanced memories
- type: impl
- owns:
  - crates/cmem-eval-locomo/**
  - configs/locomo_retrieval.toml
  - configs/locomo_vector.toml
  - configs/locomo_bm25.toml
  - configs/locomo_crossmode_embedded.toml
  - configs/locomo_crossmode_service.toml
  - configs/locomo_crossmode_service_repeat.toml
- depends_on: []
- description: |
  Read the official key forms; type the generated observations (speaker, statement, evidence dialog ids) instead of flattening them to strings; remove the lookups that never matched official data; ingest each observation as one derived memory with provenance to the episode and to the evidence observations, attached to the speaker entity when one exists; count and report evidence ids that name no turn; keep the flags and turn them on in the owned configs; tests over the full official file prove every session carries its summary and its observations with resolved provenance.
- acceptance:
  - Parsing the official file yields 272 summaries and 2541 typed observations with evidence ids; a census of unresolved evidence ids is reported.
  - Ingest tests prove one derived memory per observation with the statement as text and provenance to the episode plus the resolved evidence observations, and the summary as episode summary and Reflection derived memory under the flag.
  - The dataset admission strict set is unchanged (its tests pass); optional-annotation leniency still holds for files without the maps.
  - The owned configs index summaries and observations; no other config changes.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (service-free); the full-file census printed by a test or example"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review; independent census of summaries, observations and unresolved evidence ids over the official file; provenance check on a sample of derived memories"

### Task_2: LongMemEval repeated sessions as distinct memories
- type: impl
- owns:
  - crates/cmem-eval-longmemeval/**
- depends_on: []
- description: |
  Ingest gives later occurrences of a repeated session id the `<id>#<n>` identity on the episode and its observations, keeping each copy's date; a shared helper derives the benchmark session id from an episode id; scoring uses it so any copy of an answer session counts; tests cover an item with a repeated session (identities, dates, scoring) and prove first occurrences keep bare ids.
- acceptance:
  - Ingest test: two copies of one session id produce two episodes with distinct external ids and their own dates, and observation ids follow the episode id.
  - Scoring test: a retrieved `<id>#2` episode counts as a hit for answer session `<id>`.
  - Census over the official file: exactly 13 items produce a suffixed identity, none of them an answer session.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (service-free); the census"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review; independent census; identity stability check for non-repeated sessions (dump of episode external ids before and after, identical except the 13)"

### Task_3: Re-baseline, README, closeout
- type: mixed
- owns:
  - README.md
  - docs/coding-agent/plans/active/benchmark-derived-content-plan.md
  - docs/coding-agent/plans/completed/benchmark-derived-content-plan.md
- depends_on: [Task_1, Task_2]
- description: |
  Run `configs/locomo_bm25.toml`, `configs/locomo_retrieval.toml`, `configs/longmemeval_s_bm25.toml` and `configs/longmemeval_s_retrieval.toml` once at the base commit and once at the tip (same library pin; cleanup on; provider runs only after the decider's per-run authorization), diff each pair with the runner's diff command, and record headline metrics, diff counts and the item sets that changed in the Decision Log; state both behaviours in README's dataset section; move the plan to completed.
- acceptance:
  - Four before-and-after pairs recorded with config hashes, library commit, and diff counts; LongMemEval differences are confined to the 13 items.
  - README states that LoCoMo ingests dataset-provided summaries and observations as provenanced derived memories under the two flags, that a library-generated source is a later comparison, and that repeated LongMemEval sessions are distinct memories with their own dates.
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
- Reverting the branch restores today's parsing and identities; no persisted format changes; sealed evidence and hash-cited configs are untouched; new runs write new files only.

## Progress Log (append-only)

- 2026-09-14 Plan drafted from two decider rulings: LoCoMo uses the dataset's summaries and observations (with a library-generated source kept open for later); the 13 repeated LongMemEval sessions are distinct memories because the date changes graph retrieval and must not be overridden.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-14 Recorded for the v0.2 value audit, not decided here: whether a retrieved LoCoMo derived memory carrying evidence provenance should count toward evidence recall; today only retrieved observations count.

## Notes
- The README's enrichment section already describes the LoCoMo default as indexing summaries and observations; this plan makes that true. The BM25 baseline config had both flags on and silently ran without the content, so the lexical hurdle moves with this plan; the re-baseline records by how much.
