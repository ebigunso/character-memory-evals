---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "worker"
last_updated: "2026-09-23"
---

# Worker Repository Rules

## Repo-Specific Worker Notes

- Dataset workers must keep gold labels out of adapter metadata and use them only in scorer/result output paths.
- Never move, relocate, or delete gitignored local assets (datasets, snapshots, manifests) during validation procedures: copy them when a second location is needed, and verify the originals still exist before removing any temporary worktree or directory. These assets can be expensive or impossible to regenerate.
- Never redirect AGMSG_STORAGE_PATH or send reports to an alternate/mirror database when the registered store rejects writes: escalate the write failure instead, and verify critical handoffs landed in the registered store before ending the turn.
- Never use `git add -f` for a path under `.agent-work/`; discard scratch artifacts once read or promote cited evidence through the storage tiers in `common.md`.
- Before promotion, state the durable citation, storage tier and new-file byte total from `git ls-tree -r -l`; main receives only markdown readings and their manifest within the 256 KiB ceiling, except register-cited sealed runs, which stay whole under `evidence/`.
- Adapter tests run unconditionally with embedded stores; service lifecycle and collection-administration changes also require the `service-tests` feature suite against Qdrant, and an unavailable service is a failure.
- For every changed metric field that measures the benchmark character, Workers must trace the value from fixture input through the live adapter DTO, persisted object, retrieval telemetry, metric, and report claim before accepting evidence.
- New validators and admission checks on library-facing surfaces classify failures with an owned structured error type at introduction; notebook and one-off validators may use plain error context.
- Before generating a dataset artifact, verify that the source-only input contains every non-label field required by evaluation semantics; missing required metadata must be corrected at the source rather than replaced with fallback semantics (lesson 2026-05-04).
- Every public benchmark-fixture field must have an authoritative runtime consumer; remove fields that terminate in generation or validation, and do not expose backend-generated identities that fixture callers cannot control end to end (recurred 2026-07-14, rounds 5-6).
- A measurement runner checks every write outcome and aborts the run on a degraded one; a run that reports numbers after a degraded write is invalid evidence and is discarded, never annotated (lesson 2026-09-22).

## Repo CI / Checks Mapping

| Change Type | Required Checks | Notes |
|---|---|---|
| Shared evaluation or adapter changes | `cargo test -p cmem-eval` | Add `--features service-tests service_mode_` for service lifecycle or collection administration. |
| LongMemEval changes | `cargo test -p cmem-eval-longmemeval` | |
| LoCoMo changes | `cargo test -p cmem-eval-locomo` | |
| Runner changes | `cargo test -p cmem-eval-runner` | |
| Continuity changes | `cargo test -p cmem-eval-continuity` | Include the canonical fixture byte-identity test when fixture/generator code moves. |

## Mechanical Gate Candidates

- None.
