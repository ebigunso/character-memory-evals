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
