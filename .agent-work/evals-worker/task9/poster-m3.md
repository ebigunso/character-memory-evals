# M3: meeting someone known through 300 beliefs

Guarded final-pin evidence (2026-09-22): `poster-m3-checked63-a.json` and `poster-m3-checked63-b.json` use source commit `6e4509a1d66f8f25395e3021c038bd731ed80315` and library `63f176fa6801a8268bb00e337267a0f8c334a681`. Every run validates all 67 native write outcomes and all 366 expected vector-indexed objects before measuring. The input, measured probes and settled-matter payload match the earlier final-pin run.

Historical runs below did not inspect degraded-success write outcomes. Matching numbers do not retroactively establish that those historical stores were undegraded; use the guarded final-pin pair for the current result. Historical raw JSON files retain their original bytes and provenance.

Historical FINAL library `63f176fa6801a8268bb00e337267a0f8c334a681`, exact committed harness `0c7f57ed13bbda12fd3ab8382ea8d2b91d2dd453`: every a/b/control probe and the complete settled-matter payload are byte-identical to b470b10, including native traces. The only changed field in the complete raw JSON is `header.library_commit`. The AFTER column below therefore also gives the FINAL numbers: no-topic 12/12 fitting and 12/20 recall; with-topic 7/8 topic episodes, 12 person beliefs, 11/12 fitting; all four settled checks pass.

AFTER THE FIX: library `b470b102765f6a6c8900b692d192663ede1d697b`, committed harness `0c7f57ed13bbda12fd3ab8382ea8d2b91d2dd453`, generator SHA-256 `821c12602b2db3157770a6dfd41585bd60a1f7b7351b13ee38c61145c1fc0044`. No API adjustment was needed. Unsealed synthetic measurement, 2026-09-21.

Historical comparison: before `db40e11b1d87654ad6254e93253a0035ff2768f9`; intermediate `b1eaaf89aa3c0a2ca41e30fd9e7220147c7550f3`. Those a/b runs use harness `db19c58327725f3ccf6a36a154a73b6a5e242796`, generator SHA-256 `6641bd2850033c332e7bec01f0e961aaf8da6b2959fb42fa83e5373aa03e170d`. Every raw run retains its own provenance.

| Probe and metric | Before: db40e11 | Intermediate: b1eaaf8 | AFTER: b470b10 |
|---|---:|---:|---:|
| Meeting, no topic: pack memories | 13 | 13 | 13 |
| Meeting, no topic: fitting / returned beliefs | 12/12 (100%) | 12/12 (100%) | 12/12 (100%) |
| Meeting, no topic: fitting / person memories | 12/13 (92.3%) | 12/13 (92.3%) | 12/13 (92.3%) |
| Meeting, no topic: fitting recall | 12/20 (60%) | 12/20 (60%) | 12/20 (60%) |
| Meeting, unrelated topic: on-topic episodes | 7/8 | 7/8 | 7/8 |
| Meeting, unrelated topic: person beliefs | 12 | 12 | 12 |
| Meeting, unrelated topic: fitting / returned beliefs | 11/12 (91.7%) | 7/12 (58.3%) | 11/12 (91.7%) |
| Meeting, unrelated topic: fitting / person memories | 11/13 (84.6%) | 7/13 (53.8%) | 11/13 (84.6%) |
| Meeting, unrelated topic: fitting recall | 11/20 (55%) | 7/20 (35%) | 11/20 (55%) |
| Topic-only control: on-topic episodes | 8/8 | 8/8 | 8/8 |
| Topic-only control: person beliefs | 12 | 12 | 12 |
| Topic-only control: fitting / returned beliefs | 1/12 (8.3%) | 1/12 (8.3%) | 1/12 (8.3%) |

The final fix restores fitting beliefs with the unrelated topic from 7/12 to 11/12 (58.3% to 91.7%), while keeping 7/8 topic episodes and 12 person beliefs. All three a/b/control probes return exactly the same memory identities and order as the db40e11 baseline. The meeting-with-topic trace returns to 11 participant-cued beliefs plus one topic-cued belief, from six plus six at b1eaaf8. The remaining topic-cued belief is outside the fitting set; its admission is not evidence of semantic topic relevance.

At all three pins the no-topic pack contains 12 beliefs and one experience (13 total); the meeting-with-topic pack contains 12 beliefs, one experience and seven topic episodes (20 total). Belief precision counts returned person beliefs. Person-memory precision additionally counts the experience. Gold contains only the 20 authored fitting beliefs, without an independent judgment that the experience is irrelevant.

Conditions: 300 current application-given Reflection beliefs about one keyed person, salience 0.10–0.99; 20 high-salience fitting beliefs (10 recent); 40 experiences over a year; eight unrelated-topic episodes graded cosine 0.9–0.6; 16 background episodes about another person; deterministic 4-D embeddings; ordinary public typed writes. Defaults: floors of one, 48 vector candidates, 12 graph roots, 12 derived-memory slots and eight episode slots. Gold is scoring-only. This small synthetic store measures bounded retrieval, not human or LLM conversation quality.

Four settled-matter checks (c). The earlier section checks were at `60d9a0b8bfd3f4d9c8e5de060278c2e333f5ddba` and `cffee2b472c72ae9cc7f25c6d12e9bbd15db0425`; the AFTER column uses the same final b470b10 library as a/b. All three section-aware runs use committed harness 0c7f57e and identical generated input/config.

| Settled check | 60d9a0b | cffee2b | AFTER: b470b10 |
|---|---|---|---|
| Resolving memory present on meeting | Yes, derived_memories | Yes, derived_memories | Yes, derived_memories |
| Settled old loop: meeting state / final pack | Out of direct state; returned in open_loops with resolved_by | Out of state; absent from pack | Out of state; absent from pack |
| Topic recalls old loop marked resolved_by | Yes, open_loops | Yes, derived_memories | Yes, derived_memories |
| Truly open pre-resolution control keeps its slot | Yes, open_loops rank 1 | Yes, open_loops rank 1 | Yes, open_loops rank 1 |

At b470b10 all four checks pass. The topic result is marked resolved_by: notebook-returned. The meeting trace sends the old loop toward derived_memories, then omits it at that section’s 12-slot cap; the public lifecycle list also names the exact old loop with resolved_omitted. Its raw meeting-marker boolean is false because the object is absent, so a meeting marker is N/A. The final settled meeting pack contains the resolver, 11 beliefs and one experience (13 total). At 60d9a0b the loop occupied an additional open_loops slot (14 total) and readmission reconciliation removed its lifecycle omission entry.

The truly-open control is the same loop before resolution, not an additional unresolved loop in the settled pack. The loop is dated 2025-08-01; its later resolving Reflection is dated 2025-08-31; both concern Iris and use an ordinary public RESOLVES write. Trace evidence shows the direct ABOUT state path excludes the settled loop, while the resolving memory links to it by RESOLVES at proximity 2.

Reproducibility: final whole-input SHA-256 `51e8547982f3c4aff468e96ab306467f2704afb32975ed051825ff670cf637e5`; config SHA-256 `e189f580b69e67c0a50dfc0642359ad2602d9e423bfdbd3e688c916da6a1a5b7`. Original a/b input SHA-256 `d18606733053a54e43d2540f6475df11f3ab548478b592cd095b9e9305244f97` differs because --settled adds embedding entries and later c writes; all original a/b episode, graph, probe, gold and existing embedding data are unchanged. The committed runner performs a/b before those resolution writes.

Each pin repeats byte for byte. Raw SHA-256: db40e11 `da5a1daf49c6848681f2058b1b5c2eb5e27ea52f1b6b5b1b5bbde3deab4b6bd8`; b1eaaf8 `21cb5fdd2e9a0d85a75fbc3d9eeaac518385b289e24566e706b2fec8fc40ad86`; final b470b10 `021eeb28f71e162d870c6f5e8fa2aa827d0701c96615e38fdf9deaafff025b54`. Final full a/b/c evidence is under `after_fix` in `poster-m3.json`; prior raw runs and section comparisons remain preserved. The complete notebook and its independent repeat are byte-identical. All measurement stores are removed.

Validation at b470b10: fmt, workspace clippy, six protected hashes and both complete M3 repetitions pass. Across completed validation, 386 tests pass and two fail (plus the scoped OS 1314 exclusion). The continuity library contributes 123 passes and two failures; its five binary tests and all 258 remaining workspace tests pass. All doc-test targets pass with zero examples. These are pin-induced harness migration failures, separate from the measured results:

- `driver::tests::scene_slice_preserves_authored_input_and_checks_native_results`: the native write now embeds standalone scene words; the old test fixture has no assignment for `Quiet observatory`.
- `driver::tests::situated_writes_reject_degraded_native_outcomes`: removing the old composite `Garden` / `Setting: Glass room` / `With: Guest` input no longer degrades a write, because the library now embeds separate content and scene surfaces; the test therefore receives success where it expects an error.

At b470b10, `src/policy/embedding_surface.rs:28` builds separate setting/participant surfaces and `:55` adds them to episode records; `src/usecases/remember.rs:78` embeds those records. At 2026-09-21 11:24:04 UTC, the orchestrator explicitly directed that these tests remain unchanged today and that the scenario-driver migration be deferred to Task_5. This is not a clean workspace-test result. The sole general test exclusion remains the expressly waived Windows OS 1314 symlink test. Logs: `clippy-b470b10.log`, `workspace-b470b10.log`, `fixed-a.log`, `fixed-b.log`. Prior pins passed 388 workspace tests and doc targets (zero examples).

Reproduce: temporarily bind the workspace character-memory dependency to the isolated checked library at C:/w/cm-poster, then run `cargo run -p cmem-eval-continuity --bin measure_person_state -- <new-report.json> <library-checkout> --settled` twice and compare full JSON bytes. Outputs refuse overwrite. Restore the local manifest after verification.

Historical db40e11-to-b1eaaf8 slot attribution remains in `poster-m3-attribution.md`; the final fix restores the baseline selected identities in all three a/b/control probes.

Supplemental validation logs: `remaining-workspace-b470b10.log`, `bins-b470b10.log`, `doc-b470b10.log`. The local manifest is restored after verification.

Final 63f176f verification: both complete runs are byte-identical, raw SHA-256 `f73cf9f34f814fd722900e43bf3aee39b957f291fb46b57e43f37f50adbb6a24`. Source, input and config hashes match b470b10. fmt, workspace clippy and all six protected hashes pass; stores are removed. Full final evidence is under `final` in `poster-m3.json`. Logs: `clippy-final63.log`, `final63-a.log`, `final63-b.log`. The workspace suite was not rerun for this final measurement-only request; the previously documented two pin-induced harness migration failures remain deferred to Task_5.
