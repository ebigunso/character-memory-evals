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

## Task_15 binding resweep and recommendation

Task_15 supersedes the Task_7 recommendation with evidence from the canonical ten-scenario fixture at SHA-256 `45EFE4F85DA1A58809B702D60D471023746D8629A0BB7D92B2AE2CE3A04D2F87`. The result does not support increasing the shipped graph-root budget: selecting all 48 available roots still leaves the dormant `hub-scale` probe outside recall@5 and recall@10, worsens aggregate context reduction, and changes no aggregate recall or pollution metric. A bounded diagnostic shows that the probe survives graph expansion but is admitted at rank 16 or 17 under two pack-composition attractors when the Episode section is widened. The remaining `F-SEED-1` remedy is therefore downstream section admission or ranking, not a larger default root cap; the attractor split is recorded separately as `F-SEED-2`.

The ten full-suite live runs used the real adapter through Qdrant gRPC at `http://127.0.0.1:6334` and Character Memory source SHA `7949173d1c40580df01ed78a79454e6d9574a2c1` from branch `feature/v0-1-5-embedded-default`. The sibling checkout later advanced through docs- and CI-only commits; its `src` tree remained byte-identical to that committed source SHA. Each row is one complete ten-scenario run with a distinct run ID, namespace, and embedded store. The Task_15 shipped-default `hub-scale` trace and scenario report are exact JSON matches for the scoped Task_14 live anchor, providing a direct cross-run check of the measured root and retrieval behavior.

### Full-suite comparison

Short-, medium-, and long-gap recalls are reported together. Pollution is event/surface. The `hub-scale` root counters are unique/selected/omitted. Cap utilization is configured/selected. `Preserve` checks `hub-memory-0` through `hub-memory-5`, `archive-january`, `archive-october`, and `thread-1`; it also requires at least one relevance-labeled Episode from every scenario other than `correction-chains` and the deliberately unrecalled `hub-scale` probe scenario. `hub-scale` probe behavior is reported explicitly rather than counted as a preservation pass.

| Config | Varied point | Hub probe R@5 | Recall S/M/L | Pollution E/S | Context reduction | Roots U/S/O | Scored/fallback | Cap utilization C/S | Over budget | Preserve |
|---|---|---:|---|---|---:|---|---|---|---:|---|
| `task15_default` | shipped `alpha=1`, `gamma=1`, caps `20/5/15`, roots `12` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102768/0.170579 | 0 | pass |
| `task15_roots_24` | roots `24` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.593628 | 48/24/24 | 37/47 | 0.105717/0.174954 | 0 | pass |
| `task15_roots_48` | roots `48` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.611435 | 48/48/0 | 41/49 | 0.099873/0.169479 | 0 | pass |
| `task15_alpha_0_5` | alpha `0.5` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102768/0.155391 | 0 | pass |
| `task15_alpha_2_0` | alpha `2.0` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102572/0.181743 | 0 | pass |
| `task15_gamma_0_5` | gamma `0.5` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102768/0.150703 | 0 | pass |
| `task15_gamma_2_0` | gamma `2.0` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.100965/0.178685 | 0 | pass |
| `task15_about_10` | about cap `10` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102768/0.170579 | 0 | pass |
| `task15_thread_8` | thread cap `8` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102768/0.170579 | 0 | pass |
| `task15_participant_10` | participant cap `10` | 0 | 1/0.875/1 | 0.296296/0.296296 | -0.507275 | 48/12/36 | 31/41 | 0.102768/0.145547 | 0 | pass |

Within the recorded matrix artifacts, all alpha, gamma, and relation-cap variants return exactly the shipped-default object sets in all ten scenarios. Raising roots changes returned sets only in `recurring-hub-entity` and `hub-scale`; those additions do not improve any recall or pollution measure and make context reduction more negative. Every run has `hub_context_share=1.0` and zero over-budget decisions. Alpha changes the two scored `hub-scale` relation values from the shipped `0.588592` to `0.614534` or `0.544293`, while gamma and relation caps change selected fanout or utilization, but none changes the retrieved outcome. The preservation list passes in every run. Independent reviewer reproduction confirmed the default/root metrics and every variant-to-default returned-set identity; matrix comparability therefore stands, while individual single-run rows may still carry equal-score admission-boundary sensitivity bounded by `F-SEED-2`.

The proposed `roots=96` point was dropped before adapter execution because the validated configuration contract requires `max_graph_roots <= max_vector_candidates`; `96 > 48` is invalid, and the `roots=48` row already selects every vector candidate. It would not supply a distinct valid root-budget observation.

### Root-saturation diagnostic

At roots `48`, `hub-scale` selects all 48 unique root candidates with zero root omissions. The selected roots include the Person, Location, and Organization entities. The dormant probe's linked Episode survives the `mentions episode` expansion: all 11 linked Episodes are retained with no relation omission. It is then excluded by the downstream Episode section, whose trace assigns eight relevant Episodes and reports 36 omitted items. The full-suite run therefore remains recall@5 `0`, recall@10 `0`, with 28 returned objects.

One approved scoped diagnostic kept roots at `48` and increased only `top_k_episodes` from `8` to `64`. Back-to-back runs in one healthy environment produced two recurrent pack-composition shapes. Attractor A has trace SHA-256 `EC71FACD3A7AC341252EDC5F9B05A82309E2A4A195B20FB1785C6492CE7FFA7F`, returns `hub-scale-dormant-probe` as an Episode at rank 17, and contains 49 total objects, 29 Episode assignments, 13 section omissions, recall@5 `0`, recall@10 `0`, and context reduction `0.260012`. Attractor B has trace SHA-256 `C0FD93F6742DBAED4A9E8198B9E878504D9065E2C857F84FA3B8BA7A8F8705D9`, returns the probe at rank 16, and contains 51 total objects, 31 Episode assignments, 13 section omissions, recall@5 `0`, recall@10 `0`, and context reduction `0.231670`; the reviewer independently reproduced this trace byte-for-byte. The original diagnostic trace is byte-identical to attractor A.

Both attractors select all 48 roots with zero root omissions, preserve the probe through graph expansion, and place it below the top-ten boundary. They therefore support the same qualitative loss-point conclusion: recovering the probe in the top ten requires downstream admission or ordering work rather than a larger root budget. The differing equal-score pack composition contradicts the v0.1 backend-determinism acceptance criterion and is recorded as `F-SEED-2`; until Character Memory Task_19 closes the tie cohort and Task_9 confirms it under repeated runs, the diagnostic remains set-valued before-evidence rather than a single canonical row.

### Superseding recommendation

Recommend `alpha=1.0`, `gamma=1.0`, `about_entity.derived_memory.max=10`, `participant_entity.episode.max=5`, `part_of_thread.derived_memory.max=8`, `max_vector_candidates=48`, and `max_graph_roots=12`. This is a corpus-conditional recommendation for the canonical ten-scenario synthetic fixture at the exact source and artifact provenance above; Task_15 does not change Character Memory defaults. Each point has one full-suite live run, so the table establishes recorded within-matrix comparisons for this corpus rather than a production-distribution confidence interval; `F-SEED-2` bounds the remaining single-run tie sensitivity.

The lower about and thread caps preserve exact output and all scored outcomes while tightening unused headroom. Raising the participant cap to `10` changes utilization without an output or quality gain, so `5` remains preferable. Alpha and gamma variants affect internal scoring or fanout without improving recall, pollution, context size, or preservation, leaving `1.0` as the neutral point. Roots `24` and `48` add work and context without recovering the probe or any aggregate metric; roots `12` therefore remains the defensible default. This resweep strengthens, but does not generalize beyond, the Task_7 recommendation.

For `F-SEED-1`, the experiment demonstrates that default root omission exists and that eliminating it is insufficient: the probe reaches the expanded Episode pool at roots `48` but is lost downstream and remains below the top-ten boundary even when the Episode section is widened. The finding remains open for a Character Memory ordering/ranking remedy; increasing the default root cap is rejected as its disposition path. For `F-BASE-2`, none of the tuning points improves aggregate pollution and the larger root budgets worsen context reduction.

### Artifact references and hashes

All output paths below are under `runs/continuity/v0-1-5-task15/`; output hashes are SHA-256 over raw files. Config hashes are SHA-256 over the tracked TOML bytes materialized with LF line endings: `.gitattributes` pins `/configs/*.toml text eol=lf`, so hashing the raw file bytes from a fresh checkout reproduces the displayed value regardless of `core.autocrlf`.

| Config directory | Config | `results.jsonl` | `summary.json` | `traces.jsonl` | `report.json` |
|---|---|---|---|---|---|
| `task15_default` | `6632BEAB17B915297747B82BA4DE8E687DADD69573B7A7A3194A4EC4F1421B39` | `450BCD8949704C9CEF5D136A43F3E590BD6B747F0B2B3762B469317EA8C44254` | `72A107189F1F610E84FC112523D2984E093F62DECD6A6969E840AF7789CADCE9` | `677097CBCB221D5D0F5BBEAE63F7367611F68A91961D0CFD24D16F1F713C33B0` | `16F1C048BAFFD34F489F3D483F3D9018BFF2FFFA364FAC4B14E37314A7CA4D75` |
| `task15_roots_24` | `C3D997C2B72BF2CB46461204A1D164662BB549AFAE0688742C803F32777A9E8E` | `806E4BEE8F59E713F0EBD25AEDD077799DF62A86854C5566B7A38015FEEA66B9` | `4452FC8B8FA0C9E0A82E6A6866D7FAA8C698BF304D55464BA4284C8646B01DAC` | `98D95CFDA9F3839D6623CE9C2FC2BC3AD480388150D79446FD0238A157AF525A` | `FF4176F3CCE04D5BBBE687435821D3F2C1A9F8BE8BACDEA440B7251EB6A5EFCE` |
| `task15_roots_48` | `9407D077722817E21AAED3968A853A0E3B8328569A12A1CDB4177E81F51232C9` | `BD04DDB7F69F50D08F6DE0DA03A1DE823CB7516F2152DB13428742315517BF27` | `2798A7C4617D859F5C5387D316A53F52EDF28AEB3548D3C0BFCD0C85CCE2A2E6` | `34CBD35AA08D5BAE0183A3783FC3E459806F683558AE6B1B76D1C2F7B56EDD7B` | `7DC9205D9E36CA3E523246E02EA04B86562B2B8EB8D910419E41C308B030B373` |
| `task15_alpha_0_5` | `2485450B5A8B7909E2D3954F79CEA8CD77C71CCAA0DD9E43889D54C5E8F09754` | `51614791AD2F25FF6605BA362531C614771EB191A749E29EEE43DC27EC6F51C2` | `BF65DA51B6B0068F9224A0744486DFE7CC788FDF84B381956F31B114F740DA9F` | `BCACE7542E1F954645A86765EB454FA8E1CC6691FF224182C9BFE4D977627D80` | `97AF3AE0839BA2AAA466E85E22655AF937D931E0F31193C846F214A2FD85D27E` |
| `task15_alpha_2_0` | `03F871547B93C036B6E62A5F4CDC38B2D9057BF61B00730FB9BD871DA06B1BA3` | `E7C250B101E94972ADB97B7E8ADAD291A57FC6661904ECF3A4755742FBEE3152` | `CFAA3403D6196D0A6398E08561F72F947ECB791F36D5DAABFFF111F123119A58` | `9131ED456272DB61A3463150B9F8682CD21727550050F439EC23F8D29024A46C` | `A319A1206569191910D06D31FA9AA572F76459582805F4086A29177043A1631D` |
| `task15_gamma_0_5` | `A3C2D323DCF2653E90661331779D13657CF8ECEA6392C388FC454A5924722730` | `AB9A3EECFDE81CE1D583392CA4D0A8E94D79A5AF4BBD1727B45CF864135574BC` | `F5E825DDA46D4ED3971B3967EE136D956CD0EC91A3C277C91F73240C4BAE5679` | `12E1D446B27A1BFB37509C5C31967AC9EDFC541F497416A3DDE9754A8B420F43` | `5F8AF4D8506C8436A68A3EE9BAF71A9F6F5081465282D1AC7E8FC6407A35B7C1` |
| `task15_gamma_2_0` | `6301E6C6C45DED3FCE32C62B05337796C7F632768F7C463F2ED5D4B007A332D4` | `5ADB0CB20B0D9E0AB9CA445CF1FB0FCA4EE837021079EF9CF801491B2DC8BFA8` | `7AED73FD1750621931493A1C1AF9740DBDE5EF07EFB0B63B84007E2EBD2B4622` | `F77F1D6D7604AF9F016ECD0550401CE8405A22A0D957352C2430CA657A0F2B36` | `B272CD8F5978E6D8819E0D02272F1BD637F7E77EA8EFDAF1C33E90CA5F39B348` |
| `task15_about_10` | `69E9AAC8BDB6B24E4E8ADFD2C64F9DD6ECC50203A26D2B481A109FC455108571` | `22E4A791D1BCE908F20E808AD29A529CABC35AB688845D6BA9B73A0CFD9F4D37` | `0AC839E3C2940172D4DA33A7C1FA595C98735DA6E8BD78A705D98AE44794E10F` | `AD1A83BD4E1B9DD5A8771B02909158D11160FAA2698CE583C4E586F8652FFD02` | `62D403B7107508D01B0BA5C01988DD0F9FBA9817A99736FD686E803360C0FBDA` |
| `task15_thread_8` | `3F240D8B16D9087FF23B154AF69F474E9833D775C3531F28336B9FB15F22F364` | `23E6E72403F5009367B44AAC8BCB493F30DA5D99435B6BCF8A1445E0376912AD` | `4FEF2A13BD0CC747BDE052FBCD347110DC70A33A2EBF62B6DC9CF5D6F3BD3040` | `8355C3007B38964785C60BF4E952D67B27E2DA0BC52A212B24CD0880018F00EE` | `6BA5D8D486766384201DE2BFB935C04BAEAF9812327E4DC53DCB1B661FF962BE` |
| `task15_participant_10` | `ED433038014896E1BF7E3BF205BDC27FD0D31037F8EFCCE1AACF30D4594BF4A4` | `F6D5F03E0858760A40DF1C67E68EDBAC730DE437265FD12282672CB249258312` | `0859F68800A55870DA3E7AEA9C2D74EFD45C0D418AC74025302E6F116360C08F` | `3DBE6CC2A7578F5B0E708E3DBF70126FF703B473570850201EEDD49450A0BF9C` | `3A5AA21B917E885143B06D120BF8EC37720EBB914129F8D1EF8003741077B2A4` |
| `review-rerun/diagnostic-a` | `D8A8445392C30D86CFD215CF8DD7B89587BAA92FF38D82E5F38799A6C4E12E1A` | `AFE21C3CAA615AFFC653B43E31D24DCD680161746D69764A25408190673FD5E6` | `D53F76E4CE3462335B2EE03F9EA40F640C3207B2835EF2DBB5F0B31C3C6CE271` | `EC71FACD3A7AC341252EDC5F9B05A82309E2A4A195B20FB1785C6492CE7FFA7F` | `F9C5254778D08E175CC558569E15F32880EEB2C79B703576FC8FBEB943A8E511` |
| `review-rerun/diagnostic-b` | `D8A8445392C30D86CFD215CF8DD7B89587BAA92FF38D82E5F38799A6C4E12E1A` | `087B7277B58B5362C586FFB954FE8A9A8197E155927B880E67D1902722A79EE1` | `612EC00223A7E868198E5A3F858747173F480600F4FE2D39A409506CC5A38F8C` | `C0FD93F6742DBAED4A9E8198B9E878504D9065E2C857F84FA3B8BA7A8F8705D9` | `AAF1B0377D3C6782642A2CB9B2B08A2468B14AC0C6FDB0B9A0AD264329F1CEBD` |

The shared Qdrant service was kept below saturation by pruning 179 completed-run collections after their local artifacts and hashes were captured: 69 historical completed-run collections and 110 completed or failed-attempt Task_15 collections. Health was checked between cleanup segments; no local evidence artifact was deleted. An initial gRPC timeout occurred before the first completed comparison and is excluded from the evidence. Two long Windows embedded-store identifiers also failed before producing complete evidence rows; their reruns use shorter distinct identifiers and only the complete outputs above support the comparison.

## Findings

### F-SEED-1: Hub entity roots are truncated before graph expansion

- Finding ID: `F-SEED-1`
- Status: `CONFIRMED`
- Scenario and metric: `recurring-hub-entity`; continuity report `content.tuning_observations[id=entity_root_candidate_limit]`, generated by `crates/cmem-eval-continuity/src/report.rs` `tuning_observation`.
- Observed behavior: Historical v0.1.4 evidence asserted Entity-root truncation; in the Task_3 full-suite shipped regime, `recurring-hub-entity` reports `graph_verified_count=12`, zero selectivity decisions, zero returned derived memories, recall@5 `1.0`, and sampled pollution `0.5`, while the eval regime reports `graph_verified_count=18`, nine selectivity decisions, six returned derived memories, the same recall, and the same pollution. The Task_6 scoped shipped-default live hub run measures 21 unique graph-root candidates, 12 selected roots, and 9 omissions, proving candidate omission under 48/12; the counters are root-type-neutral, so they still do not prove that Entity roots specifically were omitted. The binding-scale Task_15 runs select all 48 roots at `max_graph_roots=48` with zero omissions but do not improve recall or pollution, while the larger root budgets worsen context reduction.
- Expected behavior: The shipped root bound keeps expansion finite without measurable recall or pollution harm on the binding fixture, and the permanent scale-sensitive regression fixture exposes any future quality loss caused by that bound.
- Severity: `major`, because the phase guidance classifies poor continuity recall and hub flooding as major findings.
- Suspected layer: `retrieval`.
- Disposition: `accept-as-designed`, user-ruled 2026-07-18.
- Disposition rationale: Bounded graph expansion is the documented philosophy and protects context size. At full saturation, roots `48` removes all root omissions without changing recall or pollution and worsens context reduction, so raising the shipped bound has measured cost without measured benefit. This conclusion is scale-conditional to the canonical fixture; `hub-scale` remains the permanent truncation-sensitive regression fixture.
- Target phase: Not applicable.
- Before report references: Historical artifacts `runs/continuity/round5-live-a/report.json` and `runs/continuity/round5-live-b/report.json`; Task_3 full-suite live artifacts `runs/continuity/v0-1-5-baseline/shipped-a/report.json`, `shipped-b/report.json`, `eval-a/report.json`, and `eval-b/report.json`; inspect `content.scenarios.recurring-hub-entity.metrics`, `fanout_decisions`, `stats_health_events`, and `content.tuning_observations[id=entity_root_candidate_limit]`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Task_14 establishes the permanent binding-scale fixture and shipped-default 48/12/36 root measurement under `runs/continuity/v0-1-5-task14/`; Task_15 compares roots `12`, `24`, and `48` under `runs/continuity/v0-1-5-task15/`, where roots `48` selects all candidates with zero omissions but leaves recall and pollution unchanged and worsens context reduction from `-0.507274788476972` to `-0.611435`. Task_9 re-confirms shipped-default metrics and returned sets twice under `runs/continuity/v0-1-5-task9/default-a/` and `default-b/`, then reproduces the Task_15 roots-24 returned sets exactly under `roots-24/`; the default pair trace hash is `977956FEF0011D5F47DD3288E5B1122D121BB1AAF73DB73AD69D28AD2BD8E733`.

### F-SEED-2: Equal-score ties produce nondeterministic context-pack composition

- Finding ID: `F-SEED-2`
- Status: `CONFIRMED`
- Scenario and metric: Scoped `hub-scale` diagnostic with `configs/continuity_binding_diagnostic_roots_48_episode_64.toml`; returned external-ID order, `section_assignment_count`, `section_assignment_relevant_episodes_count`, dormant-probe rank, and continuity recall@5/@10.
- Observed behavior: Back-to-back live runs with the same fixture, config, Character Memory source, Qdrant endpoint, and healthy environment produce two pack-composition attractors after all 48 roots are selected. Attractor A has trace hash `EC71FACD3A7AC341252EDC5F9B05A82309E2A4A195B20FB1785C6492CE7FFA7F`, returns 49 objects with 29 Episode assignments, and places `hub-scale-dormant-probe` at rank 17. Attractor B has trace hash `C0FD93F6742DBAED4A9E8198B9E878504D9065E2C857F84FA3B8BA7A8F8705D9`, returns 51 objects with 31 Episode assignments, and places the probe at rank 16; the reviewer independently reproduced B byte-for-byte. Both have 13 section omissions and recall@5/@10 `0`.
- Expected behavior: Equal-score candidates use a stable total ordering so repeated fresh-namespace retrievals under identical inputs produce byte-identical context-pack composition and deterministic report content.
- Severity: `major`, because nondeterministic backend composition contradicts the v0.1 backend-determinism acceptance criterion and can change which behavior-shaping items cross an admission boundary.
- Suspected layer: `retrieval`, specifically equal-score tie handling at the context-pack admission boundary.
- Disposition: `fix-now`, confirmed by the user gate on 2026-07-18.
- Disposition rationale: The two attractors occur within one healthy environment, so service saturation or source provenance cannot explain the split, and their nondeterministic pack composition contradicts the v0.1 backend-determinism acceptance criterion. Character Memory Task_19 owns the remedy on `feature/v0-1-5-embedded-default`: adapter-side tie-cohort closure, canonicalization, and trace-rank correction.
- Target phase: `v0.1.5`.
- Before report references: Back-to-back scoped artifacts under `runs/continuity/v0-1-5-task15/review-rerun/diagnostic-a/` and `diagnostic-b/`, produced with config `configs/continuity_binding_diagnostic_roots_48_episode_64.toml` and Character Memory branch `feature/v0-1-5-embedded-default` source SHA `7949173d1c40580df01ed78a79454e6d9574a2c1`. A raw hashes are results `AFE21C3CAA615AFFC653B43E31D24DCD680161746D69764A25408190673FD5E6`, summary `D53F76E4CE3462335B2EE03F9EA40F640C3207B2835EF2DBB5F0B31C3C6CE271`, trace `EC71FACD3A7AC341252EDC5F9B05A82309E2A4A195B20FB1785C6492CE7FFA7F`, and report `F9C5254778D08E175CC558569E15F32880EEB2C79B703576FC8FBEB943A8E511`; B raw hashes are results `087B7277B58B5362C586FFB954FE8A9A8197E155927B880E67D1902722A79EE1`, summary `612EC00223A7E868198E5A3F858747173F480600F4FE2D39A409506CC5A38F8C`, trace `C0FD93F6742DBAED4A9E8198B9E878504D9065E2C857F84FA3B8BA7A8F8705D9`, and report `AAF1B0377D3C6782642A2CB9B2B08A2468B14AC0C6FDB0B9A0AD264329F1CEBD`. The original scoped diagnostic trace under `runs/continuity/v0-1-5-task15/task15_diagnostic_roots_48_episode_64/` is byte-identical to attractor A.
- After report references: Task_9 runs `configs/continuity_task9_diagnostic_a.toml` and `continuity_task9_diagnostic_b.toml` against Character Memory fixed source `19d650e7c9b298d51054db22081dcbe75428b16f` and fixture SHA-256 `45EFE4F85DA1A58809B702D60D471023746D8629A0BB7D92B2AE2CE3A04D2F87`. The fresh-namespace pair under `runs/continuity/v0-1-5-task9/diagnostic-a/` and `diagnostic-b/` has byte-identical traces at `26CA5D6F39AEC5F4CE22963E354AE3E40057C4AEF0123684161331C8E507C0A7` and identical metadata-free report content at `E8E093CB99BA9FB3A434448C949B6101EA9B010F68220347B1C37DC34F2F50E9`. The deterministic result preserves exactly Attractor A's 49-object set with 29 Episode assignments, excludes Attractor B's two extra objects, records 11 section omissions, and places `hub-scale-dormant-probe` at canonical trace rank 19 with recall@5/@10 `0`.

### F-SEED-3: Graph-only evidence lacks admission and ranking credit

- Finding ID: `F-SEED-3`
- Status: `CONFIRMED`
- Scenario and metric: `hub-scale`; root candidate/selection/omission counters, graph expansion telemetry, returned external IDs, section assignments, dormant-probe rank, and continuity recall@5/@10.
- Observed behavior: The shipped 48/12 regime omits 36 roots and does not recall `hub-scale-dormant-probe`. Raising roots to `48` selects every root with zero root omissions, and the probe's Episode survives graph expansion, but the default eight-Episode section still excludes it. Widening the Episode section to 64 returns the probe only below the top-ten boundary: Task_15's pre-fix attractors place it at rank 16 or 17, and Task_9's deterministic post-fix trace places it at rank 19. Root capacity therefore does not explain the remaining recall loss.
- Expected behavior: Relevant graph-only evidence receives explicit admission or ranking credit so it can compete with vector-scored items without unbounded graph-root or section limits.
- Severity: `major`, because the missing credit leaves a relevant continuity memory outside recall@10 even after graph traversal successfully reaches it.
- Suspected layer: `retrieval`.
- Disposition: `defer`, user-ruled 2026-07-18.
- Disposition rationale: Ranking credit for graph evidence is a new retrieval signal rather than tuning of an existing v0.1.5 knob. It joins the F-BASE-2 residual as one v0.2 design item covering admission gating and graph-evidence ranking credit, with `hub-scale` retained as the permanent regression fixture.
- Target phase: `v0.2`.
- Before report references: Task_14 shipped-default evidence under `runs/continuity/v0-1-5-task14/`; Task_15 root-saturation and scoped diagnostic evidence under `runs/continuity/v0-1-5-task15/task15_roots_48/` and `review-rerun/diagnostic-{a,b}/`; and Task_9 deterministic confirmation under `runs/continuity/v0-1-5-task9/diagnostic-{a,b}/`. The Task_9 trace pair hash is `26CA5D6F39AEC5F4CE22963E354AE3E40057C4AEF0123684161331C8E507C0A7`, with 48/48/0 root counters, 49 returned objects, 29 Episode assignments, 11 omissions, probe rank 19, and recall@5/@10 `0`.
- After report references: Deferred to the joined v0.2 admission-gating and graph-evidence-ranking design item; the Task_9 evidence above is its pinned planning baseline.

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
- After report references: The corrected canonical fixture `crates/cmem-eval-continuity/fixtures/continuity_v2.json` explicitly targets `delivery-v1` and `delivery-v1:observation` with both derived-cascade switches disabled. The scoped Task_13 live `correction-chains` run under `runs/continuity/v0-1-5-task13/live/correction-chains/`, using ignored config `runs/continuity/v0-1-5-task13/continuity_task13_live.toml` and Character Memory branch `feature/v0-1-5-write-path-diagnostics` at `2c13d7a283d609cd70eece91e60ade3587e23f8f`, returns only `delivery-v3`, reports replacement recall `1.0`, both surface- and event-level pollution `0`, and unsafe lifecycle returned count `0`. Results hash is `905330C1B6DBA55D11F5C437EB5BE764A805B222FEBC7A95847B76DD669BB3C1`, trace hash is `ACE016595EBE1907DC102F6491B2961CEB30ED71B4C00673E9BA281CBA8C4A51`, and report hash is `F91CD1074DD41EFEB29D71406045007827E2631CCA97B3C760DEE572D9FD7B08`. Task_9 cross-checks the fix in both full-suite default runs under `runs/continuity/v0-1-5-task9/default-{a,b}/`: each returns only `delivery-v3`, with replacement recall `1.0`, surface- and event-level pollution `0`, superseded-current leakage `0`, and suppressed-memory leakage `0`; their byte-identical trace hash is `977956FEF0011D5F47DD3288E5B1122D121BB1AAF73DB73AD69D28AD2BD8E733`.

### F-BASE-2: Six scenarios amplify history and admit sampled-negative context

- Finding ID: `F-BASE-2`
- Status: `CONFIRMED`
- Scenario and metric: Full-suite aggregate plus `long-gap-recall`, `recurring-hub-entity`, `selective-entity`, `thread-drift`, `temporal-structure`, and `mixed-salience-accumulation`; `context_reduction_rate`, `retrieved_context_tokens`, `full_history_tokens`, `sampled_context_pollution_rate`, and sampled-pollution rationale shares.
- Observed behavior: Both live regimes have aggregate sampled pollution `0.5208333333333334`; six non-correction scenarios report pollution `0.5` or `0.6666666666666666` and negative context reduction from `-0.4838709677419355` to `-1.3132530120481927`. The shipped aggregate context reduction is `-0.720341408420762` and the eval aggregate is `-0.817115601969149`; sampled-pollution rationale attribution is dominated by semantic and salience categories at roughly `0.43` each, with entity attribution near `0.12` and thread attribution near `0.018`.
- Expected behavior: The continuity context pack improves or preserves relevant recall without systematically exceeding full-history size or returning fixture-declared sampled negatives across most scenarios.
- Severity: `major`, because the phase guidance classifies high pollution and poor continuity quality as major.
- Suspected layer: `retrieval`.
- Disposition: `defer`, user-ruled 2026-07-18 after a fixed-in-part v0.1.5 result.
- Disposition rationale: Task_12 identified behavior-shaping surfaces before tuning. Task_13 fixed false negative labels and added the event-level metric; Character Memory Task_5 added write-path echo warnings; Task_15 showed that existing alpha, gamma, fanout, and root knobs do not improve the remaining `0.296296296296296` aggregate pollution on the canonical binding fixture. The residual top-k admission of near-zero-score items requires a score floor or equivalent new signal, so it joins F-SEED-3's graph-evidence ranking credit as one v0.2 design item.
- Target phase: `v0.2`.
- Before report references: Full eight-scenario live `report.json` artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `content.aggregate.metrics`, each named scenario's `metrics`, and `rationale_samples`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Task_13 corrected fixture semantics and added event-level pollution evidence; Character Memory Task_5 write-plan diagnostics are present in the Task_9 fixed source `19d650e7c9b298d51054db22081dcbe75428b16f`; Task_15 records the binding sweep under `runs/continuity/v0-1-5-task15/`. Task_9 default runs under `runs/continuity/v0-1-5-task9/default-{a,b}/` reproduce Task_15 returned sets and every aggregate metric exactly: recall short/medium/long `1/0.875/1`, surface- and event-level pollution `0.296296296296296`, context reduction `-0.507274788476972`, and fanout-over-budget `0`. Alpha `2.0` and about cap `10` preserve exact returned sets; roots `24` reproduces the two expected Task_15 output changes without any recall or pollution improvement and worsens context reduction to `-0.5936282160205494`.

### F-BASE-3: Entity-only selectivity leaves measurable non-entity hub expansion

- Finding ID: `F-BASE-3`
- Status: `CONFIRMED`
- Scenario and metric: `recurring-hub-entity`; `fanout_decisions`, `stats_health_events`, `hub_context_share`, `sampled_context_pollution_rate`, `graph_relations_count`, and returned object kinds.
- Observed behavior: In the full-suite shipped regime, the hub query has zero selectivity decisions while six Episode traversal roots expand `mentions` and `involves` edges with selected caps of 16 and no fanout omissions; the context contains six Episodes and six Observations, `hub_context_share=1.0`, pollution `0.5`, and 216 graph relations. In the eval regime, all nine selectivity decisions are attached to three Entity roots, while Episode-root expansion remains outside selectivity and the context grows to six Episodes, six Observations, and six DerivedMemories with 324 graph relations, the same hub share, and the same pollution.
- Expected behavior: The §4.1 boundary is re-affirmed only if non-entity roots do not measurably concentrate hub-incident context; otherwise the limitation and its later-phase signal requirement are explicit.
- Severity: `major`, because the phase guidance classifies hub flooding and high pollution as major.
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
- Severity: `minor`, because the fallback is conservative and no cap or integrity violation occurs, but unstratified sweeps would produce misleading tuning evidence.
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
- After report references: The Task_13 regenerated fixture removed `archive-january` and `hub-memory-0` from `irrelevant_external_ids`, added `surface-contribution`, and introduced pairwise-distinct `surface_texts`; its Task_13 SHA-256 was `404353705D8AD5103A9CD4D48778C05C38C103F909C42D58F91E3640AD439E60`. The scoped Task_13 live `surface-contribution` run under `runs/continuity/v0-1-5-task13/live/surface-contribution/` returns all six labeled surfaces with visibly distinct Episode, Observation, and DerivedMemory texts and reports surface- and event-level pollution `0.5`; results, trace, and report hashes are `9D66F7C758E409046EF86163F3FA564BC43AD815A715C37636454CC321B74266`, `98A150C72E4F578027E43A7E2F807380BBA7C7FD8617E8F27826DF0DB1ABF747`, and `D0076112E9849DC3D1764116D92305C403322F479E8E9CE3D3E7803FD19ED503`. The Task_9 ten-scenario canonical fixture retains those corrections after Task_14 and has SHA-256 `45EFE4F85DA1A58809B702D60D471023746D8629A0BB7D92B2AE2CE3A04D2F87`. Both Task_9 default runs and the full mock cross-check use that fixture; `archive-january` and `hub-memory-0` remain unlabeled as negatives, `surface-contribution` retains pairwise-distinct surfaces, and its live surface- and event-level pollution remain `0.5`.

### F-HARNESS-1: Reports omit graph-root counters needed to verify the seeded finding

- Finding ID: `F-HARNESS-1`
- Status: `CONFIRMED`
- Scenario and metric: `recurring-hub-entity`; facade telemetry `unique_graph_root_candidate_count`, `selected_graph_root_count`, and `graph_root_omission_count` versus the adapter trace/report projection and hardcoded `tuning_observations[id=entity_root_candidate_limit]`.
- Observed behavior: Character Memory exposes the three root-selection counters, but the full-suite live trace and report do not project them; the tuning observation still states that the shipped limit truncated Entity roots even though the current artifact cannot identify omitted root types and its fanout trace includes Entity traversal roots.
- Expected behavior: The report projects measured root candidate, selection, and omission counters, and it makes no type-specific truncation claim unless separate type-aware evidence supports one.
- Severity: `major`, because the missing evidence blocks direct verification of a major seeded retrieval finding.
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
- Severity: `minor`, because deterministic report content remains available and correct, but the documented row recipe cannot satisfy the Task_3 distinct-run comparison without one extra normalization rule.
- Suspected layer: `fixture/harness defect`.
- Disposition: `fix-now`.
- Disposition rationale: The Task_4 user gate confirmed a Task_6 documentation fix that pins the distinct-run `run_id` normalization literally; this bounded evidence-contract defect does not count against the library.
- Target phase: `v0.1.5` Task_6.
- Before report references: `results.jsonl` under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`, plus the Task_3 reproducibility hashes above; both config identities and Character Memory `main` commit `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5` were used for the full-suite live evidence.
- After report references: `README.md` canonical repeat-run recipe using literal `__RUN__`, existing property order, compact JSON-array serialization, UTF-8 without a BOM, and no trailing newline.

## Task_9 final confirmation

Task_9 validates the final ten-scenario fixture at SHA-256 `45EFE4F85DA1A58809B702D60D471023746D8629A0BB7D92B2AE2CE3A04D2F87` against Character Memory branch `feature/v0-1-5-embedded-default` fixed source `19d650e7c9b298d51054db22081dcbe75428b16f`. Documentation commits advanced the sibling checkout during the run, but its `src` tree remained byte-identical to that fixed source. Every live config used Qdrant gRPC at `http://127.0.0.1:6334`, a fresh run ID, namespace prefix, Oxigraph path, retrieval-stat database, and identity-registry directory.

### Repeated-run result

The shipped-default pair covers all ten scenarios. Its raw traces are byte-identical at `977956FEF0011D5F47DD3288E5B1122D121BB1AAF73DB73AD69D28AD2BD8E733`, and its complete metadata-free report content is identical at `9E013A449C74F1149761A160D51156228A9B1EA1B905A1A28BDFCB5AA9361798`. Returned-object sets and all aggregate metrics are also exact matches for the Task_15 shipped-default evidence: short/medium/long recall `1/0.875/1`, surface- and event-level pollution `0.296296296296296`, context reduction `-0.507274788476972`, and fanout-over-budget `0`. The `0.875` medium-bucket recall is the known deferred F-SEED-3 `hub-scale` probe miss, not a Task_9 regression.

The scoped `hub-scale` diagnostic pair is also deterministic: raw traces are byte-identical at `26CA5D6F39AEC5F4CE22963E354AE3E40057C4AEF0123684161331C8E507C0A7`, and metadata-free report content is identical at `E8E093CB99BA9FB3A434448C949B6101EA9B010F68220347B1C37DC34F2F50E9`. The post-fix result preserves exactly the old Attractor A 49-object set with 29 Episode assignments, omits the two objects unique to Attractor B, records 11 section omissions, and places `hub-scale-dormant-probe` at canonical trace rank 19. Recall@5 and recall@10 remain `0`, confirming the separate deferred graph-evidence ranking gap without reproducing the tie-order defect.

### Spot points and mock cross-check

The alpha `2.0` and about-entity cap `10` spot points return exactly the new default object sets in all ten scenarios. Roots `24` differs only for `query-hub` and `query-hub-scale`, exactly matching the recorded Task_15 roots-24 sets; recall and pollution remain unchanged, fanout-over-budget stays `0`, and context reduction worsens to `-0.5936282160205494`. The full ten-scenario mock cross-check completes with trace hash `3ADB05C1F33B574049DB92D737BD3418C13045490CDADBC9287DE2B4F38B54DA`, matching the earlier Task_14 mock trace.

### Final acceptance audit

All eight aggregate reports and all 62 scenario report sections have empty `missing_required_metrics` lists. Across the seven live runs, `fanout_over_budget_count`, orphan-vector leakage, superseded-current leakage, suppressed-memory leakage, superseded-current returned count, suppressed-or-deleted returned count, unsafe lifecycle returned count, and graph-object-missing returned count are numeric zero. The mock report accounts for unsupported live-only metrics as explicit `null` values rather than fabricated zeroes. Both full-suite default runs return only `delivery-v3` for `correction-chains`, with replacement recall `1.0`, both pollution metrics `0`, and zero superseded or suppressed leakage. The Task_9 fixture keeps `archive-january` and `hub-memory-0` out of negative labels and retains pairwise-distinct Episode, Observation, and DerivedMemory text in `surface-contribution`. No finding block has critical severity, every finding has a final disposition and evidence path, and Task_9 exposed no new anomaly.

### Artifact hashes

Config hashes cover the tracked LF bytes. Output and report-content hashes are SHA-256; report content is compact JSON serialization of the top-level `content` object and deliberately excludes run metadata.

| Run | Config SHA-256 | Results | Summary | Traces | Report | Report content |
|---|---|---|---|---|---|---|
| `default-a` | `2CE7AD92A0E840C3F5A763C3C45DB79E65CBE56C0046D15578C913CCDB62B204` | `6B3A60DDEA81C417C5648A6F851865DA257EE1DC0FD076196C5A24DF1D22931B` | `66E89D9BB71717130E2A9D53E4D4980EF54B65C88C38BAFDF8C0E223A6E7FA3B` | `977956FEF0011D5F47DD3288E5B1122D121BB1AAF73DB73AD69D28AD2BD8E733` | `3EC114F4FC064492DAECB77F2D75A73176834DB99431E41A1321C5A622E5DB1D` | `9E013A449C74F1149761A160D51156228A9B1EA1B905A1A28BDFCB5AA9361798` |
| `default-b` | `24D33BF059760635C78D6C07E7A16BEDF9F1D34E3A66E45E7D9762D1C43BE0B1` | `2236F4DCEF3B2545F99219E839E21A7DACFF0E851FE57221A36238EEEEE3936D` | `8A9A0D995E6E1B8FC12C0A5F08B468E5450B1D1720CA7852E41888A61A29B0D1` | `977956FEF0011D5F47DD3288E5B1122D121BB1AAF73DB73AD69D28AD2BD8E733` | `7E1AE42646839ECC4A3F2EBC1EB71EB924AB86A7E964CD1F565B96DD2E9416FD` | `9E013A449C74F1149761A160D51156228A9B1EA1B905A1A28BDFCB5AA9361798` |
| `diagnostic-a` | `D89ACD8391BEBCA60FB183E9696494C58131F2D00E1D3251B7F14BAD0D1E3306` | `5276C5FF28A26D312D3601622F2BACD748AD422A1E5567FF11491DFE37714922` | `28DE951ED0B40CFDA7185F9775038ABAED6433AAB79C0C654BB4B3160577DF78` | `26CA5D6F39AEC5F4CE22963E354AE3E40057C4AEF0123684161331C8E507C0A7` | `BDBC56B28ABAB60B04F19CBB860B2179B96E8411D034E96042F64CCBE0A24C55` | `E8E093CB99BA9FB3A434448C949B6101EA9B010F68220347B1C37DC34F2F50E9` |
| `diagnostic-b` | `A2ED706F12AE8881F2B9F6EC0A28726E5A1FA476EFA9A5C055C6F825DDC65ACB` | `D2E58943A2A468CBE2AEDE3859C0C1EE2262900ADB8CAF8D6CB748B02F175478` | `1B185E83E9E874A1FDC569B87D9B62F822CA7D7B0EA2702D4AD3D5DADE9B6365` | `26CA5D6F39AEC5F4CE22963E354AE3E40057C4AEF0123684161331C8E507C0A7` | `5226A7147CB0C23AE711E95DB972CF5FA9C32A315EA334BE34CDBF45664DC92F` | `E8E093CB99BA9FB3A434448C949B6101EA9B010F68220347B1C37DC34F2F50E9` |
| `alpha-2-0` | `54AEE89D029C5392F4C2460BC2A7B8D6402ADF5BAE9D26351A922A3B0E5460D2` | `F000E5D6E9B669009A8DB3F46114274781864F696E9FE3882BE5D00D698EFAB2` | `7F224BFB8111C41A1E5256719872EEB449BD0C628A5A18EA0E4DB7F103378660` | `B43460E6D89B11EE81DC6FB9601F3C4B9A055FF675522A1562C05FD5556BF9B9` | `9370B3B459E3C61999320855EC7FA97D13CF7FCF7B2AE0E72BF11FD05932EAB2` | `A351A6C9E1B5FD85241C27C3EF3DD84BD8A09F60F8D5FA79F79B2E1599D85718` |
| `about-10` | `F2F482395326B7FEA3BE4129DBC7F2F659EB14494F0737EC1DB81E6797E8855F` | `9C16291C63019504033E14EC690A131C9B3F8900B11DD206BF46B4CD06ADF311` | `F427DFD9D4E64B99B4CBD8D8A7FDFA6459C1644D7ABAA5468F25B4DD1B6BB600` | `A7BFFC4B8DF55EE901AE722FFE49143C738A3F4E3BBEC91BB5130BD84F31C4CF` | `63F50BD8A13E7ED9D4D0EAA91CC742C896C5157EF6490988D2932847ABA36782` | `99AE6EC044886E71FADC8E11AF4ABAE08CF69ABC7F084F2C64F393A1046F6A29` |
| `roots-24` | `848177BF11F2D7718C3220A94FCC55F45C927322B6EEA38E35CAC1168596F3B4` | `8C0390D21D489C5B3EF9DD8E8800B3D08CC2531FE290A7CB680B6A18652F4CC0` | `36FBD84B9A3B18BDE111DF4D910D0CA652A9EF9641B1FA4656069E0393035E16` | `F52FE6A25A4376AEA279BC4C88DB419D08C3F9DFF94AB0DF7C9008BB07E801EE` | `A847C691D0F3694553CFD79826388836B1B55D6F226338C6B154B4F851D476E8` | `29E5DB23F9AF7BEE161FD9483545A1E85FEA2611074730C64B6D79873CA087E9` |
| `mock` | `7A4198CEE1D84AB60D8F41869650F205F94DDB9EA214D046DE2AAC08BC0816AD` | `E523C8BABAEFEFE329862B67C7CC1833A42DDC896BC145C88F0BD772CE72D773` | `7E141025BD3A399B70BAF89889A6A2032C15DE90777FD64833B19B3BACD191B5` | `3ADB05C1F33B574049DB92D737BD3418C13045490CDADBC9287DE2B4F38B54DA` | `3268EAE89E33D76494F83539C204F17C780063DB0622E850C421D7E4E0746B37` | `7802176D151F64E1D6A45DC7101546C177783F002B199062FE6D2DD7792104DB` |

## Task_22 schema-v3 catalog expansion

Task_22 replaces the canonical checked fixture with deterministic schema v3 at `crates/cmem-eval-continuity/fixtures/continuity_v3.json`. The catalog now contains 15 scenarios and 23 queries. Five new scenarios extend the prior ten-scenario suite:

- `graded-similarity` uses frozen real-model geometry for one target, two near misses, and an unrelated background memory.
- `combined-life` uses frozen real-model geometry for a 61-event life history: 53 remembers, two corrections, three explicit links, and three queries. It spans December 2024 through December 2025, interleaves the `lantern-restoration` and `harbor-garden` threads, includes recurring person, organization, and location hubs, and varies salience.
- `temporal-patterns` uses controllable similarity for explicit temporal progression.
- `entrenched-correction` uses controllable similarity for repeated misinformation followed by a correction chain.
- `autobiographical` uses controllable similarity and an ordinary `Person` character so provider judgment remains a measured result rather than fixture-side privileged identity.

The suite declares `provider = frozen` only for `graded-similarity` and `combined-life`; the other 13 scenarios declare `provider = controllable_similarity`. The committed mixed-provider config uses `text-embedding-3-large`, vector size 3072, and `task22_real_store.json`. Generation made one authorized OpenAI embedding request for 71 unique fixture texts, then all validation was offline. The store records `source = open_ai_api`, contains 71 vectors of width 3072, and has an out-of-band manifest. Ranked-cosine validation measured:

- graded similarity: target `0.861572146` > near miss `0.792349458` > background `0.180675849`;
- combined reopening: target `0.671678662` > near miss `0.650091052` > background `0.210112855`;
- combined rosemary: target `0.866143465` > near miss `0.634959161` > background `0.327862471`;
- combined mistake: target `0.602975190` > near miss `0.516644597` > background `0.377437443`.

The canonical fixture SHA-256 is `BF5E392EB3F0EB79F2F48FCA6EAD38A2E69109A7BEBD257A47AEA62F091F8EB3`. The manifest hash is `DDA314592900088234132503404ED6C4D3885F7E1C36EDF4396473CB608CC38C`; the frozen-store hash is `5BE3715FE360CF3971E4AF4F268B1D1ABE344BF39B339D53097A50BF1E8CA6DA`.

### Mock and live evidence

The expanded mock run completed all 15 scenarios and 23 queries using the committed v3 fixture and mixed config. Its results, summary, traces, and report hashes are `2B2B38798D25C3B678E35EF4D00DE9B2AE717D1CD05BB89AE1A47722F97C7CC6`, `5AE4B2CDE326CA855723F8885881824FAC31594C41529AEB612E9DFCE395B7DC`, `7069E5A02623DDE0E63F218AEC846AE935173DD5541BEAB1F93DEBF10305925C`, and `8CAD8037C50A004D7971821FFBBA005DDD3C82943701DCB4BBF31462E9661438`.

The bounded live confirmation ran only `combined-life` against Character Memory branch `feature/v0-1-5-embedded-default` at source `43a54bbfc35b66a0376f12661effdf2db8b60c4d`, Qdrant gRPC `http://127.0.0.1:6334`, and shipped retrieval defaults. The completed retry used run ID `v0-1-5-task22-combined-life-retry`, namespace prefix `cmem_eval_task22_retry`, and fresh Oxigraph, retrieval-stat, and identity-registry paths. Runner elapsed time was 129.986 seconds; observed command wall clock was 135.8 seconds. The interrupted predecessor collection was explicitly pruned before the successful retry.

The three live queries measured long-gap recall `0.5` at both 5 and 10, medium-gap recall `0.5` at 5 and `1.0` at 10, mean context pollution `0.15873015873015872`, mean event pollution `0.1111111111111111`, and mean context reduction `0.6625883632408919`. Fanout-over-budget, orphan-vector leakage, superseded-current leakage, suppressed-memory leakage, graph-object-missing returned count, and unsafe-lifecycle returned count were all numeric zero. These are confirmation measurements, not a newly introduced acceptance threshold. The top results visibly include the corrected September reopening (`life-reopening` and `life-opening-date-v3`), rosemary origin and winter promise (`life-rosemary-start` and `life-winter-rosemary-promise`), and the admitted prism mistake and its later retelling (`life-admitted-mistake` and `life-mara-retelling`).

Live config, results, summary, traces, and report hashes are `E11E71DB6A21B19118B0F8DBC39293A99BC47D4FC6689D5701F46D41C4FD1948`, `A7A4BB5B01AD3A5ACE8D8E93128AD52C1A729AF430136B54F64DE8BCF89ABD56`, `9E5858C2E1E5F3B902297DAB01D96C1BA3064CCB80D381E7B883A6863F2F24DA`, `BAA9670E989CB2279A34463BCA39CB50504C2FF14F35043A96700A2776B2786B`, and `15CA0FC1342858253C85A7E9AE3ABF3AE79B2D5D6F4E9EBA047393BF6E4EA57C`.
