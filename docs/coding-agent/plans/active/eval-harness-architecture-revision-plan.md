# Plan: Eval Harness Architecture Revision for Continuity-Readiness

- status: in_progress
- generated: 2026-07-05
- last_updated: 2026-07-05
- work_type: code

Context: this repo was assembled quickly around LongMemEval-S/LoCoMo. Before the continuity evaluation harness (sibling crate milestone v0.1.4) lands here as a new dataset crate, revise the architecture so new dataset kinds plug in without editing core dispatch, the adapter contract covers the full character_memory facade with a restart-capable identity story, and the report/metric-registry format is stable and versioned. This plan is the gating pre-phase tracked as Task_4 of the sibling repo's v0.1.4 plan.

## Goal

- Restructure CharacterMemoryEvals so a continuity dataset crate can plug in without core edits, extend the adapter contract to the full character_memory facade with restart-capable identity mapping, and stabilize the report/metric registry — before any continuity feature work lands.

## Definition of Done

- Live adapter lives in its own crate; runner `commands.rs` holds only CLI parsing and thin dispatch (≤ ~300 lines of CLI concerns).
- Adapter contract covers correct/forget/link and prepare/validate_plan/commit; mock implements all ops with documented minimal semantics.
- external_id↔MemoryId mapping is a reusable component with a documented restart/reattach lifecycle; collection naming is deterministic per (prefix, run_id, namespace); a reattach round-trip test exists.
- Adding a dataset kind requires zero edits to `cmem-eval-core` dispatch code (config validation and metric registration are extension points).
- JSONL rows and summaries carry `schema_version`; volatile values (latency) isolated from diffable metric keys.
- CI runs fmt/clippy/test + a feature-gated adapter build check + the mock smoke run; `rust-toolchain.toml` pins the toolchain.
- Repo validation green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, mock synthetic smoke run.
- Docs/rules updated: crate-boundary rules, intended home for the future continuity crate, refreshed reference-document entries.

## Scope / Non-goals

- Scope: workspace restructuring (adapter crate extraction), adapter contract v2, restart-capable identity mapping + deterministic collection naming, dataset pipeline extraction + per-dataset config seam, metric family registry + report schema versioning, CI/toolchain/manifest hygiene, docs/rules refresh.
- Non-goals: implementing the continuity dataset crate, fixture generator, controllable-similarity embeddings, or continuity metrics (those belong to the sibling repo's v0.1.4 plan, gated on this one); changing scoring semantics of existing benchmarks; changing character_memory itself; multi-backend adapter genericity (no non-CharacterMemory baselines planned); dyn/plugin dataset registry (static dispatch stays); shared ingest abstraction across longmemeval/locomo (real differences, low duplication); typed metric structs (BTree-ordered JSON already gives diffability); BM25 relocation; standalone progress-logging rework (fold into pipeline extraction opportunistically).

## Context (workspace)

- Adapter trait gap: `crates/cmem-eval-core/src/memory_adapter.rs:212-233` — only reset/remember*/retrieve; facade also has prepare/validate_plan/commit, link, correct, forget.
- Restart blockers: `crates/cmem-eval-runner/src/real_adapter.rs:36-48` (nine in-process HashMaps of external_id↔MemoryId), `:161-175` (`Uuid::new_v4` in collection names — unrecoverable after restart), `:351-364` (reset drops in-memory state only). Deterministic UUIDv5 object ids already exist (`real_adapter.rs:24,395,453`) and `raw_ref`/`source_conversation_id` are written deterministically (`:398-402,456-459`) — a store-side reattach path exists in principle.
- Live adapter placement: `commands.rs:18-20` includes the 1611-line adapter as a private feature-gated submodule of the CLI module — unreusable by other crates.
- Dataset extension pain: `config.rs:43-61` string-matched dataset validation; `commands.rs:152-573` three near-identical run pipelines; `commands.rs:827-875` duplicated full-history builders.
- Metric registry: `metrics.rs:5-33` hardcoded `REQUIRED_REGISTRY_METRICS` const in core; `results.rs:12-54` no schema_version. `metric_support` null-tracking semantics (`metrics.rs:155-182`) are good — preserve.
- Determinism positives to preserve: BTree-backed `serde_json::Map` (sorted keys), explicit ranking tie-breaks (`memory_adapter.rs:382-387,486-491`), UUIDv5 ids. Leaks to close: default embedding provider is `"openai"` (`config.rs:315-321`); latency interleaved with deterministic metric keys (`commands.rs:770-773`).
- Hygiene: no CI, no `rust-toolchain.toml`; workspace path dep carries a no-op `default-features = false`; Cargo.lock currently matches the sibling crate version (0.1.2) but nothing catches future drift.
- Repo rules consulted: `docs/coding-agent/rules/{index,common,orchestrator}.md`.

## Open Questions (max 3)

- None currently. (Q1 feature gate: resolved by orchestrator assessment — drop the gate; see Decision Log. Q2 contract shape: user approved Character-Memory-shaped main trait. Q3: user approved schema_version + no compatibility mode, with the constraint that latency remains a first-class reported metric — only isolated from diff-stability comparison, never dropped.)

## Assumptions

- A1: Restart identity (per the sibling repo's Task_2 audit, 2026-07-05): the primary mechanism is caller-supplied deterministic MemoryIds in drafts (public draft types accept ids; RememberOutcome returns persisted ids) plus a harness-persisted external_id↔MemoryId registry serialized to the run directory. Store-side rediscovery via retrieval is relevance-dependent and incomplete by design — use it only as post-restart verification, not for re-association. No sibling-crate facade change is needed or planned.
- A2: `DeterministicEmbeddingProvider` moves to `cmem-eval-core` (it is pure hashing, no heavy deps) so both the live adapter crate and the future continuity fixture generator can use it without feature-gate friction.
- A3: Mock adapter semantics stay minimal-but-honest (e.g. forget = remove) — no lifecycle re-simulation; live deterministic runs carry correctness burden.
- A4: Existing three datasets must produce byte-identical mock-run outputs (latency-masked) across the pipeline refactor.
- A5: New continuity DTOs mirror the facade's staged-write types loosely (own DTOs, thin mapping) to absorb sibling-crate API drift during v0.1.4.

## Tasks

### Task_1: Extract live adapter into a dedicated adapter crate

- type: impl
- owns:
  - crates/cmem-eval-adapter-cmem/** (new)
  - crates/cmem-eval-runner/src/real_adapter.rs (move/delete)
  - crates/cmem-eval-runner/src/commands.rs (imports/adapter construction only)
  - crates/cmem-eval-runner/Cargo.toml
  - Cargo.toml (workspace members)
- depends_on: []
- description: |
  Move the live CharacterMemory adapter out of the runner into a new crate `cmem-eval-adapter-cmem`. Remove the `real-character-memory` feature gate entirely (Q1 resolution): the adapter crate is a normal workspace member, the runner depends on it unconditionally, and `--adapter real` is always available. Update docs/commands that mention the feature flag. Move `DeterministicEmbeddingProvider` to `cmem-eval-core` (A2) with a re-export for compatibility. Behavior identical; existing unit tests move with the code.
- acceptance:
  - `real_adapter.rs` no longer exists under the runner; adapter code + tests live in the new crate; runner depends on it unconditionally.
  - No `real-character-memory` feature remains in any manifest, cfg, doc, or command; `cargo test --workspace` compiles the adapter crate.
  - `DeterministicEmbeddingProvider` importable from core without features.
  - Mock-run outputs unchanged (latency-masked diff vs pre-refactor artifact).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
  - kind: command
    required: true
    owner: worker
    detail: "Mock synthetic smoke run (rules/common.md command); diff JSONL vs pre-refactor run, latency-masked"

### Task_2: Adapter contract v2 — full facade coverage and mock parity

- type: impl
- owns:
  - crates/cmem-eval-core/src/memory_adapter.rs
  - crates/cmem-eval-adapter-cmem/** (new op implementations)
- depends_on: [Task_1]
- description: |
  Extend the adapter contract with correct, forget, link, and staged writes (prepare/validate_plan/commit) using core-owned DTOs (A5), shaped per the Q2 recommendation. Implement on the live adapter with external-id round-trip mapping and correct candidate provenance on plan-path writes. Extend MockMemoryAdapter with documented minimal semantics (A3). Existing remember/retrieve behavior unchanged.
- acceptance:
  - Contract exposes all eight facade operations with core DTOs.
  - Mock implements each new op deterministically with unit tests.
  - Live adapter round-trips external ids for the new ops (unit-testable parts; live smoke deferred to Task_3's integration test).
  - Append-only correction semantics (supersession/suppression, no deletion).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
  - kind: command
    required: true
    owner: worker
    detail: "Mock synthetic smoke run (unchanged outputs, latency-masked)"

### Task_3: Restart-capable identity mapping and deterministic collection naming

- type: impl
- owns:
  - crates/cmem-eval-adapter-cmem/** (id-map/collection-name/lifecycle code)
  - crates/cmem-eval-core/src/memory_adapter.rs (open/reattach lifecycle additions only)
  - crates/cmem-eval-core/src/config.rs (persistence-path config additions only)
- depends_on: [Task_2]
- description: |
  Deterministic collection names from (prefix, run_id, namespace) — no random UUIDs. Extract external_id↔MemoryId mapping into a reusable BTreeMap-backed registry with a documented lifecycle per A1: caller-supplied deterministic MemoryIds at write time + registry serialization to the run directory as the primary mechanism; retrieval-based store-side rediscovery only as post-restart verification. Add explicit open/reattach lifecycle to the contract so a harness can drop and reconstruct the adapter against existing persistent stores.
- acceptance:
  - Collection naming deterministic; cleanup safety-prefix guards still hold.
  - Reattach round-trip test: write with adapter A, drop, reconstruct adapter B against the same stores, retrieval returns correct external_ids (gated integration test following the sibling repo's skip-if-unavailable pattern when live Qdrant is required).
  - Registry serialization is stable (BTreeMap ordering).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
  - kind: test
    required: true
    owner: worker
    detail: "Reattach round-trip test passes (live Qdrant if available; documented skip otherwise)"

### Task_4: Dataset pipeline extraction and per-dataset config seam

- type: impl
- owns:
  - crates/cmem-eval-runner/src/commands.rs (split: cli.rs + pipeline.rs or similar)
  - crates/cmem-eval-core/src/config.rs (dataset-dispatch removal)
  - crates/cmem-eval-longmemeval/** (DatasetSpec-style glue only)
  - crates/cmem-eval-locomo/** (DatasetSpec-style glue only)
- depends_on: [Task_2]
- description: |
  Extract the shared ingest→enrich→retrieve→score→row pipeline into one generic run pipeline parameterized by a small per-dataset strategy (loader, mapper, scorer, full-history builder). Static dispatch — no plugin registry. Move per-dataset config validation behind the seam so core `validate()` has no dataset string match. Fold progress logging into `tracing` events opportunistically. Adding a dataset kind must require zero core edits.
- acceptance:
  - Single generic pipeline; `commands.rs` ≤ ~300 lines of CLI concerns.
  - Core config has no dataset-kind string dispatch.
  - All three datasets produce byte-identical mock-run outputs (latency-masked) before/after (A4).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
  - kind: command
    required: true
    owner: worker
    detail: "Mock smoke run with latency-masked diff vs pre-refactor artifact"

### Task_5: Metric family registry and report schema versioning

- type: impl
- owns:
  - crates/cmem-eval-core/src/metrics.rs
  - crates/cmem-eval-core/src/results.rs
  - crates/cmem-eval-longmemeval/** (metric-family declaration only)
  - crates/cmem-eval-locomo/** (metric-family declaration only)
- depends_on: [Task_4]
- description: |
  Replace the hardcoded `REQUIRED_REGISTRY_METRICS` const with a runtime required-metric set assembled from a core base family plus dataset-declared metric families. Add `schema_version` to rows and summaries (Q3, approved). Latency handling (user constraint): retrieval latency is an important metric — it MUST remain a first-class reported value (per-row latency fields and summary percentiles preserved or improved); the change is only to segregate latency keys from the deterministic metric keys so run-to-run diff-stability checks can mask latency without losing it (e.g. dedicated `latency` section/fields rather than interleaved in the diff-compared `metrics` map). Record the embedding provider in the summary (determinism visibility). Preserve `metric_support`/`registry_coverage` null-tracking semantics.
- acceptance:
  - Datasets declare metric families; core has no dataset-specific metric list.
  - `schema_version` present in rows and summaries.
  - Latency fully preserved as reported data (rows + summary percentiles) and cleanly separable from deterministic keys for diff purposes.
  - Summary records embedding provider; existing metric values unchanged.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
  - kind: command
    required: true
    owner: worker
    detail: "Mock smoke run; inspect summary JSON: schema_version present, metrics map latency-free, provider recorded"

### Task_6: CI, toolchain pin, and manifest hygiene

- type: chore
- owns:
  - .github/workflows/** (new CI workflow)
  - rust-toolchain.toml (new)
  - Cargo.toml (drop no-op default-features flag)
  - README.md (command/CI notes only)
- depends_on: [Task_1]
- description: |
  GitHub Actions CI: fmt, clippy, test, and the mock smoke run (post-Q1: the feature gate is gone, so plain workspace commands compile the adapter crate — no separate feature-build job needed). Linux runner (native oxrocksdb builds are heavy; dev is Windows but CI need not be). Pin toolchain via rust-toolchain.toml (edition 2024-compatible stable). Drop the no-op `default-features = false` on the path dep. Note: the sibling path dependency means CI checkout needs both repos or a vendored/git fallback — if private-repo checkout of the sibling is not straightforward, scope CI to fmt/clippy only and record the limitation (replan trigger, orchestrator decision).
- acceptance:
  - CI workflow exists and is syntactically valid; jobs cover the DoD list or a recorded, justified subset.
  - Toolchain pinned; manifest cleaned; README matches reality.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
  - kind: review
    required: true
    owner: reviewer
    detail: "CI workflow review: job coverage vs DoD, sibling-checkout handling, actionlint-style YAML sanity"

### Task_7: Docs and rules refresh for revised architecture

- type: docs
- owns:
  - README.md (architecture section)
  - docs/coding-agent/rules/common.md
- depends_on: [Task_4, Task_5]
- description: |
  Update README architecture section and repo rules: adapter crate boundary, dataset seam, metric families, and the intended home of the future continuity crate (`crates/cmem-eval-continuity`). Verify/refresh the stale reference-document entry pointing at a Downloads-folder setup guide (relocate the content into the repo or remove the entry). Update validation-command rules if the smoke command changed.
- acceptance:
  - Rules describe the post-revision crate boundaries and extension points.
  - No stale/unreachable reference-document paths remain in the rules.
  - README architecture section matches the revised workspace.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "Docs/rules accuracy vs landed architecture"

### Task_9: CI hardening and widening to full gates (user-approved follow-up)

- type: chore
- owns:
  - .github/workflows/** (ci.yml edits)
  - README.md (CI notes only)
- depends_on: [Task_6]
- description: |
  Post-Task_6 user decisions (2026-07-11): (1) add an explicit rustup toolchain install step so the 1.97.0 pin is self-enforcing; (2) filter the push trigger to main to stop duplicate PR runs; (3) SHA-pinning of actions/checkout declined. (4) Widen CI to the full local gates: the sibling `character-memory` repo is PUBLIC (verified via gh repo view) — check out this repo and the sibling side by side so the ../CharacterMemory path dep resolves, then run cargo fmt --all --check, clippy --workspace --all-targets -D warnings, cargo test --workspace, and the mock synthetic smoke run on the pinned toolchain. No credentials, deploy keys, or PATs are needed or may be added. Qdrant-gated tests must take their documented skip path on the runner. Update README CI notes to match (drop the credential-dependent expansion wording).
- acceptance:
  - CI runs full fmt/clippy/test + mock smoke with a credential-less two-repo checkout; workflow remains least-privilege (contents: read) with no secrets.
  - Explicit toolchain install step present; push trigger filtered to main.
  - README CI section matches the widened reality.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "Workflow YAML validity + exact step logic executed locally (two-repo layout simulated); pinned-toolchain fmt/clippy/test remain green locally"
  - kind: review
    required: true
    owner: reviewer
    detail: "Workflow review: two-repo checkout correctness, gate parity vs local commands, no credential/secret introduction"

### Task_8: Independent review of the revision

- type: review
- owns: []
- depends_on: [Task_6, Task_7]
- description: |
  Reviewer verifies: Definition of Done items with evidence; behavior preservation (latency-masked mock-run diffs across Tasks 1/4/5); reattach round-trip evidence (Task_3); contract coverage vs the character_memory facade; determinism discipline (no new HashMap-iteration or wall-clock leaks in changed code); CI job coverage.
- acceptance:
  - Reviewer status APPROVED with per-DoD-item evidence.
- validation:
  - kind: review
    required: true
    owner: reviewer
    detail: "DoD checklist + behavior-preservation diff evidence + contract coverage audit"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_6]
- Wave 3 (parallel): [Task_3, Task_4]
- Wave 4 (parallel): [Task_5]
- Wave 5 (parallel): [Task_7, Task_9]
- Wave 6 (parallel): [Task_8]

Task_8's dependency set extends to [Task_6, Task_7, Task_9] with Task_9's addition (final review must see the widened CI).

## Rollback / Safety

- Pure refactor + hygiene: behavior preservation enforced by latency-masked mock-run diffs at every structural task; revert = branch revert.
- No changes to the sibling character_memory crate.
- Cleanup safety-prefix guards for Qdrant collections must survive the collection-naming change (Task_3 acceptance).

## Progress Log (append-only)

- 2026-07-11 12:20 Wave 1 dispatched: Task_1 sent to codex worker3 (CME-rooted) via agmsg. Delivery now handled by the agmsg App (cdxm watchers retired). Instructions include: pre-refactor mock smoke artifact captured before edits for the latency-masked diff; git delegated — feature branch `eval-harness-architecture-revision` from main, logical commits, no push.
- 2026-07-11 12:40 Wave 1 completed: Task_1 done (worker3; commits 49f2140, cf4fa96 + docs commits da4d384, 390c381 on eval-harness-architecture-revision).
  - Summary: live adapter extracted to new crate cmem-eval-adapter-cmem (unconditional workspace member); real-character-memory feature gate removed everywhere live (orchestrator integration check caught two missed commands in scripts/README.md; fixed in cf4fa96; root-scoped rg audit now clean outside append-only plan history); DeterministicEmbeddingProvider moved to core with adapter-crate re-export.
  - Validation evidence: cargo fmt --all --check, clippy --workspace -D warnings, cargo test --workspace all pass (adapter 10, core 37, longmemeval 10, locomo 6, runner 16 + doc tests); mock synthetic smoke pre/post diff identical after recursive latency-field masking (jsonl equal, summary equal).
  - Deviations accepted: README.md + scripts/README.md live-command updates (acceptance unsatisfiable within owns; ruled), core provider files (required by A2 but omitted from owns — plan owns gap noted), Cargo.lock (mechanical), lessons.md captures.
  - Rule updates: worker.md gained root-scoped removal-audit rule (RB-CAND-ROOT-REMOVAL-AUDIT accepted) and the adapter-crate CI test mapping. RB-CAND-AGMSG-TRUST (treat team agmsg dispatches as pre-authorized) is ON HOLD pending explicit user approval — auto-apply was blocked by the permission classifier as a trust-boundary change only the user may authorize.
- 2026-07-11 12:42 Wave 2 (sequential on worker3): Task_2 dispatched. Task_6 queued behind it — worker3 is the only CME-rooted worker; user offered the option to spawn a second CME codex for parallelism.
- 2026-07-11 12:50 User decisions: RB-CAND-AGMSG-TRUST WAIVED as a repo rule (user instructed worker3 directly in its session instead; rule wording judged too strong for an always-applied rule). Commits authorized by user on both repos until all planned tasks complete — orchestrator bookkeeping commit added on this branch; sibling-repo Wave 1–2 work committed on v0-1-4-cm-groundwork there.
- 2026-07-11 13:00 Task_2 completed (worker3; commit 57a6ead, no deviations).
  - Summary: main MemoryAdapter trait extended with link/correct/forget/ prepare/validate_plan/commit using core-owned DTOs (Q2 shape: main trait, no extension split); deterministic mock semantics with tests (append-only correction — original retained but suppressed; forget = minimal remove per A3); live adapter mappings with external-ID round trips across all six object/link kinds and ADR-I-0015 producer/rationale provenance preserved on plan-path writes. Sibling repo read-only, unmodified.
  - Validation evidence: fmt/clippy(-D warnings)/test --workspace pass (adapter 12, core 41, longmemeval 10, locomo 6, runner 16 + doc tests); mock smoke latency-masked diff vs fresh pre-Task_2 baseline identical (jsonl + summary).
  - Notes: live Qdrant staged/lifecycle smoke deferred to Task_3 as planned. Task_6 dispatched next (sequential on worker3); Wave 3 (Task_3, Task_4) follows.
- 2026-07-11 13:25 Task_6 completed (worker3 commits 05dfbba, c2f8506, 64eaabe; Reviewer APPROVED).
  - Summary: GitHub Actions CI (source-only rustfmt over git-listed files, least-privilege, injection-free), rust-toolchain.toml pin 1.97.0 (minimal profile + rustfmt/clippy), no-op default-features flag dropped from the sibling path dep, README documents pinned toolchain + full local gates vs fmt-only CI. Three pre-existing collapsible_if lints surfaced by the pinned clippy fixed mechanically under authorized deviations (separate per-crate commits).
  - Validation evidence: pinned-toolchain fmt/clippy(-D warnings)/test --workspace pass; mock smoke pass; workflow YAML validated + exact CI step logic executed locally without sibling metadata. Reviewer APPROVED with per-check evidence; three MINOR hardening notes (explicit rustup install step, push-branch filter to avoid duplicate PR runs, SHA-pinning checkout) recorded as optional follow-up chore, non-blocking.
  - Scope note (per Reviewer): the plan's escape hatch said "fmt/clippy only", but clippy compiles the private sibling path dep and is impossible on a credential-less runner — even cargo fmt fails on metadata load. Fmt-only via plain rustfmt is the maximum zero-credential check; the subset is by necessity, not choice. CI widening (user-provisioned read-only deploy key/PAT + two-repo checkout) carried forward as an open user decision for Task_8/closeout.
- 2026-07-11 14:50 Task_3 completed (worker3; commits 3587f53, 056be29).
  - Summary: deterministic run-scoped collection naming (no Uuid::new_v4; cleanup safety-prefix guards proven by test); BTreeMap-backed external-id registry with byte-stable sorted serialization, assigned before writes and persisted only after successful writes; explicit fresh-open vs persistent-reattach namespace lifecycle in the contract with mock coverage (missing-reattach failure, duplicate-open rejection, restored identity count).
  - Validation evidence: pinned fmt/clippy(-D warnings)/test --workspace pass (adapter 15, core 43, longmemeval 10, locomo 6, runner 16 + doc tests); registry serialization stability test; deterministic-naming test; mock smoke latency-masked diff identical vs fresh pre-Task_3 baseline (skip_serializing_if keeps existing artifacts structurally unchanged).
  - PENDING live evidence: the gated reattach round-trip test (live_adapter_reattaches_with_external_ids) took its documented skip path — local Qdrant down, concrete structured skip evidence captured. Plan permits this at Task_3, but the LIVE reattach round-trip must be exercised before or at Task_8 (Qdrant must be up for the independent review). Carried forward.
  - Deviation accepted: Cargo.lock (tempfile dev-dependency for adapter-owned persistence tests; mechanical).
- 2026-07-11 16:05 Task_4 completed after one integration bounce (worker3; commits 88c6522, deb486e; lesson commit e794eaf).
  - Summary: single static-dispatch generic ingest→enrich→retrieve→score→row pipeline (pipeline.rs) parameterized by DatasetSpec for synthetic/ LongMemEval-S/LoCoMo; commands.rs reduced to 231 lines of CLI concerns; core config has zero dataset-kind dispatch (extension seam tested with a future_dataset probe); dataset crates own config-name validation and full-history builders.
  - Integration bounce: worker initially skipped the LongMemEval-S/LoCoMo A4 byte-diffs claiming datasets absent; orchestrator spot-check found the gitignored local assets present (rg/fd honor ignore rules — absence was an artifact of ignore-aware search). Amended validation: detached worktree at ec9eb32 produced PRE artifacts with the real assets; isolated CARGO_TARGET_DIR built HEAD POST artifacts (earlier shared-target attempts detected as stale-binary reuse and discarded); latency-masked diffs exact for synthetic (1 row), LongMemEval-S (500 rows), LoCoMo (1986 rows) and all three summaries. Pinned fmt/clippy/test remain green (95 tests).
  - Rule update: RB-CAND-NO-IGNORE-ASSET-CHECK accepted into worker.md — never infer asset absence from ignore-aware enumeration.
- 2026-07-11 16:10 Task_5 owns corrected mid-flight (plan-staleness artifact): Task_5's owns predate Task_4's relocation of the run pipelines from commands.rs into pipeline.rs. Orchestrator authorized a narrow expansion to crates/cmem-eval-runner/src/pipeline.rs strictly for (1) DatasetSpec metric-family plumbing into core initialization/summary and (2) relocating retrieval_latency_ms out of deterministic metrics into the segregated latency location (per-row latency retained). Separate logical commit + recorded deviation required; no other pipeline.rs changes sanctioned.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-07-05 Decision: initial draft from architecture-debt research (findings ranked by continuity-work impact: restart lifecycle > contract gap > dataset seam > adapter placement > metric registry/schema > determinism enforcement > hygiene). YAGNI exclusions recorded in Non-goals.
  - Trigger / new insight: user directed an architecture revision in this repo, under its own plan, before continuity features land (gating pre-phase of the sibling repo's v0.1.4 plan).
  - Tradeoffs considered: extension trait vs main-trait facade coverage (Q2); feature-gate drop vs crate-boundary relocation (Q1); run-dir id-registry persistence vs store-side reconstruction (A1: store-side primary).
  - User approval: superseded by the entries below.
- 2026-07-05 Decision: A1 restart-identity mechanism updated from the sibling repo's Task_2 audit — caller-supplied deterministic MemoryIds + harness-persisted registry primary; store-side rediscovery is verification only (public API cannot enumerate/lookup by external id after restart).
  - Trigger / new insight: sibling-repo audit findings (appendix in its v0.1.4 plan).
  - Plan delta: A1 and Task_3 description updated; no facade change planned.
  - User approval: within the direction already approved in the sibling plan.
- 2026-07-05 Decision: open questions resolved; plan approved and in_progress.
  - Q1 (feature gate) — user delegated the decision; orchestrator assessment: once the adapter is a workspace-member crate, `cargo test --workspace` compiles it regardless of any runner feature flag, so the gate no longer buys meaningful build savings while retaining rot risk and `--features` friction in every command and doc. DROP the gate. Escape hatch if local build cost proves painful: exclude the adapter crate via workspace `default-members` (recorded as fallback, not implemented now).
  - Q2 (contract shape) — user approved: Character-Memory-shaped main trait; staged writes in the main trait, no extension-trait split.
  - Q3 (report shape) — user approved schema_version + no compatibility mode, with the explicit constraint that retrieval latency remains a first-class reported metric; it is segregated from deterministic diff keys, never dropped. Task_5 description/acceptance updated accordingly.
  - User approval: yes (2026-07-05) — Wave 1 dispatch authorized.
- 2026-07-11 Decision: CI scope premise corrected; widening approved without credentials; hardening decisions recorded.
  - Trigger / new insight: Task_6 and its review assumed the sibling path dependency requires credentials in CI. Orchestrator verification (gh repo view) shows the sibling `character-memory` repository is PUBLIC; the private repository is this one (`character-memory-evals`), which Actions checks out with its own default token. Therefore full fmt/clippy/test + mock smoke CI needs NO deploy key or PAT — only a second, credential-less checkout of the public sibling laid out so the `../CharacterMemory` path dependency resolves.
  - Plan delta: new Task_9 (chore) added — CI hardening + widening: explicit rustup toolchain install step (accepted), push-trigger branch filter (accepted), SHA-pinning checkout (declined — no secrets in workflow), two-repo checkout + full local-gate parity in CI (user approved full test suite in CI). Qdrant-gated tests rely on their documented skip path on the runner. Dispatch when worker3 frees up; owns disjoint from Tasks 5/7.
  - User approval: yes (2026-07-11) — "take your recommendations on the hardenings; deploy the key and let CI run the full test suite"; the credential step is dropped as unnecessary given the public sibling.

## Notes

- Risks:
  - Pipeline refactor (Task_4) risks silent scoring changes — guarded by byte-diff acceptance (A4).
  - Reattach testing genuinely needs live Qdrant; mock cannot prove it — gated integration test mirrors the sibling repo's stabilized pattern.
  - Sibling crate v0.1.4 API work may land mid-plan — DTO mapping kept thin (A5).
  - CI + private sibling path dependency may limit CI scope (Task_6 replan note).
- Edge cases:
  - Collection-name determinism vs concurrent runs: (prefix, run_id, namespace) must stay collision-free across parallel local runs.
  - schema_version bump interacts with any user tooling that diffs old runs (Q3).
