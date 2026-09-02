---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "common"
last_updated: "2026-09-02"
---

# Common Repository Rules

## Repository Reference Documents

- `../../../README.md` is the source of truth for current benchmark commands, workspace architecture, report shape, and runtime lifecycle.
- Decision records: follow `docs/decisions/`; match the existing ADRs' numbering and sections.
- This repository plans and tracks its own work; the library repository's records state only what these measurements allow the library to decide and when they are used (ruled 2026-09-02). Rigor follows the claim a measurement supports, not the code path that produced it: a run and a diff for the inner loop, a deterministic run and a baseline diff for tuning and regression decisions, sealed reproducible evidence only for durable claims.

## Repository-Specific Validation Commands

- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` before reporting implementation done.

## Repo Documentation Wording

- Do not hard-wrap prose in committed documents: never insert line breaks mid-sentence to fit a column width. Write each sentence/paragraph/list item as one line and let editors soft-wrap. Structural line breaks (list items, headings, YAML keys, code) are fine.

## Repo Safety / Boundaries

- Gold evidence labels must be used only for scoring and must not be copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.
- Default validation must remain deterministic and service-free unless the user explicitly asks for real backend integration.
- Benchmark CLI runs default to the live Character Memory adapter; mock benchmark runs must require explicit opt-in and visibly mark outputs as mock/smoke.
- Run the service-free continuity smoke (the README recipe on `configs/continuity_smoke.toml`) before reporting continuity CLI changes done.

## Workaround Tripwire (design-debt escalation)

- The Workaround Tripwire (detection, stop-and-alert response, alert-awaits-ruling) is harness-owned: engineering-quality-baselines Drift Tripwires. Repo-specific standing exception: an artifact is sealed only when the findings register cites it by hash — working around those cited bytes is correct, changing them is not.

## Artifact Placement And Disposition

- Agents must not write task artifacts (probe outputs, scratch scripts, logs, captures, temporary fixtures) to machine-global locations such as `C:\tmp` or the user profile; every artifact lives inside the repository under the gitignored `.agent-work/` directory, in a per-role subdirectory (`.agent-work/worker/`, `.agent-work/reviewer/`, ...) (user-directed 2026-07-22).
- The producing agent owns each artifact's disposition and states it in the task report: DELETE after use (the default — clean up before reporting done), or PROMOTE as evidence worth committing (move it to a tracked location and hand it to the normal commit/review flow with the reason stated).
- Out-of-repo paths are permitted only when the purpose requires leaving the repository (for example a clean-room reproduction proving environment independence), with the purpose and exact path stated in the report and the artifact removed afterward.

## Compatibility Policy

- The `character_memory` library has no external consumers, so backwards compatibility is not a goal here either: track the library's latest surface directly and remove superseded shims, serde old-name tolerance, legacy config keys, and dual code paths in the same change that replaces them (user-directed 2026-07-21).
- This policy does not apply to artifacts cited by hash in the findings register — those bytes are sealed and must not be regenerated or edited to chase a surface change; flag conflicts to the Orchestrator instead.

## Repo Naming / Structure

- Keep backend-neutral adapter/result/metric contracts in `cmem-eval-core`; core must not dispatch on dataset names.
- Keep the live Character Memory integration in `crates/cmem-eval-adapter-cmem`, including deterministic collection naming and persisted external-ID reattach state.
- Each dataset crate must own its loader, ingest mapper, scorer, full-history builder, config-name validation, and metric-family declaration; adding a dataset may add a runner `DatasetSpec` but must not require core edits.
- The continuity benchmark lives in `crates/cmem-eval-continuity`.
- Artifacts carry the harness and library commits; readers are derived serde; old artifacts are old. Sealed register-cited evidence is guaranteed as bytes-by-hash, never as parseability by the live binary.
- Keep latency in dedicated row/summary fields rather than deterministic metrics; record one embedding-provenance header per run; metrics are typed numeric-or-null with fail-closed admission.

## Harness Sync Status

- 2026-07-23: agent-harness v0.9.0 promoted this repo's staged generalizable guidance into harness skills (agent-harness PR #41). Gate CLEARED the same day: installed Claude plugin and Codex profiles both reached 0.9.0, and the rule slimming + lessons drain were applied in this branch per the Codex per-rule verification map (agmsg 2026-07-23T11:50Z).
