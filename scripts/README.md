# Scripts

Placeholder for small helper scripts. Do not put required benchmark logic here; keep it in the Rust crates.

Required benchmark entry points live in `cmem-eval-runner`.

Use live runs for benchmark results:

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s ...
cargo run -p cmem-eval-runner -- run locomo ...
```

## Prune orphaned Qdrant collections

`qdrant_prune_collections.sh` lists Qdrant collections whose names begin with an exact prefix and deletes them only when passed `--delete`. It uses `QDRANT_REST_URL`, defaulting to `http://127.0.0.1:6333`.

Run it only while holding the orchestrator-managed live-run mutex. Known test collection families include `cmem_eval_*` and `test_collection_*`; always provide the narrowest run-specific prefix rather than either broad family prefix.

```bash
./scripts/qdrant_prune_collections.sh cmem_eval_continuity_continuity_v1_
./scripts/qdrant_prune_collections.sh cmem_eval_prune_selftest_12345 --delete
```
