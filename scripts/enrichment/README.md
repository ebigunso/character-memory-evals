# Source-Only Benchmark Sanitizer

`build_source_only.py` is the sole tool permitted to read raw LongMemEval-S and LoCoMo benchmark files during enrichment artifact regeneration. It constructs new records from hard allowlists; it never copies arbitrary input objects or exposes QA, answer, evidence, gold, evaluation, retrieval, or prediction fields.

LongMemEval-S output rows contain only `question_id`, `question_date`, `haystack_session_ids`, `haystack_dates`, and `haystack_sessions`. Each turn contains only `role` and `content`. The three haystack arrays must be aligned.

LoCoMo output rows contain only `sample_id`, `speaker_a`, `speaker_b`, and a numerically ordered `sessions` array. Each session contains only `session_id`, `date`, and `turns`; each turn contains only `dia_id`, `speaker`, and `text`. Benchmark summaries, observations, events, image metadata, and QA are not admitted.

Run the sanitizer with explicit dataset and file paths:

```text
python scripts/enrichment/build_source_only.py sanitize --dataset longmemeval-s --input <raw-json> --output <source-only-json>
python scripts/enrichment/build_source_only.py sanitize --dataset locomo --input <raw-json> --output <source-only-json>
```

Output is deterministic UTF-8 JSON with decoded Unicode text preserved exactly. Writes use a flushed temporary sibling followed by atomic replacement. Successful logs contain only the dataset mode, row count, input/output paths, and SHA-256 values; source values are never logged.

Run the service-free synthetic tests and syntax compilation without opening benchmark data:

```text
python scripts/enrichment/build_source_only.py self-test
python -m py_compile scripts/enrichment/build_source_only.py
```

The self-tests inject nested gold-bearing fields, verify that they are absent after sanitization, check that the recursive validator rejects forbidden fields including nested `has_answer`, confirm numeric LoCoMo session chronology, and prove that Unicode content including `U+2028` and `U+2029` survives unchanged.

## Provenance and snapshots

The sanitizer also writes a sibling provenance sidecar: `source_only.json` produces `source_only_provenance.json`. Its `input_path` and `input_sha256` identify the official input bytes, `output_sha256` identifies the sanitized output bytes, and `workflow_id` identifies the replay workflow. Regenerate the source-only file to create a missing sidecar; its sanitization and output bytes are unchanged.

`build_snapshots.py` requires that sidecar and checks the sanitized file hash and workflow before generating an artifact. The snapshot manifest records the official hash in `dataset.sha256` (with the dataset name in `dataset.name`); `source.sha256` continues to identify the sanitized file. LongMemEval uses `deterministic-exact-source-replay-v2`; LoCoMo retains `deterministic-exact-source-replay-v1` and its snapshot bytes are unchanged.

LongMemEval assigns episode identities over the complete session list before the question-date cutoff. The first occurrence keeps the raw session ID; later occurrences receive `#2`, `#3`, ... with every raw or previously assigned ID reserved. Identical-turn repeats keep their own dates, thread and canonical identities, and episode/observation provenance. Different-turn repeats are rejected. Thread titles and summaries retain the raw ID; memory text is copied exactly from its source turn.

```text
python scripts/enrichment/build_snapshots.py self-test
python scripts/enrichment/build_snapshots.py generate longmemeval-s --source <source-only-json> --artifact <snapshot-jsonl> --manifest <snapshot-manifest-json> --report <validation-report-md>
python scripts/enrichment/build_snapshots.py validate longmemeval-s --source <source-only-json> --artifact <snapshot-jsonl> --manifest <snapshot-manifest-json> --report <validation-report-md>
```

The snapshot self-test covers repeated and reserved IDs, assignment before the cutoff, exact source text and per-copy provenance, unique memory/link IDs, and missing, mismatched or stale provenance sidecars. Both builders' self-tests are service-free.
