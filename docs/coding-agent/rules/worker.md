---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "worker"
last_updated: "2026-07-23"
---

# Worker Repository Rules

## Repo-Specific Worker Notes

- Dataset workers must keep gold labels out of adapter metadata and use them only in scorer/result output paths.
- Never move, relocate, or delete gitignored local assets (datasets, snapshots, manifests) during validation procedures: copy them when a second location is needed, and verify the originals still exist before removing any temporary worktree or directory. These assets can be expensive or impossible to regenerate.
- Never redirect AGMSG_STORAGE_PATH or send reports to an alternate/mirror database when the registered store rejects writes: escalate the write failure instead, and verify critical handoffs landed in the registered store before ending the turn.
- Gated live tests: any change to setup, live-call phases, or skip predicate requires a service-up run in which the test exercises and passes (the service-down skip verification is harness-owned). Skip semantics: absence before the first successful live operation skips; unavailability after confirmed success fails.
- Every new public `Result`-returning artifact reader must ship corrupt or invalid-encoding, partial-input, and schema-version-rejection tests before completion.
- For every changed benchmark field, Workers must trace the value from fixture input through the live adapter DTO, persisted object, retrieval telemetry, metric, and report claim before accepting evidence.
- New validators and admission checks classify their failures with an owned structured error type AT INTRODUCTION (typed variants/fields per the design's error conventions), with tests asserting variants and fields; anyhow/prose belongs only at outer boundaries. Three same-phase recurrences of retrofitting prose validators forced this rule (2026-07-23).
- Before generating a dataset artifact, verify that the source-only input contains every non-label field required by evaluation semantics; missing required metadata must be corrected at the source rather than replaced with fallback semantics (lesson 2026-05-04).
- Every public benchmark-fixture field must have an authoritative runtime consumer; remove fields that terminate in generation or validation, and do not expose backend-generated identities that fixture callers cannot control end to end (recurred 2026-07-14, rounds 5-6).

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
