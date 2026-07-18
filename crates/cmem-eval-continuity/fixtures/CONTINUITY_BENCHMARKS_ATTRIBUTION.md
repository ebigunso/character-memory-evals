# Benchmark-Adapted Continuity Fixture Attribution

This public repository distributes the `continuity_benchmarks_v1` fixture as a noncommercial evaluation artifact adapted from selected LongMemEval-S and LoCoMo records. Source questions and selected conversation-turn text are copied byte-for-byte; provenance, selection predicates, relevance labels, and conversion metadata remain outside those text fields.

## Upstream sources and licenses

### LongMemEval-S

- Work: LongMemEval: Benchmarking Chat Assistants on Long-Term Interactive Memory
- Authors: Di Wu, Hongwei Wang, Wenhao Yu, Yuwei Zhang, Kai-Wei Chang, and Dong Yu
- Source repository: <https://github.com/xiaowu0162/LongMemEval>
- Cleaned dataset: <https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned>
- License: MIT, copyright 2024 Di Wu; see <https://github.com/xiaowu0162/LongMemEval/blob/main/LICENSE>

### LoCoMo

- Work: Evaluating Very Long-Term Conversational Memory of LLM Agents
- Authors: Adyasha Maharana, Dong-Ho Lee, Sergey Tulyakov, Mohit Bansal, Francesco Barbieri, and Yuwei Fang
- Source repository and dataset: <https://github.com/snap-research/locomo> and its `data/locomo10.json`
- License: Creative Commons Attribution-NonCommercial 4.0 International (CC BY-NC 4.0); see <https://github.com/snap-research/locomo/blob/main/LICENSE.txt>

The LoCoMo-derived portions, and this combined artifact as distributed with them, are for noncommercial use subject to the upstream CC BY-NC 4.0 terms. No upstream author or organization endorses this adaptation.

## Adaptation record

The curated selection manifest is `crates/cmem-eval-benchmark-convert/continuity_benchmarks_v1_selection.json`. Its proof separates machine-derived predicates from curator assertions: the converter re-derives session count, evidence cleanliness, LongMemEval-S gold-turn emptiness, and LoCoMo cited-evidence image absence, while `self_contained` is a human curation judgment that the converter requires to be asserted. The converter:

- selects 18 source rows and three to five sessions per row;
- maps selected conversation turns to continuity events while retaining source text bytes;
- maps the older LongMemEval-S update turn to `Remember` and the newer turn to `Correct`, with only the replacement ID labeled relevant;
- maps abstention evidence and near-misses only to sampled-negative labels;
- verifies LoCoMo cited evidence is nonempty, uniquely resolved, selected, and free of `img_url` values;
- prunes unselected sessions and omits answers, source licenses, and other provenance from behavioral text fields; and
- produces a schema-v3 frozen-embedding fixture plus a ranked-cosine manifest.

Session timestamps are normalized to UTC and intra-session event offsets are synthesized deterministically. Event IDs, namespaces, selection labels, fixture structure, and embeddings are additions made by this repository. The selected source questions and turn text are not rewritten, summarized, speaker-prefixed, or otherwise modified.

The frozen store was generated from the fixture's exact embedding inputs with OpenAI `text-embedding-3-large`. It contains numerical vectors, the model identifier, the source tag `open_ai_api`, and source text required by the frozen-store integrity contract; it contains no API credential.

## Special abstention case

LoCoMo `conv-26`, QA index 153 asks what Caroline realized after her charity race, while cited turn `D2:3` states that Melanie realized self-care matters. The converter therefore admits the cited turn as a sampled negative only and leaves the relevant set empty. This preserves the source's speaker-swapped trap without converting cited evidence into gold relevance.
