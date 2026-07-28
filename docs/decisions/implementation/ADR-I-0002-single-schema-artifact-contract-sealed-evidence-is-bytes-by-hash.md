---
status: accepted
adr_type: implementation
date: 2026-07-28
deciders: [ebigunso]
consulted: ["Claude Fable 5", "GPT-5.6 Sol"]
informed: []
warrant:
  warranted_by: "without this record, future work encountering sealed artifacts the live binary cannot read would likely re-add a legacy schema dispatch to the live readers or migrate the sealed bytes forward — both against the single-schema contract; both mishandlings actually occurred or were formally proposed in this repository's history"
  detected_signals: "cross-boundary contract and evidence-ownership shape; rejected alternatives likely to be re-proposed; a decider's ruling setting a durable governance default"
  cost_of_violation: "a re-added legacy dispatch recreates the remediated live-path exception one justified case at a time; a sealed-byte migration silently converts hash-cited evidence into new evidence wearing sealed run identities, corrupting the findings register's evidentiary meaning"
supersedes: []
superseded_by: null
supersession_scope: null
---

# ADR-I-0002: Single-schema artifact contract; sealed evidence is bytes-by-hash

## Context and Problem Statement

The evaluation workspace emits benchmark artifacts (result rows, continuity traces, run summaries, reports) under a versioned schema, and seals completed evidence runs by citing their exact bytes in the git-tracked findings register (`reports/v0-1-5-findings-register.md`). When the schema version advanced, the readers retained a bounded dispatch for the superseded version in the live read path so sealed artifacts stayed parseable. The 2026-07-24 ADR calibration ruled that state a design defect that must not be ratified: exceptions living in the live path erode the contract regardless of how bounded they start. Removing the exception forces the underlying question this record answers: what does the sealed-evidence guarantee actually consist of, and what happens to readers when the schema moves?

## Decision Drivers

- The findings register cites evidence by SHA-256 over exact bytes (raw bytes, latency-normalized row arrays, report content members), never by parseability; hash verification requires only generic JSON handling, not schema-typed readers.
- The live read path must carry zero knowledge of superseded schemas; a fenced quarantine of dead legacy readers fails the workspace's earns-its-place standard and ratifies the rejected exception shape one module over.
- Sealed artifacts must never be regenerated or edited to chase a surface change (Compatibility Policy, `rules/common.md`); a schema migration of sealed bytes is not a format change but the fabrication of new evidence under sealed run identities.
- At the 2026-07-28 ruling, 102 of the 203 register-associated sealed artifacts (every superseded-schema summary and report) were already unreadable by the live strict readers with no operational loss, demonstrating that sealed-but-unreadable is the tolerated and sufficient state.

## Decision

The artifact contract is single-schema: readers admit exactly the current schema version, fail-closed, and carry zero knowledge of superseded schema versions. On a schema bump, readers move with the schema — no dual dispatch, no legacy DTOs, no version enums in the live path, ever.

The sealed-evidence guarantee is bytes-by-hash: sealed artifacts are preserved verbatim and verified by the register's hash citations; parseability by the current binary is explicitly not part of the guarantee. When reader capability for a superseded schema is removed, the findings register records the last commit at which that capability existed (the resurrection pointer), converting deletion into archival by git history.

## Product / Philosophy Relevance

The register's authority rests on citing immutable bytes. Keeping that guarantee independent of the living codebase is what lets the code evolve aggressively (per the Compatibility Policy) without ever touching evidence.

## Implementation Impact

Live readers (`read_jsonl`, `read_continuity_traces`, summary and report readers) admit only the current schema version and return current DTOs directly, with no version wrapper types. Superseded-schema DTOs, dual dispatch, legacy projection helpers, and their re-exports are deleted. Any future genuine archival-read need is served by resurrecting the recorded reader commit into a standalone offline tool outside the workspace default build — never by re-adding live-path dispatch.

## Considered Options

1. Single-schema readers; sealed evidence preserved as hash-verified bytes with a resurrection pointer (chosen).
2. Bounded legacy dispatch retained in the live readers.
3. Migration of sealed artifacts to the current schema.
4. Containment: quarantine legacy reading in a distinct module or crate outside the live path.

## Decision Outcome

Chosen option: **single-schema readers with bytes-by-hash sealing**. It is the only option that removes live-path legacy knowledge entirely, leaves every sealed byte and register citation untouched, and scales to future schema bumps with zero accumulated surface.

### Rejected Alternatives

Bounded legacy dispatch is the remediated defect itself; ruled non-ratifiable on 2026-07-24 ("exceptions suck"). Rejected outright — re-adding it in any form recreates the defect; no reopen condition.

Migration was rejected as evidence fabrication: the superseded shapes lack fields of the current contract (dataset kind, embedding bindings, write/lifecycle outcomes, rendered context), so a rewrite requires either an invented derivation policy or reruns on exact historical frozen inputs; either way every cited hash changes and the register's citations cease to describe the bytes they cite. One register-cited artifact was also already absent locally, making a homogeneous migration infeasible. Rejected outright; no reopen condition — superseding evidence is produced by new sealed runs, never by rewriting old ones.

Containment preserves sealing but quarantines readers with zero production callers — dead code behind a fence that fails the earns-its-place standard on day one and requires a standing record ratifying a permanent legacy surface, the shape the calibration ruling refused. Reopen only via this record's revisit condition (a ruled, ongoing archival-read need), and then as a standalone offline tool, not a live-path quarantine.

## Consequences

- Positive: live readers shrink to a single strict path; the register's evidentiary meaning is uniform across the whole sealed corpus; schema bumps have a known, zero-debt playbook.
- Negative / tradeoffs: semantic readout of sealed legacy artifacts (recomputing metrics, regenerating official exports) requires resurrecting tooling from git history first; accepted because hash duties never need it, the plausible readout scenarios are better served by fresh runs, and the sealed files remain plain JSON readable by ad hoc tooling.

## Decision Boundary

Invariant: single-schema live readers with zero superseded-schema knowledge; sealed artifacts immutable and guaranteed by hash citation only; reader-capability removal always records a resurrection pointer.

Not covered: the current schema version number itself, the internal shape of future schema revisions, and the form of any future offline archival tool — all free to evolve through normal plan records.

## Validation

Reviewer greps for superseded-version literals and legacy symbols in live crates; strict-reader rejection regressions for synthesized legacy artifacts; the sealed corpus and register verified byte-identical across any reader change; `rules/common.md` Repo Naming / Structure clause enforces the strict-only contract at act time.

## Revisit When

The premise is that no ongoing consumer needs semantic readout of sealed superseded-schema artifacts. Revisit if a decider rules such a need has materialized (for example, a standing external submission pipeline over sealed legacy runs); the remedy shape is a standalone offline tool resurrected from the recorded commit, and only its necessity — not live-path dispatch — is reopened.

## Consultation impact

Forensic census (GPT-5.6 Sol, 2026-07-28) established the exact legacy surface and sealed-corpus inventory; design consult (Claude Fable 5, 2026-07-28) recommended deletion over containment and migration; ebigunso ruled Option C after a likelihood analysis of genuine readout scenarios.

## More Information

Provenance: 2026-07-24 ADR calibration ruling (agent-harness `adr-integration-plan.md` Decision Log); remediation plan `docs/coding-agent/plans/active/legacy-1-0-0-reader-removal-plan.md`. Pairs with [ADR-I-0003](ADR-I-0003-reader-admission-strictness-scoped-to-hash-cited-evidence.md), which scopes how strict the admitted-schema readers must be.
