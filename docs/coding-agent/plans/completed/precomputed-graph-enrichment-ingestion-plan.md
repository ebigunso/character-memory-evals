# Plan: Precomputed Graph Enrichment Ingestion

- status: completed
- generated: 2026-05-03
- last_updated: 2026-09-02
- work_type: code

## Goal
- Let benchmark runs mechanically inject precomputed graph enrichment objects so evals exercise Character Memory's entity/thread/derived-memory/link graph structure without runtime LLM API integration.

## Definition of Done
- Config can point to a precomputed enrichment JSONL artifact and select LoCoMo benchmark-provided derived memories.
- Enrichment records can create entities, memory threads, derived memories, and links with stable external IDs.
- Derived memories require source episode or observation provenance unless explicitly unsupported by object type.
- Gold labels, answers, questions, and evidence IDs are rejected from enrichment metadata/text fields where possible.
- The real adapter stores enriched objects through the public Character Memory draft API.
- Raw-only runs remain supported and service-free validation remains mock-backed.

## Scope / Non-goals
- Scope:
  - Enrichment schema, loader, validation, and tests.
  - Core memory adapter input contract for graph objects.
  - Real adapter mapping to Character Memory `EntityDraft`, `MemoryThreadDraft`, `DerivedMemoryDraft`, and `MemoryLinkDraft`.
  - LoCoMo generated observations/session summaries as optional derived memory inputs.
  - Runner wiring and config/docs.
- Non-goals:
  - Runtime LLM API calls from Rust.
  - Generating the enrichment artifact in this repo.
  - Storing benchmark gold labels, answers, or evidence IDs as memory.
  - Library-side extraction APIs.

## Context
- User decision: precompute enrichment data from a separate Codex/LLM session, then let this harness ingest it mechanically.
- Existing raw path maps sessions to `EpisodeInput` and turns/dialogs to `ObservationInput`.
- Character Memory public API accepts typed drafts for entities, threads, derived memories, links, episodes, and observations.
- LoCoMo release includes generated observations/session summaries that can be used as derived memories without runtime generation.

## Open Questions
- None blocking. Default behavior should stay raw-only unless enrichment config is enabled.

## Assumptions
- A1: Precomputed enrichment artifacts are JSONL and are generated from haystack/source conversations only.
- A2: Enrichment external IDs are stable within a dataset namespace.
- A3: Links may reference enriched objects and raw episode/observation external IDs.
- A4: Missing LongMemEval enrichment is acceptable; users can supply an artifact later.
- A5: LoCoMo generated observations/session summaries are benchmark-provided source data, not gold QA labels.

## Tasks

### Task_1: Add Enrichment Contract
- type: impl
- owns:
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-core/src/lib.rs`
- depends_on: []
- description: |
  Add typed graph enrichment inputs and config knobs without changing raw ingestion behavior.
- acceptance:
  - Config supports optional `ingest.enrichment_path`.
  - Config supports `index_session_summaries` and `index_generated_observations` as real supported LoCoMo-derived-memory flags.
  - Core input types model entity, thread, derived memory, and link graph objects with external IDs.
  - Derived memory input requires at least one source episode/observation ID in validation helpers.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-core`

### Task_2: Add Enrichment Loader
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/enrichment.rs`
  - `crates/cmem-eval-runner/src/main.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_1]
- description: |
  Load precomputed JSONL graph enrichment, validate provenance and gold-label guardrails, and group objects by namespace.
- acceptance:
  - JSONL records can provide namespace-scoped entities, threads, derived memories, and links.
  - Loader rejects derived memories without source episode or observation external IDs.
  - Loader rejects suspicious gold-label fields such as `answer`, `evidence`, `has_answer`, and `answer_session_ids`.
  - Loader exposes deterministic namespace grouping for runner ingestion.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner enrichment`

### Task_3: Map Enrichment To Character Memory
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
- depends_on: [Task_1]
- description: |
  Extend the adapter trait and real adapter to remember enrichment graph objects through public Character Memory drafts.
- acceptance:
  - Entity/thread/derived-memory/link inputs are converted to public draft types.
  - External IDs map to deterministic UUIDs and reverse maps where retrieval/export needs them.
  - Derived-memory provenance uses raw episode/observation external IDs resolved to remembered UUIDs.
  - Unknown provenance/link references fail clearly.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_4: Wire Dataset Ingestion
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-locomo/src/types.rs`
  - `crates/cmem-eval-locomo/src/loader.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
  - `crates/cmem-eval-longmemeval/src/ingest.rs`
- depends_on: [Task_2, Task_3]
- description: |
  Inject configured enrichment after raw episodes/observations are remembered; add LoCoMo benchmark-provided generated observation/session summary ingestion.
- acceptance:
  - LongMemEval can ingest enrichment from `ingest.enrichment_path`.
  - LoCoMo can ingest enrichment from `ingest.enrichment_path`.
  - LoCoMo can turn benchmark-provided generated observations/session summaries into derived memories with provenance.
  - Raw-only configs still run unchanged.
  - Gold labels remain scorer-only.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-locomo && cargo test -p cmem-eval-runner`

### Task_5: Docs And Review
- type: docs
- owns:
  - `README.md`
  - `configs/*.toml`
  - `docs/coding-agent/plans/active/precomputed-graph-enrichment-ingestion-plan.md`
- depends_on: [Task_4]
- description: |
  Document enrichment artifact generation boundaries, schema, config, and raw-vs-enriched run modes, then review.
- acceptance:
  - README documents enrichment JSONL schema and no-gold-label rule.
  - Config examples show enrichment disabled by default and LoCoMo benchmark-provided derived memory options.
  - Review pass checks implementation against plan acceptance.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo fmt --all --check`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo clippy --workspace --all-targets -- -D warnings`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test --workspace`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs graph enrichment plan and gold-label isolation."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4]
- Wave 4 (parallel): [Task_5]

## Rollback / Safety
- Keep enrichment disabled by default.
- Reject generated artifacts that contain obvious gold-label keys.
- Do not add runtime LLM calls to the Rust harness.

## Progress Log
- 2026-05-03 implementation started.
  - Summary: Created plan for mechanical graph enrichment ingestion from precomputed artifacts plus LoCoMo benchmark-provided derived memories.
  - Validation evidence: pending.
  - Notes: Researcher dispatched before implementation exploration.
- 2026-05-03 implementation completed.
  - Summary: Added enrichment input contracts, JSONL loading/validation, live Character Memory draft mapping, LoCoMo benchmark-derived memory ingestion, config updates, and README schema docs.
  - Validation evidence:
    - `cargo test -p cmem-eval-core`
    - `cargo test -p cmem-eval-runner enrichment`
    - `cargo test -p cmem-eval-locomo`
    - `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`
    - `cargo test -p cmem-eval-runner --features real-character-memory`
    - `cargo clippy --workspace --all-targets --features real-character-memory -- -D warnings`
    - `cargo test --workspace`
    - `cargo fmt --all --check`
    - `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`
  - Review evidence: Local diff review found no open findings after validation. A harness reviewer was started but did not complete within two ten-minute waits and was closed; no reviewer findings were returned.

## Decision Log
- 2026-05-03 Decision:
  - Trigger / new insight: User wants graph/derived-memory retrieval evaluated without wiring runtime LLM APIs into Rust.
  - Plan delta (what changed): Add precomputed enrichment ingestion and LoCoMo source-derived memory support.
  - Tradeoffs considered: Precomputed artifacts reduce runtime complexity but require strict provenance and gold-label guardrails.
  - User approval: yes.

- 2026-09-02 Plan closeout
  - Summary: Retired this already-completed plan from the active queue during the harness right-sizing audit; its implementation record remains preserved here.
  - Validation evidence: The plan already records its completed validation above; this closeout changes documentation placement only.
  - Notes: Moved to `plans/completed/`; no implementation or evidence artifact changed.
