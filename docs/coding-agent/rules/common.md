---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "common"
last_updated: "2026-09-23"
---

# Common Repository Rules

## Repository Reference Documents

- `../../../README.md` is the source of truth for current benchmark commands, workspace architecture, report shape, and runtime lifecycle.
- Decision records: follow `docs/decisions/`; match the existing ADRs' numbering and sections.
- This repository plans and tracks its own work; the library repository's records state only what these measurements allow the library to decide and when they are used (ruled 2026-09-02). Rigor follows the claim a measurement supports, not the code path that produced it: a run and a diff for the inner loop, a deterministic run and a baseline diff for tuning and regression decisions, sealed reproducible evidence only for durable claims, with register-cited sealed runs kept whole on main under `evidence/`.

## Repository-Specific Validation Commands

- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` before reporting implementation done.

## Repo Documentation Wording

- Do not hard-wrap prose in committed documents: never insert line breaks mid-sentence to fit a column width. Write each sentence/paragraph/list item as one line and let editors soft-wrap. Structural line breaks (list items, headings, YAML keys, code) are fine.

## Repo Safety / Boundaries

- Gold evidence labels must be used only for scoring and must not be copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.
- Default validation must remain deterministic and service-free unless the user explicitly asks for real backend integration.
- Benchmark runs use the embedded Character Memory adapter by default; BM25 ranks ingested text as a separate retrieval baseline.
- Run the service-free continuity smoke (the README recipe on `configs/continuity_smoke.toml`) before reporting continuity CLI changes done.

## Workaround Tripwire (design-debt escalation)

- The Workaround Tripwire (detection, stop-and-alert response, alert-awaits-ruling) is harness-owned: engineering-quality-baselines Drift Tripwires. Repo-specific standing exception: an artifact is sealed only when the findings register cites it by hash — working around those cited bytes is correct, changing them is not.

## Artifact Placement And Disposition

- Agents must not write task artifacts (probe outputs, scratch scripts, logs, captures, temporary fixtures) to machine-global locations such as `C:\tmp` or the user profile; every artifact lives inside the repository under the gitignored `.agent-work/` directory, in a per-role subdirectory (`.agent-work/worker/`, `.agent-work/reviewer/`, ...) (user-directed 2026-07-22).
- The producing agent states DELETE or PROMOTE in its report and chooses the storage tier by what cites the artifact; `.agent-work/` is never tracked, and promotion moves the chosen evidence into the tier below through the normal commit/review flow.
- A measurement reading cited by a plan in either repository must be promoted into a storage tier before that plan closes; a citation to a path under `.agent-work/` is not a retained location.
- Out-of-repo paths are permitted only when the purpose requires leaving the repository (for example a clean-room reproduction proving environment independence), with the purpose and exact path stated in the report and the artifact removed afterward.

### Storage tiers

- **Discarded (default):** delete gate logs, intermediate and repeat captures, helper scripts and worker reports/YAMLs once read; retain the numbers in the worker report and the plan's Decision Log, not the captures, unless a durable citation requires them.
- **Recoverable:** raw traces supporting a durable claim live in one parentless evidence commit under `refs/evidence/<yyyy-mm-dd>-<slug>`, outside the heads and tags fetched by a default clone; preserve the complete promoted folder and root `README.md` exactly as the source commit stores them, including readings, raw files, manifests and repeat names, at their working-tree paths, reusing the existing blobs rather than re-adding files from disk.
- **Permanent on main:** keep the markdown reading and its folder's `README.md` manifest under `docs/evidence/`; keep register-cited sealed runs whole under `evidence/`, including every file needed by `verify`, because a custom evidence ref can be deleted by a writer and is not permanent storage.
- Outside sealed runs, each promotion on main is text-only markdown readings and the manifest, totaling at most 256 KiB (262,144 bytes) of new files measured from blob sizes with `git ls-tree -r -l <commit> -- <promoted-paths>`; raw JSON tables, audits, JSONL, compressed archives, logs and scripts never belong on main at any size.
- The manifest records the evidence ref, its commit hash and the recovery commands; the commit hash binds every byte and its tree lists every path, so do not duplicate file lists or per-file hash lists in the manifest.
- A durable citation names the reading's main path followed by `CharacterMemoryEvals@<evidence-commit>` for raw data; a permalink uses `https://github.com/ebigunso/CharacterMemoryEvals/tree/<evidence-commit>/<folder>`, which remains valid independently of the measurement branch.

### Evidence ref creation and recovery

Build the complete promoted folder and root `README.md` from the source commit with a separate index; replace the placeholders before running these commands from any worktree of the repository, use an unused `evidence.index` path and ref name, and leave the working tree and normal index untouched:

```bash
export GIT_INDEX_FILE="$(git rev-parse --absolute-git-dir)/evidence.index"
git read-tree --empty
git read-tree --prefix=<folder>/ <source-commit>:<folder>
blob=$(git rev-parse <source-commit>:README.md)
git update-index --add --cacheinfo "100644,$blob,README.md"
c=$(git commit-tree "$(git write-tree)" -m "Evidence <yyyy-mm-dd>-<slug>")
git update-ref refs/evidence/<yyyy-mm-dd>-<slug> "$c"
rm -f "$GIT_INDEX_FILE"
unset GIT_INDEX_FILE
```

Repeat `read-tree --prefix` for each kept folder and `update-index --add --cacheinfo` for each kept single file. For a never-committed file, obtain its blob with `git hash-object -w --no-filters <file>` so checkout filters cannot change its bytes, then add it at its recovery path with `update-index`.

The orchestrator publishes the new ref with `git push origin refs/evidence/<yyyy-mm-dd>-<slug>`; do not rewrite an existing evidence ref. Recovery fetches only the requested evidence and restores its working-tree paths:

```bash
git fetch origin refs/evidence/<id>:refs/evidence/<id>
git restore --source=refs/evidence/<id> --worktree -- <path>
```

`/docs/evidence/** -text` preserves the stored bytes during restore; on a checkout predating that attribute, use `git -c core.autocrlf=false restore --source=refs/evidence/<id> --worktree -- <path>` instead. Restored raw files stay untracked and ignored. GitHub's evidence-commit page may say it does not belong to any branch; that is expected. CI rejects tracked `.agent-work/` paths and non-markdown paths under `docs/evidence/`, even if force-added.

## Compatibility Policy

- The `character_memory` library has no external consumers, so backwards compatibility is not a goal here either: track the library's latest surface directly and remove superseded shims, serde old-name tolerance, legacy config keys, and dual code paths in the same change that replaces them (user-directed 2026-07-21).
- This policy does not apply to artifacts cited by hash in the findings register — those bytes are sealed and must not be regenerated or edited to chase a surface change; flag conflicts to the Orchestrator instead.

## Repo Naming / Structure

- Keep shared configuration, metrics, result types, and Character Memory integration in `crates/cmem-eval`, including namespace store naming and persisted external-ID reattach state; dataset dispatch belongs in the runner.
- Each dataset crate must own its loader, ingest mapper, scorer, full-history builder, config-name validation, and metric-family declaration; adding a dataset may add a runner `DatasetSpec` but must not require core edits.
- The continuity benchmark lives in `crates/cmem-eval-continuity`.
- Artifacts carry the harness and library commits; readers are derived serde; old artifacts are old. Sealed register-cited evidence is guaranteed as bytes-by-hash, never as parseability by the live binary.
- Keep latency in dedicated row/summary fields rather than deterministic metrics; record one embedding-provenance header per run; metrics are typed numeric-or-null with fail-closed admission.

## Harness Sync Status

- 2026-07-23: agent-harness v0.9.0 promoted this repo's staged generalizable guidance into harness skills (agent-harness PR #41). Gate CLEARED the same day: installed Claude plugin and Codex profiles both reached 0.9.0, and the rule slimming + lessons drain were applied in this branch per the Codex per-rule verification map (agmsg 2026-07-23T11:50Z).
