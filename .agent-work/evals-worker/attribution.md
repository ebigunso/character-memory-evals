# Task_5 step 3 evidence attribution

Source is `499e87a8cb58389bf5a5ac7218d1dd0c06a5915f`, based on the authorized `8f49ddb1dc02d2c01b9e28a120e0da636983f2f3`. All current runs use CharacterMemory `63f176fa6801a8268bb00e337267a0f8c334a681`. This is a local pinned-library validation; CI follows library main.

## Controls and attribution

`prior-smoke` and `prior-narrative` preserve the previous Task5 evidence from the situated-cues worktree, `.agent-work/worker2/final-evidence/{smoke,narrative}-a`: harness `e15500eb5ae39ec1d18f255db3ca82247b7ccc66`, library `9ff86d6d7d84cd528893394b8f1ce4470188d3c9`. `base-*` was run before this change at 8f49ddb/63f176f; the final `*-a` and `*-b` runs use 499e87a/63f176f. Headers preserve exact hashes and run provenance. Input, configuration and embedding bindings are identical across all three controls for each dataset.

Between e15500e and 8f49ddb, the executed runner, adapter and continuity runtime source files are unchanged. The only crate changes are calibration source, its README and moving Tokio from dev-dependencies to dependencies for that binary; the workspace manifest and lockfile are unchanged. Thus non-clock prior-to-base retrieval differences are attributed to the library pin. This controls the harness implementation, but does not isolate individual commits within the library range.

`evidence-differences.json` records every differing header, trace and report field with before/after values and its attribution. `attribution.json` counts those classifications. `compare-evidence.cjs` reproduces them and fails on an unexplained harness/repeat difference, a changed protected hash, changed census status/assertion counts, an unexpected source-control difference, or retained run stores.

| Comparison | Trace: library | Trace: runtime timing | Report: harness feature gates | Report: runtime timing |
| --- | ---: | ---: | ---: | ---: |
| Smoke prior to base | 8 | 21 | 0 | 4 |
| Narrative prior to base | 507 | 41 | 0 | 4 |
| Smoke base to final | 0 | 21 | 0 | 4 |
| Narrative base to final | 0 | 39 | 54 | 2 |
| Smoke final repeat | 0 | 21 | 0 | 4 |
| Narrative final repeat | 0 | 41 | 0 | 4 |

The smoke library differences add eight empty `resolved_by` arrays. Narrative library differences add native state facts and change pack sections, retrieved identities/order/scores, context text, and selection/graph traces. These are field-diff counts, including array-index shifts, not counts of independent behavioral changes. Aggregate scenario outcomes remain unchanged.

Runtime timing covers only query latency, four aggregate latency fields, and native `created_at`/`updated_at` fields under `pack.active_threads[]` or `pack.derived_memories[].memory`. Inspection confirms the differing memory timestamps belong to runtime-created naming claims (plus legacy smoke reflection/thread construction). Authored episode, scene and derived-memory chronology is not removed. Header differences are the documented source/library pins, generation time, storage path and path hash.

Both repeat CLI diffs report zero identity, rank, metric or degradation differences (smoke: one query; narrative: seven). Exhaustive comparison also finds zero non-clock repeat trace/report differences. Raw files are not byte-identical because they retain timing and run provenance. The byte-preserving evidence attributes retain those original distinctions.

## Census and native contract limits

Before and after: 15 scenarios, three passed, one failed (`d13-my-day`, eight failed assertions), eleven not run. Removing `resolution_omission` from `d1-d8-morning-deadline`, and `elapsed_since_met` from `d11-c6-reunion-and-departure`, `d4-library-door`, and `tasks-and-favors-after-a-year` changes only missing-feature lists and associated not-run explanation strings. Their other missing features remain, so none becomes executable. All authored fixtures are unchanged and the six protected hashes match.

Elapsed assertions read native `last_interactions` through the object registry and compare the fact and authored gold duration; drift checks alter the native seconds/time or remove the native fact. Resolution assertions require actual whole-pack absence and native `ResolvedOmitted`. The settled-matter live test preserves the native resolver on admitted history and proves that provenance-carried resolved history still fails a global omission assertion. It uses a derived-to-derived resolution link, as required by this library pin.

At 63f176f the native trace has `scene_cue_omitted_counts`, but no best scene-surface score field. Description content cues do not provide the identity-resolution statuses required by the authored `DescriptionReferenceResolution` assertions; that feature stays gated. D13 remains an honest time-route failure. The smoke/narrative census does not itself establish coverage of newly supported state facts; focused native drift tests supply that evidence.

## Validation and integration

Each of fmt, all-target workspace Clippy with warnings denied, and workspace tests passed twice. Each test pass contains 389 passing tests and one filtered test: the explicit OS1314 waiver for `commands::pipeline::tests::output_leaf_links_are_rejected_before_writing_artifacts`. Doc tests are included. The initial full-suite calibration failure is retained in `test-before-calibration-fix.log`; the shared inventory caller was migrated before both final passes. No historical calibration measurements were regenerated.

The Cargo dependency path was temporarily directed to the pinned sibling for local builds, then restored byte-for-byte. Process-local safe-directory settings were used; no global Git configuration was changed. Run stores were cleaned and the sibling pin was never moved. The Worker report lists the exact commands, artifacts and remaining Reviewer-owned Tier D review.
