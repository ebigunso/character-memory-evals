# Plan: Exact Tiktoken Context Metrics

- status: completed
- generated: 2026-05-03
- last_updated: 2026-05-03
- work_type: code

## Goal
- Replace crude character/word token estimates with exact `tiktoken` token counts for retrieved-context and full-history context metrics.

## Definition of Done
- `cmem-eval-core` uses `tiktoken-rs` with `o200k_base` as the default tokenizer.
- The old character-count estimator path is removed rather than kept as a fallback or deprecated code path.
- Result structs, metric registry keys, runner computation, summaries, and README wording consistently use exact token-count semantics.
- Required repo validation and the service-free synthetic smoke command pass.

## Scope / Non-goals
- Scope:
  - Workspace/core dependency wiring for `tiktoken-rs`.
  - Core token counting helper and tests.
  - Context metric schema and metric registry token-field names.
  - Runner context metric computation for synthetic, LongMemEval-S, and LoCoMo.
  - README metric documentation.
- Non-goals:
  - Retrieval ranking changes.
  - Adapter context rendering changes beyond consuming existing `context_text`.
  - Live Character Memory API changes.
  - Official export format changes unless compile fallout requires a narrow update.
  - Configurable tokenizer selection in this pass.

## Context (workspace)
- Related files/areas:
  - `Cargo.toml`
  - `Cargo.lock`
  - `crates/cmem-eval-core/Cargo.toml`
  - `crates/cmem-eval-core/src/token_count.rs`
  - `crates/cmem-eval-core/src/lib.rs`
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-core/src/metrics.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `README.md`
- Existing patterns or references:
  - Shared result and metric contracts live in `cmem-eval-core`.
  - Scalar metrics live under `metrics`; structured context details live under `context`.
  - Default validation must stay deterministic and service-free.
- Repo reference docs consulted:
  - `docs/coding-agent/rules/index.md`
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`
  - `docs/coding-agent/lessons.md`
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`

## Open Questions
- None. Assumptions below are the planned defaults unless you ask for a change.

## Assumptions
- A1: Use `o200k_base` and `encode_ordinary(text).len()` for literal benchmark context/history text.
- A2: Direct schema cleanup is acceptable: rename `*_estimated_tokens` fields and registry keys to `*_tokens` without serde aliases or deprecated compatibility paths.
- A3: Keep char and word count fields because they are separate descriptive metrics, but do not use them for token counts.

## Quality Routing Note
- Routing level: L1
- In-scope docs:
  - `engineering-quality-baselines/references/language-gates.md`
  - `engineering-quality-baselines/references/language-rust.md`
- Out-of-scope docs:
  - UI/E2E/browser docs: no UI surface.
  - Security-boundary docs: no auth, secret handling, or trust-boundary changes.
  - Backend/frontend stack docs: no web/backend service behavior changes.
- Top risks:
  - Contract/schema compatibility: direct rename changes JSONL/summary metric keys.
  - External dependency: new tokenizer crate updates `Cargo.lock`.
  - Data comparability: new runs should not be compared directly to old estimate-based runs under the same metric names.
- Required checks:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - Service-free synthetic smoke command from repo rules.

## Tasks

### Task_1: Add Exact Token Counter
- type: impl
- owns:
  - `Cargo.toml`
  - `Cargo.lock`
  - `crates/cmem-eval-core/Cargo.toml`
  - `crates/cmem-eval-core/src/token_count.rs`
  - `crates/cmem-eval-core/src/lib.rs`
- depends_on: []
- description: |
  Add `tiktoken-rs` to the core crate and replace the heuristic estimator with an exact `o200k_base` token counter. Prefer renaming the module/function to exact-count terminology during implementation if the diff stays small.
- acceptance:
  - Core token counting uses `tiktoken_rs::o200k_base_singleton()`.
  - The count uses ordinary text encoding, not special-token interpretation.
  - The character-count estimator implementation is removed.
  - Unit tests cover empty text, ASCII text, whitespace-sensitive text, and Unicode/CJK text.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core token`

### Task_2: Rename Context Token Schema And Registry Keys
- type: impl
- owns:
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-core/src/metrics.rs`
- depends_on: [Task_1]
- description: |
  Rename exact-token result fields and scalar metric keys from `*_estimated_tokens` to `*_tokens`, without serde aliases or deprecated duplicate output.
- acceptance:
  - `ResultContextMetrics` exposes `retrieved_context_tokens` and `full_history_tokens`.
  - `REQUIRED_REGISTRY_METRICS` uses `retrieved_context_tokens` and `full_history_tokens`.
  - `insert_context_metrics` emits only exact token metric keys.
  - Existing context compression and reduction metrics continue to use the token denominator/numerator.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core metrics results`

### Task_3: Wire Runner Computation And Documentation
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `README.md`
- depends_on: [Task_2]
- description: |
  Update runner context metric computation to call the exact counter and use the renamed context fields. Update any compile fallout in official export test fixtures/default structs. Update README wording to describe exact `o200k_base` token counts.
- acceptance:
  - Synthetic, LongMemEval-S, and LoCoMo context metrics use exact token counts.
  - Compression ratio and reduction rate use exact token counts.
  - README no longer describes token metrics as heuristic character/word estimates.
  - Any test fixtures or default result constructors compile with the renamed fields.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner`

### Task_4: Workspace Validation And Smoke
- type: test
- owns:
  - `runs/`
- depends_on: [Task_3]
- description: |
  Run the repository-required deterministic checks and service-free benchmark smoke.
- acceptance:
  - Formatting passes.
  - Clippy passes with warnings denied.
  - Workspace tests pass.
  - Synthetic mock smoke writes JSONL and summary artifacts with exact token metric keys.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo fmt --all --check`
  - kind: command
    required: true
    owner: worker
    detail: `cargo clippy --workspace --all-targets -- -D warnings`
  - kind: command
    required: true
    owner: worker
    detail: `cargo test --workspace`
  - kind: command
    required: true
    owner: worker
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`

### Task_5: Review Gate
- type: review
- owns: []
- depends_on: [Task_4]
- description: |
  Review the final diff against this plan, with focus on schema consistency, absence of heuristic fallback paths, and validation evidence.
- acceptance:
  - Reviewer status is APPROVED or issues are resolved.
  - No old `estimated_tokens` result/metric paths remain except historical docs/plans.
  - Validation evidence from Task_4 is present.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review against plan acceptance criteria and validation evidence"

## Task Waves

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]
- Wave 4 (parallel): [Task_4]
- Wave 5 (parallel): [Task_5]

## E2E / Visual Validation Spec

- Not applicable: no UI/user-flow surface is impacted.

## Rollback / Safety
- Revert the plan implementation commit or restore the previous token helper, schema fields, metric keys, and lockfile entries.
- Since metric names change, old and new run summaries should be compared only after accounting for the schema break.
- Generated smoke outputs under `runs/` are ignored benchmark artifacts and should not be committed unless intentionally curated.

## Progress Log

- 2026-05-03 planning: Researcher completed focused code-surface research.
  - Summary: affected surfaces are core token helper, result schema, metric registry/emission, runner context metrics, dependency manifests, and README.
  - Validation evidence: planning only; no implementation validation run yet.
  - Notes: `cargo search tiktoken-rs --limit 1` failed locally with an SSL credential error, but docs.rs confirms the current crate/API.
- 2026-05-04 implementation: Tasks 1-4 completed locally.
  - Summary: added `tiktoken-rs`, replaced character-token heuristic with `o200k_base` counting, renamed active token result/metric keys to `*_tokens`, and updated README.
  - Validation evidence: `cargo test -p cmem-eval-core token`; `cargo test -p cmem-eval-core`; `cargo test -p cmem-eval-runner`; `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; synthetic mock smoke command.
  - Notes: initial non-escalated Cargo commands hit Windows/schannel access issues; required Cargo validation passed after escalation.

## Decision Log

- 2026-05-03 Decision: use exact `o200k_base` token counting.
  - Trigger / new insight: user accepted `o200k_base` as the default tokenizer choice.
  - Plan delta: no configurable tokenizer in this pass.
  - Tradeoffs considered: `cl100k_base` would match `text-embedding-3-*` embedding tokenization, but `o200k_base` is the better default for modern context accounting.
  - User approval: yes, for tokenizer default.
- 2026-05-03 Decision: no deprecation or fallback estimator path.
  - Trigger / new insight: user stated the character-count assumptions are too crude to be useful.
  - Plan delta: direct replacement and schema cleanup; no duplicate old/new metric output.
  - Tradeoffs considered: compatibility aliases would ease old JSONL reads but preserve stale semantics and code paths.
  - User approval: yes.

## Notes
- Risks:
  - Dependency download may require escalation if the same SSL/network issue appears during `cargo test`.
  - Direct schema rename will break downstream consumers expecting `*_estimated_tokens`.
- Edge cases:
  - Empty context text should count as zero tokens.
  - Unicode/CJK and emoji text should be covered by tokenizer unit tests because char-count heuristics fail badly there.
