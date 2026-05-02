# CharacterMemoryEvals

Evaluation harness for the Character Memory memory substrate. This repository measures retrieval quality, continuity-context construction, context-size reduction, latency, and provenance-oriented integrity signals.

Benchmark CLI runs default to the live Character Memory adapter. Mock runs are available only as explicit smoke/test runs so benchmark output is not accidentally generated from the mock adapter.

## Commands

```bash
cargo test --workspace
```

Benchmark commands default to the live Character Memory adapter. Build live runs with
the `real-character-memory` feature and provide backend settings:

```bash
export QDRANT_CONNECTION_STRING=http://localhost:6334
export OPENAI_API_KEY=...
```

For live runs that use deterministic embeddings instead of OpenAI, set
`[backend.embedding] provider = "deterministic"` in the run config.

For service-free smoke validation, opt into mock output explicitly:

```bash
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_retrieval.toml \
  --out ./runs/synthetic.jsonl \
  --summary-out ./runs/synthetic_summary.json \
  --adapter mock \
  --allow-mock-benchmark
```

Live synthetic runs use the default adapter and require the real feature:

```bash
cargo run -p cmem-eval-runner --features real-character-memory -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_retrieval.toml \
  --out ./runs/synthetic.jsonl \
  --summary-out ./runs/synthetic_summary.json
```

LongMemEval-S and LoCoMo expect local dataset files:

```bash
cargo run -p cmem-eval-runner --features real-character-memory -- run longmemeval-s \
  --dataset ./datasets/longmemeval_s_cleaned.json \
  --config ./configs/longmemeval_s_retrieval.toml \
  --out ./runs/longmemeval_s_v0_1.jsonl \
  --summary-out ./runs/longmemeval_s_v0_1_summary.json

cargo run -p cmem-eval-runner --features real-character-memory -- run locomo \
  --dataset ./datasets/locomo10.json \
  --config ./configs/locomo_retrieval.toml \
  --out ./runs/locomo_v0_1.jsonl \
  --summary-out ./runs/locomo_v0_1_summary.json
```

Gold evidence labels are used only for scoring. They are not copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.

## Precomputed Graph Enrichment

The runner can inject graph-shaped memory objects after raw episodes and
observations are stored. This is meant for enrichment prepared by a separate
LLM/Codex pass over source conversation text, avoiding runtime LLM calls inside
the eval harness.

Enable an artifact with `[ingest] enrichment_path = "..."`. The file is JSONL;
each row is scoped to one eval namespace, such as `lme:<question_id>` or
`locomo:<sample_id>`:

```json
{"namespace":"lme:example","entities":[{"external_id":"user","entity_type":"user","name":"User"}],"threads":[{"external_id":"thread:travel","title":"Travel plans","summary":"The user is planning travel.","status":"active"}],"derived_memories":[{"external_id":"dm:travel:1","derived_type":"claim","text":"The user is considering a May trip.","source_episode_external_ids":["session_1"],"thread_external_ids":["thread:travel"],"entity_external_ids":["user"]}],"links":[{"external_id":"link:dm-thread","from":{"object_type":"derived_memory","external_id":"dm:travel:1"},"relation":"part_of_thread","to":{"object_type":"memory_thread","external_id":"thread:travel"}}]}
```

Supported object types are `episode`, `observation`, `entity`,
`memory_thread`, and `derived_memory`. Supported enum values follow the public
Character Memory API snake-case names, for example `user_preference`,
`relationship_note`, `open_loop`, `character_signal`, and `project_note` for
derived memories.

Enrichment must be generated only from haystack/source conversation data. The
loader rejects common gold-label keys such as `answer`, `evidence`,
`answer_session_ids`, `has_answer`, `gold_*`, and `label` anywhere in the JSON.
Derived memories must include source episode or observation external IDs so
provenance survives round trip.

LoCoMo also has benchmark-provided session summaries and generated observations.
The default LoCoMo config indexes those as provenanced derived memories with
`index_session_summaries = true` and `index_generated_observations = true`.
LongMemEval-S does not include equivalent generated memory fields, so additional
entities, threads, links, and derived memories should come from an enrichment
JSONL artifact.

## Official Exports

Internal benchmark runs write eval JSONL first. Official-compatible artifacts are
post-processing outputs so runs, logs, summaries, and exports can be preserved
independently.

LongMemEval retrieval export writes JSONL rows with `question_id` and
`retrieval_results.ranked_items`:

```bash
cargo run -p cmem-eval-runner -- export-official longmemeval retrieval \
  --input ./runs/longmemeval_s_v0_1.jsonl \
  --out ./runs/longmemeval_s_v0_1_retrieval_official.jsonl
```

LongMemEval QA export requires explicit predictions and never fabricates empty
hypotheses. Prediction JSONL must contain `question_id` plus `hypothesis`
or `prediction`:

```bash
cargo run -p cmem-eval-runner -- export-official longmemeval qa \
  --input ./runs/longmemeval_s_v0_1.jsonl \
  --predictions ./runs/longmemeval_s_predictions.jsonl \
  --out ./runs/longmemeval_s_v0_1_qa_official.jsonl
```

LoCoMo export preserves sample/QA identity recovered from stable internal IDs
like `<sample_id>:qa:<index>`, category, question, optional prediction/context
fields, and retrieved dialog/session IDs:

```bash
cargo run -p cmem-eval-runner -- export-official locomo \
  --input ./runs/locomo_v0_1.jsonl \
  --predictions ./runs/locomo_predictions.jsonl \
  --out ./runs/locomo_v0_1_official.jsonl
```

LoCoMo official answer text is not stored in internal run JSONL, so the export
sets `answer` to `null` unless a later dataset-join export path is added.

## Timestamp And Cleanup Policy

Official benchmark timestamps are normalized before live ingestion. LongMemEval-S
dates such as `2023/05/30 (Tue) 23:40` and LoCoMo dates such as
`1:56 pm on 8 May, 2023` are treated as benchmark-local naive timestamps and
serialized as UTC RFC3339 values, for example `2023-05-30T23:40:00Z`. The raw
timestamp remains in eval metadata for debugging, but the live adapter stays
strict: any non-RFC3339 timestamp that reaches it fails with context instead of
being guessed at the backend boundary.

Backend cleanup is disabled by default. When `[backend.cleanup] enabled = true`,
the runner deletes only live Qdrant collections it created for completed eval
namespaces, and only when `require_collection_prefix` matches the configured
`namespace_prefix` after Qdrant-name sanitization. Cleanup never deletes files
under `runs/`, `reports/`, `datasets/`, or other result artifacts. Leaving
cleanup disabled preserves backend collections for inspection; enabling it makes
repeat runs practical after the JSONL and summary artifacts have been written
successfully.

## Character Memory API

The eval-side adapter contract is in `cmem-eval-core::memory_adapter`. It is written as the target public API boundary for Character Memory: external IDs, namespaces, ranks, scores, rationale, and context text must survive round trip. Live runs require the `real-character-memory` feature and backend settings; the initial embedding default is OpenAI `text-embedding-3-large`.

If the binary is built without `real-character-memory`, omitted `--adapter` and
`--adapter real` fail loudly instead of falling back to mock. Mock output is for
unit/integration smoke checks only and is marked with `adapter.mode =
"mock_smoke"` in result artifacts.
