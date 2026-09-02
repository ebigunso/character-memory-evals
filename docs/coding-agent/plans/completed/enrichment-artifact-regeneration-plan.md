# Plan: Enrichment Artifact Regeneration

- status: completed
- generated: 2026-07-12
- last_updated: 2026-07-12
- work_type: mixed

## Goal
- Recreate the lost LongMemEval-S and LoCoMo source-only enrichment inputs, deterministic graph snapshots, manifests, and validation reports without exposing benchmark QA, answers, evidence, gold labels, evaluator outputs, or future LongMemEval sessions to the snapshot-generation stage.

## Definition of Done
- `datasets/enrichment_source/longmemeval_s_source_only.json` contains exactly 500 allowlisted rows with every nested `has_answer` and all QA/gold/evidence fields absent.
- `datasets/enrichment_source/locomo_source_only.json` contains exactly 10 allowlisted samples with raw conversation chronology and turns only; the complete QA subtree and benchmark-provided summaries/observations/events/image metadata are absent.
- `datasets/enriched/longmemeval_s_online_snapshots.jsonl` contains exactly 500 deterministic question-date cutoff snapshots, with 76 rows excluding 1,475 future sessions and one row having no visible sessions, matching the previously validated benchmark revision.
- `datasets/enriched/locomo_online_snapshots.jsonl` contains exactly 10 deterministic final-session snapshots.
- Both manifests contain source/artifact SHA-256 hashes, generation/cutoff identifiers, and object/count totals; both reports record zero leakage/reference findings.
- Independent review confirms that snapshot builders consumed only source-only inputs and that every derived-memory text and provenance reference resolves to allowed visible source.

## Scope / Non-goals
- Scope:
  - Add local reproducibility helpers under `scripts/enrichment/` for allowlist sanitization, deterministic snapshot construction, and artifact validation.
  - Recreate ignored local source-only, snapshot, manifest, and report artifacts.
  - Preserve exact source turn text, including decoded Unicode separators, while keeping each snapshot on one physical JSONL line.
  - Create a checksum-verified backup copy outside the repository workspace after validation.
- Non-goals:
  - Use benchmark questions, answers, evidence, categories, evaluator outputs, or gold labels as generation inputs.
  - Use raw benchmark files directly in snapshot generation.
  - Recreate legacy full-pass enrichment artifacts.
  - Change runtime retrieval/scoring behavior or commit generated benchmark assets.

## Context (workspace)
- Related files/areas:
  - `datasets/longmemeval_s_cleaned.json`
  - `datasets/locomo10.json`
  - `datasets/enrichment_source/`
  - `datasets/enriched/`
  - `configs/longmemeval_s_retrieval.toml`
  - `configs/locomo_retrieval.toml`
  - `crates/cmem-eval-runner/src/enrichment.rs`
  - `crates/cmem-eval-longmemeval/src/ingest.rs`
  - `crates/cmem-eval-locomo/src/ingest.rs`
- Existing patterns or references:
  - `docs/coding-agent/plans/completed/longmemeval-s-online-enrichment-snapshots-plan.md`
  - `docs/coding-agent/plans/completed/locomo-online-enrichment-snapshots-plan.md`
  - `docs/coding-agent/plans/completed/eval-harness-architecture-revision-plan.md`
- Repo reference docs consulted:
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`

## Open Questions (max 3)
- None blocking. The later snapshot plans and progress logs are authoritative over the older legacy enrichment plan where they conflict.

## Assumptions
- A1: The local raw dataset files are the same benchmark revisions previously validated: 500 LongMemEval-S rows and 10 LoCoMo samples.
- A2: Deterministic source replay is sufficient to recreate the prior artifact semantics: derived-memory text is copied exactly from cited visible turns, not invented or paraphrased.
- A3: LongMemEval question-date cutoff parsing must reproduce the prior exclusion totals exactly; any mismatch blocks artifact acceptance.
- A4: Generated assets remain gitignored and local, so a checksum-verified backup is required before closeout.

## Quality Routing Note
- Routing level: L2
- In-scope docs: repository rules, testing/validation baseline, security/data-boundary checklist, Python helper quality gate, independent review.
- Out-of-scope docs: frontend/UI, browser/E2E, deployment, database migration, and network security because the task is local deterministic data transformation with no UI or service mutation.
- Top risks: data-integrity, trust-boundary leakage, temporal cutoff correctness, artifact loss.
- Required checks: sanitizer allowlist audit, recursive forbidden-key scan, exact row/cutoff counts, provenance/text/reference resolution, hash verification, consumer loader tests, repository fmt/clippy/test/smoke gates, Reviewer approval.

## Tasks

### Task_1: Implement Source-Only Sanitizer
- type: impl
- owns:
  - `scripts/enrichment/build_source_only.py`
  - `scripts/enrichment/README.md`
- depends_on: []
- description: |
  Implement separate LongMemEval-S and LoCoMo allowlist sanitizers. The sanitizer is the only process permitted to read raw benchmark files. It must never print raw values, questions, answers, evidence, or gold data to logs.
- acceptance:
  - LongMemEval output includes only `question_id`, `question_date`, session IDs/dates, and turn `role`/`content`.
  - LoCoMo output includes only `sample_id`, speaker identities, session IDs/dates, and turn `dia_id`/`speaker`/`text`.
  - Recursive forbidden-key validation covers QA/question/answer/evidence/gold/label/category/evaluator/result fields and nested `has_answer`.
  - The sanitizer writes deterministic UTF-8 JSON without semantic text normalization and reports only counts/hashes.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Run sanitizer self-tests against synthetic gold-bearing fixtures and prove forbidden fields are absent."
  - kind: review
    required: true
    owner: reviewer
    detail: "Audit allowlists, logging behavior, and raw-to-source trust boundary."

### Task_2: Implement Snapshot Builder And Validator
- type: impl
- owns:
  - `scripts/enrichment/build_snapshots.py`
- depends_on: []
- description: |
  Implement deterministic snapshot generation and validation that accepts only source-only files. Build entities, per-session threads, exact-text derived memories, and deterministic links with source episode/observation provenance. Write manifests and reports with hashes and counts.
- acceptance:
  - The builder has no raw benchmark path option and rejects forbidden keys recursively.
  - LongMemEval generation applies question-date visibility before creating graph objects and records exclusion totals.
  - LoCoMo generation creates one final-session snapshot per sample from raw conversation chronology only.
  - Validation resolves every source reference and graph endpoint, proves every derived text exactly matches cited sanitized source, verifies unique typed IDs, and checks artifact hashes.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Run builder/validator self-tests on synthetic source-only fixtures, including future-session exclusion and malformed reference rejection."
  - kind: review
    required: true
    owner: reviewer
    detail: "Audit that generation cannot access raw/gold inputs and that temporal/provenance invariants are enforced."

### Task_3: Recreate And Validate Source-Only Inputs
- type: chore
- owns:
  - `datasets/enrichment_source/longmemeval_s_source_only.json`
  - `datasets/enrichment_source/locomo_source_only.json`
- depends_on: [Task_1]
- description: |
  Run the sanitizer locally against each raw benchmark file. Snapshot-generation workers must not begin until the source-only files pass the recursive denylist and structural checks.
- acceptance:
  - LongMemEval source-only file has 500 rows and aligned session ID/date/content arrays.
  - LoCoMo source-only file has 10 samples with chronological raw conversation sessions only.
  - No forbidden key appears at any depth, and logs contain counts/hashes only.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "Run both sanitize modes and independent source-only validation; record row counts and SHA-256 values without printing content."

### Task_4: Regenerate LongMemEval-S Snapshots
- type: chore
- owns:
  - `datasets/enriched/longmemeval_s_online_snapshots.jsonl`
  - `datasets/enriched/longmemeval_s_online_snapshots_manifest.json`
  - `datasets/enriched/longmemeval_s_online_snapshots_report.md`
- depends_on: [Task_2, Task_3]
- description: |
  Generate the LongMemEval-S artifact from `datasets/enrichment_source/longmemeval_s_source_only.json` only. Do not open or reference the raw benchmark file, eval outputs, reports, or scorer data.
- acceptance:
  - 500 snapshots keyed by `question_id`, using `question_date` cutoffs.
  - Exclusion totals equal 76 affected rows and 1,475 future sessions; one row has no visible sessions.
  - Manifest hashes/counts match files; report records zero findings.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Run LongMemEval generation and strict validation against source-only input; report counts/hashes only."

### Task_5: Regenerate LoCoMo Snapshots
- type: chore
- owns:
  - `datasets/enriched/locomo_online_snapshots.jsonl`
  - `datasets/enriched/locomo_online_snapshots_manifest.json`
  - `datasets/enriched/locomo_online_snapshots_report.md`
- depends_on: [Task_2, Task_3]
- description: |
  Generate the LoCoMo artifact from `datasets/enrichment_source/locomo_source_only.json` only. Do not open or reference the raw benchmark file, QA subtree, benchmark summaries/observations/events/images, eval outputs, or scorer data.
- acceptance:
  - 10 final-session snapshots keyed by `sample_id`.
  - Manifest hashes/counts match files; report records zero findings.
  - Every derived memory and link resolves to raw sanitized conversation source.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Run LoCoMo generation and strict validation against source-only input; report counts/hashes only."

### Task_6: Integrate, Back Up, And Validate Consumers
- type: test
- owns:
  - `docs/coding-agent/plans/completed/enrichment-artifact-regeneration-plan.md`
  - `C:/tmp/character-memory-evals-enrichment-backup-20260712/**`
- depends_on: [Task_4, Task_5]
- description: |
  Verify both artifacts through the current Rust snapshot loader, run repository-required gates, create a checksum-verified backup, and update the plan evidence.
- acceptance:
  - Both configured snapshot files load with complete dataset coverage.
  - Backup copies match source/enriched artifact SHA-256 hashes.
  - All repository-required validation commands pass.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; service-free synthetic mock smoke."
  - kind: command
    required: true
    owner: orchestrator
    detail: "Run strict artifact validator for both datasets and verify backup hashes."

### Task_7: Independent Leakage And Artifact Review
- type: review
- owns: []
- depends_on: [Task_6]
- description: |
  Independently review scripts, source-only inputs, snapshots, manifests, reports, and validation evidence. The Reviewer must not inspect raw benchmark files or gold-bearing eval outputs.
- acceptance:
  - Reviewer status is APPROVED.
  - Review confirms source-only generation boundary, temporal cutoff correctness, provenance/text resolution, zero forbidden fields, and artifact/backup hash consistency.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Independent contamination-risk and artifact-integrity review with explicit evidence."

## Task Waves (explicit parallel dispatch sets)
- Wave 1 (parallel): [Task_1, Task_2]
- Wave 2 (orchestrator): [Task_3]
- Wave 3 (parallel): [Task_4, Task_5]
- Wave 4 (orchestrator): [Task_6]
- Wave 5 (review): [Task_7]

## Rollback / Safety
- Never pass raw benchmark paths to snapshot-generation workers or commands.
- Generated artifacts stay ignored and local; do not stage or commit them.
- Do not delete or move any existing ignored asset during regeneration. Write new files atomically through temporary siblings, then replace only the target path after successful validation.
- Back up validated artifacts with hashes before any temporary workspace cleanup.
- If row counts, LongMemEval exclusion totals, source-text equality, provenance resolution, forbidden-key scans, or hashes differ from expectations, block completion and preserve all diagnostic reports.

## Progress Log (append-only)
- 2026-07-12 00:00 Research wave completed: prior artifact contract and consumer schema reconstructed.
  - Summary: Confirmed both source-only and snapshot directories are empty; recovered exact artifact names, source-only allowlists, LongMemEval cutoff totals, LoCoMo final-snapshot semantics, and current loader limitations from plans and code.
  - Validation evidence: Two independent read-only Researcher reports; neither inspected raw benchmark contents or network sources.
  - Notes: User plan-approval pause waived because the user explicitly authorized recreation under the established precautions.
- 2026-07-12 00:15 Wave 1 completed: Task_1 and Task_2 implemented and integrated.
  - Summary: Added separate allowlist sanitizer and source-only snapshot builder/validator with atomic writes, deterministic ordering, exact-text provenance, manifests, reports, and synthetic tests.
  - Validation evidence: Sanitizer self-tests and Python compilation passed; snapshot builder self-tests and Python compilation passed. Integration review found and corrected LoCoMo schema mismatch, runtime namespace hashing, official LongMemEval timestamp parsing, and non-RFC3339 thread timestamps before real data execution.
  - Notes: Reviewer-owned validation remains pending. Worker agents were closed after report integration.
- 2026-07-12 00:30 Wave 2 completed: Task_3 source-only inputs recreated and validated.
  - Summary: The Orchestrator ran the allowlist sanitizer as the sole raw-data reader, producing 500 LongMemEval-S rows and 10 LoCoMo samples. A real-data empty-turn edge case was found; the builder was corrected to preserve blank source strings while skipping invalid blank derived memories.
  - Validation evidence: Both source-only files passed the sanitizer validator and the independent snapshot-source parser with zero forbidden-key findings. LongMemEval SHA-256: `add932a4fea279a96fe1e85430133cd9a316126f55e01473a3fa54cad9c2e6d1`. LoCoMo SHA-256: `7fa6181a7ec6260ad6348c01f30b5ad866eab06e4e1cde3fca36eb614c9516e6`.
  - Notes: No raw source content was printed or passed to snapshot-generation agents.
- 2026-07-12 00:50 Wave 3 completed after one LongMemEval validation correction: Task_4 and Task_5 done.
  - Summary: LoCoMo generated 10 final-session snapshots with zero findings. Initial LongMemEval generation produced 500 provisional rows but blocked on conflicting repeated session IDs; metadata-only audit found 13 affected rows. The builder was corrected to mirror the live adapter's deterministic external-ID semantics, where the last visible occurrence wins, and LongMemEval was regenerated successfully.
  - Validation evidence: LongMemEval strict validation passed with 500 unique snapshot IDs, zero duplicate typed IDs, 76 affected cutoff rows, 1,475 excluded future sessions, one no-visible-session row, 717,680 typed graph IDs, matching manifest hashes, and findings=0. LoCoMo strict validation passed with 10 snapshots, matching hashes, and findings=0.
  - Notes: The failed provisional LongMemEval artifact hash and line-count evidence were preserved in the Worker report; no failed manifest or report was accepted. All generation workers were closed after integration.
- 2026-07-12 02:25 Wave 4 completed: Task_6 consumer, repository, and backup validation passed.
  - Summary: Re-ran both strict artifact validators, exercised the Rust enrichment consumer and both benchmark crates, completed all repository quality gates, and ran the service-free synthetic CLI smoke. Created an external backup containing both source-only files, both snapshots, both manifests, both reports, and all three helper/documentation files.
  - Validation evidence: Strict LongMemEval and LoCoMo validators returned status=ok; sanitizer and snapshot-builder self-tests passed; `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` passed under `1.97.0-x86_64-pc-windows-msvc`; the synthetic smoke produced one row and a one-question summary. Backup `C:/tmp/character-memory-evals-enrichment-backup-20260712-021922` contains 11 files with zero SHA-256 mismatches.
  - Notes: The repository-pinned GNU Rust toolchain could not run because this machine lacks `dlltool.exe`; the equivalent installed MSVC 1.97.0 toolchain was used without changing repository configuration. The smoke outputs were removed after validation.
- 2026-07-12 03:00 Wave 5 review returned NEEDS_REVISION; both findings were corrected and revalidated.
  - Summary: The Reviewer found that strict validation proved source-relative consistency but did not independently enforce canonical dataset coverage, and that validate mode rewrote reports instead of checking them. Added immutable LongMemEval/LoCoMo production expectations, pre-write generation checks, read-only byte-for-byte report validation, and synthetic regressions for self-consistent truncation and stale reports.
  - Validation evidence: Python compilation and self-tests passed; both production strict validators returned status=ok; canonical truncation and report non-mutation regressions passed; formatting, strict Clippy, and the full Rust workspace tests passed again. Backup `C:/tmp/character-memory-evals-enrichment-backup-20260712-021922` was refreshed and all 11 files again matched by SHA-256.
  - Notes: The initial Reviewer found no current gold-label leakage, answer-derived content, unresolved provenance, namespace incompatibility, or artifact hash defect. Independent re-review remains required before completion.
- 2026-07-12 03:20 Wave 5 re-review completed: Task_7 APPROVED and plan complete.
  - Summary: A fresh independent Reviewer confirmed both previous validation-boundary findings are closed and found no remaining actionable leakage, integrity, admission-boundary, or validation defects.
  - Validation evidence: Reviewer independently reran sanitizer and snapshot self-tests, both strict production validators with reports, report before/after hash checks, canonical physical-line/count checks, and all 11 backup SHA-256 comparisons. LongMemEval remained 500 rows with 76 cutoff-affected rows, 1,475 excluded future sessions, and one no-visible row; LoCoMo remained 10 rows; findings=0.
  - Notes: Residual operational constraints are explicit: the Rust consumer relies on the strict external validator for manifest/source-hash integrity, and gitignored artifacts rely on the verified external backup for secondary preservation. No rule or lesson candidates remain.

## Decision Log (append-only)
- 2026-07-12 00:00 Decision: use separate sanitizer and generator processes.
  - Trigger / new insight: Both source-only files were lost, and a combined raw-to-snapshot process would weaken the guarantee that generation never sees gold-bearing inputs.
  - Plan delta (what changed): Added an explicit allowlist sanitizer stage and prohibited raw paths in snapshot builders/workers.
  - Tradeoffs considered: A single script is simpler but cannot demonstrate process-level isolation; separate stages provide a stronger contamination boundary.
  - User approval: waived as separately unnecessary because the requested precautions require this stricter boundary.
- 2026-07-12 00:00 Decision: recreate deterministic exact-source replay rather than free-form LLM summaries.
  - Trigger / new insight: The prior validated LongMemEval artifact recorded exact equality between every derived-memory text and its cited visible source turn; LoCoMo recorded deterministic raw-source replay without QA/observation/event-summary inputs.
  - Plan delta (what changed): Snapshot generation copies exact sanitized source text and builds deterministic graph objects/links.
  - Tradeoffs considered: Free-form LLM enrichment could add abstractions but introduces irreproducibility and a harder leakage audit; exact-source replay matches the previously validated artifact semantics.
  - User approval: yes, inherited from the explicit request to recreate with the same precautions.
- 2026-07-12 00:15 Decision: enforce producer-consumer schema and runtime identity in integration tests.
  - Trigger / new insight: Independent workers initially inferred different LoCoMo speaker fields and the builder initially hashed dataset IDs for namespaces.
  - Plan delta (what changed): Builder tests now consume the sanitizer's exact LoCoMo schema and assert literal `lme:<question_id>` / `locomo:<sample_id>` namespaces plus official timestamp normalization.
  - Tradeoffs considered: Hashing avoids awkward IDs but violates the runner's external namespace contract; literal runtime identity is required for snapshot lookup and provenance consistency.
  - User approval: not separately required; this corrects implementation to the documented consumer contract.
- 2026-07-12 03:00 Decision: promote benchmark coverage from plan evidence to executable validator invariants.
  - Trigger / new insight: Independent review demonstrated that a truncated source/artifact/manifest set could be internally consistent while violating the official benchmark coverage contract.
  - Plan delta (what changed): Production generation and validation now enforce fixed dataset counts and LongMemEval cutoff totals; synthetic helper calls opt out explicitly for bounded unit tests.
  - Tradeoffs considered: Fixed expectations intentionally reject alternate/subset datasets through this production path; subsets remain test-only so full-eval artifacts cannot be mistaken for complete outputs.
  - User approval: not separately required; this closes a required acceptance gap without changing generated graph content.

## Notes
- Risks:
  - The current Rust consumer does not validate manifests or source hashes, so the external strict validator is part of required acceptance.
  - Generated files are ignored by Git and need an independent backup.
  - Existing branch work is unrelated and must not be reverted or overwritten.
- Edge cases:
  - LongMemEval rows with missing/equal dates and one row with no sessions visible at cutoff.
  - Unicode `U+2028`/`U+2029` must remain semantically exact while JSONL stays one physical line per snapshot.
  - Speaker strings and session IDs may contain characters unsuitable for direct graph IDs; deterministic hashes/slugs must preserve collision resistance.
