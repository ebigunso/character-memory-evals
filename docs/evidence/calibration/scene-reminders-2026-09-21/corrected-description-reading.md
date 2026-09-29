# Corrected description calibration

Captured and promoted 2026-09-22 UTC. Unsealed calibration snapshot, superseding the degenerate-probe interpretation in [the earlier reading](after-the-fix.md). The original evidence remains preserved.

The corrected comparison preserves the topic-retention result, **3/8 before to 6/8 after**, but withdraws the prior claim that the old library failed latest-N or same-day selection and the fix repaired those failures. Corrected BEFORE already selects the latest N in every keyless row and returns all four authored same-day occasions in the default same-day probe. AFTER returns one occasion, which is one of those four. The measured change is less scene-contributed material alongside stronger topic retention; it is not evidence of a newly successful temporal predicate.

## Paired evidence

All three pins were measured twice, with both original and native-ID-opposed inputs, at the identical frozen harness source `0d057daef9c32325d12a88c4f8c88ce322a19a11`. Configuration and all generated-input hashes match across pins. Generator SHA-256 is `2a71c92218b70e86e450c1310137aa9b3e445cd02c5d013cdefbef797930d5b9`. Each A/B pair is byte-identical; the orders and repeats are controls, not independent observations.

| Library | Reports | JSON SHA-256 |
|---|---|---|
| BEFORE `979643f` | [native](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-before-native.json.gz) / [repeat](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-before-repeat.json.gz) | `60646b486d09b7c505b31c4a42f2b2142a4dcfdd3dee2f970c6519e4aa83f487` |
| AFTER `63f176f` | [native](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-after-native.json.gz) / [repeat](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-after-repeat.json.gz) | `7c940adad5860b5f025a5e268003a6edb60549f44cd0ceaba4d9b51497715ec0` |
| Trace `4cf9763` | [native](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-trace-native.json.gz) / [repeat](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-trace-repeat.json.gz) | `b09ed8b25f5a49a99ca2bd447b0f8ab365cc74155a3cf5d30f679fe5cf81492b` |

The [complete machine-readable table](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-description-table.json) is the complete 515-row table: 105 structural controls, 320 exact/reworded rows and 90 keyless rows. Every row includes all three corrected pins, historical before/after numbers, scene-contributed occasion IDs and native times, changed BEFORE fields and an explicit prior-conclusion impact. It retains every swept floor (0, 1, 2, 3, 5) and both ID orders. The compact table below uses default floors of one; counts agree between ID orders. Individual identities remain in the JSON.

## Default-floor table

Cells give **topic targets kept / 8; scene-contributed episode occasions**. Scene occasions are selected native episodes bearing place or participant cue credit, counted once even when both kinds name the same episode. They are not necessarily the whole episode section.

| Row | BEFORE 979643f | AFTER 63f176f | Trace 4cf9763 | Effect of correcting BEFORE |
|---|---|---|---|---|
| Exact-match control, place | 3/8; 5 | 6/8; 1 | 6/8; 1 | Numeric conclusion unchanged; exact-match scope only |
| Exact-match control, participant | 3/8; 5 | 6/8; 1 | 6/8; 1 | Numeric conclusion unchanged; exact-match scope only |
| Exact-match control, both | 3/8; 5 | 6/8; 1 | 6/8; 1 | Numeric conclusion unchanged; exact-match scope only |
| Corrected rewording, place | 3/8; 5 | 6/8; 1 | 6/8; 1 | Retention and reminder-volume conclusions unchanged; withdraw the old agreement-based paraphrase claim |
| Corrected rewording, participant | 3/8; 5 | 6/8; 1 | 6/8; 1 | Same limited retention conclusion; old paraphrase claim withdrawn |
| Corrected rewording, both | 3/8; 5 | 6/8; 1 | 6/8; 1 | Same limited retention conclusion; old paraphrase claim withdrawn |
| Keyless topic plus scene | 3/8; 5 | 6/8; 1 | 6/8; 1 | Latest-N already true BEFORE, so withdraw the old failure/fix interpretation |
| Keyless scene only | 0/8; 8 | 0/8; 1 | 0/8; 1 | Latest-N already true BEFORE; smaller reminder volume remains measured |
| Keyless same-day descriptions | 0/8; 8 | 0/8; 1 | 0/8; 1 | Latest-N already true; same-day changes 4/4 to 1/4 because eight returned occasions become one |
| Stranger participant with topic | 3/8; 5 | 6/8; 1 | 6/8; 1 | Unchanged by geometry repair; a false reminder cost remains |
| Stranger participant, scene only | 0/8; 8 | 0/8; 1 | 0/8; 1 | Unchanged by geometry repair; AFTER still returns one occasion |
| Stranger place with topic | 3/8; 5 | 6/8; 1 | 6/8; 1 | Unchanged by geometry repair; a false reminder cost remains |
| Stranger place, scene only | 0/8; 8 | 0/8; 1 | 0/8; 1 | Unchanged by geometry repair; AFTER still returns one occasion |

For all three keyless default rows, latest-N is true at all three corrected pins and both ID orders. In the same-day row, BEFORE includes four same-day and four other-day occasions; AFTER and trace include one same-day occasion and no other-day occasion. The available four and latest-N expectation come from authored dates; returned dates are native and checked against the authored instants. Unreturned persisted scenes have no independent public census here. The corrected BEFORE success can be explained by authored vector ranking aligned with episode index/time; it does not establish a time or date-match route in the old library.

The orthogonal-control result remains a **STRUCTURAL CONTROL**. Same-pin audits preserve that family exactly, as well as the exact-match families, all forty reworded stranger rows and topic-only controls. These cases retain their own scope and do not support a claim about freely worded scene perception.

## Geometry and withdrawn interpretation

The original held-out query shared the authored base of one stored description. Separate text noise of `1e-6` left an effectively exact anchor; native best score was 1.0. A regression test required that anchor. The corrected query has a different base and now differs from every stored wording and normalized episode surface. All 102 emitted pairs are recorded and checked, with description similarities about 0.90, 0.957 and 0.973 and normalized-episode similarities about 0.937 to 0.951. Stored normalized episode geometry remains indexed independently of wording, holding the other pressure controls fixed.

At trace pin `4cf9763`, native best fetched scene-surface scores are 0.9733244776725769 and 0.9733250141143799 for the two reworded searches, versus 0.009999990463256836 and 0.009999999776482582 for the two stranger searches. These are pre-selection best fetched scores, not probabilities or thresholds. The two searches per class are the sample; ID orders and exact repeats do not multiply it. The separate 56-pair authored diagnostic is not paired with these native searches and cannot validate them. Neither is a production embedder experiment.

The full same-pin repair audit changes 210 detailed reworded/keyless rows at each pin. At BEFORE, all 90 keyless rows change numeric latest-N and/or same-day readings; at AFTER and trace, those numeric readings do not change. The old archives are preserved verbatim. The old agreement-based paraphrase robustness claim, freely worded interpretation of the old same-day result, and old-library latest-N/same-day failure or fixed-by-AFTER claims are explicitly withdrawn.

The three pairs pass repeat, native-time, native-ID-order, geometry and protected-input audits, leave no stores and report no bounded graph failures. The [manifest](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-description-manifest.json) records all original and archive hashes. The corrected archives are promoted for citation; independent review of this correction remains pending. Integrity and same-pin geometry audits are retained as `corrected-{before,after,trace}-{integrity,geometry}-audit.json`. The separate TIME extension is not substituted into this frozen-source comparison.

## Restore and reproduce

Verify each compressed SHA-256 and the SHA-256 of its decompressed bytes against [the manifest](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/corrected-description-manifest.json). The native/repeat archives for each pin decompress to identical bytes. The raw JSON retains its original source revision and capture data; promotion does not change those bytes.

Use an isolated harness checkout at `0d057daef9c32325d12a88c4f8c88ce322a19a11` and an isolated sibling `CharacterMemory`, with the [repository prerequisites](../../../../README.md). Pin the library in turn to `979643f662013a839f428451209cac70c3b42062`, `63f176fa6801a8268bb00e337267a0f8c334a681`, and `4cf97631c5f0f8268a26f66b7fc0c819ae7e783e`. At each pin, run twice with fresh output paths, using the debug profile recorded in the archives:

```sh
cargo run --offline -p cmem-eval-continuity --bin calibrate_cue_floors -- .agent-work/calibration/corrected-a.json
cargo run --offline -p cmem-eval-continuity --bin calibrate_cue_floors -- .agent-work/calibration/corrected-b.json
git diff --no-index -- .agent-work/calibration/corrected-a.json .agent-work/calibration/corrected-b.json
```

Create the parent output directory first. Do not repin a checkout held by another task. Exact archived bytes are the retained evidence; changed source revisions, profiles or source-file line endings can change headers or source hashes even when the authored geometry is equivalent.
