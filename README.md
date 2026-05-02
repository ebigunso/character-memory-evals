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
