---
status: accepted
adr_type: implementation
date: 2026-09-13
deciders: [ebigunso]
consulted: ["Claude Fable 5.1"]
informed: []
warrant:
  warranted_by: "without this record, future work adding a dataset would likely put a convenient special case into the shared crate, or re-split the shared crate to host a second backend that does not exist"
  detected_signals: "cross-boundary contract shape; rejected alternative likely to be re-proposed; a decider's ruling setting a durable governance default"
  cost_of_violation: "each dataset dispatch arm in the shared crate erodes the extension boundary until dataset additions stop being additive"
  cost_of_wrong_preservation: "if a second real backend must be evaluated through the same contract, keeping one crate forces backend-specific branches into shared code instead of a governed seam"
supersedes: ["../superseded/ADR-I-0001-backend-neutral-core-with-dataset-owned-extension-boundary--superseded-by-ADR-I-0004.md"]
superseded_by: null
supersession_scope: full
---

# ADR-I-0004: One shared evaluation crate holds the contracts and the library integration; each dataset owns its own crate

## Context and Problem Statement

The evaluation workspace once separated a backend-neutral core crate from the live library adapter crate so that a deterministic mock and the live backend stayed comparable through one contract. The mock is gone: the library runs service-free through its embedded vector store, so every benchmark run exercises the system under test and there is no second backend to stay neutral toward. That leaves the question of where the shared contracts and the library integration live, and what a new dataset may touch.

## Decision

One shared crate holds the adapter, result, and metric contracts together with the library integration, and never dispatches on dataset names. Each dataset crate owns its loader, ingest mapping, scorer, full-history construction, and configuration validation; adding a dataset is a new crate plus a runner registration, with no shared-crate edit.

## Why

A port abstraction with a single implementation is a boundary that protects nothing, so the shared crate and the library integration are one; the dataset boundary still earns its place because it keeps every dataset addition additive and its review scope local.

## Rejected Alternatives

- Keeping separate core and adapter crates: rejected because the only other implementation of the contract was the deleted mock; reopen if a second real backend must be evaluated through the same contract.
- Dispatching on dataset names inside the shared crate: rejected because each arm erodes the extension boundary; reopen if recurring cross-dataset logic makes per-crate ownership demonstrably net-costlier than a governed shared seam.

## Decision Boundary

Invariant: no dataset dispatch in the shared crate, and no shared-crate edit as part of a dataset addition.

Not covered: the shared crate's internal module layout, the runner's registration mechanics, and the internal structure of dataset crates.

## Validation

Crate-ownership rules in `rules/common.md`; a review finding on any shared-crate edit accompanying a dataset addition; the extension path is exercised every time a dataset lands without a shared-crate diff.

## Revisit When

The premise is that one real backend is evaluated and per-crate dataset ownership stays cheap at the dataset count of 2026-09. A second real backend or recurring cross-dataset logic reopens the crate split or the dispatch question under the reopen conditions above.

## More Information

Replaces ADR-I-0001 in full. The service-free execution path is the library's embedded vector store, decided in the library repository's ADR-I-0023.
