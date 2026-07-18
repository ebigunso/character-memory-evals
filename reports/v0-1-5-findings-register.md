# v0.1.5 Findings Register

- Status: active
- Scope snapshot: 2026-07-17
- Governing source: Character Memory `docs/design/roadmap-phases/v0_1_5_eval_driven_v0_1_family_closeout.md` §3.1–§3.3

This register is the authoritative intake, disposition, and before/after evidence record for findings produced by the v0.1.5 evaluation-driven closeout.

## Record contract

Every finding records these fields:

- Finding ID
- Scenario and metric that revealed it
- Observed behavior
- Expected behavior
- Severity: `critical`, `major`, or `minor`
- Suspected layer: `retrieval`, `selectivity/fanout`, `link guard`, `write path`, `persistence`, or `fixture/harness defect`
- Disposition: `fix-now`, `defer`, or `accept-as-designed`
- Rationale for the disposition
- Target phase when deferred
- Before report references
- After report references

Before the Task_4 user disposition gate, `OPEN` marks a finding whose disposition has not been selected; `OPEN` is workflow state, not a fourth disposition.

After the Task_4 gate, `CONFIRMED` marks a finding whose disposition was selected by the user; findings deliberately held for additional evidence remain `OPEN`.

## Disposition rules

The phase document defines the disposition rules as follows:

> **fix-now:** behavior contradicts a v0.1 family acceptance criterion or philosophy invariant, and the fix does not require new concepts or signals.

> **defer:** the correct fix belongs to a later phase's concepts (for example: weak serendipity gaps belong to v0.5, richer traces belong to v0.4, scope-conditioned retrieval belongs to v0.2).

> **accept-as-designed:** the behavior is an explicit documented tradeoff (for example: missing weak associative recall under the v0.1.2 link guard).

Harness or fixture defects are fixed in the harness and do not count against the Character Memory library.

Critical findings can never be dispositioned `accept-as-designed`.

## Severity guidance

The phase document defines severity as follows:

> **critical:** correction safety violations, ungrounded behavior-influencing memory, lifecycle exclusion failures, fanout cap violations.

> **major:** poor continuity recall, hub flooding, high pollution rates, missing rationale, persistence drift.

> **minor:** suboptimal ranking, noisy diagnostics, rough report output.

## Finding template

### F-<ID>: <short title>

- Finding ID: `F-<ID>`
- Status: `OPEN`
- Scenario and metric: <scenario; metric, report field, or observation>
- Observed behavior: <measured behavior>
- Expected behavior: <acceptance criterion or invariant>
- Severity: <critical, major, or minor; mark draft until disposition review>
- Suspected layer: <one allowed layer>
- Disposition: `OPEN`
- Disposition rationale: Pending Task_4 user disposition gate.
- Target phase: Not set unless disposition is `defer`.
- Before report references: <repo-relative artifact paths and relevant report fields>
- After report references: Pending fix and confirmation run, or not applicable after final disposition.

## Task_3 baseline evidence

All live evidence in this section covers the full eight-scenario continuity suite against Character Memory `main` commit `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5` through the local Qdrant VM-IP endpoint.

The shipped regime uses `configs/continuity_baseline_shipped.toml`: no `backend.character_memory` table, `max_vector_candidates=48`, and `max_graph_roots=12`.

The eval regime uses `configs/continuity_baseline_eval.toml`: `max_vector_candidates=48`, `max_graph_roots=48`, and explicit shipped selectivity and fanout values (`alpha=1.0`, `gamma=1.0`, and budgets `0/20`, `0/5`, and `0/15`).

Each regime ran twice with distinct run IDs, namespaces, and Oxigraph, retrieval-stat SQLite, and identity-registry paths.

### Reproducibility hashes

- Shipped A: traces `1E130C9A0735F7A639C669F51D4B46DCAB3F91E64F56DFF7D67EC4CC8FA2B086`; README-normalized rows `FFA83819BC8B4BD296966C91EF35472E679C3010FB03ED5FA337C2AD8EFDD76A`; report content `978FA3064076CBA272C99EA65ED6044A03861F1A85C7D40A9F980FC2CA2D0567`.
- Shipped B: traces `1E130C9A0735F7A639C669F51D4B46DCAB3F91E64F56DFF7D67EC4CC8FA2B086`; README-normalized rows `DE573FE26B849C433D458C82B651B01163296FB69D093A1831E9A033D95480F2`; report content `978FA3064076CBA272C99EA65ED6044A03861F1A85C7D40A9F980FC2CA2D0567`.
- Eval A: traces `8C18B1A4A1D7CB4667D475FD038DB54FAB6B08049006F624C40D72AC6CA34B98`; README-normalized rows `56643D18B67E2BD3DF6683C45E6AE5BECA878E58952BACE850AE5DEEF7B55875`; report content `10EB0EE6EA81A62EA0574C8518B7D92018C01EFE0BDBD86256606C191AAAEE55`.
- Eval B: traces `8C18B1A4A1D7CB4667D475FD038DB54FAB6B08049006F624C40D72AC6CA34B98`; README-normalized rows `A59DFDC8413BF7B8BE3D8F9EA9C908065D6BB492654601CD4439F3BFEB04C7EC`; report content `10EB0EE6EA81A62EA0574C8518B7D92018C01EFE0BDBD86256606C191AAAEE55`.
- Shipped mock cross-check: traces `FDD13D62B7839482EFC7B19F2631646152516C7D5D4F3131956B3189A2CE223E`; normalized rows `3C29CD0765D0C74425A26BA0B5EF8AD0C8A9F2835782F7AFF352799631748B58`; report content `04B491B95A4F0012546F131D9361C9FFD79AB5A47F6F5B0D6B47C3FDC3FBE988`.

Within each live regime, traces and metadata-free report content are byte-identical.

The pre-Task_6 README row recipe preserved `run_id`, so the distinct-run row hashes differed; the Task_6 canonical recipe now replaces `run_id` with the literal sentinel `__RUN__` after zeroing latency, yielding matching shipped hashes `A433391E23FA4EDC100515FC143DF7D8D3A7440EF9874FE0F53AB6FDDEF37EDB` and matching eval hashes `87B537DFC216800CFA0932382919C373ED4C9140A9DD370B5E39D6B7CA11D30A`.

### Baseline section audit

- Fanout discipline — clean: every scenario in both live regimes reports `fanout_over_budget_count=0`.
- Rationale coverage — clean: aggregate retrieval and typed rationale coverage are both `1.0` in both regimes; no missing-rationale finding was observed.
- Explicit integrity metrics — clean except for the correction-chain evidence recorded in F-BASE-1: aggregate context validation and provenance coverage are `1.0`, while unsafe lifecycle, suppressed, superseded, orphan-vector, graph-missing-returned, and unprovenanced-derived-memory leakage metrics are `0`.
- Persistence and restart — clean: `cross-store-stress` restores 11 identities, preserves the returned object set, and reports zero deltas for returned objects, relevant objects, recall, graph relations, graph verification, fanout decisions, and selectivity decisions in both regimes.
- Metric registry — clean: `missing_required_metrics` is empty in both regimes; contractually unsupported metrics remain explicit `null`, not fabricated zeroes.
- Recall, pollution, and context size — findings recorded in F-BASE-1 and F-BASE-2.
- Hub root/selectivity scope — findings recorded in F-SEED-1, F-BASE-3, and F-HARNESS-1.
- Stats health and fallback activation — finding recorded in F-BASE-4.
- Cross-run row normalization — finding recorded in F-HARNESS-2.

## Task_7 continuity tuning sweep

All live metrics, returned-object comparisons, preservation checks, and artifact hashes in this section were produced against Character Memory branch `feature/v0-1-5-write-path-diagnostics` at defect-fixed commit `a23fcda6ce6ef24d52572e9afec31f5727d47ae7`, using the canonical nine-scenario `crates/cmem-eval-continuity/fixtures/continuity_v2.json` fixture at CharacterMemoryEvals commit `cd70d61d08580853e0762a61763d2e2d1d651580`. Each configuration has a distinct run ID, namespace prefix, Oxigraph path, retrieval-stat database, and identity-registry directory.

The matrix holds `max_vector_candidates=48` throughout and covers the default point (`alpha=1.0`, `gamma=1.0`, relation caps `20/5/15`, and `max_graph_roots=12`), alpha `{0.5, 2.0}`, gamma `{0.5, 2.0}`, `about_entity.derived_memory.max=10`, `part_of_thread.derived_memory.max=8`, `participant_entity.episode.max=10`, and `max_graph_roots={24, 48}`. The requested alpha/gamma `1.0` points and graph-root `12` point reuse the explicit default run, yielding ten unique live configurations.

### Comparison

Table values are aggregate means unless marked as totals or percentiles. `Recall S/M/L` is gap-bucket recall@5. `Pollution E/S` is event/surface sampled pollution. `Cap utilization C/S` is configured-cap/selected-cap utilization. `Fallback/scored` is the exact analytical split from `stats_health_events`; score distributions exclude fallback decisions. `Preserve` means recurrence context, temporal contrast, thread scope, and relevant Episode surfaces all remained present.

| Config | Varied value | Recall S/M/L | Pollution E/S | Context reduction | Cap utilization C/S | Fallback/scored | Score mean/p50/p95 | Over budget | Preserve |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `task7_default` | default | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099229/0.172224 | 40/29 | 0.355868/0.369070/0.500000 | 0 | pass |
| `task7_alpha_0_5` | alpha `0.5` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099229/0.155348 | 40/29 | 0.512683/0.557493/0.676343 | 0 | pass |
| `task7_alpha_2_0` | alpha `2.0` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099012/0.184629 | 40/29 | 0.211612/0.207519/0.317394 | 0 | pass |
| `task7_gamma_0_5` | gamma `0.5` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099229/0.150140 | 40/29 | 0.355868/0.369070/0.500000 | 0 | pass |
| `task7_gamma_2_0` | gamma `2.0` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.097493/0.180377 | 40/29 | 0.355868/0.369070/0.500000 | 0 | pass |
| `task7_about_10` | about cap `10` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099229/0.172224 | 40/29 | 0.355868/0.369070/0.500000 | 0 | pass |
| `task7_thread_8` | thread cap `8` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099229/0.172224 | 40/29 | 0.355868/0.369070/0.500000 | 0 | pass |
| `task7_participant_10` | participant cap `10` | 1/1/1 | 0.296296/0.296296 | -0.655718 | 0.099229/0.146121 | 40/29 | 0.355868/0.369070/0.500000 | 0 | pass |
| `task7_roots_24` | graph roots `24` | 1/1/1 | 0.296296/0.296296 | -0.741739 | 0.092632/0.168636 | 46/35 | 0.352997/0.369070/0.500000 | 0 | pass |
| `task7_roots_48` | graph roots `48` | 1/1/1 | 0.296296/0.296296 | -0.741739 | 0.092632/0.168636 | 46/35 | 0.352997/0.369070/0.500000 | 0 | pass |

Against the default returned-object sets, all alpha, gamma, and relation-cap variants are exact matches in every scenario. The `24`- and `48`-root variants are also exact matches except in `recurring-hub-entity`, where each adds the same six DerivedMemory surfaces (`hub-memory-0:derived` through `hub-memory-5:derived`). Those surfaces do not change recall or event/surface pollution and make aggregate context reduction more negative. The `24`- and `48`-root traces are byte-identical, so the fixture saturates by 24 roots.

The default run records 69 selectivity decisions: 29 scored and 40 conservative fallbacks. The larger-root runs record 81 decisions: 35 scored and 46 fallbacks. Alpha changes the scored distribution as intended, while gamma changes selected-cap utilization, but neither changes fallback count, returned objects, recall, pollution, or context size. The fallback path remains conservative and no relation cap is exceeded.

A true warm rerun is not available through the current CLI lifecycle: every continuity invocation resets and opens its namespace before executing the fixture. Adding a persistent reattach sweep would require harness growth outside Task_7 ownership. The required warm/cold distinction is therefore the approved analytical fallback above, using each query's `stats_health_events` to keep scored decisions separate from conservative fallback decisions. This limits alpha/gamma conclusions to the observed scored paths; it is not evidence about a separately warmed production distribution.

The preservation checks are explicit: `hub-memory-0` through `hub-memory-5` retain the recurrence series, both `archive-january` and `archive-october` retain temporal contrast, `thread-1` retains thread scope, and every non-correction scenario returns at least one relevance-labeled Episode surface. No variant improves pollution, so no apparent pollution gain conflicts with the preserve list.

### Artifact references and hashes

All paths below are under `runs/continuity/v0-1-5-task7/`; hashes are SHA-256 over the raw files.

| Config directory | `results.jsonl` | `summary.json` | `traces.jsonl` | `report.json` |
|---|---|---|---|---|
| `task7_default` | `C93E2708B4B8148F7032DBE14A37B024B4420C459141CE867B6998902CEFD89B` | `6A1B3B26BB678BC2BFF230D2E60C132DAD74021CA007D43DF417ABD7A616DDB7` | `767AF66BDAA5455D9576373268BE552F8C580DF9FB39E16D84A708A05CD1C532` | `FFC7C25DE42E67C661AE44F13AD6B7119A52CCC0B8060B435CD389E45FE0031C` |
| `task7_alpha_0_5` | `6863E534D90DCBF9FC0C50F948FFB6AC672248392F9B21B01A5846D960F28699` | `D64B697E8A3391D840687A5EABDD2A6BD6A820C52FEBC8282B1160E6059369EB` | `80C0B1DC5DB51F121600D0921A35627879F22AEC057C6B55217AA0EA2E4ADAC1` | `ECB1A64CF50119755E8D594D5191CF51E178AFDB5CA0F1E0CFE3D3A4D31B4E60` |
| `task7_alpha_2_0` | `58872383B4B5054AD6E61FBD1990744CC76E0AA9674ED7373469913DE46A6043` | `34E01660F0D6B38C4EA8740C7FB59AD6C7A4FA1DA2FB0AD49EDD15661881BCED` | `57E644899D3D49EEA67FC42D8A6A672B442851DF0C8F8957822FFA513F3F9C92` | `4309460F406BDC90C6CB3A2A72AF94A2CEEEE0F49A17C138FD71FE9E3DFB2200` |
| `task7_gamma_0_5` | `8634D8C0452ECBF9C4811618C090A9B60D838872098CBB711D1705D08654C994` | `07A9D0AA84048A2E24B070825AC85B47CA7FA8115EDBB2230899B2C5867D37D4` | `E7B70860A5FD9031C528AF78B9320F8EF767E8143C9A3183C11B767D866F699B` | `8DCEF86573FD45686ACB1E41BC923B41D08D176F8BE43FCF6D97CFA334CE2083` |
| `task7_gamma_2_0` | `A2505641BB5C8205193571FA8ED5857CE72504AA23660C795FED4E19BE1D8A68` | `495A857A13FC8B3C85DA968A61FFEBE9929ADF9A48825737F1C92F0B0D1FE87A` | `29B9410BBD359A3EC854B47FEC861A0CD3E836D6901DDB8CE1A2C00B95A79DB1` | `85DF41052EED884C45C777E81F2F1B501C6A6A204D59E56B102D7E6B5D78FD48` |
| `task7_about_10` | `CE70184D16B83A9E76A4EF746B445A00B828EE526231839D187048E964E3CFCE` | `A985C8328F886FE2984392B21946FD39552738E9CEFFF5B55B850976140BD093` | `E00CC2271B3F899A36A83CB16F4805C8E35AF2B3EFB1FE32D2B952B0410F6744` | `C6D68947BA95FBAD4C86CA4B30579F448F0C72EB5810A2062658790CA7FE9A8C` |
| `task7_thread_8` | `9B557998B0CE118170D51CA593F1004584FDAD58616A1EE8C7C4FB8F3EAD8AE7` | `B72662CA9D8703C5830055092EA10E73C7AF8B16A7052B86B0D44FBD43979EFC` | `16419E43C12DEA6AD54750B984704DDDF8596D735FB86A12BD77116F5784609C` | `66971A1AC5C7AEE18C1A671BAE6D4F3D002F155A6512D655113BC6EEF49A7B98` |
| `task7_participant_10` | `BFD66757A91968DEC980B9B6B3AF87E0C06BDCF3C433199179B2F83858DA0943` | `916317E8C3479002AA87003ED64C61DDF7B3BFC5C48B7E98B5BC396E955E4DF7` | `41E208B2B6338BBBF34D9068C984AFE287ACB4ED17398199F812162F16E6D024` | `CE2FD25A0E1FEC99D90429434BD60D37B6B27F8BAA6EB674098D4CD079897BC6` |
| `task7_roots_24` | `6B34279FCC7CBEC394504E9BD96A1998234A767D772097B85EA2171484429F9A` | `0E59BC25D110499092B7CEEE3DE5E0507879A332B4EA79EAECD07F77A6C15406` | `638EB38BDEDD1E9BF81B53B3F58E2FA4E6A57848FCBEABC056FA6BB767777E13` | `454BAE7B25DADCEBCE0D5D3A3D1AB79D22060A799C484101F9188237C5BF82B2` |
| `task7_roots_48` | `0A8D42BEEA24D241FAC084E939434FC6D480B683ECC1BE2C52D6E4FC733BF28A` | `F4E1A34591317AEC63EB5E71B8959567690FF63ABB26D1F2EC816C15848FFE45` | `638EB38BDEDD1E9BF81B53B3F58E2FA4E6A57848FCBEABC056FA6BB767777E13` | `FF151952778734314023D06FB216A50F6F65FCB41CBF9C1E55DF7B04EA0A54C1` |

### Recommendation for user review

Recommend `alpha=1.0`, `gamma=1.0`, `about_entity.derived_memory.max=10`, `participant_entity.episode.max=5`, `part_of_thread.derived_memory.max=8`, `max_vector_candidates=48`, and `max_graph_roots=12`. This is a recommendation only; Task_7 does not change Character Memory defaults.

These recommended values are conditional on the canonical nine-scenario synthetic fixture and must be revalidated on other corpora; the `alpha` and `gamma` conclusions are additionally limited to this fixture's cold-statistics, conservative-fallback-dominated regime.

The two lower relation caps preserve exact returned-object sets, every scored quality metric, all four required continuity contexts, and zero over-budget decisions while tightening unused headroom. Raising the participant cap to `10` lets decisions select as many as 10 Episodes but yields no output or quality gain, so the existing `5` remains preferable. Alpha and gamma variants move internal scores or selected-cap utilization without any measured outcome improvement, so the current `1.0` values remain the defensible neutral point under the warm-run limitation. Raising graph roots admits six echo-like hub DerivedMemory surfaces, increases fallback work, and worsens context reduction without improving recall or pollution, so `12` remains preferred.

For the active findings, this sweep confirms F-BASE-4's conservative fallback dominance without weakening it; supplies the requested post-fixture tuning evidence for F-BASE-2; and shows that the measured root omission in F-SEED-1 does not reduce this fixture's recall or required recurrence context, while larger root limits worsen context size. It does not resolve F-SEED-1's still-missing root-type attribution.

## Task_14 binding-scale fixture evidence

Task_14 adds `hub-scale` alongside the existing recurrence fixture so the Task_15 resweep can measure a regime in which write-derived selectivity statistics and graph-root truncation both bind. This section records fixture construction and the single shipped-default live smoke; it does not compare tuning variants or supersede the Task_7 recommendation.

The regenerated ten-scenario canonical fixture uses seed `20260712` and has SHA-256 `45EFE4F85DA1A58809B702D60D471023746D8629A0BB7D92B2AE2CE3A04D2F87`. `hub-scale` contains 48 routine incidents distributed `4/10/17/17` across four deterministic similarity clusters, cycles salience through `0.15/0.35/0.65/0.95`, and assigns three hub entities to distinct routine clusters. Its only relevance label is dormant memory `hub-scale-dormant-probe`; the probe is linked only to `Scale Hub C`, occupies a fifth cluster orthogonal to the query cluster, and has no sampled-negative labels. Generator tests pin more than 48 positively correlated memory/entity vector objects ahead of the orthogonal probe, the exact cluster population and hub assignment, canonical checked-fixture identity, and cross-process byte identity.

The guarded mock smoke ran all ten scenarios. Under `runs/continuity/v0-1-5-task14/mock/`, raw SHA-256 hashes are results `159654AB4E929050EF9209570DA2171FC14ADCE69F5AA974E12E2B4A529515D6`, summary `BACB5473ADC047E2FA6678E732EC0E2610556ECCDAF43977D43148A0DC340EA2`, traces `3ADB05C1F33B574049DB92D737BD3418C13045490CDADBC9287DE2B4F38B54DA`, and report `1545923EA8237EEEDD94E8513C4361E94E2C88CFC0270AD36357EE94A165C0B3`.

The scoped live smoke used Character Memory branch `feature/v0-1-5-embedded-default` at committed post-service-removal source SHA `7949173d1c40580df01ed78a79454e6d9574a2c1`. The ignored config `runs/continuity/v0-1-5-task14/continuity_task14_live.toml` has SHA-256 `E06C9B36E0C214DB0FC6D113B99BF310DA643153398CDBDE1F7C695E385B2849` and explicitly pins the shipped point: `alpha=1.0`, `gamma=1.0`, relation caps `20/5/15`, `max_vector_candidates=48`, and `max_graph_roots=12`. The run was limited to `hub-scale` and used the real adapter through the local Qdrant gRPC endpoint.

At that point, the trace records 48 vector candidates, 48 unique graph-root candidates, 12 selected roots, and 36 root omissions. Selectivity status is `scored_with_fallback`: two nonfallback decisions are scored at `0.588591910067779` from entity count `4` and global count `49`, while one thread decision uses conservative fallback. The dormant probe is absent from returned vector/context objects and measured `continuity_recall_fraction_gap_medium@5=0.0` (also `@10=0.0`). Those recall values are recorded observations, not expected outcomes; Task_15 owns the controlled comparison needed to attribute any change to tuning or root selection.

Under `runs/continuity/v0-1-5-task14/live/`, raw SHA-256 hashes are results `1E4235318FEB5710B403EC3519EEDBA4FDADB96C627335844BF4EEBF1D05CAEB`, summary `F10B19402CC9361A141E74767B8DAE68FDA9A0E92CB9E91E508286533D21274D`, traces `C2A1D3253F2E790E564ABA2582DEEA4F3CCA5676B340A8E489DDCBD59DF4196D`, and report `033320B58BD344CF1EC6A4B664D5B449D1F1747D2615EE8DCAC3DBA12C232FEF`.

## Findings

### F-SEED-1: Hub entity roots are truncated before graph expansion

- Finding ID: `F-SEED-1`
- Status: `OPEN`
- Scenario and metric: `recurring-hub-entity`; continuity report `content.tuning_observations[id=entity_root_candidate_limit]`, generated by `crates/cmem-eval-continuity/src/report.rs` `tuning_observation`.
- Observed behavior: Historical v0.1.4 evidence asserted Entity-root truncation; in the Task_3 full-suite shipped regime, `recurring-hub-entity` reports `graph_verified_count=12`, zero selectivity decisions, zero returned derived memories, recall@5 `1.0`, and sampled pollution `0.5`, while the eval regime reports `graph_verified_count=18`, nine selectivity decisions, six returned derived memories, the same recall, and the same pollution. The Task_6 scoped shipped-default live hub run measures 21 unique graph-root candidates, 12 selected roots, and 9 omissions, proving candidate omission under 48/12; the counters are root-type-neutral, so they still do not prove that Entity roots specifically were omitted.
- Expected behavior: The recurring hub entity remains available as a graph-expansion root within the shipped candidate-limit regime so continuity retrieval can use its connected context without hidden object-type starvation.
- Severity: `major` (draft), because the phase guidance classifies poor continuity recall and hub flooding as major findings.
- Suspected layer: `retrieval`.
- Disposition: `OPEN` pending the measured root counters produced by the F-HARNESS-1 fix.
- Draft disposition proposal: Withheld until the root-counter evidence can distinguish measured candidate omission from the historical type-specific starvation claim.
- Disposition rationale: The Task_4 user gate deliberately left this finding open because the baseline artifacts do not directly identify omitted root object types.
- Target phase: Not set while disposition is `OPEN`.
- Before report references: Historical artifacts `runs/continuity/round5-live-a/report.json` and `runs/continuity/round5-live-b/report.json`; Task_3 full-suite live artifacts `runs/continuity/v0-1-5-baseline/shipped-a/report.json`, `shipped-b/report.json`, `eval-a/report.json`, and `eval-b/report.json`; inspect `content.scenarios.recurring-hub-entity.metrics`, `fanout_decisions`, `stats_health_events`, and `content.tuning_observations[id=entity_root_candidate_limit]`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Scoped live hub artifacts under `runs/continuity/v0-1-5-task6/live-hub/`, produced with ignored config `runs/continuity/v0-1-5-task6/continuity_task6_live_hub.toml` against Character Memory `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`; inspect `traces.jsonl` telemetry and `report.json` `content.tuning_observations[id=entity_root_candidate_limit]`. Trace hash is `A1AF43585E5338AACC2C529FBB7B77DCC6B262550B7C48D796ACAB6C3F8E2357`, normalized-row hash is `2B7DB48DE986A7B05BCD5CE783658B4BD6EE1CEAE9066165E8856389AD8D9947`, and report-content hash is `BB624B101476A268DD90EA2717A6DE3E0D5C10F943DE8EB038DE4FA0BD5FB778`. Final disposition remains pending because the projected counters do not identify omitted root types.

### F-BASE-1: Correction retrieval returns only a stale pre-correction observation

- Finding ID: `F-BASE-1`
- Status: `CONFIRMED`
- Scenario and metric: `correction-chains`; `continuity_recall_fraction_gap_medium@5`, `supersession_replacement_recall`, `sampled_context_pollution_rate`, `sampled_event_pollution_rate`, correction/lifecycle metrics, and the returned external IDs in the query trace.
- Observed behavior: In both full-suite shipped and eval regimes, the query expects replacement `delivery-v3` and samples `delivery-v1` and `delivery-v2` as negatives, but the only returned item is `delivery-v1:observation`; recall@5 and replacement recall are `0`, pollution is `1.0`, while correction safe-admission is `1.0` and the explicit suppressed, superseded, and unsafe lifecycle metrics remain `0`.
- Expected behavior: Retrieval after correction returns the current replacement and excludes behavior-influencing observation surfaces derived from superseded content, with the correction metrics reflecting any stale surface that reaches the context pack.
- Severity: `major`, because the invalid fixture produced a false critical library signal and invalidated the scenario's correction evidence without demonstrating a library lifecycle violation.
- Suspected layer: `fixture/harness defect`.
- Disposition: `fix-now` in Task_13; re-dispositioned from a suspected Character Memory write-path defect on 2026-07-17.
- Disposition rationale: The Character Memory diagnosis showed that the fixture corrected v1→v2→v3 with every replacement provenanced to the v1 Episode, then forgot that Episode under the default `apply_to_derived_from_target=true` cascade, which correctly suppressed current v3, while the separately admitted v1 Observation was never targeted and therefore remained current. Character Memory upheld explicit-target, source-retention, provenance-cascade, and no-inference contracts. Task_13 fixes the harness by explicitly targeting both v1 source surfaces and setting both `suppress_derived_from_target=false` and `apply_to_derived_from_target=false`, so v3 survives.
- Target phase: `v0.1.5`.
- Before report references: Full eight-scenario live shipped and eval artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `report.json` `content.scenarios.correction-chains.metrics` and `traces.jsonl` trace `fixture_id=correction-chains`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: The corrected canonical fixture `crates/cmem-eval-continuity/fixtures/continuity_v2.json` explicitly targets `delivery-v1` and `delivery-v1:observation` with both derived-cascade switches disabled. The scoped Task_13 live `correction-chains` run under `runs/continuity/v0-1-5-task13/live/correction-chains/`, using ignored config `runs/continuity/v0-1-5-task13/continuity_task13_live.toml` and Character Memory branch `feature/v0-1-5-write-path-diagnostics` at `2c13d7a283d609cd70eece91e60ade3587e23f8f`, returns only `delivery-v3`, reports replacement recall `1.0`, both surface- and event-level pollution `0`, and unsafe lifecycle returned count `0`. Results hash is `905330C1B6DBA55D11F5C437EB5BE764A805B222FEBC7A95847B76DD669BB3C1`, trace hash is `ACE016595EBE1907DC102F6491B2961CEB30ED71B4C00673E9BA281CBA8C4A51`, and report hash is `F91CD1074DD41EFEB29D71406045007827E2631CCA97B3C760DEE572D9FD7B08`. A remaining trace-evidence gap is recorded rather than fixed here: suppressed-omission decisions expose counts and reasons through the CME adapter/core telemetry but not the omitted object ID, and those projection types are outside Task_13's owned continuity/register paths.

### F-BASE-2: Six scenarios amplify history and admit sampled-negative context

- Finding ID: `F-BASE-2`
- Status: `CONFIRMED`
- Scenario and metric: Full-suite aggregate plus `long-gap-recall`, `recurring-hub-entity`, `selective-entity`, `thread-drift`, `temporal-structure`, and `mixed-salience-accumulation`; `context_reduction_rate`, `retrieved_context_tokens`, `full_history_tokens`, `sampled_context_pollution_rate`, and sampled-pollution rationale shares.
- Observed behavior: Both live regimes have aggregate sampled pollution `0.5208333333333334`; six non-correction scenarios report pollution `0.5` or `0.6666666666666666` and negative context reduction from `-0.4838709677419355` to `-1.3132530120481927`. The shipped aggregate context reduction is `-0.720341408420762` and the eval aggregate is `-0.817115601969149`; sampled-pollution rationale attribution is dominated by semantic and salience categories at roughly `0.43` each, with entity attribution near `0.12` and thread attribution near `0.018`.
- Expected behavior: The continuity context pack improves or preserves relevant recall without systematically exceeding full-history size or returning fixture-declared sampled negatives across most scenarios.
- Severity: `major` (draft), because the phase guidance classifies high pollution and poor continuity quality as major.
- Suspected layer: `retrieval`.
- Disposition: `fix-now` after contribution analysis.
- Disposition rationale: The Task_4 user gate confirmed fix-now only after new Task_12 identifies which memory surfaces contribute the excess context; Task_7 tuning targets must derive from that analysis rather than raw pollution deltas.
- Target phase: `v0.1.5`.
- Before report references: Full eight-scenario live `report.json` artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `content.aggregate.metrics`, each named scenario's `metrics`, and `rationale_samples`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Pending Task_12 memory-surface contribution analysis, the resulting Task_7 tuning evidence, and confirmation runs.

### F-BASE-3: Entity-only selectivity leaves measurable non-entity hub expansion

- Finding ID: `F-BASE-3`
- Status: `CONFIRMED`
- Scenario and metric: `recurring-hub-entity`; `fanout_decisions`, `stats_health_events`, `hub_context_share`, `sampled_context_pollution_rate`, `graph_relations_count`, and returned object kinds.
- Observed behavior: In the full-suite shipped regime, the hub query has zero selectivity decisions while six Episode traversal roots expand `mentions` and `involves` edges with selected caps of 16 and no fanout omissions; the context contains six Episodes and six Observations, `hub_context_share=1.0`, pollution `0.5`, and 216 graph relations. In the eval regime, all nine selectivity decisions are attached to three Entity roots, while Episode-root expansion remains outside selectivity and the context grows to six Episodes, six Observations, and six DerivedMemories with 324 graph relations, the same hub share, and the same pollution.
- Expected behavior: The §4.1 boundary is re-affirmed only if non-entity roots do not measurably concentrate hub-incident context; otherwise the limitation and its later-phase signal requirement are explicit.
- Severity: `major` (draft), because the phase guidance classifies hub flooding and high pollution as major.
- Suspected layer: `selectivity/fanout`.
- Disposition: `defer`.
- Disposition rationale: The Task_4 user gate confirmed deferral because widening selectivity to non-entity roots requires non-entity-keyed statistics, which are a new signal outside v0.1.5.
- Target phase: `v0.2`.
- Before report references: Full-suite live artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `report.json` `content.scenarios.recurring-hub-entity` and `traces.jsonl` trace `fixture_id=recurring-hub-entity`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Deferred to the v0.2 measurement and design work.

### F-BASE-4: Conservative fallback dominates cold baseline selectivity

- Finding ID: `F-BASE-4`
- Status: `CONFIRMED`
- Scenario and metric: Every full-suite scenario; `stats_health_events`, `conservative_fallback_activation_count`, and selectivity decision counts.
- Observed behavior: In the eval regime, correction is fallback-only at `9/9`; temporal uses fallback for `7/9`; long-gap, selective, thread, and cross-store use fallback for `5/9`; hub and mixed-salience use fallback for `3/9`. The aggregate mean is `5.25` fallback activations per query; the shipped regime is similar except hub has no decisions and thread uses `2/6` fallbacks, for an aggregate mean of `4.5`.
- Expected behavior: Task_7 tuning evidence distinguishes cold fallback-controlled paths from warmed scored paths so alpha and gamma are judged only where they actually bind, without weakening conservative fallback.
- Severity: `minor` (draft), because the fallback is conservative and no cap or integrity violation occurs, but unstratified sweeps would produce misleading tuning evidence.
- Suspected layer: `selectivity/fanout`.
- Disposition: `accept-as-designed`.
- Disposition rationale: The Task_4 user gate accepted conservative fallback as designed because the phase forbids weakening it; Task_7 is required to split warm and cold statistics so fallback-controlled paths are not misread as alpha/gamma evidence.
- Target phase: Not applicable.
- Before report references: Full-suite live `report.json` artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect each scenario's `stats_health_events` and `metrics.conservative_fallback_activation_count`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Required Task_7 warm/cold split sweep reports.

### F-FIXTURE-1: Pollution labels penalize behavior-shaping temporal and recurrence context, and echo surfaces mask per-surface value

- Finding ID: `F-FIXTURE-1`
- Status: `CONFIRMED`
- Scenario and metric: `temporal-structure` and `recurring-hub-entity` `expected.irrelevant_external_ids`; all scenarios' persisted Episode, Observation, and DerivedMemory texts; `sampled_context_pollution_rate` and `sampled_event_pollution_rate`.
- Observed behavior: The Task_3 baseline fixture labels `archive-january` as a sampled negative even though it is the change-over-time complement of the relevant October event and appears below it at ranks 6–8 versus 1–3. It labels `hub-memory-0` as a sampled negative even though it and relevant `hub-memory-5` are template-aligned recurrence instances with identical salience and equivalent semantics, differing only in ordinal token and timestamp, while `hub-memory-1` through `hub-memory-4` are unlabeled. An admission policy therefore cannot exclude `hub-memory-0` as noise without a recency or ordering judgment that would equally exclude relevant instances. Every baseline scenario also persists byte-identical Episode, Observation, and DerivedMemory text, so per-surface contribution is unmeasurable and the surface-level context metric can count one event three times.
- Expected behavior: Pollution labels mark only context that should not influence behavior, such as stale lifecycle state, low-selectivity background, orthogonal distractors, or sub-threshold trivia. Temporal predecessors and recurrence instances are evaluated through ordering/currentness rather than negative admission labels, at least one scenario provides distinct persisted text per surface, and the report retains surface-level pollution while adding an event-level view deduplicated by Episode root.
- Severity: `major`, because the labels and echo surfaces invalidate the interpretation of a central quality metric.
- Suspected layer: `fixture/harness defect`.
- Disposition: `fix-now` in Task_13, user-ruled 2026-07-17.
- Disposition rationale: The labels contradicted the continuity philosophy's temporal change-over-time and recurrence criteria. Task_13 removes both false negative labels, adds the `surface-contribution` scenario with two relevance-labeled events whose three persisted surface texts are pairwise distinct, and registers `sampled_event_pollution_rate` alongside the retained surface-level metric.
- Target phase: `v0.1.5` Task_13.
- Before report references: Task_3 baseline artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/` and `runs/continuity/v0-1-5-baseline/eval-a/`, especially `report.json` and `traces.jsonl`; the pre-fix `crates/cmem-eval-continuity/fixtures/continuity_v2.json`; and sibling Character Memory `docs/project_philosophy.md` §§6 and 12.
- After report references: The regenerated canonical fixture removes `archive-january` and `hub-memory-0` from `irrelevant_external_ids`, adds `surface-contribution`, and carries explicit distinct `surface_texts`; its SHA-256 is `404353705D8AD5103A9CD4D48778C05C38C103F909C42D58F91E3640AD439E60`. The scoped Task_13 live `surface-contribution` run under `runs/continuity/v0-1-5-task13/live/surface-contribution/`, using ignored config `runs/continuity/v0-1-5-task13/continuity_task13_live.toml` and Character Memory branch `feature/v0-1-5-write-path-diagnostics` at `2c13d7a283d609cd70eece91e60ade3587e23f8f`, returns all six labeled surfaces with each event's Episode, Observation, and DerivedMemory texts visibly distinct and reports surface-level pollution `0.5` plus event-level pollution `0.5`. Results hash is `9D66F7C758E409046EF86163F3FA564BC43AD815A715C37636454CC321B74266`, trace hash is `98A150C72E4F578027E43A7E2F807380BBA7C7FD8617E8F27826DF0DB1ABF747`, and report hash is `D0076112E9849DC3D1764116D92305C403322F479E8E9CE3D3E7803FD19ED503`.

### F-HARNESS-1: Reports omit graph-root counters needed to verify the seeded finding

- Finding ID: `F-HARNESS-1`
- Status: `CONFIRMED`
- Scenario and metric: `recurring-hub-entity`; facade telemetry `unique_graph_root_candidate_count`, `selected_graph_root_count`, and `graph_root_omission_count` versus the adapter trace/report projection and hardcoded `tuning_observations[id=entity_root_candidate_limit]`.
- Observed behavior: Character Memory exposes the three root-selection counters, but the full-suite live trace and report do not project them; the tuning observation still states that the shipped limit truncated Entity roots even though the current artifact cannot identify omitted root types and its fanout trace includes Entity traversal roots.
- Expected behavior: The report projects measured root candidate, selection, and omission counters, and it makes no type-specific truncation claim unless separate type-aware evidence supports one.
- Severity: `major` (draft), because the missing evidence blocks direct verification of a major seeded retrieval finding.
- Suspected layer: `fixture/harness defect`.
- Disposition: `fix-now`.
- Disposition rationale: The Task_4 user gate confirmed a Task_6 harness fix because the facade counters already exist; the projection adds no Character Memory concept or retrieval signal and does not count against the library.
- Target phase: `v0.1.5` Task_6.
- Before report references: Full-suite live artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`, plus `crates/cmem-eval-continuity/src/report.rs` `tuning_observation`; inspect each run's `report.json` and `traces.jsonl`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Scoped Task_6 live hub artifacts under `runs/continuity/v0-1-5-task6/live-hub/` use ignored config `runs/continuity/v0-1-5-task6/continuity_task6_live_hub.toml` and Character Memory `main` commit `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`; both `traces.jsonl` telemetry and `report.json` `content.tuning_observations[id=entity_root_candidate_limit]` record 21 unique candidates, 12 selected roots, and 9 omissions. Trace hash is `A1AF43585E5338AACC2C529FBB7B77DCC6B262550B7C48D796ACAB6C3F8E2357`, normalized-row hash is `2B7DB48DE986A7B05BCD5CE783658B4BD6EE1CEAE9066165E8856389AD8D9947`, and report-content hash is `BB624B101476A268DD90EA2717A6DE3E0D5C10F943DE8EB038DE4FA0BD5FB778`.

### F-HARNESS-2: Canonical row hashes retain run identity across required distinct runs

- Finding ID: `F-HARNESS-2`
- Status: `CONFIRMED`
- Scenario and metric: Full-suite reproducibility pairs; README canonical normalized-row hashing recipe.
- Observed behavior: Same-regime traces and metadata-free report content are byte-identical, but the pre-fix README-normalized row hashes differ because `run_id` remains in every row; replacing only `run_id` with the literal `__RUN__` sentinel after the documented latency normalization makes each pair byte-identical.
- Expected behavior: The canonical semantic row comparison for distinct-run reproducibility either normalizes identity metadata explicitly or states that row hashes are only comparable when run IDs match.
- Severity: `minor` (draft), because deterministic report content remains available and correct, but the documented row recipe cannot satisfy the Task_3 distinct-run comparison without one extra normalization rule.
- Suspected layer: `fixture/harness defect`.
- Disposition: `fix-now`.
- Disposition rationale: The Task_4 user gate confirmed a Task_6 documentation fix that pins the distinct-run `run_id` normalization literally; this bounded evidence-contract defect does not count against the library.
- Target phase: `v0.1.5` Task_6.
- Before report references: `results.jsonl` under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`, plus the Task_3 reproducibility hashes above; both config identities and Character Memory `main` commit `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5` were used for the full-suite live evidence.
- After report references: `README.md` canonical repeat-run recipe using literal `__RUN__`, existing property order, compact JSON-array serialization, UTF-8 without a BOM, and no trailing newline.
