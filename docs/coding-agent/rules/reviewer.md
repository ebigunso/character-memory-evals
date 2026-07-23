---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "reviewer"
last_updated: "2026-07-14"
---

# Reviewer Repository Rules

## Repo-Specific Reviewer Notes

- Reviewer dispatches in this repo are mutation-free (owns=[]): report findings, lessons, and rule candidates through the orchestrator instead of editing files.
- Review in an isolated worktree pinned at the exact requested commit; never switch or run Cargo in a checkout another agent occupies.
- The `../CharacterMemory` path dependency resolves per-worktree; pin and state the sibling commit used for any live or `--locked` validation.

## Review Risk Hotspots

- admission_before_side_effect: public parsers/readers (fixture, trace) must reject malformed, partial, wrong-version, or contract-violating input before any backend I/O or state mutation.
- derived_artifact_congruence: assemblers of derived artifacts (report assembly, summaries) necessarily run after backend I/O; they must validate input identity, count, and order congruence before returning or publishing the artifact, failing closed on mismatch.
- fake_vs_production_contract: mock and live admission must agree; a value the mock accepts but the live adapter rejects (or vice versa) is a finding even when today's fixtures never hit it.
- parallel_list_drift: any vocabulary/enum duplicated across layers needs one canonical owner or a compile-time-exhaustive parity test; a duplicated list lacking such a parity mechanism is a finding (a layered duplicate protected by an exhaustive parity test is an accepted design).
- collection_semantics: item/object counts deduplicate stable identities; decision multiplicity belongs only in explicitly named volume fields; published rates must be bounded.
- summarize_parity: any re-derivation path (summarize, re-assembly) must reproduce the original run's registry/config/coverage exactly; parity regressions required.
- determinism_and_canonicalization: canonical-hash recipe changes must be reconciled against historical artifacts before accepting moved hashes; nondeterminism belongs only in declared metadata/normalization policy.
- dead_contract_surface: schema fields that no caller or runtime path reads are misleading surface; flag them for removal rather than documentation.

## Required Reviewer-Owned Evidence

| Trigger | Evidence Required | Source |
|---|---|---|
| Continuity driver/report/metric changes | Independent full-suite two-run reproducibility on the committed config: raw traces, normalized rows, and report content byte-identical outside the declared metadata/normalization policy | `cmem-eval run continuity` + README canonical recipes |
| Live adapter or persistence changes | Live adapter suite exercised (not skipped) with executed-test counts stated; restart/reattach assertions verified against real stores | `cargo test -p cmem-eval-adapter-cmem` with Qdrant up |
| Fixture/generator changes | Regenerated fixture byte-identity vs the checked artifact (state both SHA256 values) | generator CLI |
| Hash/evidence claims from Worker | Independently reproduce at least the canonical content hashes; unexplained movement is a blocker, not a footnote | committed configs + recipes |

## Review Heuristics

- Distinguish environment from delta before filing service-failure findings: control-run a known-good commit; an identical failure classifies the blocker as environmental (record the matching phase/signature), but the delta stays unvalidated until its required evidence actually succeeds.
- Evidence without provenance is not evidence: scenario scope, config identity, and CM sibling commit must be stated; scoped runs must be labeled scoped.
- Verify docs field-by-field against the serialized types they describe; reserve the word "snapshot" for embedded content sufficient to reconstruct the source.
- When validating cardinality-stable JSON in PowerShell, assert raw leading/trailing brackets or parse with `ConvertFrom-Json -NoEnumerate`; pipeline unrolling makes one-element arrays look scalar.

## Recurring Misses And Prevention

- Mock-passing hid a live-contract violation (source corrections with no original refs): for lifecycle/facade changes, demand a production-reachable regression, not mock coverage alone.
- "Exhaustive" sweeps missed sibling entrypoints (canonical-counts path, trace reader, fixture reader): boundary-closure audits trace every public mode in execution order through its first counting/hashing/sorting/indexing use.
- CRLF checkout normalization broke byte-identity tests only on fresh clones: verify portability evidence from a fresh materialization, never from existing worktree state.
- Concurrent Cargo commands on one target directory contend on the build lock, and unqualified `--exact` filters exit 0 with zero tests executed: serialize compile/test/lint on a shared target dir and always check the executed-test count.

## Mechanical Gate Candidates

- None.

- The full live two-run reproducibility gate triggers on changes that CAN alter successfully emitted artifact bytes (producers, serialization, DTO shapes) — not on any touch of a reader/report file; failure-path-only admission changes take offline evidence (trigger intent refined 2026-07-23 after a correct procedural hold).
