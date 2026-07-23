---
status: accepted
adr_type: design
date: 2026-07-24
deciders: [ebigunso]
consulted: ["Claude Fable 5", "GPT-5.6 Codex"]
informed: []
warrant: "a, e / violate / costing silently invalid benchmark evidence that looks real"
depends_on: []
implements: []
supersedes: []
superseded_by: null
supersession_scope: null
---

# ADR-D-0001: Benchmark runs default to the live adapter; mock is loud opt-in

## Context and Problem Statement

The benchmark CLI can execute against the live Character Memory backend or a deterministic mock. A mock run that presents itself as a benchmark produces plausible-looking numbers with no evidential value. The failure is silent: nothing in an unlabeled mock result reveals that it never touched the system under test.

## Decision Drivers

- Benchmark evidence is the product; its trustworthiness outranks convenience.
- Failing loudly beats falling back silently at every point where evidence could be corrupted.
- Tests and CI still need deterministic, service-free execution — as validation, not as benchmark evidence.

## Decision

Benchmark CLI runs default to the live Character Memory adapter. Mock execution requires an explicit opt-in flag, and every mock-produced output is visibly labeled as mock/smoke. Service-free deterministic defaults remain the rule for tests and CI validation, which are a distinct execution class from benchmark runtime.

## Product / Philosophy Relevance

Evaluation evidence users can trust is this repository's core purpose. A default that can silently substitute the system under test with its mock violates that purpose even when every number computes correctly.

## Considered Options

1. Live default; mock as loud opt-in (chosen).
2. Mock default with live opt-in — likely to be re-proposed, because it is convenient for local iteration and CI symmetry. Reopen condition: the live-service dependency becomes prohibitively expensive or unavailable for routine benchmark work.
3. Automatic fallback from live to mock on service failure — rejected outright: it converts an infrastructure failure into silently invalid evidence.

## Decision Outcome

Chosen option: **live default, loud mock opt-in**. Accidental invalidity becomes impossible to miss: a benchmark run either exercises the real system or announces that it did not.

## Consequences

### Positive

- No unlabeled mock numbers can enter registers, reports, or comparisons.
- Infrastructure problems surface as failures at run time, not as anomalies discovered in analysis.

### Negative / Tradeoffs

- Routine benchmark work requires the live service to be up.
- Two execution classes (benchmark runtime vs. validation) must stay separately configured and documented.

## Decision Boundary

Invariant: the default-and-labeling contract for benchmark runs. Not covered: which flag spells the opt-in, label wording, and the service-free defaults of the test/CI class — those are calibrated surfaces owned by rules and configuration.

## Validation

The live-default and mock-opt-in rules in `rules/common.md`; smoke invocations pass the explicit opt-in flag; reviewers treat unlabeled mock output paths as findings.

## Revisit When

The premise is that a local live Character Memory service remains cheap enough for routine benchmark runs. Revisit under option 2's reopen condition.

## Consultation impact

Encoded retrospectively from the public-API eval-adapter plan's Decision Log after the 2026-07 decision-record survey; the user's original correction ("make the default run be a live eval run") is the founding ruling.

## More Information

Origin: character-memory-public-api-eval-adapter plan Decision Log and `rules/common.md` safety rules, 2026-07. The rules remain the always-loaded enforcement; this record owns the why and the reopen conditions.
