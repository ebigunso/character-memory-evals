---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "reviewer"
last_updated: "2026-09-02"
---

# Reviewer Repository Rules

## Repo-Specific Reviewer Notes

- Review in an isolated worktree pinned at the exact requested commit; never switch or run Cargo in a checkout another agent occupies.
- The `../CharacterMemory` path dependency resolves per-worktree; pin and state the sibling commit used for any live or `--locked` validation.

## Review Risk Hotspots

- Optional-diagnostics metric staging: every emitted metric binds to exactly one named stage of the chained limiter pipeline (eligible -> hub cap -> fanout cap); enumerate per-stage producer cardinality before approving optional diagnostics.

- admission_before_side_effect: public parsers/readers (fixture, trace) must reject malformed, partial, wrong-version, or contract-violating input before any backend I/O or state mutation.
- coupled_config_invariants: coupled configuration invariants must be validated consistently at configuration admission, artifact production, persisted metadata, and the production-reachable live consumer (recurred 2026-07-12/2026-07-20).
- recursive_config_admission: configuration schema changes that introduce or tighten nested overrides must audit fail-closed unknown-field admission through every deserialized container from the edited leaf to the run-config root, testing incomplete atomic groups plus a typo at each covered level (recurred 2026-07-17/2026-07-21).
- label_conflict_precedence: metrics that project labels across provenance or grouping boundaries must define and test precedence for conflicting explicit-object and derived-root labels (broken pollution metric, 2026-07-17).
- scenario_metric_dispatch: changes to `ScenarioPattern` must classify every variant through each semantic metric dispatcher and prove numeric support with a run plus a diff against the previous run, explaining rank, metric, and degradation movement for every newly routed family (broken metric output, 2026-07-21).
- converter_attribution: dataset converters whose source turns carry speaker, author, participant, or actor metadata must preserve behavioral text bytes and encode that attribution through native fixture references, with evidence for both properties (broken benchmark graphs, 2026-07-21).
- collection_semantics: item/object counts deduplicate stable identities; decision multiplicity belongs only in explicitly named volume fields; published rates must be bounded.
- determinism_and_canonicalization: seal command recipe changes must be reconciled against historical artifacts before accepting moved hashes.

## Required Reviewer-Owned Evidence

| Trigger | Evidence Required | Source |
|---|---|---|
| Continuity driver/report/metric changes | Run `diff` against the stored baseline; run twice only when sealing | `cmem-eval diff` |
| Live adapter or persistence changes | Live adapter suite exercised (not skipped) with executed-test counts stated; restart/reattach assertions verified against real stores | `cargo test -p cmem-eval-adapter-cmem` with Qdrant up |
| Fixture/generator changes | Regenerated fixture byte-identity vs the checked artifact (state both SHA256 values) | generator CLI |
| Sealing changes | Independently reproduce at least the canonical content hashes; unexplained movement is a blocker, not a footnote | committed configs + seal command recipe |
| Adapter lifecycle changes | Fresh open, intended reattach, and fresh-instance reset/cleanup tested across every durable store and identity, including phase-local configuration isolation and a surviving sibling for destructive scope | six recurrences, 2026-07-12 |

## Review Heuristics

- Verify docs field-by-field against the serialized types they describe; reserve the word "snapshot" for embedded content sufficient to reconstruct the source.

## Recurring Misses And Prevention

- CRLF checkout normalization broke byte-identity tests only on fresh clones: verify portability evidence from a fresh materialization, never from existing worktree state.
- Concurrent Cargo commands on one target directory contend on the build lock: serialize compile/test/lint commands that share a target dir.

## Mechanical Gate Candidates

- None.

- Run twice only when sealing; otherwise compare the run to the stored baseline with `diff`.
