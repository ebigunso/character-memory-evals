# Plan: LoCoMo Online Enrichment Snapshots

- status: completed
- generated: 2026-05-04
- last_updated: 2026-09-02
- work_type: mixed

## Goal
- Build the LoCoMo enrichment dataset independently as final per-sample graph snapshots exported from chronological replay through the real Character Memory graph DB.
- Keep normal LoCoMo eval runs deterministic and cheap by loading persisted final snapshots instead of replaying memory generation every run.

## Definition of Done
- The runner can generate `datasets/enriched/locomo_online_snapshots.jsonl` from `datasets/enrichment_source/locomo_source_only.json`.
- The runner writes `datasets/enriched/locomo_online_snapshots_manifest.json` with source/artifact hashes, replay settings, workflow/model identifiers, and object counts.
- LoCoMo eval can consume the final sample snapshot by `sample_id` before answering that sample's QA.
- Snapshot validation rejects forbidden QA/gold/evidence keys, missing source references, unresolved graph references, duplicate typed IDs, and stale source manifests.

## Scope / Non-goals
- Scope:
  - Add any shared snapshot types/config needed for LoCoMo.
  - Add LoCoMo replay-window helpers.
  - Add graph export support needed to write portable LoCoMo snapshot rows.
  - Add LoCoMo snapshot generation and eval consumption.
  - Add LoCoMo docs/config examples.
- Non-goals:
  - Build LongMemEval-S artifacts.
  - Persist per-step audit traces, prompt inputs, operation logs, or debug evidence by default.
  - Replace the real graph DB with a parallel operation reducer.
  - Regenerate snapshots during normal eval runs.
  - Store or expose benchmark QA/gold/evidence fields to the enrichment generator.

## Context
- Local verification of `datasets/locomo10.json` found 10 samples and 1,986 QA rows.
- QA is top-level, not interleaved into conversation sessions.
- QA rows have no time/date/session fields.
- 58 QA rows cite evidence from the final recorded session for their sample.
- Therefore the current LoCoMo benchmark should use one final graph snapshot per sample.

## Assumptions
- A1: LoCoMo uses one final snapshot per sample for the current benchmark data.
- A2: The current highest-level workflow boundary is the exposed public Character Memory API surface.
- A3: Replay generation mirrors intended usage: an LLM remembering process prepares graph-schema data for each source conversation thread/window while consulting relevant existing graph state, then submits the prepared data through the public remember API.
- A4: Source-only generation and replay should prefer byte-exact source strings; whitespace-only normalization is tolerated only when semantic-preserving and consistent with documented runtime ingestion behavior.

## Artifacts
- Primary artifact:
  - `datasets/enriched/locomo_online_snapshots.jsonl`
- Manifest artifact:
  - `datasets/enriched/locomo_online_snapshots_manifest.json`
- Snapshot row shape:
  - `snapshot_id`: stable snapshot identifier, for example `locomo:conv-26@final`.
  - `namespace`: runtime namespace, for example `locomo:conv-26`.
  - `dataset_item_id`: LoCoMo `sample_id`.
  - `cutoff`: `{ "type": "final_session", "value": "<last_session_id>" }`.
  - `graph`: portable `GraphEnrichmentInput` snapshot exported from the real graph DB.
- Manifest shape:
  - artifact path and hash.
  - dataset kind.
  - source-only path and hash.
  - replay mode and cutoff policy.
  - workflow/model version identifiers.
  - namespace/snapshot/object counts.

## Tasks

### Task_1: Define LoCoMo Snapshot Contract
- type: design
- owns:
  - `docs/coding-agent/plans/completed/locomo-online-enrichment-snapshots-plan.md`
  - `README.md`
  - `datasets/enrichment_source/README.md`
- depends_on: []
- description: |
  Document the LoCoMo final-snapshot artifact contract, manifest contract, lookup key, and validation invariants.
- acceptance:
  - LoCoMo snapshot schema and manifest schema are documented.
  - `sample_id` lookup and final-session cutoff semantics are documented.
  - Non-persisted debug/evidence artifacts are explicitly out of default scope.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Review LoCoMo artifact minimality and final-snapshot cutoff semantics."

### Task_2: Add Snapshot Types And Config Needed By LoCoMo
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

### Task_3: Add LoCoMo Snapshot Loading And Validation
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/enrichment.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_2]
- description: |
  Implement snapshot JSONL loading, manifest loading, LoCoMo sample lookup, and validation for forbidden keys, duplicate graph IDs, source references, graph references, and source hash compatibility.
- acceptance:
  - Snapshot loader supports LoCoMo lookup by `sample_id`.
  - Validation rejects missing selected LoCoMo snapshots before or at sample execution.
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

### Task_5: Build LoCoMo Replay Windows
- type: impl
- owns:
  - `crates/cmem-eval-locomo/src/ingest.rs`
  - `crates/cmem-eval-locomo/src/types.rs`
- depends_on: [Task_1]
- description: |
  Add LoCoMo helpers that convert source-only records into chronological replay windows using official session numbers/dates and `dia_id` observation IDs.
- acceptance:
  - LoCoMo windows are ordered by session number/date.
  - Source observations preserve `dia_id` values as graph provenance IDs.
  - Helpers do not expose QA/gold/evidence fields.
  - Unit tests cover ordering and source ID preservation.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo"

### Task_6: Generate LoCoMo Snapshots
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/main.rs`
  - `crates/cmem-eval-runner/src/*.rs`
- depends_on: [Task_3, Task_4, Task_5]
- description: |
  Add the LoCoMo snapshot generation command. It replays each source-only sample chronologically through all conversation sessions, runs LLM remembering preparation for each source window, submits prepared graph-schema data through the public Character Memory remember API, exports one final snapshot per sample, and writes the LoCoMo snapshot JSONL plus manifest.
- acceptance:
  - Command supports LoCoMo source-only input and writes one final snapshot per sample.
  - Command writes a minimal manifest with source hash, artifact hash, replay mode, cutoff policy, workflow/model identifiers, and counts.
  - Command fails if any LoCoMo sample lacks a final generated snapshot.
  - Command does not write per-step audit logs or debug snapshots unless a future explicit debug flag is added.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"

### Task_7: Consume LoCoMo Snapshots During Eval
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `configs/locomo_*.toml`
  - `configs/*locomo*online*.toml`
- depends_on: [Task_3]
- description: |
  Update LoCoMo eval execution to select and inject the final sample graph snapshot before QA retrieval when snapshot enrichment is configured.
- acceptance:
  - LoCoMo selects the final sample snapshot by sample ID after raw source ingestion and before QA retrieval.
  - Missing LoCoMo sample snapshots fail fast with actionable errors.
  - Existing `ingest.enrichment_path` behavior remains available for legacy/offline enrichment configs.
  - The implementation can later be extended to per-QA LoCoMo snapshots without changing the artifact family.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-runner"

### Task_8: LoCoMo Validation And Documentation
- type: test
- owns:
  - `README.md`
  - `configs/*locomo*online*.toml`
  - `datasets/enrichment_source/README.md`
  - `docs/coding-agent/plans/completed/locomo-online-enrichment-snapshots-plan.md`
- depends_on: [Task_6, Task_7]
- description: |
  Add LoCoMo user-facing docs and validate a small fixture or limited local run that exercises LoCoMo snapshot generation and eval consumption without requiring full benchmark regeneration.
- acceptance:
  - README documents LoCoMo final per-sample online snapshots as the preferred fair enrichment artifact for the current benchmark.
  - Config examples cover LoCoMo snapshot generation and eval consumption.
  - A small validation path proves LoCoMo snapshot generation and snapshot consumption are wired together.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo test -p cmem-eval-locomo && cargo test -p cmem-eval-runner"
  - kind: review
    required: true
    owner: reviewer
    detail: "Review LoCoMo snapshot generation/consumption for scope, artifact minimality, and final-snapshot cutoff semantics."

## Task Waves

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_5]
- Wave 3 (parallel): [Task_3, Task_4]
- Wave 4 (parallel): [Task_6, Task_7]
- Wave 5 (parallel): [Task_8]

## Rollback / Safety
- Keep existing `ingest.enrichment_path` behavior intact until LoCoMo snapshot consumption is validated.
- Gate LoCoMo snapshot generation behind an explicit CLI command.
- Do not enable cleanup against broad backend prefixes; use a dedicated LoCoMo replay namespace prefix.
- Do not commit generated benchmark artifacts unless repository policy explicitly allows them.
- If real graph export is incomplete, block snapshot generation rather than falling back to a parallel reducer.

## Progress Log

- 2026-05-04 Plan drafted.
  - Summary: Created standalone LoCoMo plan for online replay final snapshots.
  - Validation evidence: Not run; planning-only change.
  - Notes: Split out from the earlier combined dataset plan so LoCoMo can be built independently.
- 2026-05-04 Legacy artifact archived and first LoCoMo snapshot artifact generated.
  - Summary: Moved the legacy LoCoMo full-pass enrichment files to `datasets/enriched/archive/legacy_full_pass_2026-05-04/` and generated `datasets/enriched/locomo_online_snapshots.jsonl`, manifest, and report.
  - Validation evidence: Custom JSON validation passed: 10 snapshot rows, 0 forbidden-key findings, 0 duplicate typed IDs, 0 missing source episode references, and 0 unresolved graph endpoints.
  - Notes: The generated artifact uses deterministic source-summary replay semantics and does not persist per-step debug evidence. It does not use QA, answer, evidence, observation, or event-summary fields.
- 2026-05-04 LoCoMo snapshot consumption wired into eval runner.
  - Summary: Added snapshot structs/config, JSONL snapshot loading/validation, and LoCoMo sample snapshot injection before QA retrieval. Updated `configs/locomo_retrieval.toml` to point at the new snapshot artifact and manifest.
  - Validation evidence: `cargo fmt --all --check`, `cargo test -p cmem-eval-core`, `cargo test -p cmem-eval-locomo`, `cargo test -p cmem-eval-runner enrichment`, `cargo test -p cmem-eval-runner`, `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` passed.
  - Notes: The first sandboxed Rust commands failed with local toolchain execute permission errors, then passed when rerun with approved escalation.

## Decision Log

- 2026-05-04 Decision: Use final per-sample LoCoMo snapshots for the current benchmark.
  - Trigger / new insight: Local data verification showed QA is top-level and not timestamped, with some evidence in final sessions.
  - Plan delta: LoCoMo plan targets one final snapshot per sample.
  - Tradeoffs considered: Per-QA LoCoMo snapshots are unnecessary for the current data but can be added later.
  - User approval: yes, direction provided in discussion.
- 2026-05-04 Decision: Replay generation should use the public Character Memory API boundary.
  - Trigger / new insight: User clarified that the library is early and does not yet expose convenient workflow wrappers.
  - Plan delta: The replay command includes explicit LLM remembering preparation followed by public remember API calls.
  - Tradeoffs considered: A higher-level workflow wrapper would be cleaner if it existed, but adding one now would invent API shape beyond current library maturity.
  - User approval: yes, direction provided in discussion.

## Notes
- Risks:
  - The public Character Memory API may require explicit orchestration around LLM preparation, graph-state lookup, and remember calls because workflow convenience APIs do not yet exist.
  - Real graph export may reveal backend-specific fields unless the export layer is carefully scoped.
  - Some LoCoMo evidence cells contain irregular formatting, but this plan does not expose evidence to generation.
- Edge cases:
  - Source sessions with missing timestamps.
  - Superseded or suppressed memories that must remain exported because retrieval behavior depends on lifecycle state.

## Closeout

- 2026-09-02 Plan closeout
  - Summary: Closed the stale active record after its logged implementation completed and archived it during the harness right-sizing audit.
  - Validation evidence: The completed progress log above records the service-free and workspace validation used at delivery.
  - Notes: Moved to `plans/completed/`; no generated dataset or evidence artifact changed.
