# Plan: Integrate Character Memory Public API Into Eval Adapter

- status: draft
- generated: 2026-04-30
- last_updated: 2026-04-30
- work_type: code

## Goal
- Add a feature-gated real Character Memory adapter to the eval runner using the active public facade, and make live Character Memory the default for benchmark CLI runs.

## Definition of Done
- `--adapter real` works when `real-character-memory` is enabled and fails actionably when it is not.
- Episode and observation external IDs round-trip into `RetrievedItem` output used by scorers.
- Eval-side namespace reset/isolation is implemented without requiring Character Memory library changes.
- Retrieval output maps Character Memory packs/traces into eval ranks, scores, rationales, text, and context counts.
- Backend embeddings are config-selectable, with OpenAI `text-embedding-3-large` as the initial real-run default.
- Official-compatible LongMemEval and LoCoMo export formats are available for downstream benchmark scripts.
- Omitting `--adapter` on benchmark `run` commands selects the live real adapter.
- Mock benchmark CLI usage requires an unmistakable opt-in guard and marks outputs as mock/smoke.
- Default validation remains explicitly mock-backed and service-free; live real-adapter validation is gated/skippable.

## Scope / Non-goals
- Scope:
  - Typed real-backend config in eval core.
  - Feature-gated real adapter construction in eval runner.
  - Eval-owned namespace state and deterministic external-ID mapping.
  - Character Memory draft/retrieval mapping.
  - Eval-owned context text rendering and official benchmark export adapters.
  - Eval-side cleanup for benchmark-owned Qdrant collections where configured.
  - CLI adapter default and mock-run safeguards.
  - Focused unit tests, feature-gated compile checks, and optional live smoke test.
- Non-goals:
  - Changing Character Memory public APIs.
  - Adding library-native namespace/reset/delete APIs.
  - Adding library-native context text rendering.
  - Adding first-class entity resolution for speaker/participant names.
  - Adding LLM extraction, reranking, judges, or answer generation.
  - Making default CI depend on Qdrant, OpenAI, or network access.
  - Removing `MockMemoryAdapter` from internal tests.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-core/src/results.rs`
  - `crates/cmem-eval-runner/Cargo.toml`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/**`
  - `configs/*.toml`
  - `README.md`
  - Official LongMemEval scripts expect retrieval logs containing `retrieval_results.ranked_items` and QA JSONL containing `question_id` and `hypothesis`.
  - Official LoCoMo QA evaluation is sample/QA-entry oriented and uses model-specific prediction/context keys; eval exports should preserve sample IDs, QA metadata, predictions, and retrieved context IDs.
- Existing patterns or references:
  - `MockMemoryAdapter` remains the deterministic internal test/smoke adapter.
  - `cmem-eval-longmemeval` and `cmem-eval-locomo` already keep gold labels scorer-only.
  - Character Memory exposes `CharacterMemory::new_with_embedding_provider`, `remember`, `retrieve`, `correct`, and `forget`.
  - Character Memory public drafts accept caller-supplied UUIDs and source refs.
  - Character Memory retrieval exposes structured packs plus trace/rationale, not eval-flat output.
- Repo reference docs consulted:
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`
  - `docs/coding-agent/lessons.md`
  - `https://github.com/xiaowu0162/LongMemEval`
  - `https://github.com/snap-research/locomo`

## Open Questions
- None blocking.

## Assumptions
- A1: Character Memory library changes stay out of this eval-stack plan.
- A2: Eval can generate deterministic UUID v5 IDs from `(namespace, kind, external_id)`.
- A3: Eval can use `EpisodeDraft.raw_ref`, `EpisodeDraft.source_conversation_id`, and `ObservationDraft.raw_ref` as source pointers, while maintaining its own reverse maps.
- A4: `query_date` has no Character Memory retrieval field today and will remain eval-side metadata/no-op until the library adds temporal retrieval semantics.
- A5: Real adapter tests that require Qdrant/settings must skip cleanly when services are unavailable.
- A6: Cleanup should be possible from the eval stack for benchmark-owned collections, because waiting on a library reset/delete API would block reproducible runs. A library-native cleanup/reset API remains a separate Character Memory design item.
- A7: The real adapter should support config-selectable embeddings; the initial real-run default is OpenAI `text-embedding-3-large`, while deterministic embeddings remain available for smoke tests.
- A8: Internal context text is a benchmark-owned rendering for fixed-reader experiments; official script compatibility is handled through explicit LongMemEval/LoCoMo export shapes.
- A9: Benchmark runtime defaults and CI validation defaults are intentionally different: real/live for user-facing runs, explicit mock/test mode for service-free validation.

## Tasks

### Task_1: Define Typed Real Backend Config
- type: impl
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `configs/*.toml`
- depends_on: []
- description: |
  Replace opaque real-backend handling with typed config fields needed by the real adapter, while keeping existing TOML compatible.
- acceptance:
  - Backend config can express collection prefix/name strategy, reset and cleanup policy, trace/debug behavior, and embedding-provider mode.
  - Embedding provider config is selectable and defaults real runs to OpenAI `text-embedding-3-large`.
  - Deterministic embedding mode is available for service-controlled smoke tests.
  - Config can record adapter kind/run mode in outputs so mock/smoke artifacts are visibly distinguishable from live results.
  - Retrieval config includes currently ignored flags needed by Character Memory mapping, including threads/entities/debug rationale.
  - Existing configs deserialize without changing mock behavior.
  - Explicit mock/test mode remains deterministic and service-free, but benchmark CLI runs default live.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core`

### Task_2: Add Feature-Gated Real Adapter Construction
- type: impl
- owns:
  - `crates/cmem-eval-runner/Cargo.toml`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_1]
- description: |
  Add the real adapter module and wire `AdapterKind::Real` to construct it only when `real-character-memory` is enabled.
- acceptance:
  - No `--adapter` on benchmark `run` commands selects `AdapterKind::Real`.
  - `--adapter real` and omitted adapter both construct the live adapter when the feature is enabled.
  - Without the feature, default live selection returns a clear compile/runtime message explaining `real-character-memory` build instructions.
  - Adapter construction uses public Character Memory APIs only.
  - Direct Qdrant cleanup dependency, if added, is scoped to eval-owned benchmark collection cleanup and not used to bypass Character Memory retrieval semantics.
  - Mock adapter remains available to tests and explicit guarded smoke runs, but is not the benchmark runtime default.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo check -p cmem-eval-runner --features real-character-memory`

### Task_2a: Add Mock CLI Safeguard
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-core/src/results.rs`
- depends_on: [Task_2]
- description: |
  Prevent accidental benchmark runs with the mock adapter while preserving an explicit service-free smoke path for CI and local sanity checks.
- acceptance:
  - `--adapter mock` is rejected for benchmark CLI runs unless paired with an explicit guard such as `--allow-mock-benchmark`.
  - Mock-run errors explain that mock is for tests/smoke only and live is the default for real evals.
  - Mock outputs include adapter/run-mode metadata that makes them visibly non-live.
  - Internal tests can still construct and use `MockMemoryAdapter` without CLI guard friction.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner`

### Task_3: Implement Namespace Isolation And External ID State
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_2a]
- description: |
  Maintain eval-side namespace state, Character Memory instances, and bidirectional mappings between eval external IDs and Character Memory UUIDs.
- acceptance:
  - `reset_namespace` isolates subsequent retrievals for that namespace.
  - Configured cleanup can remove eval-owned benchmark collections, while no cleanup remains the safe fallback.
  - Episode and observation IDs are deterministic UUID v5 values derived from namespace/kind/external ID.
  - Observation ingestion fails clearly if its parent episode external ID is unknown.
  - Reverse maps support converting retrieved Character Memory IDs back to eval external IDs.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_4: Map Eval Ingestion To Character Memory Drafts
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_3]
- description: |
  Convert `EpisodeInput` and `ObservationInput` into Character Memory `EpisodeDraft` and `ObservationDraft` values.
- acceptance:
  - Episode summary, parsed timestamps, deterministic UUID, source conversation ID, and raw ref are preserved.
  - Observation text, parsed timestamp, deterministic UUID, parent episode UUID, and raw ref are preserved.
  - Gold evidence labels are not introduced into Character Memory drafts.
  - Speaker and participant names do not create implicit entity semantics.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-longmemeval && cargo test -p cmem-eval-locomo`

### Task_5: Map Retrieval Context And Flatten Outputs
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_4]
- description: |
  Convert `RetrieveInput` into Character Memory `RetrievalContext`, call retrieval with trace when needed, and flatten `RetrieveOutcome` into eval `RetrievedContextPack`.
- acceptance:
  - `top_k_episodes`, `top_k_observations`, derived-memory flags, threads/entities flags, and trace/debug flags are mapped where Character Memory supports them.
  - Episodes and observations produce eval `RetrievedItem` values with internal UUID, external ID, parent episode ID, rank, score, rationale, and text.
  - Derived memory sections are included only when configured and expose provenance-derived source external IDs where possible.
  - Trace scores/ranks are used when available, with deterministic fallback ordering for graph-expanded items.
  - Lifecycle/stale omission summaries contribute to rationale/integrity metrics where available.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_6: Render Context Text And Official Export Shapes
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-runner/src/official_exports.rs`
  - `crates/cmem-eval-core/src/token_estimate.rs`
  - `crates/cmem-eval-core/src/results.rs`
- depends_on: [Task_5]
- description: |
  Render Character Memory's structured context pack into stable eval `context_text`, `context_char_count`, and `context_word_count`, and add official-compatible export helpers for downstream benchmark scripts.
- acceptance:
  - Context text ordering is stable across runs.
  - Section labels and item text are sufficient for downstream fixed-reader experiments.
  - Counts match the rendered context text.
  - Empty retrieval returns empty context and zero counts.
  - LongMemEval export can emit retrieval logs with `retrieval_results.ranked_items` and QA JSONL rows with `question_id` and `hypothesis`.
  - LoCoMo export can emit sample/QA-entry records preserving `sample_id`, QA metadata, model prediction fields, and retrieved context IDs for its evaluator flow.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory context official_exports`

### Task_7: Add Gated Live Smoke Validation And Docs
- type: test
- owns:
  - `crates/cmem-eval-runner/src/**`
  - `README.md`
  - `scripts/README.md`
- depends_on: [Task_6]
- description: |
  Add a feature-gated live smoke test and document how to run the real adapter without changing the default service-free validation path.
- acceptance:
  - Live real-adapter test skips cleanly when Qdrant/settings are unavailable.
  - Live test verifies synthetic ingest/retrieve preserves external IDs through real Character Memory.
  - README documents live default runs, real feature flag, backend settings, OpenAI `text-embedding-3-large` default, explicit guarded mock smoke mode, cleanup policy, official export commands, and known library-side limitations.
  - Repository validation docs/rules show service-free synthetic validation with explicit mock guard, so CI does not silently become live.
  - No default test requires Qdrant, Oxigraph service setup, OpenAI, or network access.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test --workspace`
  - kind: command
    required: true
    owner: worker
    detail: `cargo check -p cmem-eval-runner --features real-character-memory`
  - kind: command
    required: false
    owner: user
    detail: `cargo test -p cmem-eval-runner --features real-character-memory -- --ignored`

### Task_8: Full Workspace Quality Gate
- type: review
- owns: []
- depends_on: [Task_7]
- description: |
  Run the required repository validation gate and review the final diff against this plan and the handoff.
- acceptance:
  - Required commands pass or blockers are explicitly recorded.
  - Reviewer confirms real adapter code uses only public Character Memory APIs.
  - Reviewer confirms gold labels remain scorer-only.
  - Reviewer confirms library-side gaps were not implemented in the eval repo.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo fmt --all --check`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo clippy --workspace --all-targets -- -D warnings`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo test --workspace`
  - kind: command
    required: true
    owner: orchestrator
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json`
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review vs plan acceptance criteria and Character Memory public API boundary."

## Task Waves
- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_2a]
- Wave 4 (parallel): [Task_3]
- Wave 5 (parallel): [Task_4]
- Wave 6 (parallel): [Task_5]
- Wave 7 (parallel): [Task_6]
- Wave 8 (parallel): [Task_7]
- Wave 9 (parallel): [Task_8]

## Rollback / Safety
- The real adapter is feature-gated; disabling `real-character-memory` should restore mock-only behavior.
- If live integration is unstable, default benchmark commands should fail actionably rather than silently falling back to mock.
- Mock CLI runs must require an explicit guard, and generated artifacts must identify themselves as mock/smoke output.
- Cleanup must be restricted to eval-owned collection names/prefixes and must fail closed if the target collection is outside configured benchmark ownership.
- Do not modify the sibling Character Memory library as part of this plan.

## Progress Log
- 2026-04-30 draft created.
  - Summary: Planned eval-owned public API integration while excluding library-side gaps.
  - Validation evidence: Researcher report `019ddc52-8329-7593-b912-c790c3dd7a96`.
  - Notes: No UI scope.
- 2026-04-30 draft updated from user decisions.
  - Summary: Made embedding provider config-selectable with OpenAI `text-embedding-3-large` as initial real default, added eval-side cleanup capability, and expanded context rendering into official benchmark export compatibility.
  - Validation evidence: User answers to Q1-Q3 and primary-source review of official benchmark repositories/scripts.
  - Notes: Character Memory cleanup/reset API remains out of eval plan.
- 2026-04-30 draft updated for live default runs.
  - Summary: Changed benchmark runtime default from mock to live real adapter and added explicit mock CLI safeguard.
  - Validation evidence: Researcher report `019ddc7f-2af5-75a2-bdfd-fb404ef0ffae`.
  - Notes: Service-free validation remains explicit mock/test mode.
- 2026-04-30 implementation started through Task_3.
  - Summary: Added typed backend/retrieval/ingest config, adapter metadata, live default CLI selection, explicit mock guard, initial feature-gated real adapter, deterministic UUID mapping, LoCoMo caption config wiring, and integrity metrics.
  - Validation evidence: `cargo test -p cmem-eval-core`; `cargo test -p cmem-eval-runner`; `cargo test --workspace`; `cargo check -p cmem-eval-runner --features real-character-memory`; `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`; explicit mock synthetic smoke command; `cargo clippy --workspace --all-targets -- -D warnings`; package-scoped `cargo fmt -p ... --check`.
  - Notes: Full `cargo fmt --all --check` is blocked by unrelated dirty sibling `CharacterMemory` files.

## Decision Log
- 2026-04-30 Decision:
  - Trigger / new insight: Character Memory public facade is active enough for eval integration.
  - Plan delta (what changed): Replace hard-stub `--adapter real` with feature-gated real adapter tasks.
  - Tradeoffs considered: Eval owns mapping/rendering/isolation conventions; native namespace/reset/context renderer remain library-side.
  - User approval: pending.
- 2026-04-30 Decision:
  - Trigger / new insight: User selected config-selectable embeddings, requested cleanup capability, and specified official benchmark script compatibility as the context/output target.
  - Plan delta (what changed): Added OpenAI default model, deterministic smoke mode, cleanup policy, and official LongMemEval/LoCoMo export requirements.
  - Tradeoffs considered: Eval-side cleanup unblocks benchmark reproducibility now, while native library cleanup/reset remains a separate API design concern.
  - User approval: yes.
- 2026-04-30 Decision:
  - Trigger / new insight: User clarified that mock should not be the exposed default for real eval runs because accidental mock evals waste time.
  - Plan delta (what changed): Omitted adapter now means live real adapter; mock CLI use requires an explicit guard and output labeling.
  - Tradeoffs considered: Default CLI can fail when the real feature/services are unavailable, but failing loudly is preferable to producing mock benchmark artifacts accidentally.
  - User approval: yes.

## Notes
- Risks:
  - Retrieval trace may omit scores/ranks for graph-expanded objects; deterministic fallbacks are required.
  - Collection cleanup is operationally sensitive and must be restricted to eval-owned prefixes.
  - Date parsing must tolerate dataset format variation and fall back to `None`.
  - Derived memories may only map to source episode/observation external IDs, not dataset-native derived IDs.
  - Official benchmark script formats can drift; export tests should pin representative fixtures from the expected shape.
  - Feature-disabled default live runs must fail clearly or users may misdiagnose configuration issues.
- Edge cases:
  - `query_date` currently has no public Character Memory retrieval field.
  - Speaker/participant strings should not silently create entity semantics.
  - Real adapter tests must not make default validation service-dependent.
  - Mock config snippets should be labeled smoke/test-only and should not be easy to copy into real benchmark runs accidentally.
