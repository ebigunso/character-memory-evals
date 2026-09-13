# Decision Records

This directory contains decision records (ADRs), split into two tracks so high-level design decisions do not mix with implementation choices. New records follow [template.md](template.md).

## Directory layout
```text
docs/decisions/
  README.md
  template.md
  design/
    (none)
  implementation/
    ADR-I-0004-...
  superseded/
    ADR-D-0002-...--superseded-by-ADR-D-0009.md
    ADR-I-0003-...--retired.md
```

## Numbering and tracks

Separate numbering per track; IDs are never reused.
- `ADR-D-NNNN` — design track: use when overlooking the decision would risk violating the project's core philosophy.
- `ADR-I-NNNN` — implementation track: use when the decision is primarily about how the system is built (storage, APIs, schemas, tooling, operations).

## Lifecycle and the superseded/ archive
- Active track directories list only governing decisions; numbering gaps signal archived history.
- On full supersession or retirement, the record moves to `superseded/` (a record superseded by another carries `superseded` and reciprocal links; a record retired without a replacement carries `deprecated`) — a single flat folder where the track prefix in the filename preserves identity — renamed with a self-describing suffix: `--superseded-by-ADR-X-NNNN` or `--retired`.
- Partial supersession stays in place: the record remains authoritative for its surviving clauses, with `supersession_scope` and reciprocal frontmatter links recording the split.

## Status values
`proposed` (branch only: a drafted record awaiting the decider's explicit acceptance; it never merges to `main` in this state), `accepted`, `rejected`, `superseded`, `deprecated`. Records capture decisions, not undecided proposals; a proposed record is a decision awaiting its yes, and the plan's Decision Log carries the proposal.
