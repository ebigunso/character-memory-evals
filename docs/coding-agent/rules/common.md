# Common Rules

## Repository Reference Documents

- `C:\Users\Kohta\Downloads\character_memory_eval_repo_setup_guide.md`

## Repository-Specific Validation Commands

- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` before reporting implementation done.
- Run the service-free synthetic smoke command before reporting benchmark CLI changes done: `cargo run -p cmem-eval-runner -- run synthetic --dataset ./fixtures/synthetic_small.json --config ./configs/synthetic_retrieval.toml --out ./runs/synthetic.jsonl --summary-out ./runs/synthetic_summary.json --adapter mock --allow-mock-benchmark`.

## Repo Safety / Boundaries

- Gold evidence labels must be used only for scoring and must not be copied into `EpisodeInput`, `ObservationInput`, or adapter metadata.
- Default validation must remain deterministic and service-free unless the user explicitly asks for real backend integration.
- Benchmark CLI runs default to the live Character Memory adapter; mock benchmark runs must require explicit opt-in and visibly mark outputs as mock/smoke.

## Repo Naming / Structure

- Keep dataset-specific logic in the dataset crates and shared adapter/result/metric contracts in `cmem-eval-core`.

## Global Migration Candidates (Placeholder)

- None.
