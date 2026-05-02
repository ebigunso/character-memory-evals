# Plan: LoCoMo Dataset Ingestion Fidelity

- status: done
- generated: 2026-05-02
- last_updated: 2026-05-02
- work_type: code

## Goal
- Make LoCoMo ingestion honor dataset fields that affect retrieval, especially image captions, while keeping QA evidence scorer-only.

## Definition of Done
- LoCoMo caption/search metadata is parsed into dataset-owned types.
- `include_image_captions` changes indexed observation text only when enabled.
- Default v0.1 behavior remains text-only with captions disabled.
- QA evidence and answers are never copied into `EpisodeInput`, `ObservationInput`, or metadata.
- LoCoMo tests cover captions enabled/disabled and evidence isolation.

## Scope / Non-goals
- Scope:
  - LoCoMo loader/types/ingest fidelity.
  - Caption text formatting when enabled.
  - LoCoMo-specific tests and config docs.
- Non-goals:
  - Raw image retrieval or multimodal claims.
  - LoCoMo official export output; see `official-benchmark-export-formats-plan.md`.
  - Shared config validation except for consuming the existing `include_image_captions` field.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-locomo/src/types.rs`
  - `crates/cmem-eval-locomo/src/loader.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
  - `crates/cmem-eval-locomo/src/scoring.rs`
  - `configs/locomo_retrieval.toml`
  - `README.md`
- Existing patterns or references:
  - Dataset-specific logic stays in dataset crates.
  - LoCoMo images are not released; captions/search metadata may be treated as text only.
  - Gold evidence is scorer-only.
  - Official LoCoMo `conversation` is an object containing `speaker_a`, `speaker_b`, `session_N_date_time`, and `session_N` arrays.
  - Official LoCoMo turns contain `speaker`, `dia_id`, and `text`; image turns additionally contain `img_url`, `blip_caption`, and `query`.
  - Official LoCoMo `qa` entries contain `question`, `answer`, numeric `category`, and optional `evidence`; they do not usually include `question_id`.
  - Official LoCoMo generated observations are keyed as `session_N_observation` with speaker names mapping to `[observation_text, dia_id]` pairs.
  - Official LoCoMo session summaries are keyed as `session_N_summary`; event summaries are keyed as `events_session_N`.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `https://github.com/snap-research/locomo`
  - `https://raw.githubusercontent.com/snap-research/locomo/main/data/locomo10.json`

## Open Questions
- None blocking.

## Assumptions
- A1: Caption fields may appear as `blip_caption`, `caption`, or nested image metadata.
- A2: Search query fields are metadata unless explicitly chosen as indexed caption text.
- A3: Caption text should be appended with a stable label so downstream context remains inspectable.
- A4: When official QA rows lack `question_id`, the loader should assign stable internal IDs from `(sample_id, qa_index)` while preserving the original QA index for export.

## Tasks

### Task_1: Extend LoCoMo Types For Captions
- type: impl
- owns:
  - `crates/cmem-eval-locomo/src/types.rs`
  - `crates/cmem-eval-locomo/src/loader.rs`
- depends_on: []
- description: |
  Parse caption and image-adjacent metadata from LoCoMo turns without changing scorer semantics.
- acceptance:
  - `LoCoMoTurn` can hold optional caption text.
  - Loader recognizes released `img_url`, `blip_caption`, and `query` fields.
  - Loader parses keyed `conversation.session_N` arrays and associated `session_N_date_time` values.
  - Loader preserves `speaker_a` and `speaker_b` as sample/session participants where applicable.
  - Loader assigns stable IDs to QA rows that lack `question_id`.
  - Unknown image metadata remains available in raw JSON for debugging.
  - Existing fixtures and tests continue to pass.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-locomo`

### Task_2: Make Caption Flag Affect Indexed Text
- type: impl
- owns:
  - `crates/cmem-eval-locomo/src/ingest.rs`
  - `configs/locomo_retrieval.toml`
- depends_on: [Task_1]
- description: |
  Use `include_image_captions` to include caption text in `ObservationInput.text` only when enabled.
- acceptance:
  - Caption text is absent from observation text when the flag is false.
  - Caption text is present with a stable label when the flag is true.
  - Observation metadata does not contain QA evidence or answer fields.
  - Default config remains `include_image_captions = false`.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-locomo`

### Task_3: Add LoCoMo Fidelity Tests
- type: test
- owns:
  - `crates/cmem-eval-locomo/src/loader.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
- depends_on: [Task_2]
- description: |
  Add targeted tests for caption parsing/indexing and evidence isolation.
- acceptance:
  - Fixture with caption verifies loader parsing.
  - Ingest test verifies captions on/off.
  - Evidence isolation test covers metadata and text fields.
  - Tests use small inline fixtures and no external dataset.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-locomo`

### Task_4: LoCoMo Review
- type: review
- owns: []
- depends_on: [Task_3]
- description: |
  Review LoCoMo changes against the v0.1 text-only boundary and prior caption findings.
- acceptance:
  - Reviewer confirms the caption flag is no longer a no-op.
  - Reviewer confirms no raw multimodal support is claimed.
  - Reviewer confirms evidence remains scorer-only.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-locomo`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs LoCoMo ingestion fidelity criteria."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]

## Rollback / Safety
- Keep `include_image_captions = false` as the default.
- Do not store QA evidence or answers in adapter inputs or metadata.

## Progress Log
- 2026-05-02 draft created.
  - Summary: Split LoCoMo caption/indexing fidelity into a dataset-owned plan.
  - Validation evidence: Review findings 4 and 9.
  - Notes: No UI scope.
- 2026-05-02 execution started.
  - Summary: Started Plan 3 after completing and committing Plan 2; dispatched focused LoCoMo researcher.
  - Validation evidence: Researcher `019de5f0-16f5-7d73-a229-e26190c35e02`.
  - Notes: No UI scope.
- 2026-05-02 implementation through Task_3 completed.
  - Summary: Extended LoCoMo types for participants, image URLs, captions, search queries, and QA indexes; parsed official keyed conversation shape; made caption text affect indexed observation text only when enabled; added official-shape and caption on/off tests.
  - Validation evidence: `cargo test -p cmem-eval-locomo`; `cargo clippy -p cmem-eval-locomo --all-targets -- -D warnings`; `cargo test --workspace`; `cargo fmt -p cmem-eval-locomo --check`; synthetic mock smoke command.
  - Notes: No UI scope.
- 2026-05-02 review completed.
  - Summary: Harness reviewer approved Plan 3 changes with no findings.
  - Validation evidence: Reviewer `019de5ff-9090-7b12-89c2-ce4497dc99b0` APPROVED.
  - Notes: Residual risk around additional caption field variants beyond released fixture shape.

## Decision Log
- 2026-05-02 Decision:
  - Trigger / new insight: The caption flag reached LoCoMo ingestion but did not alter indexed text.
  - Plan delta (what changed): Created a LoCoMo-only ingestion plan.
  - Tradeoffs considered: Captions are text hints only, not multimodal support.
  - User approval: pending.

## Notes
- Risks:
  - Released LoCoMo caption field names may vary.
  - Appending captions can change retrieval metrics, so the default remains disabled.
- Edge cases:
  - Empty caption strings should not add labels to observation text.
  - Numeric QA `category` values should be preserved even if internal `question_type` remains a string for result compatibility.
  - Evidence IDs compare against `dia_id` values such as `D1:3`, not generated turn indexes.
