# M3 attribution: the twelve belief slots with an unrelated topic

Before library `db40e11b1d87654ad6254e93253a0035ff2768f9`; after library `b1eaaf89aa3c0a2ca41e30fd9e7220147c7550f3`; harness `db19c58327725f3ccf6a36a154a73b6a5e242796`; generator SHA-256 `6641bd2850033c332e7bec01f0e961aaf8da6b2959fb42fa83e5373aa03e170d`; input SHA-256 `d18606733053a54e43d2540f6475df11f3ab548478b592cd095b9e9305244f97`; config SHA-256 `e189f580b69e67c0a50dfc0642359ad2602d9e423bfdbd3e688c916da6a1a5b7`. Probe: `meeting-with-topic`, same small synthetic store, default caps, deterministic embedding. Both runs repeat byte for byte. Source: `poster-m3-before.json` and `poster-m3-after-a.json`.

The native section selection changes from eleven Participant beliefs and one Topic belief to six Participant beliefs and six Topic beliefs. Five beliefs leave, all fitting; five enter, one fitting. This explains the net fitting count 11 -> 7 without changing the twelve-slot count. Every entering belief carries exactly {topic}, and every entering belief is credited by native section-level `floor_admissions` to Topic. Their native cue scores are approximately 1.5e-12–2e-12, so these are weak topic admissions in controlled geometry.

Native trace supplies slot rank, cue-kind set, section assignment reason, score components and floor credits. Salience below comes from native scores and was checked against the exact public write input. Created_at comes from that hashed write input, forwarded through DerivedMemoryInput.created_at to DerivedMemoryDraft.created_at; the persisted stores were cleaned and the saved native trace does not expose timestamps. All timestamps are 12:00:00 UTC, so the tables abbreviate them to the date. Recency rank is 1 + the number of the 300 beliefs with a strictly newer created_at; timestamp ties share a rank. State rank sorts the same 300 beliefs by descending salience, then descending created_at, then ID for deterministic ties. All saliences are distinct here: recency does not break any salience tie, so this run cannot isolate the recency tie-break rule.

Before, the returned beliefs are in descending salience order, but their selected set already differs from the global top twelve because of one Topic admission. After, both the selected set and the returned order differ from salience-then-recency: the returned global state ranks are 1, 2, 3, 4, 5, 6, 35, 78, 19, 26, 48, 156 (before: 1 through 11, then 35). The Participant-selected prefix itself remains the first six beliefs in salience order. For example, entering belief-222 (salience 0.544, state rank 78) precedes entering belief-298 (salience 0.900, state rank 19), while fitting belief-286 (salience 0.960, state rank 7, recency rank 7) is omitted. A newer-created_at explanation cannot account for these choices.

## All twelve slots, side by side

`floor=none` means no native section floor_admissions entry, not an inferred alternative cause. The native floor record includes spare cue turns. The final pack is score-ordered after admission; the side-by-side rows align output slots, not one-to-one causal evictions.

| Slot | Before: db40e11 | After: b1eaaf8 |
|---:|---|---|
| 1 | `belief-280`; gold=yes; salience=0.990; created=2025-08-31 (recency rank 1); state rank=1; cues={participant}; floor=none; score=0.711499929 | `belief-280`; gold=yes; salience=0.990; created=2025-08-31 (recency rank 1); state rank=1; cues={participant}; floor=none; score=0.711499929 |
| 2 | `belief-281`; gold=yes; salience=0.985; created=2024-10-25 (recency rank 291); state rank=2; cues={participant}; floor=none; score=0.710999966 | `belief-281`; gold=yes; salience=0.985; created=2024-10-25 (recency rank 291); state rank=2; cues={participant}; floor=none; score=0.710999966 |
| 3 | `belief-282`; gold=yes; salience=0.980; created=2025-08-29 (recency rank 3); state rank=3; cues={participant}; floor=none; score=0.710499942 | `belief-282`; gold=yes; salience=0.980; created=2025-08-29 (recency rank 3); state rank=3; cues={participant}; floor=none; score=0.710499942 |
| 4 | `belief-283`; gold=yes; salience=0.975; created=2024-10-23 (recency rank 292); state rank=4; cues={participant}; floor=none; score=0.709999979 | `belief-283`; gold=yes; salience=0.975; created=2024-10-23 (recency rank 292); state rank=4; cues={participant}; floor=none; score=0.709999979 |
| 5 | `belief-284`; gold=yes; salience=0.970; created=2025-08-27 (recency rank 5); state rank=5; cues={participant}; floor=none; score=0.709499955 | `belief-284`; gold=yes; salience=0.970; created=2025-08-27 (recency rank 5); state rank=5; cues={participant}; floor=none; score=0.709499955 |
| 6 | `belief-285`; gold=yes; salience=0.965; created=2024-10-21 (recency rank 293); state rank=6; cues={participant}; floor=none; score=0.708999932 | `belief-285`; gold=yes; salience=0.965; created=2024-10-21 (recency rank 293); state rank=6; cues={participant}; floor=none; score=0.708999932 |
| 7 | `belief-286`; gold=yes; salience=0.960; created=2025-08-25 (recency rank 7); state rank=7; cues={participant}; floor=none; score=0.708499968 | `belief-265`; gold=no; salience=0.630; created=2024-11-10 (recency rank 276); state rank=35; cues={topic}; floor=topic; score=0.313000023 |
| 8 | `belief-287`; gold=yes; salience=0.955; created=2024-10-19 (recency rank 294); state rank=8; cues={participant}; floor=none; score=0.707999945 | `belief-222`; gold=no; salience=0.544; created=2024-12-23 (recency rank 233); state rank=78; cues={topic}; floor=topic; score=0.304399997 |
| 9 | `belief-288`; gold=yes; salience=0.950; created=2025-08-23 (recency rank 9); state rank=9; cues={participant}; floor=none; score=0.707499981 | `belief-298`; gold=yes; salience=0.900; created=2025-08-23 (recency rank 9); state rank=19; cues={topic}; floor=topic; score=0.173333347 |
| 10 | `belief-289`; gold=yes; salience=0.945; created=2024-10-17 (recency rank 295); state rank=10; cues={participant}; floor=none; score=0.706999958 | `belief-274`; gold=no; salience=0.648; created=2024-11-01 (recency rank 285); state rank=26; cues={topic}; floor=topic; score=0.148133337 |
| 11 | `belief-290`; gold=yes; salience=0.940; created=2025-08-31 (recency rank 1); state rank=11; cues={participant}; floor=none; score=0.706499934 | `belief-252`; gold=no; salience=0.604; created=2024-11-23 (recency rank 263); state rank=48; cues={topic}; floor=topic; score=0.143733338 |
| 12 | `belief-265`; gold=no; salience=0.630; created=2024-11-10 (recency rank 276); state rank=35; cues={topic}; floor=topic; score=0.313000023 | `belief-144`; gold=no; salience=0.388; created=2025-03-11 (recency rank 155); state rank=156; cues={topic}; floor=topic; score=0.122133337 |

## Beliefs that left and beliefs that entered

The two cohorts are set differences, not paired causal replacements. Every LEFT belief remains available to section selection after the change and changes from selected to omitted_by_limit; every REPLACEMENT was already available before and changes from omitted_by_limit to selected. Cue sets and final scores for all ten are unchanged across pins.

| Change | ID | Gold fitting | Native salience | Created_at UTC | Recency rank | State rank | Before native assignment | After native assignment |
|---|---|---|---:|---|---:|---:|---|---|
| LEFT | belief-286 | yes | 0.960 | 2025-08-25 | 7 | 7 | rank 7; cues={participant}; floor=none; score=0.708499968 | omitted_by_limit; cues={participant}; floor=none; score=0.708499968 |
| LEFT | belief-287 | yes | 0.955 | 2024-10-19 | 294 | 8 | rank 8; cues={participant}; floor=none; score=0.707999945 | omitted_by_limit; cues={participant}; floor=none; score=0.707999945 |
| LEFT | belief-288 | yes | 0.950 | 2025-08-23 | 9 | 9 | rank 9; cues={participant}; floor=none; score=0.707499981 | omitted_by_limit; cues={participant}; floor=none; score=0.707499981 |
| LEFT | belief-289 | yes | 0.945 | 2024-10-17 | 295 | 10 | rank 10; cues={participant}; floor=none; score=0.706999958 | omitted_by_limit; cues={participant}; floor=none; score=0.706999958 |
| LEFT | belief-290 | yes | 0.940 | 2025-08-31 | 1 | 11 | rank 11; cues={participant}; floor=none; score=0.706499934 | omitted_by_limit; cues={participant}; floor=none; score=0.706499934 |
| REPLACEMENT | belief-222 | no | 0.544 | 2024-12-23 | 233 | 78 | omitted_by_limit; cues={topic}; floor=none; score=0.304399997 | rank 8; cues={topic}; floor=topic; score=0.304399997 |
| REPLACEMENT | belief-298 | yes | 0.900 | 2025-08-23 | 9 | 19 | omitted_by_limit; cues={topic}; floor=none; score=0.173333347 | rank 9; cues={topic}; floor=topic; score=0.173333347 |
| REPLACEMENT | belief-274 | no | 0.648 | 2024-11-01 | 285 | 26 | omitted_by_limit; cues={topic}; floor=none; score=0.148133337 | rank 10; cues={topic}; floor=topic; score=0.148133337 |
| REPLACEMENT | belief-252 | no | 0.604 | 2024-11-23 | 263 | 48 | omitted_by_limit; cues={topic}; floor=none; score=0.143733338 | rank 11; cues={topic}; floor=topic; score=0.143733338 |
| REPLACEMENT | belief-144 | no | 0.388 | 2025-03-11 | 155 | 156 | omitted_by_limit; cues={topic}; floor=none; score=0.122133337 | rank 12; cues={topic}; floor=topic; score=0.122133337 |

## Boundary supported by this evidence

All ten changed beliefs reach section assignment at both pins with identical native cue sets and final scores. The lost fitting beliefs retain higher final scores (0.7065–0.7085) than every entrant (0.1221–0.3044), yet the final section chooses the entrants as Topic-floor admissions. This directly places the observed tradeoff at bounded section admission, rather than changed salience or timestamp input. It does not prove the identity of a single evictor per lost belief or natural-language relevance of a Topic-labelled item.
