# CharacterMemoryEvals

Evaluation harness for the Character Memory memory substrate. This repository measures retrieval quality, continuity-context construction, context-size reduction, latency, and provenance-oriented integrity signals.

Benchmark CLI runs default to the live Character Memory adapter. Mock runs are available only as explicit smoke/test runs so benchmark output is not accidentally generated from the mock adapter.

## Commands

```bash
cargo test --workspace
cargo run -p cmem-eval-runner -- run synthetic \
  --dataset ./fixtures/synthetic_small.json \
  --config ./configs/synthetic_retrieval.toml \
  --out ./runs/synthetic.jsonl \
  --summary-out ./runs/synthetic_summary.json
```

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

LongMemEval-S and LoCoMo expect local dataset files:

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s \
  --dataset ./datasets/longmemeval_s_cleaned.json \
  --config ./configs/longmemeval_s_retrieval.toml \
  --out ./runs/longmemeval_s_v0_1.jsonl \
  --summary-out ./runs/longmemeval_s_v0_1_summary.json

cargo run -p cmem-eval-runner -- run locomo \
  --dataset ./datasets/locomo10.json \
  --config ./configs/locomo_retrieval.toml \
  --out ./runs/locomo_v0_1.jsonl \
  --summary-out ./runs/locomo_v0_1_summary.json
```

Gold evidence labels are used only for scoring. They are not copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.

## Character Memory API

The eval-side adapter contract is in `cmem-eval-core::memory_adapter`. It is written as the target public API boundary for Character Memory: external IDs, namespaces, ranks, scores, rationale, and context text must survive round trip. Live runs require the `real-character-memory` feature and backend settings; the initial embedding default is OpenAI `text-embedding-3-large`.
