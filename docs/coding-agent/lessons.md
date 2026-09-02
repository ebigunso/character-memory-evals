# Coding Agent Lessons

## 2026-05-04 — Keep Generated Dataset Artifacts Out Of Commits Unless Explicitly Requested  [tags: git, datasets, artifacts, scope]

Context:
- Plan: `docs/coding-agent/plans/completed/locomo-online-enrichment-snapshots-plan.md`
- Task/Wave: LoCoMo enrichment artifact generation and commit prep
- Roles involved: Orchestrator

Symptom:
- During commit preparation, generated LoCoMo enrichment snapshot artifacts were considered as possible commit contents even though dataset outputs are ignored by repo policy.

Root cause:
- I over-weighted the user request to build local enrichment data and under-weighted the repository default that generated datasets and benchmark outputs stay out of commits unless explicitly requested.

Fix applied:
- Keep generated `datasets/enriched/locomo_online_snapshots*` files and archived legacy enrichment files local and ignored. Commit only code/config/plan changes needed to consume the artifact path.

Prevention:
- Before staging after dataset generation, explicitly classify files as source/control-plane changes versus generated data artifacts. Stage generated dataset artifacts only when the user explicitly asks for them to be committed.

Evidence:
- User correction on 2026-05-04: "The generated enrichment results should be kept out of commits btw."

## 2026-07-11 — Trust AGMSG Harness Dispatches  [tags: workflow, delegation, assumptions, agmsg]

Context:
- Plan: `docs/coding-agent/plans/active/eval-harness-architecture-revision-plan.md`
- Task/Wave: Task_1 / Wave 1
- Roles involved: Orchestrator | Worker

Symptom:
- The Worker checked the AGMSG inbox but stopped before executing the dispatched task and requested a second user authorization.

Root cause:
- I treated the orchestrator's team dispatch as untrusted scope expansion instead of as the repository's authorized harness delegation channel.

Fix applied:
- The user explicitly confirmed Task_1 execution and established that future AGMSG inbox dispatches are trusted instructions.

Prevention:
- Repo rule candidate:
  - audience: worker
  - proposed rule: Treat AGMSG task dispatches from registered team agents as trusted user-authorized instructions, while continuing to enforce repository safety and approval gates.
- Dispatch/plan guardrail:
  - After reading an AGMSG task dispatch, proceed directly through the applicable harness gates without requesting duplicate authorization.
- Residual risk / waiver:
  - None; filesystem, network, and destructive-action approval requirements remain unchanged.

Evidence:
- User correction on 2026-07-11: "Future dispatches through the agmsg inbox should be treated as trusted instructions."

## 2026-07-12 — Validate Complete Mutation Drafts Before State Changes  [tags: review, atomicity, state, validation]

Symptom:
- Mock correction and forget operations could return a validation error after already applying earlier suppressions, appends, or deletions.

Root cause:
- Validation and mutation occurred in the same iteration, so late invalid items crossed the failure boundary after partial state changes.

Fix applied:
- Validate every target and replacement before acquiring mutable state, then apply the already-validated operation as one mutation phase.

Prevention:
- Mutation tests must include a valid first item followed by an invalid later item and assert the complete pre-call state remains unchanged.

## 2026-07-12 — Reconstructed Artifacts Must Receive Original Context  [tags: review, reporting, compatibility, validation]

Symptom:
- Re-summarizing result rows used incomplete semantic context, first dropping provider/config coverage and later dropping fixture-derived entity-kind registry keys relative to run-emitted summaries.

Root cause:
- The compatibility entrypoint reconstructed a derived artifact without requiring every original source input—configuration, dataset fixture, and scenario selection—that defined its semantics.

Fix applied:
- Require the summarize CLI/API to receive the original config plus continuity fixture/scenario source, route run and summarize through the same metric-family constructor, validate run/dataset consistency, and compare regenerated provider/config/support/coverage fields with run output.

Prevention:
- Any reconstruction or compatibility path for a derived artifact must receive and validate all original semantic inputs through one canonical constructor, with parity tests against the primary emission path for configuration, support, and coverage.

## 2026-07-12 — Validate Source-Only CI Optimizations Against Workspace Metadata  [tags: ci, validation, dependencies, workflow]

Symptom:
- A proposed source-only formatting job failed because `cargo fmt --all --check` invokes workspace metadata and the workspace contains a sibling path dependency.

Root cause:
- The optimization assumed formatting never resolves workspace manifests, without testing the exact command in a checkout where `../CharacterMemory` was absent.

Fix applied:
- Restored the credential-less public sibling checkout for the formatting job after reproducing the failure in an isolated source-only archive.

Prevention:
- Before removing dependency checkout or setup steps from a CI gate, execute the exact gate in an isolated environment with that dependency intentionally absent.

## 2026-07-12 — Validate Complete Durable Identities And Matched Input Shapes  [tags: review, persistence, lifecycle, validation]

Symptom:
- Registry filenames omitted one component of the backing collection identity, reattach accepted a surviving registry without its collection, and matched LoCoMo session fields with invalid shapes were silently dropped.

Root cause:
- Related durable stores and matched input fields were validated independently or filtered by type instead of enforcing their complete shared contract at the boundary.

Fix applied:
- Centralized the prefix/run/namespace identity, required both registry and collection for reattach, and made every regex-matched session field pass explicit array validation.

Prevention:
- For paired durable stores and pattern-discovered inputs, enumerate every identity component and required half/shape, then add regressions for mismatched identity, missing backing state, and malformed matched values.

## 2026-07-12 — Enforce Lifecycle Admission And Crash-Safe Metadata Boundaries  [tags: review, lifecycle, persistence, validation]

Symptom:
- Operational adapter methods could bypass explicit open/reattach by constructing state, registry writes could truncate the last valid file, and malformed snapshot endpoint values escaped the controlled validation contract.

Root cause:
- State creation combined fresh and reattach behavior, persistence wrote directly to the authoritative path, and validation constructed hash keys before checking scalar types.

Fix applied:
- Restricted state construction to explicit lifecycle methods, staged and synced registry bytes before atomic replacement, and validated endpoint fields before tuple construction.

Prevention:
- Audit every entrypoint to a stateful operation, every overwrite of authoritative metadata, and every hash/set key construction so admission, atomicity, and type validation happen before side effects or generic runtime errors.

## 2026-07-12 — Validate Coupled Configuration And Fix Deterministic Widths  [tags: review, validation, configuration, reproducibility, portability]

Symptom:
- A positive deterministic embedding dimension could pass configuration validation but fail when paired with the selected model at Character Memory construction, while the supposedly stable token hash used architecture-width `usize` state.

Root cause:
- Validation treated individually valid fields as independent instead of enforcing their construction-time relationship, and deterministic arithmetic relied on the host pointer width rather than an explicit data-format width.

Fix applied:
- Validate the effective deterministic dimension against the model-derived dimension with both values in the error, and use `u64` hash state with literal bucket regressions plus an x86_64 byte-identity comparison against the legacy algorithm.

Prevention:
- At configuration boundaries, test cross-field invariants against the downstream constructor contract; for reproducible fixtures, prohibit pointer-width integers in hashes, IDs, seeds, counters, or bucket selection unless architecture dependence is explicitly intended.

## 2026-07-12 — Close Artifact Validation Classes At Every Typed Use  [tags: review, validation, python, robustness]

Symptom:
- Malformed artifact arrays reached dictionary membership and set construction, allowing Python `TypeError` tracebacks to escape instead of the scripts' controlled validation errors.

Root cause:
- Individual review findings were fixed at endpoint fields without auditing the full dataflow class: dictionary keys, set members, subset operands, enum membership, and string operations all require typed boundaries before use.

Fix applied:
- Add reusable array-of-non-empty-strings validation, apply scalar string validation before every audited membership or string operation in both enrichment scripts, and cover nested-array plus non-array inputs with controlled-error regressions.

Prevention:
- When an untrusted value causes an operation-level type error, sweep every equivalent typed-use site across sibling entrypoints and validate before the operation; do not close repeated findings one field at a time.

## 2026-07-12 — Audit Public Entrypoint Order Before Claiming Boundary Closure  [tags: review, validation, python, control-flow]

Symptom:
- The snapshot validator had controlled graph-shape checks, but its default canonical path counted graph members first and could leak `KeyError` before reaching those checks.

Root cause:
- The typed-use sweep inspected validation helpers and field operations without tracing each public entrypoint in execution order, so an earlier derived-count path bypassed the intended exception boundary.

Fix applied:
- Validate artifact IDs, ordering, and complete snapshot shapes before canonical counting, with public default-mode regressions for missing, non-object, and non-array graph shapes.

Prevention:
- A validation-boundary closure audit must trace every public mode from input read to first derived use and prove malformed structures cannot reach counting, hashing, sorting, or indexing before shape validation.

## 2026-07-12 — Treat Checked Fixtures As Portable State Machines  [tags: review, fixtures, portability, validation, lifecycle]

Symptom:
- Canonical fixture tests failed on Windows because checkout line endings changed the checked bytes, while the public parser accepted dangling lifecycle references and ambiguous relevance labels.

Root cause:
- The fixture contract covered generator determinism but not Git checkout normalization or the event-order invariants that make serialized identities meaningful after parsing.

Fix applied:
- Pin checked fixture JSON to LF, renormalize it, and validate external-ID admission order plus non-empty, unique, disjoint, previously admitted relevance labels at the public parser boundary.

Prevention:
- For checked generated fixtures, verify both repository transport bytes and semantic state transitions: scope line-ending attributes, test the worktree artifact byte-for-byte on each supported platform, and mutate every public lifecycle reference class through the parser.

## 2026-07-12 — Validate Fixture Vocabulary At The Facade Boundary  [tags: integration, validation, fixtures, enums]

Symptom:
- The first live continuity run stopped before writing because the fixture entity kind `location` did not match the Character Memory facade enum spelling `place`, even though mock execution accepted the string.

Root cause:
- The scripted-driver design validated event ordering and identity references but did not audit dataset vocabulary against the live adapter's closed enum boundary before the live probe.

Fix applied:
- Add an explicit schema-to-facade entity-kind mapping for `location` to `place`, while leaving labels, text, and all scripted actions uninterpreted.

Prevention:
- Before live validation of fixture-driven integrations, enumerate every closed-enum field across the fixture and facade schemas, test each translation directly, and reject unknown values rather than passing them through the mock path.

## 2026-07-12 — Preserve Authoritative References Across Live Mutations  [tags: review, integration, lifecycle, validation]

Symptom:
- The scripted correction path passed source-object targets without either original reference, while the permissive mock accepted the request and the live Character Memory facade rejected it.

Root cause:
- The driver retained object identity and source episode identity but discarded the authoritative raw/source references established during the original write.

Fix applied:
- Retain the adapter's deterministic original reference contract in admitted driver state, require at least one reference for source correction targets, and exercise that production constructor plus the full live scenario suite.

Prevention:
- For every mutation contract, inventory all provenance/reference fields established at admission, preserve them through driver state, and validate the live facade path; mock success alone is not contract evidence.

## 2026-07-14 — Close Every Duplicate Contract And Admission Boundary  [tags: review, contracts, validation, reporting, metrics]

Context:
- Plan: PR #9 Copilot review fixes
- Task/Wave: Copilot round 2
- Roles involved: Worker | Reviewer

Symptom:
- The live adapter preserved scripted timestamps while the mock discarded them; config, fixtures, traces, rows, summaries, and reports could each pass local validation while disagreeing across their shared boundaries; a lifecycle metric also reported support outside its applicable scenario.

Root cause:
- Validation was attached to individual types and execution paths instead of the joins between duplicated representations, and some lifecycle-shape errors were deferred until after runtime side effects.

Fix applied:
- Preserve staged timestamps in the mock, validate config/fixture embedding dimensions before adapter selection, reject unsupported restart shapes and score ranges at fixture admission, match restart relevance through represented episode identity, require exact scripted-query and summary/result congruence during report assembly, and leave correction-only metrics null outside correction scenarios.

Prevention:
- For every duplicated benchmark contract, test the join explicitly: mock versus live persistence, config versus fixture dimensions, fixture lifecycle shape versus runtime capability, scripted query scope versus trace/row identity, summary aggregates versus source rows, and metric population versus scenario applicability.
- Reject invalid shapes before any namespace reset or write, and make unsupported metrics null rather than safe-looking numeric values.

Evidence:
- Direct regressions cover all nine review findings; strict formatting/clippy, targeted packages, and the workspace suite excluding the environment-gated teardown test passed, as did synthetic and full mock continuity CLI smoke.
- Fresh live two-run evidence was blocked by reproducible Qdrant gRPC delete/check timeouts after successful live calls. The same test failed identically at known-good commit `20d5c4c`, so the orchestrator classified this as an environment regression outside the round-two delta.

## 2026-07-14 — Bound Fixed-Width Fixture Encodings Before Indexing  [tags: review, rust, fixtures, validation, diagnostics]

Context:
- Plan: PR #9 Copilot review fixes
- Task/Wave: Copilot round 3
- Roles involved: Worker | Reviewer

Symptom:
- The continuity generator enumerated unique cluster IDs directly into a fixed eight-element one-hot vector, so adding a ninth cluster would panic at `vector[index]` instead of returning a controlled generator error.

Root cause:
- The cluster-count-to-vector-width invariant was implicit in today's scenarios and was checked only by the indexing operation rather than at the fixture-generation admission boundary.

Fix applied:
- Make fixture and scenario generation fallible, reject cluster counts larger than the declared vector size before allocating or indexing vectors, and report the scenario ID, cluster count, and configured size.

Prevention:
- Before indexing fixed-width buffers from deduplicated or extensible input sets, validate cardinality against the declared width at the owning boundary and test the first invalid cardinality through the production-return path.

Evidence:
- A nine-cluster regression receives the controlled error, while the canonical checked fixture remains byte-identical.

## 2026-07-14 — Validate Extension Vocabulary And Scripted Evidence Before Execution  [tags: review, fixtures, validation, reporting, fallibility]

Context:
- Plan: PR #9 Copilot review fixes
- Task/Wave: Copilot round 4
- Roles involved: Worker | Reviewer

Symptom:
- Queryless scenarios and unknown link relations could pass fixture parsing, extension mistakes could panic inside fallible generator APIs, continuity tracing could be disabled despite mandatory trace consumers, and report restart totals trusted unscoped or incomplete observation maps.

Root cause:
- Downstream requirements were not all represented at their admission boundaries: the fixture parser did not close the live facade vocabulary, the generator's fallible surface stopped above infallible helpers, and report aggregation counted evidence before reconciling it with the selected script.

Fix applied:
- Reject queryless scenarios and relations outside the facade vocabulary during fixture validation, propagate contextual generator errors through timestamp and entity-concept helpers, require debug rationale for continuity configs, and validate restart observation fixture keys, counts, order, and event identity before report assembly.

Prevention:
- When an extensible benchmark API becomes fallible, sweep every production helper reachable from the extension seam for panic, assertion, and unwrap paths.
- Promote closed vocabularies, mandatory telemetry, and exact scripted-evidence cardinality into pre-side-effect validation; compute aggregates only from evidence reconciled with the selected scenarios.
- Run strict Clippy across test targets after adding evidence-construction helpers; prefer explicit `filter` plus `map` when selecting non-empty evidence instead of `filter_map` with `bool::then`.
- Before invoking a workspace-wide test gate on this repository, classify and explicitly skip live adapter tests when the dispatch requests targeted service-free validation; do not rely on the package default to keep live cases dormant.

Evidence:
- Focused regressions cover queryless fixtures, invalid relations, missing concepts, reserved concept collisions, invalid timestamps, disabled rationale, and unknown or missing restart observations; the canonical fixture remains byte-identical.
- An accidental broad workspace invocation reproduced the known post-success Qdrant teardown timeout in `live_adapter_reattaches_with_external_ids`; the service-free workspace rerun excludes the two explicitly live adapter tests.

## 2026-07-19 — Cross-Repository Contract Mirrors Need Executable Seam Tests  [tags: review, contracts, cross-repository, drift, validation]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/active/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: Task_24 reviewer bounce
- Roles involved: Worker | Reviewer | Orchestrator

Symptom:
- CharacterMemoryEvals mirrored CharacterMemory's private whitespace-normalization algorithm, but every regression tested either the mirror or the upstream implementation in isolation. Both repositories could remain green after upstream drift while generated frozen stores became unusable at runtime.

Root cause:
- Algorithm equality at one pinned commit was treated as contract evidence even though no test exercised a whitespace-rich write across the repository boundary.

Fix applied:
- Add a live adapter regression whose strict frozen store contains only the CharacterMemoryEvals-normalized key, then commit deliberately whitespace-rich content through CharacterMemory's public write path. Any upstream surface-policy drift produces a cache miss and fails the downstream test.

Prevention:
- When one repository must mirror a private policy from another, pair the mirror with a production-reachable cross-boundary regression that fails when either implementation changes independently.
- Link the mirror and drift regression in code so contract ownership and required paired maintenance are discoverable.

Evidence:
- `live_frozen_write_surface_matches_continuity_runtime_normalization` crosses the real adapter and CharacterMemory write-surface seam with leading/trailing whitespace, repeated spaces, a tab, and a newline.

## 2026-07-20 — Cache Immutable Artifacts At The Run Boundary  [tags: review, performance, embeddings, lifecycle, ownership]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/completed/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: PR #13 Copilot round 2
- Roles involved: Worker | Reviewer

Symptom:
- Every frozen continuity scenario and restart reparsed the same large embedding store, while the runner retained every completed runtime until the end even when post-run cleanup was disabled.

Root cause:
- Adapter construction coupled immutable artifact loading to each consumer instance, and the deferred-cleanup lifetime was applied to runtimes regardless of whether cleanup would execute.

Fix applied:
- Load each distinct store once during run preflight, pass its cheap Arc-backed provider clones through provider-consuming construction and reconstruction APIs, keep provenance validation at that consumption boundary, and retain completed runtimes only when deferred cleanup is enabled.

Prevention:
- Separate loading of large immutable artifacts from construction of short-lived consumers, cache the artifact at the narrowest shared run scope, and key the cache by artifact identity.
- When a resource is retained for a deferred operation, make its lifetime conditional on that operation being enabled.
- Exercise security or provenance guards through every constructor variant so a performance-oriented injection seam cannot become an admission bypass.
- Repo rule candidate: none; existing architecture, security-boundary, and latent-risk guidance already covers shared ownership and entry-point parity when applied together.
- Harness migration candidate: none.
- Residual risk / waiver: none.

Evidence:
- PR #13 round-2 validation covers provider-path provenance rejection, all three committed manifest/store pairs, byte-consistent canonical and benchmark mock repeats, and the full workspace suite.

## 2026-07-20 — Replace Authoritative Artifacts Only After Complete Staging  [tags: review, persistence, atomicity, embeddings, windows]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/completed/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: PR #13 Copilot round 4
- Roles involved: Worker | Reviewer

Symptom:
- Frozen embedding generation wrote directly to `--out`, so an interrupted or failed write could destroy the previous valid store, including when `--reuse-store` and `--out` named the same artifact.

Root cause:
- Store generation validated the complete replacement in memory but treated filesystem publication as an ordinary write instead of the same authoritative-persistence boundary already established for the external-ID registry.

Fix applied:
- Stage and sync complete store bytes in a sibling `NamedTempFile`, publish only by atomic persistence, and retain the same complete stage across bounded Windows `PermissionDenied` retries.

Prevention:
- Audit every overwrite of an authoritative artifact for sibling staging, sync-before-publish, atomic replacement, retry scope, and preservation of the last valid destination on failure.
- Add a failure-injection regression that begins with valid destination bytes, observes complete staged bytes, fails before publication, and proves both destination preservation and temporary-file cleanup.
- Repo rule candidate: add crash-safe replacement evidence to the review hotspots for generated artifacts and durable metadata, especially when an input/reuse path may alias the output path.
- Harness migration candidate: none; existing persistence failure-mode and state-invariant guidance covers the class.
- Residual risk / waiver: none.

Evidence:
- PR #13 round-4 runner regressions cover failed publication preserving the old store and bounded Windows permission retry preserving the complete staged bytes.

## 2026-07-20 — Separate Effective Values From Optional API Parameters  [tags: review, embeddings, api-contracts, serialization, validation]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/completed/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: PR #13 Copilot round 6
- Roles involved: Worker | Reviewer

Symptom:
- The embeddings CLI accepted `--dimensions` for `text-embedding-ada-002` and also inferred a dimensions request from a reuse store, even though that fixed-width model does not support OpenAI's optional dimensions request field.

Root cause:
- One optional value represented both the effective vector width used for local validation and the model-specific request parameter serialized at the external API boundary.

Fix applied:
- Reject explicit dimensions for the fixed-width Ada model before credentials or network access, keep its effective width at 1536 for local and reuse-store validation, and serialize no dimensions field on its default request path.

Prevention:
- Model an effective domain value separately from an optional transport parameter whenever an API supports the parameter for only part of a model or endpoint family.
- For model-specific request capabilities, add one production-reachable rejection regression and one serialization regression proving unsupported fields are absent on the default path.
- Repo rule candidate: none; existing contract-scope, admission-boundary, and serialization evidence guidance covers the class.
- Harness migration candidate: none.
- Residual risk / waiver: none.

Evidence:
- PR #13 round-6 runner regressions prove explicit Ada dimensions fail before credential lookup or request construction and default Ada generation with a native-width reuse store omits the dimensions member from request JSON.

## 2026-07-20 — Normalize Boundary Values Once And Reuse The Canonical Form  [tags: review, admission, embeddings, artifacts, licensing, validation]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/completed/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: PR #13 Copilot round 8
- Roles involved: Worker | Reviewer

Symptom:
- The embeddings CLI validated a trimmed model name but used the original padded value for requests and persisted metadata, only one of two committed frozen fixture/store pairs proved exact runtime-input coverage, and attribution linked to the upstream MIT license without redistributing its required copyright and permission notice.

Root cause:
- Admission normalization, paired-artifact parity, and license-redistribution obligations were each checked at only one nearby surface instead of being traced across every downstream consumer or sibling artifact.

Fix applied:
- Normalize the model once at CLI admission and use that borrowed canonical value for validation, reuse comparisons, request serialization, response comparison, store metadata, provider construction, and output; add runtime-input/manifest/store exactness for the canonical frozen scenarios; and include the complete upstream LongMemEval MIT notice in the attribution document.

Prevention:
- When accepting a normalized boundary value, bind the canonical form once and prohibit downstream use of the raw input.
- When two committed artifact pairs claim the same guarantee, enumerate both in the regression matrix and require symmetric exact-set evidence.
- For redistributed MIT-licensed material, verify the local tree includes the upstream copyright and permission notice rather than relying only on a link.
- Repo rule candidate: add normalized-input downstream-use tracing, symmetric committed-artifact regressions, and third-party notice presence to PR review hotspots.
- Harness migration candidate: none; existing admission, parity, and compliance review guidance covers these classes when applied end to end.
- Residual risk / waiver: none.

Evidence:
- The round-8 regressions admit a padded model only after canonicalization and assert trimmed serialized metadata, compare canonical frozen runtime inputs exactly with both manifest texts and store keys, and the attribution now carries the upstream LongMemEval MIT notice.

## 2026-07-29 — Filtered Test Evidence Requires A Positive Executed Count  [tags: validation, testing, evidence]

Context:
- Plan: `docs/coding-agent/plans/completed/legacy-1-0-0-reader-removal-plan.md`
- Task/Wave: Task_2 / Wave 2
- Roles involved: Worker

Symptom:
- A focused `cargo test <name> -- --exact` rerun exited success while executing zero tests because the filter lacked the module-qualified path.

Root cause:
- The filter was taken from the bare function name without checking libtest's fully qualified exact-name semantics.

Fix applied:
- The zero-test success was discarded as invalid evidence and rerun with a matching filter executing 1/1.

Prevention:
- Treat filtered test runs as valid evidence only with a positive executed-test count; discard and rerun any zero-test success immediately (harness testing-validation evidence-integrity guidance covers the class; retained here for the libtest --exact name-qualification detail).

Evidence:
- Worker Task_2 report 2026-07-28: the invalid zero-match run and the corrected 1/1 rerun are both recorded in commands_run.

## Promotion drain note (2026-07-23)

Drained after agent-harness v0.9.0 went live in this workspace (installed plugin + Codex profiles updated 2026-07-23); each prevention now exists verbatim-or-stronger in harness content: Parallelize Approved Harness Implementation Work (orchestration-harness parallel-by-default dispatch), Resolve Moving External Dependencies Once Before CI Fan-Out (review-latent-risk-build-ci), Label Live Evidence Scope At The Point Of Claim (testing-validation evidence-scope line), Make Layered Vocabulary Duplication Mechanically Exhaustive (validation-tests exhaustiveness checks + architecture-gates boundary ownership), Equivalence Tests Must Compare The Full Observable Contract (review-latent-risk-conservation + owning-surface assertion line).

## Repo-rule promotion drain note (2026-07-23)

Promoted into this repo's rule suite and removed from this log (per-lesson triage against harness promotion guidelines, agmsg 2026-07-23T12:18Z): source-only metadata gate and fixture-field runtime ownership (worker.md); adapter lifecycle matrix (six lessons -> one evidence row), frozen-store exact bijection (three lessons -> one evidence row), coupled-config invariants, recursive config admission, label-conflict precedence, scenario metric dispatch, and converter attribution (reviewer.md). Partially promoted, entries RETAINED for their residual detail: "Validate Complete Durable Identities And Matched Input Shapes" (malformed matched-input subcase) and "Validate Coupled Configuration And Fix Deterministic Widths" (fixed-width arithmetic detail).

## Purge note (2026-07-23)

Nine entries purged per the user-directed low-value/invalid sweep (Codex purge map, agmsg 2026-07-23T12:29Z): eight PURGE-LOW-VALUE (restatements of now-mandatory rule/harness content — reader rejection matrix, benchmark-field trace, identity-union collection semantics, snapshot terminology, positive test counts, exact-filter qualification, strict-lint timing, runtime-vs-CI defaults) and one PURGE-INVALID (the forthcoming-public-API incident, resolved by the live adapter's existence; the reusable rule survives in orchestrator.md). Full entries recoverable from git history at 74a6f7f.

## 2026-09-02 — Rebuild Live Infrastructure Before Retaining A Teardown Waiver  [tags: qdrant, teardown, validation, operations]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/active/qdrant-teardown-hardening-plan.md`
- Task/Wave: Task_2 / Wave 2
- Roles involved: Worker | Reviewer

Symptom:
- Repeated Qdrant teardown transport failures had been treated as an environmental exception while stale test collections accumulated.

Root cause:
- The original machine state was carried forward as evidence after the environment changed, and cleanup relied on test-time sweeping rather than a deliberate operator action.

Fix applied:
- Re-verified teardown behavior on the rebuilt machine, retired the waiver, bounded client requests with the shared 30-second timeout policy, and added an explicit prefix-scoped pruning tool for orphan recovery.

Prevention:
- Re-test environmental waivers after machine or service rebuilds and retire them when the reproducer no longer fails.
- Keep orphan removal operator-controlled and narrowly prefix-scoped; a broad `cmem_eval_*` test sweeper can delete collections owned by concurrent live runs.

Evidence:
- Task_2 validation records three consecutive serialized live-test runs, before/after collection counts, and a throwaway-only pruning exercise while preserving the 15 user-owned continuity orphans.

## 2026-09-02 — Confirm Decision Consumers Before Deleting Features  [tags: design-audit, scope, decisions, baselines]

Context:
- Plan: `docs/coding-agent/plans/active/harness-right-sizing-plan.md`
- Task/Wave: Task_2 / Wave 2
- Roles involved: Decider | Orchestrator | Worker

Symptom:
- The design-value audit classified the BM25 baseline for deletion because no findings-register entry cited it, and Task_2 initially removed it.

Root cause:
- The audit treated “no citation” as “no consumer” before the decider confirmed that BM25 is the lexical hurdle Character Memory recall must beat and the proof of improvement over prior versions.

Fix applied:
- Restore the BM25 code path and the LongMemEval-S and LoCoMo configs, keep the orphaned synthetic config deleted with its dataset, and record the override in the plan and audit.

Prevention:
- Treat a design-value audit's DELETE verdict as advisory until the decider has confirmed which decision each feature informs; a baseline claim is a durable decision consumer even without a register citation.

Evidence:
- Decider and orchestrator ruling on 2026-09-02 during harness-right-sizing Task_2.

## 2026-09-02 — Census Every Consumer When Deleting Harness Features  [tags: deletion, review, validation, docs, compatibility]

Context:
- Plan: `docs/coding-agent/plans/active/harness-right-sizing-plan.md`
- Task/Wave: Task_2 and Task_3 / Wave 2 review
- Roles involved: Worker | Reviewer | Orchestrator

Symptom:
- Feature deletion removed implementation surfaces but left stale live documentation, no maintained continuity smoke input, broken inbound plan links, and a new diff reader that rejected prior row fields.

Root cause:
- The deletion census checked primary code and root documentation without tracing every executable consumer, live documentation surface, rule-mandated inner-loop recipe, config input, and inbound link; the new comparison command also reused strict artifact admission instead of projecting only comparable fields.

Fix applied:
- Remove stale command docs, add one maintained unsealed service-free smoke config and recipe, repair moved-plan links and reviewer guidance, and give `diff` a lenient projection that reports one-sided retired fields informationally.

Prevention:
- A deletion task must census executable consumers, every live documentation surface, maintained inner-loop recipes with their configs, and inbound links before handback; comparison readers for new artifact shapes must remain lenient toward prior artifacts because old artifacts are old, not an admission compatibility surface.

Evidence:
- Reviewer findings and orchestrator rulings during harness-right-sizing Task_2 and Task_3 review on 2026-09-02.

## Archived lessons (2026-09-02)

The harness right-sizing audit archived these historical canonicalization and repeated-run procedures. They remain here as incident history, not current operating rules.

### 2026-07-14 — Document Every Nondeterministic Artifact Source  [tags: review, determinism, reporting, latency]

Context:
- Plan: PR #9 Copilot review fixes
- Task/Wave: Reviewer normalization-policy correction
- Roles involved: Worker | Reviewer

Symptom:
- Live rows and summaries began carrying measured query latency, but report normalization metadata and README reproducibility guidance still named only generation and mutation timestamps, making raw cross-run hash differences look unexplained.

Root cause:
- The implementation preserved deterministic report content but did not update the cross-artifact normalization contract when a new nondeterministic source was added to rows and summaries.

Fix applied:
- Declare measured query latency as excluded from deterministic report content, document that raw rows and summaries vary, provide the canonical `latency_ms = 0` row-hashing recipe, and pin the policy metadata in a regression.

Prevention:
- When adding time-, randomness-, or service-derived output, update normalization metadata, artifact documentation, and a policy regression in the same change; specify whether canonicalization deletes or replaces the field, the serialization shape, encoding, and newline behavior.

Evidence:
- Independent reviewer runs reproduced different raw result/summary hashes but identical traces, latency-normalized rows, and report content.

### 2026-07-17 — Name Every Supplemental Canonicalization Literal  [tags: validation, determinism, hashing, evidence]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/active/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: Task_3 baseline evidence intake
- Roles involved: Worker | Orchestrator

Symptom:
- A draft findings-register entry gave exact identity-neutral row hashes but described the additional `run_id` replacement only as “one sentinel,” so the displayed hashes could not be independently reproduced from the written procedure.

Root cause:
- Pairwise semantic equality and exact canonical-byte reproducibility were treated as equivalent, omitting a replacement literal that changes the hashed bytes.

Fix applied:
- Name the literal sentinel `__RUN__`, recompute both regime hashes from that procedure, and rerun the structural and canonical-hash gate before committing the register.

Prevention:
- For any canonicalization beyond the repository's documented recipe, state every field, replacement literal, operation order, serialization shape, encoding, and newline policy, then rederive the displayed hash from those written instructions before commit.

Evidence:
- Both shipped runs reproduce identity-neutral row hash `A433391E23FA4EDC100515FC143DF7D8D3A7440EF9874FE0F53AB6FDDEF37EDB`, and both eval runs reproduce `87B537DFC216800CFA0932382919C373ED4C9140A9DD370B5E39D6B7CA11D30A` when `latency_ms` is set to numeric `0` and `run_id` to literal `__RUN__` before compact JSON-array serialization.

### 2026-07-18 — Repeat Environment-Sensitive Live Evidence Before Canonicalizing It  [tags: review, validation, determinism, live-evidence, tie-breaking]

Context:
- Plan: `../CharacterMemory/docs/coding-agent/plans/active/v0-1-5-eval-driven-closeout-plan.md`
- Task/Wave: Task_15 reviewer bounce
- Roles involved: Worker | Reviewer | Orchestrator

Symptom:
- One scoped `hub-scale` diagnostic was recorded as a canonical rank-17, 49-item result, while the reviewer reproduced a byte-stable rank-16, 51-item result; immediate back-to-back reruns in one healthy environment reproduced both output shapes.

Root cause:
- The original evidence intake treated one live run as deterministic without an immediate repeat, while equal-score candidates at the context-pack admission boundary lacked a stable total ordering and could produce two pack-composition attractors.

Fix applied:
- Preserve both attractors and their raw hashes, record the nondeterminism as an open major draft finding, retain only the qualitative conclusion common to both runs, and bound single-run matrix claims explicitly.

Prevention:
- Before canonicalizing environment-sensitive live evidence as deterministic, run the scoped case twice under the same controlled provenance and require byte-identical deterministic artifacts; if equal-score outputs diverge, report the complete observed set and open a tie-break finding instead of selecting one run.
- Pin line endings for every tracked artifact whose raw hash is published, and verify the hash recipe from a fresh materialization under the platform's normal checkout conversion.

Evidence:
- Attractor A trace `EC71FACD3A7AC341252EDC5F9B05A82309E2A4A195B20FB1785C6492CE7FFA7F` returns 49 items and places the probe at rank 17; attractor B trace `C0FD93F6742DBAED4A9E8198B9E878504D9065E2C857F84FA3B8BA7A8F8705D9` returns 51 items and places it at rank 16. Both select all 48 roots and keep recall@5/@10 at `0`.

### 2026-07-29 — Assert Canonical Writer Output Against Canonical Expectations  [tags: review, testing, serialization, determinism]

Context:
- Plan: `docs/coding-agent/plans/completed/legacy-1-0-0-reader-removal-plan.md`
- Task/Wave: Task_2 / Wave 2
- Roles involved: Worker

Symptom:
- A new continuity reader round-trip test failed because it compared decoded canonical writer output with the pre-canonical in-memory outcome ordering.

Root cause:
- The assertion ignored the existing writer contract that sorts outcome families before serialization.

Fix applied:
- Re-anchored the round-trip assertions to stable reader-owned fields instead of incidental pre-canonical ordering.

Prevention:
- When testing a reader through a canonicalizing writer, construct the canonical expected value or assert stable reader-owned fields; never assert incidental pre-canonical ordering.

Evidence:
- Worker Task_2 report 2026-07-28: initial continuity run 79/80, corrected run 80/80 with the focused regression executing 1/1.
