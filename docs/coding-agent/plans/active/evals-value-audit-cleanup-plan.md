# Plan: Evals value-audit cleanup

- status: draft
- generated: 2026-09-24
- last_updated: 2026-09-24
- work_type: code

## Goal
- The evals workspace keeps only the code and tests that earn their place under the 2026-09-24 value audits (ruling 77). The stale scene embedding-input list is already fixed on the integrated base, inherited from the stack top through `499e87a`. Task_1 then finishes the two known-failing tests, so the workspace suite runs green with no exclusions. Every later deletion is shown to leave run output unchanged, apart from the fields it deletes.
- Decision this informs: the library's v0.2 readings come from a harness with fewer paths to misread and a suite that fails only for real reasons. Task_12 lets the library's value-audit cleanup land without breaking this workspace's tests.

## Definition of Done
- Task_1 is merged into the stack, and `cargo test --workspace` passes with no exclusions. Its executed count is recorded.
- Every item in the Evals section of the verdicts is either applied by a task below or kept as ruled (EP13, EP23, EP24, EP25, ET14).
- Every task that edits production code on the run path records its behavior-free proof in the Progress Log. The proof is defined under Tasks.
- At the stack tip, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass, with executed counts and no exclusions.
- Task_12 passes the companion build and test at the pre-cleanup library pin `c0ed9e21` and at the library cleanup's final tip. This is the gate the library cleanup plan requires before its stack merges.
- No sealed or register-cited byte changes. The six protected assets listed in `docs/coding-agent/plans/active/v0-2-situated-recall-scenarios-plan.md` keep their hashes.
- The plan closes in `docs/coding-agent/plans/completed/`.

## Planner-added requirements
- Task_1 also deletes `output_leaf_links_are_rejected_before_writing_artifacts`, the same deletion as evals PR 68 (commit `2789092`). This applies only if the test is still present and failing on the integrated base. Needed because: PR 68 is still open against main. The test is not removed on this line (it is still at `18bc97a`), although the verdicts say it is already on main. On this machine it fails without the link privilege, so the suite cannot run green with no exclusions while it is present. Making the identical deletion lets the stack and main merge without a conflict.
- Every behavior-free proof takes its base run and its tip run at one pinned library commit, and records that commit beside the numbers. Needed because: otherwise a difference in the library reads as a difference in the harness, and the proof proves nothing.
- Task_5 loads both local official dataset files (`datasets/locomo10.json`, `datasets/longmemeval_s_cleaned.json` in the main checkout) at base and at tip, and shows that the loaded items are identical. Needed because: the continuity smoke never reaches these loaders. The verdict rests on the claim that no real file uses the deleted shapes, and the benchmark runs deferred to the v0.2 closeout will read exactly these files.
- Task_7 keeps a scenario with a restart in the trimmed scripted-scenarios pipeline test. Needed because: ET10 deletes the pipeline restart test on the strength of that test's restart assertion. If the test were cut to scenarios without a restart, the report's restart count, handed over from the runner, would be tested nowhere through the command path. Challenged: the driver library test also covers restart stability, but it does not go through the runner. The choice of scenario costs nothing, so the requirement stays.
- Task_10 keeps the retrieved-context and full-history token counts in the metrics map. The duplicate that goes is the copy in the context block. Needed because: `summarize_rows` (`crates/cmem-eval/src/results.rs:176`) aggregates only the metrics map, and the library's ADR-I-0022 context-size baseline and the v0.1.5 findings register read those keys. Dropping them from the map would remove the context-size reading from every report, and the situated plan's Task_6 re-measurement would lose comparability.
- Considered and not added:
  - Removing the reviewer hotspot `alias_destination_assertion` once the loader aliases are gone. It is idle, it costs nothing, and the verdicts did not ask for it.
  - A smoke run for Tasks 3, 4, 5 and 6. None of them touches the run path.
  - A regression test for the embedding-input fix beyond the rewritten scene-slice test. The rewritten test exercises the fix.
  - Keeping Task_12 unlinked from the GitHub stack until the library's LP3 pull request is approved. This was dropped on 2026-09-24: Task_12 passes at both library pins, so either repository can merge first.

## Scope / Non-goals
- Scope:
  - `crates/cmem-eval`, `crates/cmem-eval-runner`, `crates/cmem-eval-continuity` (library code and tests).
  - The loaders and admission tests of `crates/cmem-eval-locomo` and `crates/cmem-eval-longmemeval`.
  - `scripts/qdrant_prune_collections.sh` and `scripts/README.md`.
  - The root `README.md`, in the sections each task names.
  - `docs/coding-agent/rules/reviewer.md`, the `path_identity` entry only. The orchestrator makes this edit when it integrates Task_7.
  - Mechanical consumer updates in `crates/cmem-eval-continuity/src/bin/**`, where a type change forces them.
- Non-goals:
  - The kept items: EP13 (typed fixture admission errors), EP23 (calibrator flags; re-audited when the slice-end measurement closes), EP24 (the deterministic embedding provider), EP25 (the scenario embedding serialization), and ET14 (`continuity_benchmarks_v2.json`, its test and the `continuity_benchmarks_v1` entry in `frozen_embeddings.rs`; the benchmark runs deferred to the v0.2 closeout still use them).
  - The service-mode tests.
  - `crates/cmem-eval-benchmark-convert`.
  - The continuity crate README.
  - Any library change.
  - Any sealed, register-cited or protected byte.
  - New scenarios or measurements.
  - Tests beyond those the verdicts ask for.

## Design
- Chosen: the verdicts are grouped by concern into twelve tasks, and no two tasks in a wave own the same file.
  - Task_1 goes alone and first, on the integrated base: the obligations family committed, and the stack top `18bc97a` forward-merged into it by the orchestrator.
  - Wave 2 runs five tasks that touch no file the obligations work touches.
  - From Wave 3 on, the hot files (`pipeline.rs`, `adapter.rs`, `driver.rs`, `fixture.rs`, `memory_adapter.rs`, `results.rs`, `config.rs`) have one owner per wave.
  - The library companion change (Task_12) is last. It passes at both the pre-cleanup library pin and the library cleanup's final tip, so it constrains neither repository's merge order.
  - Each task is one pull request, and every pull request is behavior-free and proven so on its own.
  - Structure: no component is added. The cleanup removes concepts (the dataset kind, the retry policy, the second enrichment input, typed errors with no consumer, the path-containment guard). Because owns are disjoint within a wave, integration never has to merge edits to the same file.
  - Evolution: review fixes to the obligations family can still land in `driver.rs`, `pipeline.rs`, `adapter.rs`, `memory_adapter.rs`, `bm25.rs`, the calibrator bin or the continuity README. Those fixes forward-merge through Waves 1 and 2 without conflict: Task_1 edits two of those files only in test hunks far from the obligations hunks, and Wave 2 edits none of them. Deleting a task means reverting one pull request.
  - Verification: from Wave 2 on the suite is green, so every task reports counts with no exclusions. Each smoke diff can be traced to one concern's deleted fields.
  - Operation: seven waves on two workers. Wave 2 queues five tasks. Waves 4 to 7 are serial because each needs `pipeline.rs`. The harness itself gets slightly cheaper to run: no store re-serialization per scenario, and a shorter slowest test.
  - Human: each pull request states one outcome and can be reviewed alone.
  - Safety: gold handling, the frozen stores and sealed evidence are untouched.
  - (Consumers: see the Compatibility stance.)
- Alternative A: start the Wave 2 tasks now, before the obligations commit and before Task_1. They touch no obligations file.
  - Structure: the same.
  - Evolution: they finish earlier.
  - Verification: each proof would be taken at a base that lacks the obligations commit, the forward-merged stack top and Task_1. Every proof would then have to be redone at the task's final integrated parent. Until then, the counts would carry the known exclusions.
  - Operation: the saving is lost to the rerun proofs, and the obligations commit is close.
  - Human: the same.
  - Safety: the same.
- Alternative B: one task per hot file (every `pipeline.rs` verdict in one task, every `adapter.rs` verdict in another).
  - Structure: fewer, larger tasks.
  - Evolution: a revert takes unrelated verdicts with it.
  - Verification: one smoke diff mixes the deletions of EP3, EP8, EP10 and EP11, so a stray difference cannot be traced to a verdict. The verdicts that cross files (EP8, EP10, EP11, EP12, EP17) would still need several hot files at once, which breaks the split per file anyway.
  - Operation: about four waves instead of seven.
  - Human: diffs of more than 1,500 lines.
  - Safety: the same.
- Alternative C: a Task_12 that follows the library, validated only at the library cleanup tip and landed at the level where this stack's library pin moves.
  - Structure: the same.
  - Evolution: the evals stack and the library stack would have to merge in lockstep.
  - Verification: it proves nothing about the pre-cleanup pin, which every lower level of this stack uses.
  - Operation: the evals and library schedules become coupled.
  - Human and Safety: the same.
- Why chosen: it gives a green suite as early as the integrated base allows. It keeps early work off the files the obligations review may still change, and makes each behavior-free proof traceable to one concern. Because the companion change passes at both library pins, the library schedule stays out of every task.
- Fit:
  - The compatibility policy in `docs/coding-agent/rules/common.md` (no shims; sealed bytes excepted).
  - The rule in the same file that adding a dataset must not require core edits, which EP8 restores by removing `DatasetKind` from the shared crate.
  - The stacking rule in `docs/coding-agent/rules/orchestrator.md`.

## Compatibility stance
- surface:
  - Run rows: the always-null metric keys, the top-level context counts and the full-history character and word counts.
  - The run header: `dataset_kind`.
  - Continuity traces: the restart before and after diagnostics and the signed deltas.
  - The continuity report: `tuning_observations`.
  - The run config: `dataset` becomes a plain string with the same values; `ingest.enrichment_path` and `backend.openai_api_key_env` are removed, so a config naming them now fails as an unknown key.
  - The `embeddings generate` flags `--api-key-env` and `--dimensions`.
  - The string form of the snapshot manifest.
  - The LoCoMo and LongMemEval input aliases.
  - The seal command's link and name checks.
  - Public test-only functions.
- stance: break
- justification:
  - Every consumer is in this workspace. `common.md` rules out shims and dual paths.
  - Artifact readers are derived serde, so old artifacts stay old and still parse (ADR-I-0005).
  - Sealed evidence is guaranteed as bytes by hash, not as parseable by the live binary.
  - None of the 42 committed configs sets a removed key.
  - The manifests the configs point to use the object form with a hash (Assumption A3).
  - The promoted scene-reminders evidence JSON that still contains `configured_object_types` is data under `docs/evidence` and is never parsed by the live binary.

## Context (workspace)
- Related files and areas:
  - The inputs: the verdicts (Evals section and Constraints) and the audits `audit-evals-prod.md` (EP) and `audit-evals-tests.md` (ET), evidence at `df636bf`.
  - The library embedding surface: `src/policy/embedding_surface.rs` at the pinned library commit.
  - The library audit item LP3: `audit-lib-prod.md` item 3.
- Obligations files: at 2026-09-24 the uncommitted obligations work touches:
  - `crates/cmem-eval-continuity/README.md`;
  - `crates/cmem-eval-continuity/src/bin/calibrate_cue_floors.rs` and its folder (`consolidation.rs`, `time_and_obligations.rs`, new `obligations.rs`);
  - `crates/cmem-eval-continuity/src/driver.rs`;
  - `crates/cmem-eval-runner/src/pipeline.rs`;
  - `crates/cmem-eval/src/adapter.rs`, `bm25.rs` and `memory_adapter.rs`.

  The edits in `driver.rs`, `pipeline.rs`, `bm25.rs` and `memory_adapter.rs` add one field, `lifecycle_policy`, to `RetrieveInput` and to its callers.
- Integrated base: the obligations family committed on `feature/2026-09-23/prospective-obligations`. After the active AFTER capture, the orchestrator forward-merges the stack top `18bc97a` into it through consolidation-families. That merge brings the separate-surface helper, the calibrator consumer corrections and the stronger native drift guard through `499e87a` (review note 61).
- Library pin: every task except Task_12 validates at the library commit the obligations family validates against. Task_1 records it in the Decision Log. Task_12 validates at the pre-cleanup pin `c0ed9e21` and at the library cleanup's final tip.
- Library dependency lines: the library cleanup plan's section "Cross-repo dependency: the companion's test modules stop reading removed surfaces" (`.agent-work/orchestrator/v0-2-value-audit-cleanup-plan-draft.md` in the library repository) is carried by Task_12. Its `get_oxigraph_path` part is satisfied by Task_8's ET4 deletion.
- Team:
  - Workers: the Codex agents `evals-worker` and `evals-worker2`, each in a task worktree at `<evals repo>/.worktrees/<slug>`.
  - Reviewer: `evals-reviewer`, Tier D, in an isolated worktree under `C:/w` pinned at the review commit.
  - The orchestrator integrates each wave.
- Existing patterns: the smoke recipe and the situated recipe in `README.md` ("Run a service-free continuity smoke", "Run situated scenarios"), with `diff` and `compare-continuity`. The reviewer evidence table in `docs/coding-agent/rules/reviewer.md`.
- Design records consulted, and deviations from their acceptance:
  - This repository's ADR-I-0004 and ADR-I-0005, and the library's ADR-I-0022 and ADR-I-0024. There is no deviation.
  - The one deviation from a verdict's wording is in the EP10 treatment of token counts, which is logged below.

## Open Questions (max 3)
- None. The interpretations the verdicts leave open are decided and logged in the Decision Log, under the standing instruction to decide and log.

## Assumptions
- A1: The obligations family's commit touches no more files than the list above. Source: `git diff --stat` and `git status` of `.worktrees/prospective-obligations` on 2026-09-24. Checked by the orchestrator before Wave 2 is dispatched; a Wave 2 task that meets a newly touched file moves to a later wave.
- A2: The integrated base already embeds scenes as the pinned library does: the setting on its own, and each participant's name and description joined, with participants separated by newlines. Source: review note 61; `runtime_scene_embedding_texts` in `fixture.rs` at `18bc97a` (`499e87a`). Checked by Task_1, which reports the failures that actually remain at the integrated base.
- A3: The snapshot manifests the committed configs point to use the object form with a hash (`.worktrees/situated-groundwork/datasets/enriched/*_groundwork_snapshots_manifest.json`, checked 2026-09-24). Only the unreferenced `datasets/enriched/locomo_online_snapshots_manifest.json` uses the string form. Checked again by Task_9 before the string form is dropped.
- A4: The retired partition key is tested by `retired_partition_key_and_reason_are_rejected_in_both_formats` in `fixture.rs`. It sits at the stack top (`situated-scene-surfaces`), and this line gets it when the orchestrator merges the stack top forward after the consolidation AFTER run; that test stays as it is. The one retired config key kept in `config.rs` is `namespace_prefix`. Source: verdicts, Tests; the orchestrator's check on 2026-09-24.
- A5: The evals uses of library surfaces the library cleanup removes are all in test modules:
  - `configured_object_types` at `pipeline.rs:2513`, `adapter.rs:2900` and `adapter.rs:3239`;
  - `CURRENT_SCHEMA_VERSION` at `adapter.rs:2773` and `adapter.rs:5355`;
  - `Settings::get_oxigraph_path` at `adapter.rs:3039`.

  Evals production uses none of them. Source: the library cleanup plan's dependency lines, review finding F1, and a grep for these names at `df636bf`.

## Tasks

Behavior-free proof applies to Tasks 1, 2, 7, 8, 9, 10 and 11: every task that edits production code on the run path.

Base and tip:
- The base is the task's final PR's integrated parent. For a task developed in parallel and then stacked, that is the level directly below its final PR, not the wave's common parent. Otherwise a forward merge would bring another task's legitimate deletions into this task's proof.
- The tip is the task's final tip.
- Both runs use the same pinned library commit.

Runs:
- Run the README smoke recipe once at base and once at tip, into fresh directories under `.agent-work/worker/<task>/`.
- Run both situated fixtures (`situated_v1.toml` and `situated_loud_topic_v1.json` with `configs/continuity_situated.toml`) once at base and once at tip.

Checks:
- The CLI checks stay: `diff` base against tip for the smoke and for each situated fixture, and `compare-continuity` for each situated pair.
- A disposable structured comparison is added. It is a scratch script under `.agent-work/worker/<task>/`, deleted with the captures, and it is never repository code.
  - It reads the raw serialized JSON of every artifact of each run: the rows or `traces.jsonl`, `header.json` and `report.json`. It never goes through the harness's typed readers, so removed keys stay visible.
  - It checks every remaining value for equality, and it lists every key path that is present at base and absent at tip, or the reverse.
- These fields are normalized as provenance and timing, and nothing else is:
  - in the header: `harness_commit`, `generated_at`, `storage_root` and `storage_root_sha256`;
  - in rows and traces: `latency_ms` and `query_latencies_ms`;
  - in the report: the `latency` aggregate.

  If a native timestamp the library writes from its own clock also varies, the worker names its exact path in the report as a further normalization, and the reviewer rejects any normalization that could hide a value the task changes.
- It passes when:
  - `compare-continuity` reports no differences;
  - every `diff` difference is a field the task deletes;
  - the structured comparison finds no changed value, and its list of removed paths equals the task's allowed deletions. Task_2 is the one exception: the `registry_coverage` counts (`required_metrics_total`, `present`, `null_only`) and `missing_required_metrics` may change, but only by the amounts derived from the exact deleted keys and each row's support. The worker states the expected values before the run and the comparison checks them. `registry_coverage` is never normalized or ignored as a whole.

Recording:
- The worker report records the structured comparison's field differences before the captures are discarded.
- Readings follow the Discarded tier of the storage rules in `docs/coding-agent/rules/common.md` on main. Captures and the script are deleted once read. The numbers go in the worker report and this plan's logs: rows compared, the removed paths, the normalized paths, the `compare-continuity` result and the library commit. Nothing is promoted, because nothing durable cites these readings.

Every validation item below that names "the behavior-free proof" includes the CLI checks and the structured comparison.

Tasks 3, 4, 5, 6 and 12 do not touch the run path, and each names its own proof. Task_5 uses the same structured comparison on the loaded dataset items.

Reviewer evidence applies to every task below. Besides the diff review, the Reviewer produces what `docs/coding-agent/rules/reviewer.md` requires for the files the task touches:
- a `diff` against the stored baseline for driver, report or metric changes;
- the embedded adapter suite with executed counts, plus the service-mode tests with Qdrant up, for adapter or persistence changes;
- both SHA-256 values of a regenerated fixture for generator changes;
- an independent `verify` of the sealed runs for seal changes.

### Task_1: The workspace suite runs green with no exclusions
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/driver.rs (the two known-failing tests and the `checked_write_outcome` unit test only)
  - crates/cmem-eval-runner/src/pipeline.rs (deleting `output_leaf_links_are_rejected_before_writing_artifacts` only, identical to commit `2789092`)
- depends_on: [] (external: the integrated base, meaning the obligations family committed and the stack top `18bc97a` forward-merged into it)
- description: |
  Start from the integrated base. It already carries the embedding-input fix through `499e87a`: the separate-surface helper, the calibrator consumer corrections and the stronger native drift guard (review note 61, Assumption A2). That work is inherited, not redone.
  First, run `cargo test --workspace` at the integrated base and report the failures that actually remain. Then do only what remains of the following:
  - Reduce `scene_slice_preserves_authored_input_and_checks_native_results` to one or two embedded runs with plain wording. Keep the negative native checks: authored scene fields reach the native scene, and the checker fails on a wrong time or on missing facts. Check the place-variant mapping through `map_situated_input` without a store.
  - Delete `situated_writes_reject_degraded_native_outcomes`, and add its pure write-outcome cases to the `checked_write_outcome` unit test.
  - Delete the link-privilege test as the same change as PR 68, if it is still present and failing (planner-added).
  This task touches `driver.rs` and `pipeline.rs`, which the obligations work also touches, so it starts only on the integrated base. Its edits there are test hunks away from the obligations hunks.
- acceptance:
  - The report lists the failures found at the integrated base, and what this task did about each. No inherited work is redone.
  - The scene-slice test makes one or two embedded runs, and it keeps the negative native checks.
  - The `checked_write_outcome` unit test covers a stats failure, a vector-indexing failure, repair-needed markers and an empty indexed list, and the degraded-outcome embedded test is gone.
  - `cargo test --workspace` passes with no exclusions, and the report gives the executed count.
  - The behavior-free proof shows no difference. If a run changes, the task stops and reports.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with executed counts and no exclusions; the behavior-free proof"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; confirm the pipeline.rs hunk equals commit 2789092"

### Task_2: Rows stop carrying metrics nothing produces
- type: impl
- owns:
  - crates/cmem-eval/src/metrics.rs
  - crates/cmem-eval/src/results.rs
  - crates/cmem-eval/src/config.rs (the unknown-key test only)
- depends_on: [Task_1]
- description: |
  EP4: delete the always-null and duplicated metric keys. EP5: delete the dead metric helpers. EP17: delete the `RunAdapterMetadata` `Default` implementation. ET7: keep the typo cases at every container level, plus exactly one retired config key, `namespace_prefix` (Assumption A4), and delete the other retired-key cases. The fixture's retired partition test is not touched.
- acceptance:
  - These keys are gone from the registry and from rows: the five QA keys, `cross_store_id_validation_pass_rate`, `returned_items_with_authoritative_validation`, and the plural `returned_items_without_external_ids`. `insert_integrity_metrics` and its test, `initialize_registry_metrics`, `registry_coverage_summary`, `MetricsRecord::new` and the `Default` implementation are gone.
  - The unknown-key test holds the typo cases and `namespace_prefix`.
  - The behavior-free proof lists only the deleted keys, in rows and in registry coverage.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the behavior-free proof"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause"

### Task_3: Generating a frozen store makes one plain request
- type: impl
- owns:
  - crates/cmem-eval/src/openai_embedding.rs
  - crates/cmem-eval-runner/src/frozen_embeddings.rs (the generate path only; the committed-stores test is unchanged, per ET14)
  - README.md (the "Generate and validate frozen real embeddings" section only)
- depends_on: [Task_1]
- description: |
  EP6: turn the retry machinery into a single request. EP12: turn the OpenAI `ResponseError` into message errors with the same text. EP15: remove the flag part, `--api-key-env` and `--dimensions`, and read `OPENAI_API_KEY` directly; the config key goes in Task_11.
- acceptance:
  - `EmbeddingRetryPolicy`, the backoff and the `endpoint` field are gone, and generation still sends exactly one request.
  - The command has no `--api-key-env` or `--dimensions`, and the README section matches it.
  - Proof: the offline tests pass, `embeddings validate` accepts every committed pair, no committed store or manifest byte changes, and no provider call is made.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; sha256 of the six protected assets"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review"

### Task_4: Continuity fixture and generator tests pay for each contract once
- type: test
- owns:
  - crates/cmem-eval-continuity/src/fixture.rs
  - crates/cmem-eval-continuity/src/generator.rs
  - crates/cmem-eval-continuity/src/report.rs (the no-overwrite test only)
  - crates/cmem-eval/src/controllable_similarity_embedding.rs (`concept_for_text` only)
- depends_on: [Task_1]
- description: |
  The fixture and generator tests: ET5, ET8, ET12, ET22. The report test: ET17. EP17 covers `read_fixture`, `scenario_patterns` and `concept_for_text`: each test-only public function leaves the production surface. If moving a helper across crates would cost more than it removes, keep it and say why. EP13 is kept: tests that assert admission kinds stay.
- acceptance:
  - Admission checks run once through JSON. The non-finite float test and the TOML/JSON parity test stay.
  - One schema-version rejection test remains, and the retired identity-field cases are gone. The retired partition test stays (Assumption A4).
  - The cross-process generator test and the pinned scenario-ID list are gone, and the byte-for-byte canonical comparison stays.
  - The report no-overwrite test no longer writes to `env::temp_dir()`.
  - Proof: the checked generated fixtures are still reproduced byte for byte, and the six protected hashes match. No production behavior changes; only test-only helpers move.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause (fixture regeneration hashes)"

### Task_5: Dataset loaders read the official shapes only
- type: impl
- owns:
  - crates/cmem-eval-locomo/src/loader.rs
  - crates/cmem-eval-longmemeval/src/loader.rs
  - crates/cmem-eval-locomo/tests/admission.rs
  - crates/cmem-eval-longmemeval/tests/admission.rs
- depends_on: [Task_1]
- description: |
  EP14 and ET13: delete the root wrappers, the alias keys, LoCoMo's array-of-session-objects branch, and LongMemEval's record-ID session objects with their collision rules. Delete their tests and the module docs that describe them.
- acceptance:
  - Only the official `locomo10.json` and `longmemeval_s_cleaned.json` shapes are admitted, and a former alias now fails admission.
  - The tests of the official shapes stay.
  - Proof (planner-added): both local official files are loaded through `load_path` at base and at tip. The complete loaded items are serialized to raw JSON and compared with the structured comparison, with no normalized fields and no allowed deletions. Counts alone are not enough.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the base-versus-tip structured comparison of the complete loaded items of both official files"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review"

### Task_6: The seal command keeps its guarantee, and the stale prune script goes
- type: impl
- owns:
  - crates/cmem-eval-runner/src/seal.rs
  - scripts/qdrant_prune_collections.sh
  - scripts/README.md
- depends_on: [Task_1]
- description: |
  EP19: drop the check that the evidence root is not a link, the regular-file check through `symlink_metadata`, and the file-name validation of `run_id` and seal entries. Keep `create_new`, hashing and `verify`. EP20: delete the prune script and its README section.
- acceptance:
  - Sealing still refuses to overwrite, and it hashes and verifies as before. The seal tests pass.
  - `verify` passes on `evidence/pr13r9ba` and `evidence/pr13r9bb` with the tip binary.
  - The script and its section are gone, and nothing else refers to them.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; verify on both sealed runs"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause (independent verify of the canonical hashes)"

### Task_7: The runner admits outputs with one existence check and tests each contract once
- type: impl
- owns:
  - crates/cmem-eval-runner/src/pipeline.rs
  - README.md ("Read run artifacts": the admission sentences only)
- depends_on: [Task_2, Task_3, Task_4, Task_5, Task_6]
- description: |
  - EP3: output admission becomes a `.jsonl` check, a no-clobber existence check on the three fixed names (`OutputPathExists` stays), and `create_dir(stores)`. `OutputPathInStores`, the canonicalization and the `other_outputs` generality go. The worker does not edit `docs/coding-agent/rules/reviewer.md`. The rule narrowing is an orchestrator integration action listed under validation.
  - EP1: `ContinuitySpec` becomes a free `validate_continuity_config` plus an inline count of query and probe events.
  - EP22: progress output becomes a start line, a line per item and an end line.
  - Tests: delete ET1, ET2, ET3 and ET10. Trim ET11 in its pipeline part, ET15, ET16 and ET19. ET11's trimmed test keeps a scenario with a restart (planner-added).
- acceptance:
  - An existing output name still fails admission before anything is written, and outputs are still created new.
  - The runner has no `DatasetSpec` implementation for continuity, and `run_pipeline` serves only LongMemEval and LoCoMo.
  - Each deleted test is gone; each trimmed test keeps the behavior the audit names: keeping or cleaning stores, `storage_root_sha256`, and the writing, header and report-registry checks.
  - The behavior-free proof shows no artifact difference (standard error output is expected to change).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the behavior-free proof"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; the orchestrator's path_identity edit matches the code as built"
  - kind: review
    required: true
    owner: orchestrator
    detail: "Integration action: narrow path_identity in docs/coding-agent/rules/reviewer.md to what the runner still does (an existing destination fails by name; every writer creates new; any cleanup that compares paths is reviewed as before), committed on Task_7's level after the worker's commits"

### Task_8: The adapter has one way to be built and no test-only seams
- type: impl
- owns:
  - crates/cmem-eval/src/adapter.rs
  - crates/cmem-eval/src/fs_util.rs
  - crates/cmem-eval/src/memory_adapter.rs
  - crates/cmem-eval/src/bm25.rs
  - crates/cmem-eval-continuity/src/fixture.rs (the frozen drift-guard test's adapter construction only)
- depends_on: [Task_2, Task_3, Task_4, Task_5, Task_6]
- description: |
  - EP2: delete the adapter constructors that only tests call, or that nothing calls. Their tests build a binding and call `new_with_binding` or `reconstruct_with_binding`.
  - EP16, ET9 and ET6: `save` and `persist_with_retry` call one `atomic_replace`, and the duplicate retry test goes. Keep the atomic write and the Windows permission-denied retry.
  - ET4: delete the `OXIGRAPH_PATH` child-process test and its probe. This also removes the only `Settings::get_oxigraph_path` use (`adapter.rs:3039`), which satisfies that part of the library cleanup plan's dependency lines. No replacement test is added.
  - EP12: `UnsupportedCorrectionCreatedAt` and `TimeRangeInputError` become message errors with the same text.
  - EP17: delete `IngestedObjectRefs` and `RetrievedExternalRef`, and move `rank_documents` into the tests.
- acceptance:
  - Only `new`, `new_with_binding` and `reconstruct_with_binding` build an adapter.
  - `atomic_replace_with_before_persist` and `save_with_before_persist` are gone. The tests of preserve-on-failure and the permission-denied retry stay.
  - The listed types are gone or moved, and every error message is unchanged.
  - The behavior-free proof shows no difference.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the behavior-free proof"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause (embedded adapter suite and service-mode tests with Qdrant up)"

### Task_9: The run configuration names its dataset once and takes enrichment from the snapshot only
- type: impl
- owns:
  - crates/cmem-eval/src/config.rs
  - crates/cmem-eval/src/runtime.rs
  - crates/cmem-eval/src/results.rs (the header's `dataset` type and `dataset_kind`, their imports, and the header's test constructor only)
  - crates/cmem-eval-runner/src/pipeline.rs
  - crates/cmem-eval-runner/src/enrichment.rs
  - crates/cmem-eval-runner/src/commands_tests.rs
  - crates/cmem-eval-locomo/src/lib.rs
  - crates/cmem-eval-locomo/tests/benchmark_derived_content.rs
  - crates/cmem-eval-longmemeval/src/lib.rs
  - README.md (the `enrichment_path` part of "Precomputed Graph Enrichment" only)
  - crates/cmem-eval/src/adapter.rs, crates/cmem-eval-continuity/src/driver.rs, crates/cmem-eval-continuity/src/fixture.rs, crates/cmem-eval-continuity/src/bin/** (mechanical updates for the `dataset` field type only)
- depends_on: [Task_7, Task_8]
- description: |
  - EP8: `dataset` becomes a plain `String`. `DatasetId`, `DATASET_REGISTRY`, `DatasetKind` and the header's `dataset_kind` go, and each dataset has one name check.
  - EP7: delete `ingest.enrichment_path`, the `configured` parameter of `DatasetSpec::enrichment`, and the README section.
  - EP12: `ConfigError` in both dataset crates, `BaselineSurfacePolicyError` and `EnrichmentError` become message errors with the same text.
  - EP21 and ET18: re-check Assumption A3. If no manifest a committed config points to uses the string form, drop that form and require the hash for LoCoMo as well. If one does, stop and report.
- acceptance:
  - A config whose dataset mismatches its subcommand still fails before any side effect.
  - Every current config admits for its subcommand.
  - The archived configs the README names stay byte-identical and keep being rejected: `continuity_retrieval.toml`, `continuity_baseline_*.toml`, `continuity_binding_*.toml`, `continuity_task9_*.toml` and `continuity_task9b_*.toml`.
  - Evidence is a base-versus-tip admission census of every file under `configs/`, giving each config's result and, for a rejection, its reason. The expected historical failures are recorded as such, for example the retired `backend.namespace_prefix` in `continuity_retrieval.toml`. Tip matches base config by config.
  - No `enrichment_path` remains in code, tests or the README, and snapshot enrichment is unchanged.
  - Only the object manifest form with a hash is accepted. The report names the manifests checked.
  - Every check the removed typed errors carried still fails closed, with the same message.
  - The behavior-free proof lists only `dataset_kind`.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the behavior-free proof; the local manifest census; the base-versus-tip admission census of configs/"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; recursive config admission (reviewer.md hotspot) still rejects unknown keys at every level"

### Task_10: Rows, traces and reports carry each value once
- type: impl
- owns:
  - crates/cmem-eval-continuity/src/driver.rs
  - crates/cmem-eval-continuity/src/report.rs
  - crates/cmem-eval/src/results.rs
  - crates/cmem-eval/src/metrics.rs (the source of the token metrics only)
  - crates/cmem-eval/src/memory_adapter.rs
  - crates/cmem-eval/src/adapter.rs (the correction cascade mapping and its tests only)
  - crates/cmem-eval-runner/src/diff.rs
  - crates/cmem-eval-runner/src/pipeline.rs
  - README.md ("Read run artifacts" and "Compare runs" only)
- depends_on: [Task_9]
- description: |
  - EP9: a restart observation keeps the returned IDs, the recall before and after, and a stable flag.
  - EP10: delete the top-level `context_char_count` and `context_word_count`, and the unread full-history character and word counts. Token counts stay in the metrics map and leave the context block (planner-added; see the Decision Log).
  - EP18: delete the `entity_root_candidate_limit` tuning observation and its tests.
  - EP17: delete `CorrectionCascadePolicyInput` and `diff.rs`'s `normalize()`. Move `read_summary` and `read_continuity_traces` into the tests that use them.
  - ET21: fold the correction-forget assertion into the driver's library run.
- acceptance:
  - Restart stability is still asserted, and the report's restart count is unchanged.
  - Each value appears once in a row. The report still aggregates the retrieved-context and full-history token metrics.
  - `tuning_observations` is gone from the report, and the README describes the artifacts as built.
  - The behavior-free proof lists only the deleted fields.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the behavior-free proof"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause; README checked field by field against the serialized types"

### Task_11: The embedding binding is checked once, at load
- type: impl
- owns:
  - crates/cmem-eval/src/frozen_embedding.rs
  - crates/cmem-eval/src/adapter.rs
  - crates/cmem-eval/src/config.rs
  - crates/cmem-eval-runner/src/pipeline.rs
  - crates/cmem-eval-runner/src/frozen_embeddings.rs (consumer updates only)
  - crates/cmem-eval-continuity/src/bin/** (mechanical consumer updates only)
- depends_on: [Task_10]
- description: |
  - EP11: validate the frozen store once, at load. `FrozenEmbeddingProviders` becomes an `Option<FrozenEmbeddingProvider>`. Delete the branches for states the preflight already rules out, and check the model and size in one place.
  - EP15: the config part. Delete `backend.openai_api_key_env` and read `OPENAI_API_KEY`.
  - ET20: keep the custom-model test at the layer that keeps the check, and delete the other.
- acceptance:
  - Each store is parsed and validated once per run, and per-scenario hashing no longer re-serializes it. The header's store hashes are unchanged.
  - The model and vector size are checked in one place, and a mismatch still fails before an adapter is built.
  - The behavior-free proof shows no difference.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts; the behavior-free proof"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review with the reviewer evidence clause"

### Task_12: The evals test modules stop reading surfaces the library cleanup removes
- type: test
- owns:
  - crates/cmem-eval-runner/src/pipeline.rs (the `#[cfg(test)]` module only: the `configured_object_types` read in the vector-only restart test)
  - crates/cmem-eval/src/adapter.rs (the `#[cfg(test)]` module only: the two `configured_object_types` reads and the `CURRENT_SCHEMA_VERSION` uses at `:2773` and `:5355`)
- depends_on: [Task_11] (external: the library cleanup's final tip exists, for the second gate)
- description: |
  This task carries the library cleanup plan's section "Cross-repo dependency: the companion's test modules stop reading removed surfaces". Its three parts land here:
  - `configured_object_types` (LP3). The two per-kind split checks (`adapter.rs:2900`, `pipeline.rs:2513`) either observe some other way that each measured kind gets its own retrieval, or are deleted if the echo was all they tested. The check at `adapter.rs:3239` keeps its own contract, that a hybrid request excludes Entity: it observes that exclusion another way, or only its echo-only assertion goes. It is not asked to prove the per-kind split.
  - `CURRENT_SCHEMA_VERSION` becomes `DEFAULT_SCHEMA_VERSION` (LP13), at `adapter.rs:2773` and `:5355`.
  - `Settings::get_oxigraph_path` (LP14) is already gone through Task_8's ET4 deletion. This task only confirms that no use remains.
  The change compiles against both library pins, so it can land in either order relative to the library stack, as the library plan's dependency lines state (`depends_on: none`). It is the top level of this stack. The library cleanup stack does not merge until the orchestrator has pinned the companion checkout to this level and run its test gate against the library cleanup's final tip.
- acceptance:
  - No evals code uses `configured_object_types`, `CURRENT_SCHEMA_VERSION` or `Settings::get_oxigraph_path`.
  - The hybrid Entity-exclusion contract is still asserted.
  - At the pre-cleanup pin `c0ed9e21` and at the library cleanup's final tip, `cargo build` and `cargo test --workspace` pass, with counts and no exclusions. fmt and clippy also pass at both pins.
  - The diff is test-only. Proof: the diff itself.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "The three repository validation commands with counts at library c0ed9e21, and again at the library cleanup's final tip, the sibling checkout pinned by the orchestrator each time"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D diff review; the 3239 check keeps its Entity-exclusion contract"
  - kind: command
    required: true
    owner: orchestrator
    detail: "The library plan's merge gate: the companion checkout pinned to this level, cargo test against the library cleanup's final tip, with both commits recorded in this plan's and the library cleanup plan's Decision Logs"

## Task Waves (explicit parallel dispatch sets)

- Wave 1: [Task_1]. It starts on the integrated base: the obligations family committed and the stack top `18bc97a` forward-merged into it. The second worker stays free for the obligations family's review fixes.
- Wave 2 (parallel): [Task_2, Task_3, Task_4, Task_5, Task_6]. None of these touches an obligations file. `evals-worker` and `evals-worker2` take them in task order.
- Wave 3 (parallel): [Task_7, Task_8]
- Wave 4: [Task_9]
- Wave 5: [Task_10]
- Wave 6: [Task_11]
- Wave 7: [Task_12]. Its second gate runs once the library cleanup's final tip exists.

Each task is one pull request, and the pull requests form one linear stack on the obligations line. The plan branch `feature/2026-09-29/evals-value-audit-cleanup` starts at the integrated base, carries this plan, and is Task_1's level. Later tasks branch from the level below them, and branch names are set at dispatch.

Tasks that a wave runs in parallel are each developed from the wave's integrated parent, then stacked in task order, with each later one merging the earlier forward. Owns within a wave are disjoint, so these merges are clean. Each stacked task's behavior-free proof is taken against its final PR's integrated parent (see Tasks).

The decider merges the stack. Task_12 is linked like every other level, because it passes at both library pins. The CI state of each pull request follows the obligations line below it. The required evidence is the pinned local validation.

## Rollback / Safety
- Tasks 1 to 11: revert the task's pull request. Each task is behavior-free and self-contained.
- Removed config keys: a config naming one now fails admission loudly, and no committed config does.
- Task_12: while the library cleanup is unmerged, revert freely. Once it is merged, a revert breaks the evals test build against library main, so Task_12 is reverted only together with the library cleanup.
- Sealed evidence, the frozen stores and the protected fixtures are never written, and tasks that work near them check the hashes.

## Progress Log (append-only)

Append-only editing rule (applies to both logs below): when appending an entry, anchor the edit on the previous entry and reproduce it (or anchor on the section's tail marker) so the edit inserts rather than replaces, and verify afterward that the log grew.

- None yet.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-24 Decision: the evals value-audit cleanup is planned from ruling 77 (value-audit cleanup, 2026-09-24).
  - Trigger / new insight: the four value audits of 2026-09-24 and the orchestrator's verdicts on them, logged as ruling 77. This plan carries the Evals section, and the evals side of the library's LP3.
  - Plan delta:
    - Twelve tasks as listed.
    - The same day, ET14 was revised to KEEP. The benchmark runs deferred to the v0.2 closeout will still use `continuity_benchmarks_v2.json`, so no task removes it, its test or the `continuity_benchmarks_v1` entry in `frozen_embeddings.rs`.
  - Interpretations decided at drafting:
    - (1) PR 68 is open, not merged, so Task_1 carries the identical deletion.
    - (2) The retired partition test lives in `fixture.rs` and stays; the kept retired config key is `namespace_prefix` (Assumption A4).
    - (3) EP10's "single context block" holds for character and word counts. For token counts, the context block's copy is removed and the metrics map keeps them, because the report aggregates only the metrics map and the library's ADR-I-0022 context-size baseline reads them. This departs from the verdict's wording, not its intent.
    - (4) "Same stack position as the library change" means the evals level where the library pin crosses LP3. That is the top of this stack, so the other eleven tasks never wait on the library.
    - (5) The EP21 census was run ahead of time (Assumption A3), and Task_9 repeats it.
    - (6) EP17's cross-crate helper moves are done only where moving costs less than it removes.
  - Tradeoffs considered: see Design, Alternatives A to C.
  - User approval: the verdicts are the orchestrator's under ruling 77. Plan approval is pending.
  - Record proposed: none. This is a cleanup under existing records, and no decision here passes the ADR admission test.

- 2026-09-24 Decision: the plan is revised after evals-reviewer's REQUEST_CHANGES on draft `d72ea470` (`.agent-work/reviewer/evals-value-audit-cleanup-plan-review.md` in the evals repository), under the orchestrator's rulings on findings F1 to F6 and review notes 61 and 64.
  - Trigger / new insight: the review found five things:
    - Task_12 missed part of the library plan's dependency.
    - `diff` and `compare-continuity` cannot see context text, whole reports, the header, or keys removed before typed deserialization.
    - Task_9's owns could not remove `DatasetId` from the header.
    - Task_9's acceptance required the archived configs to admit, although they are invalid by design.
    - The integrated base already carries the embedding-input fix.
  - Plan delta (what changed):
    - F1:
      - Task_12 now owns the `CURRENT_SCHEMA_VERSION` to `DEFAULT_SCHEMA_VERSION` migration at `adapter.rs:2773` and `:5355`.
      - Its gate is build and test at both `c0ed9e21` and the library cleanup's final tip, reconciled with the library plan's dependency lines.
      - Task_8's ET4 deletion is recorded as satisfying the `get_oxigraph_path` part.
      - The `adapter.rs:3239` check keeps its hybrid Entity-exclusion contract; only an echo-only assertion may go.
      - The false risk note about LP13 and LP14 is corrected.
      - This supersedes interpretation (4) of the first entry. Task_12 no longer marks the level where the pin crosses LP3, and the planner-added "unlinked until LP3 is approved" requirement is dropped.
    - F2: every behavior-free proof keeps the CLI checks and adds a disposable structured comparison of the raw serialized rows or traces, header and report. It normalizes named provenance and timing fields only, lists allowed deletions, and records the field differences in the worker report before the captures are discarded.
    - F3: Task_9's owns cover the header's `dataset` type, its import and the header test constructor in `results.rs`.
    - F4: Task_9's acceptance is that every current config admits, while the archived configs the README names stay byte-identical and rejected. Evidence is a base-versus-tip admission census with the expected historical failures recorded.
    - F5: Task_2 keeps only `namespace_prefix` among the retired config keys.
    - F6: the `path_identity` edit is an orchestrator integration action for Task_7, and `reviewer.md` leaves the worker's owns.
    - Note 61:
      - Task_1 starts from the integrated base: obligations committed, and the stack top `18bc97a` forward-merged, carrying the fix through `499e87a`.
      - It reports the failures that actually remain there, keeps the scene-test reduction, the negative native checks and the pure write-outcome cases, and drops the inherited `fixture.rs` work and its owns.
      - Design Alternative A is replaced accordingly.
    - Note 64: each stacked task's proof base is its final PR's integrated parent, with the same library pin on both runs.
  - Tradeoffs considered: a permanent comparison tool was rejected. The scratch script leaves no product code, and the CLI checks stay for what they already cover.
  - User approval: rulings by the orchestrator on 2026-09-24. Plan approval is still pending.
  - Record proposed: none.

- 2026-09-29 Ruling for every behavior-free proof: a pair that produces no rows at both pins (the loud-topic fixture, feature-gated at both) cannot pass the CLI `diff`, which rightly refuses empty runs. For such a pair the CLI diff is recorded as not applicable, with the reason and the 0/0 row counts, and the raw structured comparison plus `compare-continuity` stand as its proof. Every other pair keeps the full CLI diff. The library's wall-clock `created_at`/`updated_at` on active threads and derived memories are named exact paths under the timestamp allowance; authored scene and elapsed times stay compared.

## Notes
- Risks:
  - The obligations family's commit or its review fixes may touch more files (Assumption A1).
  - The evals test modules do use surfaces that LP13 and LP14 remove: `CURRENT_SCHEMA_VERSION` at `adapter.rs:2773` and `:5355`, and `Settings::get_oxigraph_path` at `adapter.rs:3039` (Assumption A5). Task_12 migrates the first, and Task_8's ET4 deletion removes the second. No evals use of LP4 or LP7 surfaces was found. Task_12's gate at the library cleanup's final tip catches any other break, which is reported to the library plan's owner.
  - Tasks that turn typed errors into messages must keep the texts, because tests and readers match on substrings.
- Edge cases:
  - Task_7 deletes two Windows-only tests and one that spawns a process.
  - Task_4's switch to JSON only must keep the TOML-specific tests.
  - The situated runs at the obligations pin may still report some scenarios as not run. The proof compares outcomes, whatever they are.
