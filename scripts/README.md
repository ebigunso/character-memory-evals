# Scripts

Small helper scripts. Keep required benchmark logic in the Rust crates.

Required benchmark entry points live in `cmem-eval-runner`.

Use live runs for benchmark results:

```bash
cargo run -p cmem-eval-runner -- run longmemeval-s ...
cargo run -p cmem-eval-runner -- run locomo ...
```

## Dataset censuses

- Set `LOCOMO_OFFICIAL_DATASET` to the local official LoCoMo JSON file, then run `bash scripts/locomo_census.sh` to print annotation and derived-memory counts (`cargo run -p cmem-eval-locomo --example official_derived_content_census`).
- Set `LONGMEMEVAL_DATASET` to the local official LongMemEval JSON file and `LONGMEMEVAL_IDENTITY_DUMP` to an output JSON path, then run `bash scripts/longmemeval_census.sh` to print repeated-session counts and write the identity mapping (`cargo run -p cmem-eval-longmemeval --example official_repeated_session_census`). The dump path must not exist; the census refuses to overwrite a file, so it can never truncate the dataset it reads.
