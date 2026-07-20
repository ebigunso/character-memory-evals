---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "common"
last_updated: "2026-07-21"
---

# Common Repository Rules

## Repository Reference Documents

- `../../../README.md` is the source of truth for current benchmark commands, workspace architecture, report shape, and runtime lifecycle.

## Repository-Specific Validation Commands

- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` before reporting implementation done.
- Run the service-free synthetic smoke command before reporting benchmark CLI changes done: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`.
- Targeted-test evidence must state an executed-test count greater than zero; a successful exit code alone is insufficient because an unmatched filter can execute no tests.

## Repo Documentation Wording

- Do not hard-wrap prose in committed documents: never insert line breaks mid-sentence to fit a column width. Write each sentence/paragraph/list item as one line and let editors soft-wrap. Structural line breaks (list items, headings, YAML keys, code) are fine.

## Repo Safety / Boundaries

- Gold evidence labels must be used only for scoring and must not be copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.
- Default validation must remain deterministic and service-free unless the user explicitly asks for real backend integration.
- Benchmark CLI runs default to the live Character Memory adapter; mock benchmark runs must require explicit opt-in and visibly mark outputs as mock/smoke.

## Compatibility Policy

- The `character_memory` library has no external consumers, so backwards compatibility is not a goal here either: track the library's latest surface directly and remove superseded shims, serde old-name tolerance, legacy config keys, and dual code paths in the same change that replaces them (user-directed 2026-07-21).
- This policy does not apply to frozen embedding stores, their hashes, or committed evidence artifacts — those are sealed and must not be regenerated or edited to chase a surface change; flag conflicts to the Orchestrator instead.

## Repo Naming / Structure

- Keep backend-neutral adapter/result/metric contracts in `cmem-eval-core`; core must not dispatch on dataset names.
- Keep the live Character Memory integration in `crates/cmem-eval-adapter-cmem`, including deterministic collection naming and persisted external-ID reattach state.
- Each dataset crate must own its loader, ingest mapper, scorer, full-history builder, config-name validation, and metric-family declaration; adding a dataset may add a runner `DatasetSpec` but must not require core edits.
- The continuity benchmark lives in `crates/cmem-eval-continuity`.
- Emit report schema version `1.0.0` on rows and summaries; keep latency in dedicated row/summary fields rather than deterministic metrics, record the embedding provider, and represent unsupported required metrics as `null`.
