---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "worker"
last_updated: "2026-07-14"
---

# Worker Repository Rules

## Repo-Specific Worker Notes

- Workers must not edit files outside their `owns` scope unless they report the reason and the exact paths changed.
- Dataset workers must keep gold labels out of adapter metadata and use them only in scorer/result output paths.
- Never move, relocate, or delete gitignored local assets (datasets, snapshots, manifests) during validation procedures: copy them when a second location is needed, and verify the originals still exist before removing any temporary worktree or directory. These assets can be expensive or impossible to regenerate.
- Never redirect AGMSG_STORAGE_PATH or send reports to an alternate/mirror database when the registered store rejects writes: escalate the write failure instead, and verify critical handoffs landed in the registered store before ending the turn.
- Any change touching a gated live test's setup, live-call phases, or skip predicate requires BOTH verification runs before completion: service deliberately unavailable (test must skip with its documented marker, full suite green) and service up (test must exercise and pass). Skip semantics: absence before the first successful live operation skips; unavailability after confirmed success fails.
- When validation depends on local or generated asset existence (datasets, fixtures, snapshots, manifests), use direct filesystem checks (`Test-Path`/`test -f`) or an explicit no-ignore search (`rg --files --no-ignore`); never infer absence from default rg/fd or tracked-file enumeration — gitignored assets can exist and be required.
- For repository-wide removal acceptance criteria (features, flags, identifiers, command forms), run the audit search from the repository root and explicitly exclude only documented historical or generated paths (e.g. `rg -n <removed-token> . --glob '!docs/coding-agent/plans/**'`); handpicked-path searches do not count as acceptance evidence.
- Every new public `Result`-returning artifact reader must ship corrupt or invalid-encoding, partial-input, and schema-version-rejection tests before completion.
- For every changed benchmark field, Workers must trace the value from fixture input through the live adapter DTO, persisted object, retrieval telemetry, metric, and report claim before accepting evidence.
- Every live-evidence claim must state scenario scope, config identity, and the Character Memory sibling commit/branch provenance inline; label scoped evidence as scoped when first reported.

## Repo CI / Checks Mapping

| Change Type | Required Checks | Notes |
|---|---|---|
| Core changes | `cargo test -p cmem-eval-core` | |
| LongMemEval changes | `cargo test -p cmem-eval-longmemeval` | |
| LoCoMo changes | `cargo test -p cmem-eval-locomo` | |
| Runner changes | `cargo test -p cmem-eval-runner` | |
| Adapter changes | `cargo test -p cmem-eval-adapter-cmem` | |
| Continuity changes | `cargo test -p cmem-eval-continuity` | Include the canonical fixture byte-identity test when fixture/generator code moves. |

## Mechanical Gate Candidates

- None.
