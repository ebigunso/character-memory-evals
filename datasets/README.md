# Datasets

Place benchmark data files here. Do not commit large dataset files.

Expected files for the initial benchmark harness:

```text
longmemeval_s_cleaned.json
locomo10.json
```

## Sources, licenses, and public redistribution

- LongMemEval-S comes from [LongMemEval](https://github.com/xiaowu0162/LongMemEval) and its [cleaned dataset](https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned). The upstream repository identifies the work as MIT-licensed, copyright 2024 Di Wu; retain its license and attribution when redistributing source-derived material.
- LoCoMo comes from the [LoCoMo repository](https://github.com/snap-research/locomo) and `data/locomo10.json`. It is licensed under [Creative Commons Attribution-NonCommercial 4.0 International](https://github.com/snap-research/locomo/blob/main/LICENSE.txt); use and redistribution must remain noncommercial and preserve attribution.
- The checked `continuity_benchmarks_v1` adaptation contains selected byte-for-byte source text and combines both sources, so its LoCoMo-derived portions and the combined artifact are subject to CC BY-NC 4.0. See [`CONTINUITY_BENCHMARKS_ATTRIBUTION.md`](../crates/cmem-eval-continuity/fixtures/CONTINUITY_BENCHMARKS_ATTRIBUTION.md) for authors, source links, license details, and the adaptation record.

Gold labels must only be used for scoring. Do not ingest gold evidence labels into Character Memory.
