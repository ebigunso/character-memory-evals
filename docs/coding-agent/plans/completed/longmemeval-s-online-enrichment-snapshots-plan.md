# Plan: LongMemEval-S Online Enrichment Snapshots

- status: completed
- generated: 2026-05-04
- last_updated: 2026-07-05
- work_type: mixed

## Goal
- Build the LongMemEval-S enrichment dataset independently as per-question cutoff graph snapshots exported from chronological replay through the real Character Memory graph DB.
- Keep normal LongMemEval-S eval runs deterministic and cheap by loading persisted per-question snapshots instead of replaying memory generation every run.

## Definition of Done
- `datasets/enriched/longmemeval_s_online_snapshots.jsonl` can be generated from corrected `datasets/enrichment_source/longmemeval_s_source_only.json`.
- Generation writes `datasets/enriched/longmemeval_s_online_snapshots_manifest.json` with source/artifact hashes, replay settings, workflow/model identifiers, and object counts.
- LongMemEval-S eval can consume the correct graph snapshot by `question_id` before retrieval.
- Snapshot validation rejects forbidden QA/gold/evidence keys, missing source references, unresolved graph references, duplicate typed IDs, and stale source manifests.

## Scope / Non-goals
- Scope:
  - Add any shared snapshot types/config needed for LongMemEval-S.
  - Add LongMemEval-S replay-window helpers.
  - Add graph export support needed to write portable LongMemEval-S snapshot rows.
  - Add LongMemEval-S snapshot generation and eval consumption.
  - Add LongMemEval-S docs/config examples.
- Non-goals:
  - Build LoCoMo artifacts.
  - Persist per-step audit traces, prompt inputs, operation logs, or debug evidence by default.
  - Replace the real graph DB with a parallel operation reducer.
  - Regenerate snapshots during normal eval runs.
  - Store or expose benchmark QA/gold/evidence fields to the enrichment generator.

## Context
- LongMemEval-S is question-scoped in the current runner and resets namespace per question.
- The current source-only file keeps `question_id`, `haystack_session_ids`, `haystack_dates`, and `haystack_sessions`.
- The source-only file intentionally excludes QA/gold/evidence labels.
- Prior audit found 25 source-only string mismatches where official `U+2028` separators were normalized; exact preservation is preferred, with only semantic-preserving whitespace normalization tolerated when documented.

## Assumptions
- A1: LongMemEval-S requires one snapshot per question cutoff, keyed by `question_id`.
- A2: The current highest-level workflow boundary is the exposed public Character Memory API surface.
- A3: Replay generation mirrors intended usage: an LLM remembering process prepares graph-schema data for each source conversation thread/window while consulting relevant existing graph state, then submits the prepared data through the public remember API.
- A4: Source-only generation and replay should prefer byte-exact source strings; whitespace-only normalization is tolerated only when semantic-preserving and consistent with documented runtime ingestion behavior.

## Artifacts
- Primary artifact:
  - `datasets/enriched/longmemeval_s_online_snapshots.jsonl`
- Manifest artifact:
  - `datasets/enriched/longmemeval_s_online_snapshots_manifest.json`
- Snapshot row shape:
  - `snapshot_id`: stable snapshot identifier, for example `lme:e47becba@question_date`.
  - `namespace`: runtime namespace, for example `lme:e47becba`.
  - `dataset_item_id`: LongMemEval-S `question_id`.
  - `cutoff`: `{ "type": "question_date", "value": "<normalized_question_date_or_source_cutoff>" }`.
  - `graph`: portable `GraphEnrichmentInput` snapshot exported from the real graph DB.
- Manifest shape:
  - artifact path and hash.
  - dataset kind.
  - source-only path and hash.
  - replay mode and cutoff policy.
  - workflow/model version identifiers.
  - namespace/snapshot/object counts.

## Tasks

### Task_1: Define LongMemEval-S Snapshot Contract
- type: design
- owns:
  - `docs/coding-agent/plans/active/longmemeval-s-online-enrichment-snapshots-plan.md`
  - `README.md`
  - `datasets/enrichment_source/README.md`
- depends_on: []
- description: |
  Document the LongMemEval-S per-question snapshot artifact contract, manifest contract, lookup key, cutoff policy, and validation invariants.
- acceptance:
  - LongMemEval-S snapshot schema and manifest schema are documented.
  - `question_id` lookup and question-cutoff semantics are documented.
  - Non-persisted debug/evidence artifacts are explicitly out of default scope.
  - Source string preservation and tolerated whitespace-only normalization are documented.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Review LongMemEval-S artifact minimality, cutoff semantics, and source-preservation policy."

### Task_2: Add Snapshot Types And Config Needed By LongMemEval-S
- type: impl
- owns:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-core/src/lib.rs`
- depends_on: [Task_1]
- description: |
  Add portable graph snapshot structs, snapshot cutoff metadata, and config fields for snapshot artifact paths and manifest validation.
- acceptance:
  - Core exposes a `GraphSnapshotInput` type containing snapshot metadata and a `GraphEnrichmentInput` graph.
  - Ingest config can specify snapshot path and manifest path without breaking existing `enrichment_path` configs.
  - Config validation rejects ambiguous or incomplete snapshot settings.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-core"

### Task_3: Add LongMemEval-S Snapshot Loading And Validation
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/enrichment.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_2]
- description: |
  Implement snapshot JSONL loading, manifest loading, LongMemEval-S question lookup, and validation for forbidden keys, duplicate graph IDs, source references, graph references, and source hash compatibility.
- acceptance:
  - Snapshot loader supports LongMemEval-S lookup by `question_id`.
  - Validation rejects missing selected LongMemEval-S snapshots before or at question execution.
  - Existing namespace-level enrichment loader remains supported.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner enrichment"

### Task_4: Add Real Graph Export API
- type: impl
- owns:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_2]
- description: |
  Expose real-adapter graph export as portable `GraphEnrichmentInput`, with deterministic ordering and no internal DB IDs, embedding vectors, or backend-only fields.
- acceptance:
  - Real adapter exports entities, threads, derived memories, and links for a namespace.
  - Export preserves lifecycle/supersession fields needed by retrieval behavior.
  - Export order is deterministic for stable JSONL output.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner --features real-character-memory real_adapter"

### Task_5: Build LongMemEval-S Replay Windows
- type: impl
- owns:
  - `crates/cmem-eval-longmemeval/src/ingest.rs`
  - `crates/cmem-eval-longmemeval/src/types.rs`
- depends_on: [Task_1]
- description: |
  Add LongMemEval-S helpers that convert source-only records into chronological replay windows using official session IDs, turn IDs, timestamps, and question cutoffs.
- acceptance:
  - Windows are ordered by source session date/order.
  - Windows preserve official session IDs and turn provenance IDs.
  - Helpers do not expose QA/gold/evidence fields.
  - Unit tests cover ordering, cutoff handling, source ID preservation, and whitespace-preserving content handling.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-longmemeval"

### Task_6: Generate LongMemEval-S Snapshots
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/main.rs`
  - `crates/cmem-eval-runner/src/*.rs`
- depends_on: [Task_3, Task_4, Task_5]
- description: |
  Add the LongMemEval-S snapshot generation command. It replays each source-only question namespace chronologically, runs LLM remembering preparation for each source window, submits prepared graph-schema data through the public Character Memory remember API, exports one snapshot per question cutoff, and writes the LongMemEval-S snapshot JSONL plus manifest.
- acceptance:
  - Command supports LongMemEval-S source-only input and writes one snapshot per question.
  - Command writes a minimal manifest with source hash, artifact hash, replay mode, cutoff policy, workflow/model identifiers, and counts.
  - Command fails if any LongMemEval-S question lacks a generated snapshot.
  - Command does not write per-step audit logs or debug snapshots unless a future explicit debug flag is added.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"

### Task_7: Consume LongMemEval-S Snapshots During Eval
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `configs/longmemeval_s_*.toml`
  - `configs/*longmemeval_s*online*.toml`
- depends_on: [Task_3]
- description: |
  Update LongMemEval-S eval execution to select and inject the correct graph snapshot before retrieval when snapshot enrichment is configured.
- acceptance:
  - LongMemEval-S selects snapshot by question ID after raw source ingestion and before retrieval.
  - Missing LongMemEval-S snapshots fail fast with actionable errors.
  - Existing `ingest.enrichment_path` behavior remains available for legacy/offline enrichment configs.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"

### Task_8: LongMemEval-S Validation And Documentation
- type: test
- owns:
  - `README.md`
  - `configs/*longmemeval_s*online*.toml`
  - `datasets/enrichment_source/README.md`
  - `docs/coding-agent/plans/active/longmemeval-s-online-enrichment-snapshots-plan.md`
- depends_on: [Task_6, Task_7]
- description: |
  Add LongMemEval-S user-facing docs and validate a small fixture or limited local run that exercises LongMemEval-S snapshot generation and eval consumption without requiring full benchmark regeneration.
- acceptance:
  - README documents LongMemEval-S per-question online cutoff snapshots as the preferred fair enrichment artifact for the current benchmark.
  - Config examples cover LongMemEval-S snapshot generation and eval consumption.
  - A small validation path proves LongMemEval-S snapshot generation and snapshot consumption are wired together.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-longmemeval && cargo test -p cmem-eval-runner"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review LongMemEval-S snapshot generation/consumption for scope, artifact minimality, and future-context leakage prevention."

## Task Waves

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_5]
- Wave 3 (parallel): [Task_3, Task_4]
- Wave 4 (parallel): [Task_6, Task_7]
- Wave 5 (parallel): [Task_8]

## Rollback / Safety
- Keep existing `ingest.enrichment_path` behavior intact until LongMemEval-S snapshot consumption is validated.
- Gate LongMemEval-S snapshot generation behind an explicit CLI command.
- Do not enable cleanup against broad backend prefixes; use a dedicated LongMemEval-S replay namespace prefix.
- Do not commit generated benchmark artifacts unless repository policy explicitly allows them.
- If real graph export is incomplete, block snapshot generation rather than falling back to a parallel reducer.

## Progress Log

- 2026-05-04 Plan drafted.
  - Summary: Created standalone LongMemEval-S plan for online replay per-question cutoff snapshots.
  - Validation evidence: Not run; planning-only change.
  - Notes: Split out from the earlier combined dataset plan so LongMemEval-S can be built independently.
- 2026-05-04 Legacy artifact archived and first LongMemEval-S snapshot artifact generated.
  - Summary: Moved the legacy LongMemEval-S full-pass enrichment files to `datasets/enriched/archive/legacy_full_pass_2026-05-04/` and generated `datasets/enriched/longmemeval_s_online_snapshots.jsonl`, manifest, and report.
  - Validation evidence: Custom JSON validation passed: 500 snapshot rows, 0 forbidden-key findings, 0 duplicate typed IDs, 0 missing source references, 0 unresolved graph endpoints, and every derived-memory text exactly matches its cited source turn text.
  - Notes: The generated artifact uses deterministic source-turn replay semantics and does not persist per-step debug evidence. It does not use QA, answers, answer-session IDs, `has_answer`, or any gold/evaluation fields.
- 2026-05-04 LongMemEval-S snapshot consumption wired into eval runner.
  - Summary: Updated LongMemEval-S eval execution to select snapshots by `question_id` and inject the selected graph before retrieval. Updated `configs/longmemeval_s_retrieval.toml` to point at the new snapshot artifact and manifest.
  - Validation evidence: `cargo fmt --all --check`, `cargo test -p cmem-eval-longmemeval`, `cargo test -p cmem-eval-runner enrichment`, `cargo test -p cmem-eval-runner`, one-row LongMemEval-S mock snapshot smoke run, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the service-free synthetic mock smoke command passed.
  - Notes: Generated data remains local and ignored; it should not be committed unless explicitly requested.
- 2026-05-04 Source-only cutoff metadata corrected and snapshots regenerated.
  - Summary: Rebuilt `datasets/enrichment_source/longmemeval_s_source_only.json` with `question_date`, recursively removed nested forbidden keys such as `has_answer`, archived the invalid no-question-date snapshot artifact, and regenerated `longmemeval_s_online_snapshots.*` with question-date cutoffs.
  - Validation evidence: Custom validation passed: 500 source rows, 500 snapshot rows, 0 findings, 76 rows with future sessions excluded, 1,475 excluded future sessions, 1 row without visible sessions, and every derived-memory text exactly matches a cited visible source turn. One-row LongMemEval-S mock snapshot smoke run passed after regeneration.
  - Notes: The corrected JSONL escapes Unicode line separators so each snapshot remains one physical JSONL line while decoded content remains exact.
- 2026-07-05 Plan closed.
  - Summary: Closed the LongMemEval-S plan around the delivered artifact set and runner consumption path. Generated benchmark/source files remain ignored and local. The committed runner path loads per-question snapshots by `question_id` and fails fast when a configured snapshot is missing.
  - Validation evidence: `cargo fmt --all --check`, `cargo test -p cmem-eval-runner enrichment`, `cargo test -p cmem-eval-runner`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the service-free synthetic mock smoke command were rerun before closeout.
  - Notes: A reusable runner command that performs full real-graph replay/export through the Character Memory backend remains future work; this closeout does not claim that CLI generation path exists.

## Decision Log

- 2026-05-04 Decision: Use per-question cutoff snapshots for LongMemEval-S.
  - Trigger / new insight: LongMemEval-S eval is question-scoped and can require different source visibility per question.
  - Plan delta: LongMemEval-S plan targets one snapshot per `question_id`.
  - Tradeoffs considered: Final-only snapshots are simpler but risk future-context leakage.
  - User approval: yes, direction provided in discussion.
- 2026-05-04 Decision: Replay generation should use the public Character Memory API boundary.
  - Trigger / new insight: User clarified that the library is early and does not yet expose convenient workflow wrappers.
  - Plan delta: The replay command includes explicit LLM remembering preparation followed by public remember API calls.
  - Tradeoffs considered: A higher-level workflow wrapper would be cleaner if it existed, but adding one now would invent API shape beyond current library maturity.
  - User approval: yes, direction provided in discussion.
- 2026-05-04 Decision: Prefer exact source preservation with limited whitespace tolerance.
  - Trigger / new insight: User clarified byte-for-byte exactness is best, but semantic-preserving line break/space differences can be tolerated.
  - Plan delta: Source-only and replay helpers should preserve exact strings when practical and document any whitespace-only normalization.
  - Tradeoffs considered: Strict byte equality is easier to audit but may over-fail on harmless runtime normalization; broad normalization risks masking content changes.
  - User approval: yes, direction provided in discussion.

## Notes
- Follow-up:
  - Add a reusable snapshot-generation command if the project needs repeatable in-repo regeneration instead of the current local deterministic generation procedure.
- Risks:
  - The public Character Memory API may require explicit orchestration around LLM preparation, graph-state lookup, and remember calls because workflow convenience APIs do not yet exist.
  - Real graph export may reveal backend-specific fields unless the export layer is carefully scoped.
  - LongMemEval-S timestamp normalization must be aligned between source-only data, replay windows, and query cutoff selection.
- Edge cases:
  - Source sessions with equal or missing timestamps.
  - Question cutoffs that do not align cleanly with source session dates.
  - Superseded or suppressed memories that must remain exported because retrieval behavior depends on lifecycle state.
