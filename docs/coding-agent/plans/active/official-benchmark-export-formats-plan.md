# Plan: Official Benchmark Export Formats

- status: draft
- generated: 2026-05-02
- last_updated: 2026-05-02
- work_type: mixed

## Goal
- Add official-compatible LongMemEval and LoCoMo export paths while preserving the internal JSONL scorer contract.

## Definition of Done
- LongMemEval export emits retrieval logs with `retrieval_results.ranked_items`.
- LongMemEval QA export emits JSONL rows with `question_id` and `hypothesis` when predictions exist.
- LoCoMo export preserves sample IDs, QA metadata, prediction/context fields, and retrieved context IDs.
- Export commands operate as post-processing over existing eval JSONL where possible.
- README documents export commands, expected inputs, and limitations.

## Scope / Non-goals
- Scope:
  - Runner export subcommands.
  - Export helper module and tests.
  - Dataset-specific export shape helpers where needed.
  - README/scripts documentation.
- Non-goals:
  - Running official benchmark scripts.
  - LLM answer generation or judge integration.
  - Changing internal scorer metrics.
  - Downloading datasets.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-longmemeval/src/*`
  - `crates/cmem-eval-locomo/src/*`
  - `README.md`
  - `scripts/README.md`
- Existing patterns or references:
  - Internal run output is `PerQuestionResult` JSONL.
  - Official LongMemEval retrieval scripts expect `retrieval_results.ranked_items`.
  - Official LongMemEval QA evaluator expects JSONL containing `question_id` and `hypothesis`.
  - Official LongMemEval cleaned files contain `question_id`, `question_type`, `question`, `answer`, `question_date`, `haystack_session_ids`, `haystack_dates`, `haystack_sessions`, and `answer_session_ids`.
  - LongMemEval `haystack_sessions` is a list of session turn lists; turns contain `role`, `content`, and optional `has_answer`.
  - Official LoCoMo evaluation is sample/QA-entry oriented; QA rows usually contain no `question_id`, so exports must preserve `sample_id` and QA index/category.
  - Official LoCoMo evidence IDs are dialog IDs such as `D1:3`.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `https://github.com/xiaowu0162/LongMemEval`
  - `https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned`
  - `https://github.com/snap-research/locomo`
  - `https://raw.githubusercontent.com/snap-research/locomo/main/data/locomo10.json`

## Open Questions
- None.

## Assumptions
- A1: Retrieval export can be generated from internal JSONL without rerunning benchmarks.
- A2: QA export should require predictions rather than invent empty hypotheses for official scoring.
- A3: Export fixtures can be small synthetic rows that pin expected shape without external datasets.
- A4: Export commands should be grouped under `export-official` with dataset subcommands, so official-format artifacts are clearly separate from internal result JSONL.
- A5: QA export should require an explicit prediction input file or explicit prediction field mapping; it must fail rather than emit empty `hypothesis` values.
- A6: Export outputs are analysis artifacts and should be written under `runs/` or `reports/` without being affected by backend cleanup.

## Tasks

### Task_1: Design Export CLI Shape
- type: design
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `README.md`
  - `docs/coding-agent/plans/active/official-benchmark-export-formats-plan.md`
- depends_on: []
- description: |
  Decide the export command structure and required inputs for retrieval and QA exports.
- acceptance:
  - Command names and arguments are documented as `export-official longmemeval ...` and `export-official locomo ...`.
  - Retrieval export works from internal result JSONL.
  - QA export requires an explicit prediction input file or prediction field mapping and fails if hypotheses are missing.
  - Export errors are actionable when required fields are missing.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Export CLI design recorded before implementation dispatch."

### Task_2: Implement LongMemEval Export
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `crates/cmem-eval-core/src/results.rs`
- depends_on: [Task_1]
- description: |
  Add LongMemEval official-compatible retrieval and QA export helpers.
- acceptance:
  - Retrieval export writes rows containing `question_id` and `retrieval_results.ranked_items`.
  - Ranked items preserve external IDs, ranks, scores, kind, and text when available.
  - QA export writes `question_id` and `hypothesis` from an explicit prediction source.
  - Tests pin representative output JSON.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner official_exports`

### Task_3: Implement LoCoMo Export
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `crates/cmem-eval-locomo/src/types.rs`
- depends_on: [Task_1]
- description: |
  Add LoCoMo-compatible export helpers that preserve sample/QA identity and retrieved context IDs.
- acceptance:
  - Export includes `sample_id` and stable QA index for every row.
  - Export preserves numeric/string `category`, question text, answer when available, prediction fields, and retrieved dialog/session IDs.
  - Missing `sample_id` or QA index fails clearly or requires a dataset join input.
  - Export does not assume official LoCoMo rows have `question_id`.
  - Tests pin representative output JSON.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner official_exports && cargo test -p cmem-eval-locomo`

### Task_4: Export Documentation
- type: docs
- owns:
  - `README.md`
  - `scripts/README.md`
- depends_on: [Task_2, Task_3]
- description: |
  Document how to produce internal JSONL first, then export official-compatible artifacts.
- acceptance:
  - README includes LongMemEval retrieval export example.
  - README includes LongMemEval QA export policy and example.
  - README includes LoCoMo export example and known limitations.
  - Docs do not claim answer generation is implemented unless it is.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Docs review against official export behavior."

### Task_5: Export Review
- type: review
- owns: []
- depends_on: [Task_4]
- description: |
  Review export behavior against official-script compatibility requirements.
- acceptance:
  - Reviewer confirms internal JSONL remains stable.
  - Reviewer confirms LongMemEval export shape contains required official fields.
  - Reviewer confirms LoCoMo export preserves sample/QA context needed downstream.
  - Reviewer confirms no LLM/judge dependency was added to default benchmark path.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test -p cmem-eval-runner official_exports`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test --workspace`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs official export acceptance criteria."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4]
- Wave 4 (parallel): [Task_5]

## Rollback / Safety
- Keep exports as post-processing commands so benchmark run behavior remains unchanged.
- Do not add LLM generation or judge calls to default commands.
- Preserve internal JSONL schema compatibility unless explicitly versioned.

## Progress Log
- 2026-05-02 draft created.
  - Summary: Split official-compatible exports into a separate mixed code/docs plan.
  - Validation evidence: Review finding 7 and active plan export acceptance criteria.
  - Notes: No UI scope.

## Decision Log
- 2026-05-02 Decision:
  - Trigger / new insight: Official export compatibility is a downstream artifact concern, not the same as retrieval scoring.
  - Plan delta (what changed): Created a dedicated export plan.
  - Tradeoffs considered: Post-processing exports minimize risk to internal run JSONL.
  - User approval: pending.
- 2026-05-02 Decision:
  - Trigger / new insight: User needs to preserve and analyze logs/results from separate LongMemEval-S and LoCoMo runs.
  - Plan delta (what changed): Resolved export CLI to grouped `export-official` dataset subcommands; QA exports require explicit predictions and never fabricate hypotheses.
  - Tradeoffs considered: A grouped command keeps official artifacts discoverable while separate dataset subcommands keep schemas independent.
  - User approval: yes.

## Notes
- Risks:
  - Official script formats can drift; tests should pin representative expected shapes.
  - LoCoMo sample ID and QA index may need dataset join input if internal rows only store generated QA IDs.
- Edge cases:
  - QA export before fixed-reader predictions should fail clearly rather than emit misleading blank hypotheses.
  - Backend cleanup must not remove official export artifacts or internal JSONL used for later analysis.
  - LongMemEval abstention IDs ending in `_abs` should remain exportable even when retrieval metrics are skipped.
