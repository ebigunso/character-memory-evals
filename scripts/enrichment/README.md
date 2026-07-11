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
