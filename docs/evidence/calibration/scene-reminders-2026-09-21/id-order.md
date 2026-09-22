# Native-ID order control

Captured 2026-09-21; promoted 2026-09-22. Unsealed calibration snapshot. See [README](README.md) for archives, original hashes, reproduction, and the scope of the evidence.

External labels originally ascend with authored time, but native UUID order is nonmonotonic. The new variants prepare all 48 native IDs through the public adapter without committing planning writes, then make native ID order strictly oppose time. The audit verifies every nonidentity input is preserved and checks prepared IDs against the observed native traces.

TOPIC scores are not one 48-way tie. The original merged scene/body embeddings had graded topic similarity. After scene separation, concept-seeded discrete noise creates several exactly tied subsets near zero; episode/observation companions can also tie. The comparison JSON records score-frequency distributions for retained shared episode candidates, not a complete pre-limit census. The unchanged result under opposed native IDs rules out ascending ID/time order as the explanation for these families.

All 270 original BEFORE rows and their controls exactly match the approved 8f49ddb capture. Their original writes/probes and embedding assignments are unchanged; two new unused bindings and four extra probes were added. Across those common rows, b470b10 and 63f176f have identical target counts, pack counts, occasion/latest/day counts and orthogonal exclusive-cue counts. Raw traces and scores remain available; no conclusion about unmeasured cases is implied.

## Results in both orders

| Family / pressure | BEFORE 979643f | INTERMEDIATE b470b10 | AFTER 63f176f |
|---|---:|---:|---:|
| identical / place | 3/8 | 6/8 | 6/8 |
| identical / participant | 3/8 | 6/8 | 6/8 |
| identical / both | 3/8 | 6/8 | 6/8 |
| reworded / place | 3/8 | 6/8 | 6/8 |
| reworded / participant | 3/8 | 6/8 | 6/8 |
| reworded / both | 3/8 | 6/8 | 6/8 |
| opposed-identical / place | 3/8 | not run | 6/8 |
| opposed-identical / participant | 3/8 | not run | 6/8 |
| opposed-identical / both | 3/8 | not run | 6/8 |
| opposed-reworded / place | 3/8 | not run | 6/8 |
| opposed-reworded / participant | 3/8 | not run | 6/8 |
| opposed-reworded / both | 3/8 | not run | 6/8 |
| identical topic-alone | 6/8 | 6/8 | 6/8 |
| reworded topic-alone | 6/8 | 6/8 | 6/8 |
| keyless topic-alone | 6/8 | 6/8 | 6/8 |
| opposed-identical topic-alone | 6/8 | not run | 6/8 |
| opposed-reworded topic-alone | 6/8 | not run | 6/8 |
| opposed-keyless topic-alone | 6/8 | not run | 6/8 |

Counts are the eight authored target episode objects; companion observations are separate slots. Original order means external labels ascend with time, while native UUID order is nonmonotonic. Opposed order makes native shared episode IDs strictly decrease with authored time; every nonidentity input is preserved. New probes/opposed variants were not run at b470b10; their intermediate entries are unavailable.

| Keyless family / probe / measure | BEFORE 979643f | INTERMEDIATE b470b10 | AFTER 63f176f |
|---|---:|---:|---:|
| keyless / topic-and-scene / native occasions | 5 | 1 | 1 |
| keyless / topic-and-scene / latest N expected by AUTHORED times? | no | yes | yes |
| keyless / topic-and-scene / native same-day / AUTHORED available | 0/0 | 0/0 | 0/0 |
| keyless / topic-and-scene / topic targets | 3/8 | 6/8 | 6/8 |
| keyless / scene-only / native occasions | 8 | 1 | 1 |
| keyless / scene-only / latest N expected by AUTHORED times? | no | yes | yes |
| keyless / scene-only / native same-day / AUTHORED available | 0/0 | 0/0 | 0/0 |
| keyless / scene-only / topic targets | 0/8 | 0/8 | 0/8 |
| keyless / same-day-by-descriptions / native occasions | 8 | 1 | 1 |
| keyless / same-day-by-descriptions / latest N expected by AUTHORED times? | no | yes | yes |
| keyless / same-day-by-descriptions / native same-day / AUTHORED available | 0/4 | 1/4 | 1/4 |
| keyless / same-day-by-descriptions / topic targets | 0/8 | 0/8 | 0/8 |
| opposed-keyless / topic-and-scene / native occasions | 5 | not run | 1 |
| opposed-keyless / topic-and-scene / latest N expected by AUTHORED times? | no | not run | yes |
| opposed-keyless / topic-and-scene / native same-day / AUTHORED available | 0/0 | not run | 0/0 |
| opposed-keyless / topic-and-scene / topic targets | 3/8 | not run | 6/8 |
| opposed-keyless / scene-only / native occasions | 8 | not run | 1 |
| opposed-keyless / scene-only / latest N expected by AUTHORED times? | no | not run | yes |
| opposed-keyless / scene-only / native same-day / AUTHORED available | 0/0 | not run | 0/0 |
| opposed-keyless / scene-only / topic targets | 0/8 | not run | 0/8 |
| opposed-keyless / same-day-by-descriptions / native occasions | 8 | not run | 1 |
| opposed-keyless / same-day-by-descriptions / latest N expected by AUTHORED times? | no | not run | yes |
| opposed-keyless / same-day-by-descriptions / native same-day / AUTHORED available | 0/4 | not run | 1/4 |
| opposed-keyless / same-day-by-descriptions / topic targets | 0/8 | not run | 0/8 |

Latest-N and same-day availability are expectations from AUTHORED scene times. Returned scenes/counts use native recorded values and every returned time matches its authored value. No public all-scenes census is captured; persisted times of unreturned occasions are not independently verified. Keyed and keyless chronology differ, so this is not an isolated causal estimate of removing keys.

The native mappings remain in `opposed_input` in each restored native JSON. The [BEFORE audit](before-the-fix-native-audit.json) and [AFTER audit](after-the-fix-native-audit.json) retain strict-order and nonidentity-input checks. Retained shared-episode topic-score distributions are in `retained_shared_episode_topic_scores` in [the comparison JSON](after-the-fix.json.gz). These distributions describe retained candidates, not a full pre-limit score census.
