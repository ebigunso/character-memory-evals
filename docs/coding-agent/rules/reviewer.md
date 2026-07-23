---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "reviewer"
last_updated: "2026-07-23"
---

# Reviewer Repository Rules

## Repo-Specific Reviewer Notes

- Review in an isolated worktree pinned at the exact requested commit; never switch or run Cargo in a checkout another agent occupies.
- The `../CharacterMemory` path dependency resolves per-worktree; pin and state the sibling commit used for any live or `--locked` validation.

## Review Risk Hotspots

- Optional-diagnostics metric staging: every emitted metric binds to exactly one named stage of the chained limiter pipeline (eligible -> hub cap -> fanout cap); enumerate per-stage producer cardinality before approving optional diagnostics.

- admission_before_side_effect: public parsers/readers (fixture, trace) must reject malformed, partial, wrong-version, or contract-violating input before any backend I/O or state mutation.
- derived_artifact_congruence: assemblers of derived artifacts (report assembly, summaries) must validate input identity, count, and order congruence before returning or publishing, failing closed on mismatch (conservation generality is harness-owned: review-latent-risk-conservation).
- collection_semantics: item/object counts deduplicate stable identities; decision multiplicity belongs only in explicitly named volume fields; published rates must be bounded.
- summarize_parity: any re-derivation path (summarize, re-assembly) must reproduce the original run's registry/config/coverage exactly; parity regressions required.
- determinism_and_canonicalization: canonical-hash recipe changes must be reconciled against historical artifacts before accepting moved hashes; nondeterminism belongs only in declared metadata/normalization policy.

## Required Reviewer-Owned Evidence

| Trigger | Evidence Required | Source |
|---|---|---|
| Continuity driver/report/metric changes | Independent full-suite two-run reproducibility on the committed config: raw traces, normalized rows, and report content byte-identical outside the declared metadata/normalization policy | `cmem-eval run continuity` + README canonical recipes |
| Live adapter or persistence changes | Live adapter suite exercised (not skipped) with executed-test counts stated; restart/reattach assertions verified against real stores | `cargo test -p cmem-eval-adapter-cmem` with Qdrant up |
| Fixture/generator changes | Regenerated fixture byte-identity vs the checked artifact (state both SHA256 values) | generator CLI |
| Hash/evidence claims from Worker | Independently reproduce at least the canonical content hashes; unexplained movement is a blocker, not a footnote | committed configs + recipes |

## Review Heuristics

- Verify docs field-by-field against the serialized types they describe; reserve the word "snapshot" for embedded content sufficient to reconstruct the source.

## Recurring Misses And Prevention

- CRLF checkout normalization broke byte-identity tests only on fresh clones: verify portability evidence from a fresh materialization, never from existing worktree state.
- Concurrent Cargo commands on one target directory contend on the build lock: serialize compile/test/lint commands that share a target dir.

## Mechanical Gate Candidates

- None.

- The full live two-run reproducibility gate triggers on changes that CAN alter successfully emitted artifact bytes (producers, serialization, DTO shapes) — not on any touch of a reader/report file; failure-path-only admission changes take offline evidence (trigger intent refined 2026-07-23 after a correct procedural hold).
