# Plan: Vector-Only Retrieval Baseline

- status: completed
- generated: 2026-05-04
- last_updated: 2026-05-04
- work_type: code

## Goal
- Add `retrieval.mode = "vector_only"` as a live baseline that ingests through Character Memory, then bypasses Character Memory retrieval and searches the namespace Qdrant collection directly with basic vector similarity.

## Definition of Done
- `retrieval.mode = "vector_only"` is accepted in config and remains TOML-only.
- Vector-only runs require the real adapter and never use the mock adapter.
- Query embeddings use the configured run embedding provider/model.
- Qdrant search uses only benchmark-provided raw candidate types: episodes and observations.
- Vector-only results preserve benchmark external IDs, scores, ranks, context text, and existing metric output contracts.
- Existing `hybrid` and `bm25_only` behavior remains unchanged.

## Scope / Non-goals
- Scope:
  - Retrieval mode config and runner validation.
  - Real-adapter vector-only query embedding and direct Qdrant search.
  - Mapping Qdrant payload hits to `RetrievedItem`.
  - Vector-only configs and README guidance.
  - Unit tests plus optional live synthetic validation.
- Non-goals:
  - Character Memory public API changes.
  - Graph expansion, reranking, lifecycle validation, or continuity pack assembly in vector-only mode.
  - Treating Character Memory-generated derived memories, threads, entities, enrichment, or generated summaries as baseline candidates.
  - Service-free vector-only execution; this baseline uses live Qdrant and configured embedding services.

## Context (workspace)
- Related files/areas:
  - `crates/cmem-eval-core/src/config.rs`
  - `crates/cmem-eval-core/src/memory_adapter.rs`
  - `crates/cmem-eval-runner/src/commands.rs`
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-runner/Cargo.toml`
  - `configs/*_retrieval.toml`, `configs/*_bm25.toml`
  - `README.md`
- Existing patterns or references:
  - BM25 added the `RetrievalMode` enum, TOML-only baseline selection, baseline configs, and adapter guardrails.
  - The real adapter owns namespace state, generated Qdrant collection names, and reverse maps from internal `MemoryId` to benchmark external IDs.
  - Character Memory Qdrant payload contains `object_id`, `object_type`, `embedding_text`, `content_text`, `raw_ref`, and relationship hints.
  - Character Memory Qdrant vectors are unnamed cosine vectors.
- Repo reference docs consulted:
  - `docs/coding-agent/rules/common.md`
  - `docs/coding-agent/rules/orchestrator.md`
  - `docs/coding-agent/lessons.md`
  - `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`

## Open Questions (max 3)
- None blocking.

## Assumptions
- A1: "Benchmark-provided candidates only" means episodes and observations for v1 vector-only.
- A2: BM25 already follows the same baseline-candidate rule by indexing only episodes and observations.
- A3: Vector-only is a live baseline and may use Qdrant plus OpenAI embeddings when the config requests OpenAI.
- A4: Deterministic embedding support is required for local integration tests without OpenAI network calls.
- A5: Vector-only telemetry must not fabricate graph validation evidence.

## Tasks

### Task_1: Extend Retrieval Mode And Configs
- type: impl
- owns:
  - `crates/cmem-eval-core/src/config.rs`
  - `configs/synthetic_vector.toml`
  - `configs/longmemeval_s_vector.toml`
  - `configs/locomo_vector.toml`
- depends_on: []
- description: |
  Add `VectorOnly` to the shared retrieval mode enum and add dataset configs with distinct run IDs and namespace prefixes.
- acceptance:
  - Config accepts `retrieval.mode = "vector_only"`.
  - Existing configs remain default-hybrid or explicit BM25 unchanged.
  - Vector configs use distinct run IDs and output-safe namespace prefixes.
  - Vector configs include only raw-candidate retrieval flags by default.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-core config`

### Task_2: Add Runner Adapter Guardrails
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/commands.rs`
- depends_on: [Task_1]
- description: |
  Enforce valid adapter/mode combinations before adapter construction or ingestion.
- acceptance:
  - `vector_only` rejects `--adapter mock`.
  - `vector_only` allows the default real adapter path.
  - Feature-disabled real adapter failures remain actionable.
  - `bm25_only` mock-only guard remains unchanged.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner`

### Task_3: Add Query Embedding Helper
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
  - `crates/cmem-eval-runner/Cargo.toml`
- depends_on: [Task_1]
- description: |
  Add real-adapter-owned query embedding generation that mirrors the configured ingestion embedding provider.
- acceptance:
  - Deterministic provider reuses the existing stable local vector function.
  - OpenAI provider builds the same embedding request model/input shape as Character Memory.
  - Missing OpenAI API key fails before Qdrant search with an actionable error.
  - Query embedding vector size matches the configured collection vector size.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_4: Implement Direct Qdrant Vector Search
- type: impl
- owns:
  - `crates/cmem-eval-runner/src/real_adapter.rs`
- depends_on: [Task_2, Task_3]
- description: |
  In vector-only mode, search the namespace Qdrant collection directly and convert episode/observation hits to `RetrievedItem`.
- acceptance:
  - Search uses unnamed vectors with payload enabled and vectors disabled.
  - Qdrant filter restricts candidates to `object_type in ["episode", "observation"]`.
  - Hits map through `reverse_episode_ids` and `reverse_observation_ids`; unmapped hits are skipped or reported clearly without panic.
  - Per-kind top-k limits are honored after Qdrant scoring.
  - `content_text` is used as item text and Qdrant score as item score.
  - Telemetry reports vector candidate count and avoids graph evidence claims.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`

### Task_5: Docs And Safety Guidance
- type: docs
- owns:
  - `README.md`
- depends_on: [Task_1, Task_2, Task_4]
- description: |
  Document vector-only as a live baseline and explain isolation expectations.
- acceptance:
  - README explains vector-only uses live Qdrant and configured embeddings.
  - README shows a synthetic vector-only command with real adapter feature.
  - README warns to use distinct run IDs/output paths and avoid active benchmark resources unless intentionally sharing load.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "README guidance matches implemented mode constraints."

### Task_6: Final Validation And Review
- type: review
- owns: []
- depends_on: [Task_1, Task_2, Task_3, Task_4, Task_5]
- description: |
  Run repository-required checks and route the diff through a reviewer gate.
- acceptance:
  - Required checks pass or failures are documented with actionable cause.
  - Existing hybrid and BM25 behavior is preserved.
  - Reviewer status is APPROVED.
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
    detail: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`
  - kind: command
    required: false
    owner: orchestrator
    detail: `cargo run -p cmem-eval-runner --features real-character-memory -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_vector.toml --out ./runs/synthetic_vector.jsonl --summary-out ./runs/synthetic_vector_summary.json`
  - kind: review
    required: true
    owner: reviewer
    detail: "Reviewer verifies correctness, candidate-scope policy, live-resource guardrails, and validation evidence."

## Task Waves (explicit parallel dispatch sets)

Interpretation:
- Tasks listed in the same wave are intended to be dispatched in parallel by default,
  when `owns` are disjoint and dependencies are met.
- Waves are executed sequentially.

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4]
- Wave 4 (parallel): [Task_5]
- Wave 5 (parallel): [Task_6]

## E2E / Visual Validation Spec

- Not applicable. This plan has no UI or browser-facing changes.

## Rollback / Safety
- `vector_only` is gated by `retrieval.mode`; omitting it preserves current hybrid behavior.
- Use method-specific run IDs, namespace prefixes, and output files for all vector-only runs.
- Do not run vector-only against active benchmark Qdrant/OpenAI resources unless the operator intentionally accepts shared load.
- Optional live validation can be skipped if Qdrant/OpenAI services are unavailable; required deterministic/unit validation must still pass.

## Progress Log (append-only)

- 2026-05-04 Draft created: [Task_1, Task_2, Task_3, Task_4, Task_5, Task_6]
  - Summary: Initial vector-only retrieval baseline implementation plan.
  - Validation evidence: Not run; planning only.
  - Notes: Plan based on prior planning discussion and current merged BM25 baseline.
- 2026-05-04 Implementation completed: [Task_1, Task_2, Task_3, Task_4, Task_5, Task_6]
  - Summary: Added vector_only config mode, real-adapter direct Qdrant vector search, vector configs, README guidance, and config policy coverage.
  - Validation evidence: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; service-free synthetic smoke; `cargo test -p cmem-eval-runner --features real-character-memory real_adapter`; `cargo clippy -p cmem-eval-runner --features real-character-memory --all-targets -- -D warnings`.
  - Review evidence: Final harness reviewer status APPROVED; no remaining findings.
  - Notes: Optional live vector synthetic run was not executed because it requires live Qdrant/OpenAI resources.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-05-04 Decision: Vector-only searches benchmark-provided raw candidates only.
  - Trigger / new insight: User clarified that extra generated Character Memory fields should not be baseline comparison targets.
  - Plan delta (what changed): Candidate scope restricted to episodes and observations.
  - Tradeoffs considered: Raw-only improves baseline comparability; honoring all include flags would mix baseline retrieval with Character Memory-specific enrichment.
  - User approval: yes.

- 2026-05-04 Decision: Query embeddings follow run config.
  - Trigger / new insight: User selected same-config query embeddings.
  - Plan delta (what changed): Vector-only supports deterministic and OpenAI query embedding paths.
  - Tradeoffs considered: Same-config embeddings preserve benchmark parity; deterministic-only would be easier but not representative.
  - User approval: yes.

## Notes
- Risks:
  - OpenAI query embeddings add live service cost and rate-limit risk.
  - Direct Qdrant payload parsing may drift if Character Memory changes payload field names.
  - Per-kind top-k may require Qdrant oversampling if candidate type balance is skewed.
- Edge cases:
  - Empty query embedding request.
  - Missing or malformed Qdrant payload fields.
  - Qdrant returns duplicate object IDs across vector surfaces.
  - Unmapped internal object IDs after partial ingestion failure.
