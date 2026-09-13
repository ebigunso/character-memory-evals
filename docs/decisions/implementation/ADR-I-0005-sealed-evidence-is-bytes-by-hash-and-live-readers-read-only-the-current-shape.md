---
status: proposed
adr_type: implementation
date: 2026-09-13
deciders: [ebigunso]
consulted: ["Claude Fable 5.1"]
informed: []
warrant:
  warranted_by: "without this record, future work meeting a sealed artifact the live binary cannot parse would likely re-add a schema version and strict readers, or migrate the sealed bytes"
  detected_signals: "cross-boundary evidence-ownership shape; rejected alternatives likely to be re-proposed; a decider's ruling setting a durable governance default"
  cost_of_violation: "a re-added version ritual taxes every shape change while serving no decision; a sealed-byte migration silently turns cited evidence into new evidence"
  cost_of_over_extension: "treating input contracts (fixture, config, frozen-store admission) as artifact readers would strip the fail-closed checks that protect measurement inputs"
supersedes: ["../superseded/ADR-I-0002-single-schema-artifact-contract-sealed-evidence-is-bytes-by-hash--superseded-by-ADR-I-0005.md"]
superseded_by: null
supersession_scope: full
---

# ADR-I-0005: Sealed evidence is guaranteed as bytes by hash; live readers read only the current artifact shape

## Context and Problem Statement

Evaluation artifacts (result rows, traces, summaries, reports) change shape as the harness shrinks, and sealed evidence cited by the findings register must stay trustworthy across those changes. Two guarantees were once conflated: that sealed bytes are preserved and verifiable, and that the live binary can parse every sealed artifact. Keeping the second guarantee forced strict versioned readers, version bumps on every shape change, and resurrection rituals, none of which any everyday decision needs: running the evaluation and comparing two runs, or judging a tuning or regression question, never requires the live binary to read an artifact from an older shape; only a durable claim cited in a decision record does, and that claim is served by the sealed bytes and their hashes.

## Decision

Sealed evidence is guaranteed as bytes verified by the hashes the findings register cites, never as parseability by the live binary. Live readers deserialize the current artifact shape through derived serde and carry no schema version, no strictness against unknown fields, and no knowledge of superseded shapes; every run states its provenance in one run header. An old artifact is old: semantic readout of a superseded shape is served by fresh runs or by an offline tool resurrected from the commit the register names, never by dispatch in the live readers.

## Why

The evidence the register protects is bytes, and hashes protect bytes completely; a parser promise on top of that buys nothing for the everyday run-and-compare and tuning decisions the harness serves, and costs a versioning ritual on every change.

## Rejected Alternatives

- Strict single-schema readers that fail closed on any shape drift (the superseded record): rejected because the strictness guarded a parser promise nobody consumes; reopen if a consumer outside this repository must read artifacts across versions.
- Live-path dispatch on superseded schema versions: rejected outright; it recreates a permanent exception in the read path.
- Migrating sealed bytes to the current shape: rejected outright; it silently turns cited evidence into new evidence.

## Decision Boundary

Invariant: the evidence guarantee is bytes by hash, and live readers know one shape.

Not covered: the run header's field list, which artifact families exist, and the form of any offline resurrection tool.

## Validation

Readers are plain derived serde with no version constant or unknown-field rejection; every run writes a header; the register's hash checks pass byte-identically across reader changes; a review question on any reader change asks whether it added a shape check that serves no decision.

## Revisit When

The premise is that no consumer outside this repository needs the live binary to read superseded artifacts. A standing external consumer of sealed artifacts reopens the reader promise, not the bytes guarantee.

## More Information

Replaces ADR-I-0002 in full; the register keeps the resurrection pointers ADR-I-0002 required, recorded when reader capability is removed.
