---
status: accepted
adr_type: implementation
date: 2026-07-24
deciders: [ebigunso]
consulted: ["Claude Fable 5", "GPT-5.6 Sol"]
informed: []
warrant:
  signals: "cross-boundary contract shape; rejected alternative likely to be re-proposed"
  mode: "violate"
  cost: "incremental structural erosion whose unwind is a workspace-wide refactor"
depends_on: []
implements: []
supersedes: []
superseded_by: null
supersession_scope: null
---

# ADR-I-0001: Backend-neutral core with dataset-owned extension boundary

## Context and Problem Statement

The evaluation workspace hosts multiple benchmark datasets (synthetic, LongMemEval, LoCoMo, continuity) against a live Character Memory backend and a deterministic mock. Every new dataset or backend feature creates pressure to add convenient special cases to the core crate. Without a recorded boundary, the workspace's extension architecture erodes one justified exception at a time, and each exception passes review individually.

## Decision Drivers

- Adding a dataset must not destabilize or even touch the shared contracts other datasets depend on.
- Metrics, adapter, and result contracts must stay backend-neutral so mock and live execution stay comparable.
- Review scope for a dataset change should be the dataset's own crate plus at most a runner registration.

## Decision

`cmem-eval-core` holds only backend-neutral adapter, result, and metric contracts and never dispatches on dataset names. The live Character Memory integration lives exclusively in `crates/cmem-eval-adapter-cmem`, including deterministic collection naming and persisted reattach state. Each dataset crate owns its loader, ingest mapping, scorer, full-history construction, configuration-name validation, and metric-family declaration. Adding a dataset may add a runner `DatasetSpec`; it must not require editing core.

## Implementation Impact

The boundary is enforced by the crate ownership rules in `docs/coding-agent/rules/common.md`; reviewers treat any core diff inside a dataset addition as a finding. Internal structure within a dataset crate is unconstrained by this decision.

## Considered Options

1. Backend-neutral core with dataset-owned crates and runner registration (chosen).
2. Core dispatch on dataset names.
3. Extension-trait split of the adapter contract.

## Decision Outcome

Chosen option: **backend-neutral core with dataset-owned crates**. It keeps the seam reviewable, keeps mock/live comparability intact, and localizes dataset risk to dataset crates.

### Rejected Alternatives

Core dispatch on dataset names is likely to be re-proposed: it is the fastest way to ship a cross-dataset feature. Rejected because each dispatch arm erodes the neutrality the shared contracts depend on. Reopen if dataset proliferation or shared cross-dataset logic makes per-crate ownership demonstrably net-costlier than a governed core seam.

The extension-trait split was rejected during the eval-harness architecture revision: the Character-Memory-shaped main trait with staged writes carries the contract without a parallel trait hierarchy. Rejected outright — no reopen condition; it may be re-argued only on genuinely new evidence.

## Consequences

### Positive

- Dataset additions are additive: new crate plus runner registration, core untouched.
- Backend-neutral contracts keep every dataset's metrics comparable across mock and live runs.

### Negative / Tradeoffs

- Genuinely cross-dataset features need deliberate contract work in core instead of a quick special case.
- Some duplication across dataset crates is accepted as the price of ownership isolation.

## Decision Boundary

Invariant: core neutrality and the no-core-edits extension path.

Not covered: internal dataset-crate structure, and the runner's registration mechanics, which may evolve freely.

## Validation

Crate-ownership rules in `rules/common.md`; review finding on any core edit accompanying a dataset addition; the extension path is exercised every time a dataset lands without a core diff.

## Revisit When

The premise is that per-crate ownership stays cheap at the current dataset count and shape. Revisit if dataset proliferation or recurring cross-dataset logic makes the boundary net-costly, per option 2's reopen condition.

## Consultation impact

Identified as the strongest unrecorded architecture decision by the 2026-07 decision-record survey of this workspace; encoded retrospectively from the architecture-revision plan's Decision Log with calibration by ebigunso.

## More Information

Origin: eval-harness architecture-revision plan (Q2) and `rules/common.md` crate-ownership rules, 2026-07. Enforcing rules remain authoritative for day-to-day execution; this record owns the rationale and revisit conditions.
