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

The README row recipe intentionally preserves `run_id`, so the distinct-run row hashes differ; replacing only `run_id` with the literal sentinel `__RUN__` after zeroing latency yields matching shipped hashes `A433391E23FA4EDC100515FC143DF7D8D3A7440EF9874FE0F53AB6FDDEF37EDB` and matching eval hashes `87B537DFC216800CFA0932382919C373ED4C9140A9DD370B5E39D6B7CA11D30A`.

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

## Findings

### F-SEED-1: Hub entity roots are truncated before graph expansion

- Finding ID: `F-SEED-1`
- Status: `OPEN`
- Scenario and metric: `recurring-hub-entity`; continuity report `content.tuning_observations[id=entity_root_candidate_limit]`, generated by `crates/cmem-eval-continuity/src/report.rs` `tuning_observation`.
- Observed behavior: Historical v0.1.4 evidence asserted Entity-root truncation; in the Task_3 full-suite shipped regime, `recurring-hub-entity` reports `graph_verified_count=12`, zero selectivity decisions, zero returned derived memories, recall@5 `1.0`, and sampled pollution `0.5`, while the eval regime reports `graph_verified_count=18`, nine selectivity decisions, six returned derived memories, the same recall, and the same pollution. The projected trace contains Entity traversal roots in both regimes but omits the facade's unique, selected, and omitted graph-root counters, so the current artifacts show a candidate-limit effect without directly proving type-specific Entity-root truncation.
- Expected behavior: The recurring hub entity remains available as a graph-expansion root within the shipped candidate-limit regime so continuity retrieval can use its connected context without hidden object-type starvation.
- Severity: `major` (draft), because the phase guidance classifies poor continuity recall and hub flooding as major findings.
- Suspected layer: `retrieval`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `accept-as-designed` for the measured 12-root bounding behavior because it preserves hub recall and pollution while returning six fewer duplicate derived surfaces; final disposition should remain conditional on the direct root-counter evidence requested by F-HARNESS-1.
- Disposition rationale: Pending the Task_4 user gate; the current evidence does not reproduce the historical type-specific starvation claim directly.
- Target phase: Not set while disposition is `OPEN`.
- Before report references: Historical artifacts `runs/continuity/round5-live-a/report.json` and `runs/continuity/round5-live-b/report.json`; Task_3 full-suite live artifacts `runs/continuity/v0-1-5-baseline/shipped-a/report.json`, `shipped-b/report.json`, `eval-a/report.json`, and `eval-b/report.json`; inspect `content.scenarios.recurring-hub-entity.metrics`, `fanout_decisions`, `stats_health_events`, and `content.tuning_observations[id=entity_root_candidate_limit]`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Pending disposition, remediation if selected, and the corresponding confirmation run.

### F-BASE-1: Correction retrieval returns only a stale pre-correction observation

- Finding ID: `F-BASE-1`
- Status: `OPEN`
- Scenario and metric: `correction-chains`; `continuity_recall_fraction_gap_medium@5`, `supersession_replacement_recall`, `sampled_context_pollution_rate`, correction/lifecycle metrics, and the returned external IDs in the query trace.
- Observed behavior: In both full-suite shipped and eval regimes, the query expects replacement `delivery-v3` and samples `delivery-v1` and `delivery-v2` as negatives, but the only returned item is `delivery-v1:observation`; recall@5 and replacement recall are `0`, pollution is `1.0`, while correction safe-admission is `1.0` and the explicit suppressed, superseded, and unsafe lifecycle metrics remain `0`.
- Expected behavior: Retrieval after correction returns the current replacement and excludes behavior-influencing observation surfaces derived from superseded content, with the correction metrics reflecting any stale surface that reaches the context pack.
- Severity: `critical` (draft), because the phase guidance classifies correction safety and lifecycle exclusion violations as critical.
- Suspected layer: `write path`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `fix-now`, because the observed stale surface contradicts correction safety and appears testable through existing episode/observation provenance and lifecycle signals.
- Disposition rationale: Pending the Task_4 user gate; critical findings cannot be accepted as designed.
- Target phase: Not set while disposition is `OPEN`.
- Before report references: Full eight-scenario live shipped and eval artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `report.json` `content.scenarios.correction-chains.metrics` and `traces.jsonl` trace `fixture_id=correction-chains`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Pending disposition, remediation if selected, and the corresponding confirmation run.

### F-BASE-2: Six scenarios amplify history and admit sampled-negative context

- Finding ID: `F-BASE-2`
- Status: `OPEN`
- Scenario and metric: Full-suite aggregate plus `long-gap-recall`, `recurring-hub-entity`, `selective-entity`, `thread-drift`, `temporal-structure`, and `mixed-salience-accumulation`; `context_reduction_rate`, `retrieved_context_tokens`, `full_history_tokens`, `sampled_context_pollution_rate`, and sampled-pollution rationale shares.
- Observed behavior: Both live regimes have aggregate sampled pollution `0.5208333333333334`; six non-correction scenarios report pollution `0.5` or `0.6666666666666666` and negative context reduction from `-0.4838709677419355` to `-1.3132530120481927`. The shipped aggregate context reduction is `-0.720341408420762` and the eval aggregate is `-0.817115601969149`; sampled-pollution rationale attribution is dominated by semantic and salience categories at roughly `0.43` each, with entity attribution near `0.12` and thread attribution near `0.018`.
- Expected behavior: The continuity context pack improves or preserves relevant recall without systematically exceeding full-history size or returning fixture-declared sampled negatives across most scenarios.
- Severity: `major` (draft), because the phase guidance classifies high pollution and poor continuity quality as major.
- Suspected layer: `retrieval`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `fix-now`, starting with existing candidate limits, section bounds, selectivity, and fanout budgets before considering any new signal.
- Disposition rationale: Pending the Task_4 user gate; the behavior is reproducible across both regimes and the planned Task_7 sweep can test whether existing knobs resolve it.
- Target phase: Not set while disposition is `OPEN`.
- Before report references: Full eight-scenario live `report.json` artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `content.aggregate.metrics`, each named scenario's `metrics`, and `rationale_samples`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Pending disposition, remediation if selected, and the corresponding confirmation run.

### F-BASE-3: Entity-only selectivity leaves measurable non-entity hub expansion

- Finding ID: `F-BASE-3`
- Status: `OPEN`
- Scenario and metric: `recurring-hub-entity`; `fanout_decisions`, `stats_health_events`, `hub_context_share`, `sampled_context_pollution_rate`, `graph_relations_count`, and returned object kinds.
- Observed behavior: In the full-suite shipped regime, the hub query has zero selectivity decisions while six Episode traversal roots expand `mentions` and `involves` edges with selected caps of 16 and no fanout omissions; the context contains six Episodes and six Observations, `hub_context_share=1.0`, pollution `0.5`, and 216 graph relations. In the eval regime, all nine selectivity decisions are attached to three Entity roots, while Episode-root expansion remains outside selectivity and the context grows to six Episodes, six Observations, and six DerivedMemories with 324 graph relations, the same hub share, and the same pollution.
- Expected behavior: The §4.1 boundary is re-affirmed only if non-entity roots do not measurably concentrate hub-incident context; otherwise the limitation and its later-phase signal requirement are explicit.
- Severity: `major` (draft), because the phase guidance classifies hub flooding and high pollution as major.
- Suspected layer: `selectivity/fanout`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `defer` to v0.2, because the governing plan records that non-entity selectivity would require non-entity-keyed stats, which is a new signal outside v0.1.5.
- Disposition rationale: Pending the Task_4 user gate; current evidence answers §4.1 “yes” for measurable non-entity hub expansion, but the documented guardrail forbids inventing the required signal in this phase.
- Target phase: `v0.2` if the user selects `defer`.
- Before report references: Full-suite live artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect `report.json` `content.scenarios.recurring-hub-entity` and `traces.jsonl` trace `fixture_id=recurring-hub-entity`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Pending final disposition and any later-phase measurement.

### F-BASE-4: Conservative fallback dominates cold baseline selectivity

- Finding ID: `F-BASE-4`
- Status: `OPEN`
- Scenario and metric: Every full-suite scenario; `stats_health_events`, `conservative_fallback_activation_count`, and selectivity decision counts.
- Observed behavior: In the eval regime, correction is fallback-only at `9/9`; temporal uses fallback for `7/9`; long-gap, selective, thread, and cross-store use fallback for `5/9`; hub and mixed-salience use fallback for `3/9`. The aggregate mean is `5.25` fallback activations per query; the shipped regime is similar except hub has no decisions and thread uses `2/6` fallbacks, for an aggregate mean of `4.5`.
- Expected behavior: Task_7 tuning evidence distinguishes cold fallback-controlled paths from warmed scored paths so alpha and gamma are judged only where they actually bind, without weakening conservative fallback.
- Severity: `minor` (draft), because the fallback is conservative and no cap or integrity violation occurs, but unstratified sweeps would produce misleading tuning evidence.
- Suspected layer: `selectivity/fanout`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `accept-as-designed` for runtime fallback behavior, with a required warm-versus-cold stats split in Task_7 sweep evidence.
- Disposition rationale: Pending the Task_4 user gate; the phase explicitly forbids weakening conservative fallback, so the actionable change belongs to measurement design rather than the safety policy.
- Target phase: Not applicable if accepted as designed.
- Before report references: Full-suite live `report.json` artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`; inspect each scenario's `stats_health_events` and `metrics.conservative_fallback_activation_count`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Task_7 cold/warm sweep reports if the user accepts the draft proposal.

### F-HARNESS-1: Reports omit graph-root counters needed to verify the seeded finding

- Finding ID: `F-HARNESS-1`
- Status: `OPEN`
- Scenario and metric: `recurring-hub-entity`; facade telemetry `unique_graph_root_candidate_count`, `selected_graph_root_count`, and `graph_root_omission_count` versus the adapter trace/report projection and hardcoded `tuning_observations[id=entity_root_candidate_limit]`.
- Observed behavior: Character Memory exposes the three root-selection counters, but the full-suite live trace and report do not project them; the tuning observation still states that the shipped limit truncated Entity roots even though the current artifact cannot identify omitted root types and its fanout trace includes Entity traversal roots.
- Expected behavior: A finding about root truncation is supported by measured root-selection counters and type-aware evidence rather than a hardcoded prose assertion.
- Severity: `major` (draft), because the missing evidence blocks direct verification of a major seeded retrieval finding.
- Suspected layer: `fixture/harness defect`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `fix-now` in Task_6 by projecting the existing facade counters and deriving the tuning observation from measured telemetry; this remains a harness defect and does not count against the library.
- Disposition rationale: Pending the Task_4 user gate; no new Character Memory concept or signal is required because the counters already exist.
- Target phase: Not set while disposition is `OPEN`.
- Before report references: Full-suite live artifacts under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`, plus `crates/cmem-eval-continuity/src/report.rs` `tuning_observation`; inspect each run's `report.json` and `traces.jsonl`. Config identities are `configs/continuity_baseline_shipped.toml` and `configs/continuity_baseline_eval.toml`; Character Memory provenance is `main` at `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5`.
- After report references: Pending Task_6 disposition and rerun.

### F-HARNESS-2: Canonical row hashes retain run identity across required distinct runs

- Finding ID: `F-HARNESS-2`
- Status: `OPEN`
- Scenario and metric: Full-suite reproducibility pairs; README canonical normalized-row hashing recipe.
- Observed behavior: Same-regime traces and metadata-free report content are byte-identical, but README-normalized row hashes differ because `run_id` remains in every row; replacing only `run_id` with one sentinel after the documented latency normalization makes each pair byte-identical.
- Expected behavior: The canonical semantic row comparison for distinct-run reproducibility either normalizes identity metadata explicitly or states that row hashes are only comparable when run IDs match.
- Severity: `minor` (draft), because deterministic report content remains available and correct, but the documented row recipe cannot satisfy the Task_3 distinct-run comparison without one extra normalization rule.
- Suspected layer: `fixture/harness defect`.
- Disposition: `OPEN` pending the Task_4 user gate.
- Draft disposition proposal: `fix-now` in Task_6 or the adjacent harness documentation by defining the distinct-run identity normalization precisely.
- Disposition rationale: Pending the Task_4 user gate; this is a bounded evidence-contract defect and does not count against the library.
- Target phase: Not set while disposition is `OPEN`.
- Before report references: `results.jsonl` under `runs/continuity/v0-1-5-baseline/shipped-a/`, `runs/continuity/v0-1-5-baseline/shipped-b/`, `runs/continuity/v0-1-5-baseline/eval-a/`, and `runs/continuity/v0-1-5-baseline/eval-b/`, plus the Task_3 reproducibility hashes above; both config identities and Character Memory `main` commit `85b5f84f34c9f9601f3b5d4573ee5be3bd8b74f5` were used for the full-suite live evidence.
- After report references: Pending Task_6 disposition and rerun.
