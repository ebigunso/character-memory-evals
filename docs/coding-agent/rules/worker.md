# Worker Rules

## Repo-Specific Worker Notes

- Workers must not edit files outside their `owns` scope unless they report the reason and the exact paths changed.
- Dataset workers must keep gold labels out of adapter metadata and use them only in scorer/result output paths.
- When validation depends on local or generated asset existence (datasets, fixtures, snapshots, manifests), use direct filesystem checks (`Test-Path`/`test -f`) or an explicit no-ignore search (`rg --files --no-ignore`); never infer absence from default rg/fd or tracked-file enumeration — gitignored assets can exist and be required.
- For repository-wide removal acceptance criteria (features, flags, identifiers, command forms), run the audit search from the repository root and explicitly exclude only documented historical or generated paths (e.g. `rg -n <removed-token> . --glob '!docs/coding-agent/plans/**'`); handpicked-path searches do not count as acceptance evidence.

## Repo CI / Checks Mapping

- Core changes: run `cargo test -p cmem-eval-core`.
- LongMemEval changes: run `cargo test -p cmem-eval-longmemeval`.
- LoCoMo changes: run `cargo test -p cmem-eval-locomo`.
- Runner changes: run `cargo test -p cmem-eval-runner`.
- Adapter changes: run `cargo test -p cmem-eval-adapter-cmem`.

## Global Migration Candidates (Placeholder)

- None.
