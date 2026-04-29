# Worker Rules

## Repo-Specific Worker Notes

- Workers must not edit files outside their `owns` scope unless they report the reason and the exact paths changed.
- Dataset workers must keep gold labels out of adapter metadata and use them only in scorer/result output paths.

## Repo CI / Checks Mapping

- Core changes: run `cargo test -p cmem-eval-core`.
- LongMemEval changes: run `cargo test -p cmem-eval-longmemeval`.
- LoCoMo changes: run `cargo test -p cmem-eval-locomo`.
- Runner changes: run `cargo test -p cmem-eval-runner`.

## Global Migration Candidates (Placeholder)

- None.
