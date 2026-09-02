---
status: deprecated
adr_type: implementation
date: 2026-07-28
deciders: [ebigunso]
consulted: ["Claude Fable 5"]
informed: []
warrant:
  warranted_by: "without this record, future work would likely either weaken evidence-reader admission to mirror producer serde permissiveness, or extend manual strict Deserialize across every mirrored type in the workspace — one direction erodes the trust boundary, the other proliferates hand-written deserialization with no measurement payoff"
  detected_signals: "deliberately bounded scope; cross-boundary contract shape; a decider's ruling setting a durable governance default"
  cost_of_violation: "a reader that inherits producer-derive tolerance admits shapes the producer can never emit, so corrupted or hand-edited evidence parses silently and hash-cited findings lose their fail-closed guarantee"
  cost_of_over_extension: "manual Deserialize scattered across non-evidence mirrors is a standing maintenance tax and drift surface justified by no trust boundary, and each instance passes review individually"
depends_on: ["../implementation/ADR-I-0002-single-schema-artifact-contract-sealed-evidence-is-bytes-by-hash.md"]
supersedes: []
superseded_by: null
supersession_scope: null
---

# ADR-I-0003: Reader admission strictness scoped to hash-cited evidence

## Context and Problem Statement

The workspace mirrors serialization types across the producer/reader boundary for its benchmark artifacts. Producer types use derived serde, which tolerates more than the producer can ever emit (unknown fields, duplicate keys, shape variants). If evidence readers inherit that tolerance, artifacts that no run could have produced still parse, and the hash-cited findings register sits on readers that cannot vouch for what they admit. The opposite failure is just as real: once strict manual deserialization exists, every mirrored type in the workspace becomes a candidate for it, one plausible extension at a time.

## Decision Drivers

- Admission strictness is a property of the trust boundary a reader guards, not a mirror of the producer's serde behavior.
- The trust boundary in this workspace is the hash-cited evidence surface: result rows, run summaries, continuity traces, and reports.
- Strictness has real cost (manual Deserialize implementations, serialization-shape fidelity proofs); cost without a guarded boundary is pure debt.

## Decision

A strict evidence reader must accept everything its producer can emit — proven by round-tripping every emittable variant — and may reject anything beyond that, regardless of what the producer's own derived Deserialize would tolerate. Producer derive permissiveness is never a license to weaken reader admission.

This strictness binds exactly the hash-cited evidence readers: result rows, run summaries, continuity traces, and reports. It does not extend to other mirrored types; extending it to a new boundary requires new evidence that the boundary guards trust, not analogy to this one.

Where serde attributes cannot express the required strictness, implement manual Deserialize on the mirrored type itself — never per-field `deserialize_with` scatter.

## Implementation Impact

Evidence readers are strict fail-closed for the current schema (per [ADR-I-0002](../implementation/ADR-I-0002-single-schema-artifact-contract-sealed-evidence-is-bytes-by-hash.md)): duplicate-key rejection, unknown-shape rejection, and round-trip fidelity tests against every producer-emittable variant. Non-evidence mirrored types keep derived serde unless a future ruling establishes a new trust boundary.

## Considered Options

1. Strictness as a trust-boundary property, scoped to hash-cited evidence readers (chosen).
2. Reader admission mirrors producer serde behavior symmetrically.
3. Workspace-wide strict manual deserialization for all mirrored types.

## Decision Outcome

Chosen option: **trust-boundary-scoped strictness**. It puts the full strictness cost exactly where hash-cited evidence depends on it and nowhere else.

### Rejected Alternatives

Producer-mirroring admission is the intuitive default and will be re-proposed whenever a strict reader rejects something a derive would accept. Rejected because the producer's derive tolerance describes what serde happens to permit, not what the producer emits; the reader's job is to vouch for evidence, and the asymmetry is the point. Reopen only if the evidence surface stops being hash-cited.

Workspace-wide strictness was rejected by the 2026-07-22 design-value audit: unbounded application licenses manual-Deserialize proliferation without measurement payoff. Reopen per the boundary clause — a specific new surface may adopt strictness on evidence that it guards a trust boundary, which is an extension ruling, not a default.

## Consequences

- Positive: evidence admission is fail-closed and provable; strictness cost stays bounded to four reader families; new-boundary proposals get a named test instead of drifting in by analogy.
- Negative / tradeoffs: the strict readers carry manual Deserialize and round-trip proof obligations that derived serde would not; intentional asymmetry between producer and reader code must be understood by maintainers rather than looking like an inconsistency to "fix".

## Decision Boundary

Invariant: the accept-everything-emittable / may-reject-everything-else asymmetry for hash-cited evidence readers, and the scope limit to exactly those readers.

Not covered: which serde mechanics implement strictness for a given type (beyond the no-per-field-scatter rule), and test organization for round-trip proofs.

## Validation

Round-trip tests over every producer-emittable variant for each evidence reader; rejection regressions for beyond-emittable shapes; reviewer finding on any per-field `deserialize_with` in evidence types or any manual Deserialize appearing outside the four evidence families without a recorded extension ruling.

## Revisit When

The premise is that hash-cited evidence readers are the workspace's only trust boundary of this kind. Revisit when a new reader surface is claimed to guard trust (rule on extension with evidence), or if evidence citation stops being hash-based (the asymmetry's rationale expires with it).

## Consultation impact

Consult-ruled 2026-07-22 (Claude design consult, structured-verdict phase); scope bounded by the 2026-07-22 design-value audit; encoded 2026-07-28 with calibration by ebigunso.

## More Information

Origin: `rules/common.md` Repo Naming / Structure admission-strictness clause. Retired 2026-09-02 because the harness right-sizing audit found its premise contradicted ADR-I-0002: hash verification needs no typed reader, so the strict admission machinery did not protect the cited evidence claim.
