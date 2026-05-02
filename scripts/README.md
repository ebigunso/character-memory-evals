# Scripts

Placeholder for small helper scripts. Do not put required benchmark logic here; keep it in the Rust crates.

Required benchmark entry points live in `cmem-eval-runner`.

Use live runs for benchmark results:

```bash
cargo run -p cmem-eval-runner --features real-character-memory -- run longmemeval-s ...
cargo run -p cmem-eval-runner --features real-character-memory -- run locomo ...
```

Use mock runs only for service-free smoke validation:

```bash
cargo run -p cmem-eval-runner -- run synthetic ... --adapter mock --allow-mock-benchmark
```

Official-format exports are post-processing commands over saved internal JSONL:

```bash
cargo run -p cmem-eval-runner -- export-official longmemeval retrieval --input ./runs/longmemeval_s_v0_1.jsonl --out ./runs/longmemeval_s_v0_1_retrieval_official.jsonl
cargo run -p cmem-eval-runner -- export-official longmemeval qa --input ./runs/longmemeval_s_v0_1.jsonl --predictions ./runs/longmemeval_s_predictions.jsonl --out ./runs/longmemeval_s_v0_1_qa_official.jsonl
cargo run -p cmem-eval-runner -- export-official locomo --input ./runs/locomo_v0_1.jsonl --predictions ./runs/locomo_predictions.jsonl --out ./runs/locomo_v0_1_official.jsonl
```
