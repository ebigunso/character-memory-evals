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
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md`
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
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md`
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

## 2026-09-03 — Gate Commits On The Tool's Exit Status, Never On Grep Of Its Output  [tags: workflow, validation, git, orchestrator]

Context:
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md`
- Task/Wave: Copilot review round 2 on the waves 1–2 PR
- Roles involved: Orchestrator

Symptom:
- Two consecutive pushes to the PR carried a failing test: the first with a syntax error in a new diff test, the second with the same test still failing on its assertion.

Root cause:
- The one-line command chain gated the commit on `cargo test ... | grep -E "^test result|FAILED"` (grep succeeds when it matches FAILED) and joined a lint step to the commit with `;`, so neither a failing test nor a failing format run stopped the commit and push.

Fix applied:
- Repaired the test and re-pushed with the chain `cargo fmt --all && cargo test ... >/dev/null 2>&1 && cargo clippy ... >/dev/null 2>&1 && git commit && git push`, reading output in a separate step.

Prevention:
- A commit or push is gated only on the exit status of the validating command itself, joined with `&&`; output filtering for reading happens in a separate command. Never place `;` between a gate and a commit.

Evidence:
- Commits 1ec5991 (syntax error) and 683a6ca (failing assertion) on `chore/harness-right-sizing-w1`, corrected in 403d261.

## 2026-09-06 — Cross-Mode Evaluation Against A Pre-Merge Library Tip  [tags: evaluation, environment, windows, serde, isolation, planning]

Context:
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md` (Task_8 step 1, re-sequenced by decider ruling to run before the library's phase merge)
- Task/Wave: Task_8 step 1 / Wave 6
- Roles involved: Worker | Orchestrator | Reviewer

Symptom:
- Six deviations surfaced while adapting the harness to the pre-merge library and running the same datasets in service and embedded vector modes: nested relative embedded store paths failed at shard open on Windows; the sealed fixture generator's canonical-byte tests failed under workspace feature unification; the first continuity attempt rejected a provider value the README still documented; conventional configs silently shared one cwd-relative retrieval-statistics file across modes; debug-build and full-input probes projected many hours per run; service-mode write phases showed 30–70 second outliers with no failure telemetry to attribute them.

Root cause:
- Windows native stores accept different path forms: the embedded engine needs a canonical (verbatim) root while RocksDB rejects verbatim paths, so canonicalising every store root breaks the graph store.
- `qdrant-edge` enables `serde_json/preserve_order`, and Cargo feature unification turns it on for the whole workspace, so `serde_json::Value` maps stop sorting keys and any serializer that relied on implicit ordering produces different bytes.
- Stale README guidance plus a schema test that excluded historical continuity configs hid an invalid new config until runtime.
- Omitting `retrieval_stats_path` inherits the library's cwd-relative default, which persists across runs with deterministic object ids.
- Per-question graph ingestion dominates conventional runs; debug builds and full inputs are impractical for side-by-side batches.
- Saved logs and rows carry no retry counters, so a write-phase delay cannot be attributed to retries, ties, or embedding noise.

Fix applied:
- The settings bridge creates and canonicalises only the embedded vector directory (the library adapter now canonicalises its root itself); the continuity embedding serializer sorts keys explicitly with sealed bytes untouched; comparison configs follow the maintained smoke config and every new config is parsed by the schema test; every comparative run and mode gets fresh per-namespace statistics and store paths; release builds and decider-approved deterministic subsets (first 100 LongMemEval-S questions, first two LoCoMo conversations) with retained manifests; outliers reported as phase timings and ids without a cause claim.

Prevention:
- Canonicalise only the store roots whose engine needs it and exercise restart plus isolated cleanup on Windows; serialize sealed bytes through explicit canonical ordering and validate in the workspace feature context; parse every current config in tests and keep documentation aligned with admitted values; give every comparative run and mode fresh statistics and store paths; measure release-build item throughput and agree common deterministic subsets before long batches; report timing outliers with phase and id evidence only.

Evidence:
- Worker report `.agent-work/evals-worker/task8-step1-report.yaml` and `.agent-work/evals-worker/crossmode/REPORT.md` (2026-09-06); library fix commit af40e66 on the v0.1.6 stack.

## 2026-09-10 — Source-Check The Exact Live-Gate Variable Before A Forced-Live Run  [tags: review, validation, environment, evidence]

Context:
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md` (Task_8 step 1 follow-up review)
- Task/Wave: Task_8 step 1 / Wave 6
- Roles involved: Reviewer

Symptom:
- A reviewer's forced-live adapter run was executed with a similarly named but wrong environment variable; the live bodies still executed because the service was reachable, but the evidence could not prove forced mode and the run had to be repeated.

Root cause:
- The variable name was recalled from memory instead of read from the skip guard in source, and the run metadata recorded a generic "require live" flag rather than the exact environment map.

Fix applied:
- The run was repeated with the exact variable read from the skip guard, and the corrected command and environment map were saved beside the evidence.

Prevention:
- Before any forced-live run, read the exact skip-guard variable name from source and record the exact environment map with the evidence; a generic "require live" note is not evidence of forced mode.

Evidence:
- Reviewer follow-up report `.agent-work/evals-reviewer/task8-followup-review.md` (2026-09-10).

## 2026-09-10 — Re-run revision resolution after a companion pin changes

Symptom:
- After the library merge, re-running only failed jobs on CME #25 still validated the earlier companion revision.

Root cause:
- GitHub Actions reused the successful revision-resolution job's output when only failed jobs were re-run.

Prevention:
- After a companion pin changes on main, maintainers must re-run the entire workflow or push a new revision so revision resolution executes again. A failed-jobs-only rerun does not validate the new pin.

Evidence:
- CME #25 follow-up after the CharacterMemory v0.1.6 merge (2026-09-10).

## 2026-09-13 — Verify child-test execution after moving Rust modules [tags: validation, crate-layout]

Symptom:
- The first workspace run after merging the adapter crate reported success for an environment-isolation test whose child process selected zero tests.

Root cause:
- The exact child-test filter still used the old crate-root module path; Rust's test harness exits successfully when an exact filter matches nothing.

Fix applied:
- Updated the filter to `adapter::tests::oxigraph_env_cannot_redirect_graph_path_probe` and required the child output to report one passed test.

Prevention:
- When moving a Rust test module, update literal subprocess filters and assert that the intended child test executed, in addition to checking its exit status.

Evidence:
- Task_8 step 3 validation under `.agent-work/evals-worker/task8-step3/`; the execution-count assertion remains in `crates/cmem-eval/src/adapter.rs`.

## 2026-09-13 — Retain evaluation evidence, clean up evaluation stores [tags: lifecycle, cleanup, evaluation]

Symptom:
- Cross-mode and A/B runs left 222 `cmem_eval` service collections that had to be deleted by hand on 2026-09-13.

Root cause:
- Evaluation store lifetimes extended beyond the runs even though retrospective evidence was already recorded in result artifacts.

Decider ruling and prevention:
- Evaluation runs clean up every store they create when the run ends, including service collections, embedded store directories, graph files and retrieval-stat files, unless the configuration explicitly retains them for retrospective inspection.
- A run's evidence is its result rows, traces and report, never its stores.
- Cleanup enabled by default, an explicit retain switch and per-run directories are Task_8 step 4 implementation work. This step records the policy and leaves the current cleanup behavior unchanged.

Evidence:
- Decider ruling relayed by the orchestrator on 2026-09-13, citing the manual removal of 222 collections left by the cross-mode and A/B runs.

## 2026-09-13 — Audit every consumer when one outcome becomes a collection [tags: review, aggregation, validation]

Symptom:
- Vector-only retrieval retained one native outcome per selected kind, but restart snapshots and report aggregates read only the first outcome. Single-kind and hybrid checks did not reveal the omission.

Root cause:
- The native-type migration preserved singleton assumptions in downstream consumers; the review did not exercise two outcomes that both contributed trace data.

Fix applied:
- Summed all native outcomes, flattened their traces, and preserved query-level sample counts. Added an embedded episode-plus-observation restart/report regression with links that make both kinds emit fanout decisions; extended existing checks for selectivity and an absent first trace.

Prevention:
- When a result becomes a collection, census every first-element/index reader through the final report and test at least two contributing outcomes. Distinguish no trace from an empty trace, and retain the intended sample unit when summing counters. Derive test expectations from the producer's supported behavior: these vector-only kinds emit fanout, while native selectivity requires Entity roots.

Evidence:
- CME #27 Copilot round: `.agent-work/evals-worker/task8-step3-copilot/`; the embedded regression reproduced a count of 2 instead of 4 before the fix.

## 2026-09-13 — Preserve measurement units and support when replacing telemetry [tags: review, metrics, validation]

Symptom:
- Native missing-object decisions were counted as objects, including the stale-omission and lifecycle entries emitted for the same missing candidate. Continuity also emitted numeric graph-integrity values for raw retrieval baselines that the conventional pipeline correctly marked unsupported.

Root cause:
- Replacing mirrored counters with native traces changed the source of measurements without retaining the object-identity unit and retrieval-mode support boundary. Trace presence alone did not establish graph validation of the returned baseline items.

Fix applied and prevention:
- Deduplicate both returned and omitted missing-object counts by stable ID. Gate continuity integrity on the same retrieval mode as conventional rows. The leakage regression repeats decisions within and across outcomes; an embedded CLI test checks numeric hybrid integrity and null vector-only integrity while preserving item-derived metrics.
- When replacing telemetry projections, trace each count's identity unit and each metric's support condition through all row producers. Keep retry identity and canonical serialization order distinct from execution order in documentation.

Evidence:
- CME #27 Copilot round 2: `metrics::tests::context_validation_rate_accounts_for_lifecycle_leakage` and `commands::pipeline::tests::continuity_integrity_support_follows_retrieval_mode`. The pre-fix checks reproduced a validation rate of 0 instead of 0.5 and a vector-only graph-integrity value of 1.0 instead of null.

## 2026-09-13 — Census dataset metrics when enforcing retrieval support [tags: review, metrics, validation]

Symptom:
- The common integrity fields correctly became null for raw retrieval, but continuity still emitted numeric graph-derived correction, hub-expansion and rationale metrics.

Root cause:
- The previous support-boundary fix and its regression covered common row fields without auditing the dataset-specific metric producers. Native traces existed even when they did not describe the returned raw candidates.

Fix applied and prevention:
- Gate native outcome inputs once at the continuity metric entry point. Preserve label/item-derived metrics and retain native evidence in traces and report samples.
- Census every metric family through its actual input producer when changing retrieval support. Exercise representative scenarios for each producer and verify both unsupported nulls and supported numeric values; a generic scenario cannot establish coverage for conditional metric families.

Evidence:
- CME #27 Copilot round 2B: the extended mode regression reproduced `rationale_category_share_entity = 0.0` instead of null before the fix. Existing correction coverage now checks all 19 native-derived fields in hybrid, vector-only and BM25 modes; the embedded CLI regression covers recurring-hub and entrenched-correction scenarios.

## 2026-09-13 — The Orchestrator Decides Every Change On Product And Architectural Design  [tags: orchestrator, design, delegation, rulings]

Context:
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md` (Task_8 step 4) and the library's settings construction
- Task/Wave: Task_8 step 4 / Wave 6
- Roles involved: Orchestrator, Worker (evaluation and library)

Symptom:
- The evaluation worker reported its typed-construction item as blocked on a library API, and the right-sizing audit's wording assumed a "typed construction surface" should exist, so the Orchestrator dispatched a new public options constructor to the library. The decider had to ask for a design scrutiny; the scrutiny showed the library's intent is externalized configuration through the config crate, the real defect was three settings required in modes that never use them, and the library already held the right rule (the service connection string is optional and validated only in service mode). A second constructor for one consumer would have been sub-par design, and a design decision had been delegated to audit text and a worker's blocker.

Root cause:
- The Orchestrator treated inputs (audit wording, plan text, a worker's "blocked, need X") as decisions instead of deciding itself from the product and architectural design; the design check was skipped because the request looked like execution.

Fix applied:
- The typed-constructor dispatch was withdrawn before any edit; the library change was narrowed to "settings are required only by the selected mode" (graph path only in persistent mode; OpenAI key and model only for the OpenAI constructor; an injected provider supplies its own dimension), with no new construction path; the evaluation adapter drops its placeholders and keeps the config-crate path.

Prevention:
- Every ruling or brief that authorizes a change, of any size and in any layer, is decided on what is best for the overall product and architectural design: the Orchestrator states the design intent it serves (decision records, philosophy, README consumer path, phase documents), what the change would make worse, and the alternative it rejected. Audit text, plan text and worker findings are inputs, not decisions; a worker's "blocked, need X" is a symptom to diagnose, not a specification to forward. A change whose only justification is "the plan or audit says so" is not authorized until that check is written.

Evidence:
- Decider feedback 2026-09-13 in the orchestration session; the withdrawn brief `.agent-work/cm-worker/typed-settings-dispatch.txt` and its replacement `settings-follow-mode-dispatch.txt` in the library repository (transient).

## 2026-09-13 — Validate every stack level against the library revision CI resolves [tags: ci, dependencies, stacks]

Symptom:
- Every evaluation pull request in the stack failed to compile when library main advanced beyond the locally validated branch pin. Library #82 removed candidate embedding text after the harness had validated against the #81 tip.

Root cause:
- Local validation used a fixed library branch tip, while CI resolves the library at main. The local pin did not constrain the dependency revision those pull requests would meet.

Fix applied:
- Remove the obsolete embedding-text argument and its text-only helpers at the stack base, retain target and provenance, and replay the higher branches while preserving their changes. Validate every level against library main `7528daf`.

Prevention:
- Before merging a harness stack, re-pin its local library checkout to the library main revision CI will resolve and run the required gates at every stack level. Record that library revision with each result; an earlier branch-pin result does not establish compatibility with a newer main.

Evidence:
- Stack-wide CI failure after library #82; the five VectorIndexCandidate constructor calls and the old candidate-text assertion in the Task_8 step 3 typed batch builder.

## 2026-09-13 — Keep Windows engine paths short and dependency checkouts pinned [tags: windows, validation, persistence]

Symptom:
- RocksDB-backed validation failed in a deeply nested reviewer worktree, and concurrent library edits changed the dependency source seen by the evaluation checkout.

Fix applied and prevention:
- Use a short evaluation worktree such as `C:/w/cme`, with `C:/w/CharacterMemory` pointing to an isolated sibling checkout pinned at the reviewed library commit. Keep run output paths short too: the stores live beside their result artifacts under `OUT_DIR/stores`.
- Pass ordinary absolute filesystem paths to the engine; a verbatim Windows path does not remove the engine's path restrictions. Record both commits in validation evidence. Isolate the checkout and dependency before Cargo starts; do not redirect the dependency in the working manifest while another agent edits the sibling repository.

Evidence:
- Task_8 step 3 reviewer validation used the short-worktree and sibling-junction arrangement. Step 4 retains that arrangement for embedded and service validation.

## 2026-09-13 — Use the acquired run root to isolate service resources [tags: review, lifecycle, concurrency, ownership]

Symptom:
- Two runs with the same run ID and namespace but different fresh output roots raced through Qdrant collection admission; the rejected run's cleanup deleted the successful run's collection.

Root cause:
- The local namespace directory was treated as ownership of a service collection whose name omitted the acquired run root. A preceding existence check could not establish ownership across concurrent runs.

Fix applied:
- Keep atomic `create_dir` admission of `OUT_DIR/stores` as the ownership boundary. Include a short hash of its canonical absolute path in every service collection name and record the full hash in the run header. Different roots own different collections; a second admission to the same root fails before creating namespace resources. The library continues to own collection creation and schema details.

Prevention:
- When cleanup authority follows a local token, derive every remote resource identity from that token and test simultaneous acquisition as well as sequential reuse. The regression opens the same run ID and namespace concurrently in two roots and checks that each cleanup preserves the other collection; the runner regression proves exactly one same-root admission succeeds.
- No new rule candidate: this is the executable application of the existing owned-state lifecycle rule; keep the regression with the admission and cleanup code.

Evidence:
- Step 4 reviewer public-adapter race probe at parent `0581b79`; Orchestrator design ruling 2026-09-13; `service_mode_concurrent_runs_with_same_identity_preserve_each_other` and `run_root_admission_preserves_an_existing_directory`.

## 2026-09-13 — Check output disposition before storage admission and preserve cleanup scope [tags: review, lifecycle, cleanup, outputs]

Symptom:
- A caller could place a report or trace under the run's disposable `stores` directory, so successful cleanup deleted the requested artifact. With retention enabled, cleaning one namespace closed every open namespace.

Root cause:
- Output writers and store cleanup were reviewed separately. The retention branch reused a run-wide release operation despite the namespace-scoped API; earlier sibling coverage exercised deletion but not retained cleanup.

Fix applied:
- Admit every named output against the reserved store root before creating the output directory or stores, including normalized parent components and Windows case variations. Retained cleanup removes and closes only the named namespace and leaves durable files intact; run-wide release remains explicit.

Prevention:
- Follow each artifact through creation and cleanup when changing output placement. Exercise a surviving open sibling for both deletion and retention, including repeated cleanup of an already detached namespace. The existing reviewer lifecycle rule covers surviving siblings for destructive scope; the Worker proposes extending it explicitly to retained cleanup.

Evidence:
- Accepted Copilot findings on CME #28; `every_output_is_admitted_before_creating_the_run_directory`, `derived_outputs_cannot_enter_the_reserved_stores_root`, and `retained_cleanup_closes_only_the_named_namespace`.


## 2026-09-13 — Resolve path identity through the filesystem [tags: review, paths, cleanup]

Symptom:
- The reviewer used lowercase e-acute in the output parent and uppercase E-acute in the summary parent on Windows. The summary passed lexical admission, landed under the same stores directory, and was deleted on cleanup.

Root cause:
- ASCII case folding compared path spellings instead of the filesystem identities that the writers and cleanup used.

Fix applied:
- Acquire the reserved root atomically, create required output parents, canonicalize both sides through the filesystem, and compare path components. On rejection, remove only the root acquired by this invocation before any artifact write.

Prevention:
- Admission and cleanup decisions comparing paths must canonicalize both sides and compare components; never infer identity from spelling or case folding. The reviewer hotspot and Unicode-alias regression carry this rule.

Evidence:
- Step 4 P1B reviewer finding on 13932a6 and unicode_case_alias_cannot_place_output_in_stores, with plain nested-output and existing-root ownership regressions.

## 2026-09-13 — Inspect output leaves without following links [tags: review, paths, cleanup]

Symptom:
- A dangling summary-output symlink into the future stores root passed admission, then the writer followed it and cleanup deleted the artifact.

Root cause:
- A canonicalize NotFound result was treated as proof of a nonexistent output leaf. It also describes a dangling symlink, whose later write can target disposable state.

Fix applied:
- Inspect every named output leaf with symlink_metadata. Reject all links and non-file leaves by output name, preserve regular-file overwrite, and compare only canonical existing parents with the acquired root. Remove the leaf canonicalization fallback.

Prevention:
- Test absent leaves, regular files, live links and dangling links whenever write admission depends on the destination. A missing target does not mean the link itself is absent. The existing path-identity rule remains applicable; this lesson adds the leaf-state regression to its execution.
- On Windows, the file-link regression requires Developer Mode or the symlink privilege; keep the test unconditional and report a missing privilege as an environment failure.

Evidence:
- Step 4 P1C reviewer finding on 9e9c0ae; output_leaf_links_are_rejected_before_writing_artifacts and existing_regular_output_files_remain_writable.

## 2026-09-13 — Follow artifact serde semantics through nested types [tags: review, serde, artifacts]

Symptom:
- Additive fields under trace expected labels and header dimension policy still failed after the artifact readers adopted ordinary serde.

Root cause:
- The outer artifact records reused input DTOs, so their nested deny_unknown_fields attributes silently preserved fixture admission rules.

Fix applied:
- Record expected labels in a plain artifact-side DTO converted from the fixture. Remove only the controllable policy serde denial; fixture validation and runtime padding/admission are unchanged.

Prevention:
- Walk every nested type reachable from rows, traces, headers and reports when changing artifact reader semantics. Preserve input admission separately and exercise additive fields below the artifact root.

Evidence:
- Accepted Copilot findings on CME #30; trace_reader_accepts_additive_expected_fields and header_accepts_additive_controllable_policy_fields; Worker artifact-type census.

## 2026-09-13 — Store retention intent once in derived artifacts [tags: review, artifacts, invariants]

Symptom:
- A persisted run header could pair a retention flag with a contradictory optional reason after artifact readers adopted ordinary serde. Independently named output files also allowed duplicate destinations.

Fix applied:
- Persist retention as one optional reason; its presence means retention. Configuration still validates its existing flag/reason pair. Derive header.json and report.json beside the sole .jsonl output, so the three names cannot collide; existing-leaf links remain rejected.

Prevention:
- When simplifying readers, remove redundant artifact fields that encode the same choice. Exercise both retained and non-retained headers and output-name collisions at the producer; do not rebuild a validator for contradictions the artifact need not represent.

Evidence:
- Accepted Copilot findings on CME #28 and the Task_5 design ruling; cli_rejects_non_jsonl_output_before_creating_directories and continuity_run_cleans_or_retains_stores_on_success_and_admission_failure.

## 2026-09-13 — Require fresh output names instead of permitting overwrite [tags: review, filesystem, artifacts]

Symptom:
- Two existing hard links named traces.jsonl and header.json passed output admission; writing the header then destroyed the trace while the run reported success.

Root cause:
- Admission rejected symbolic links but permitted regular-file overwrite. Hard links are regular files, so distinct names did not establish distinct writable objects.

Fix applied:
- Reject every existing output leaf by name before artifact writes, including regular files, links and directories. A caller chooses a fresh output directory or deliberately removes prior outputs.

Prevention:
- Treat fresh output names as the run contract; do not reintroduce overwrite admission. Keep the hard-link preservation regression and exercise each artifact name independently. The durable review hotspot is output ownership, including hard links as well as symbolic links.

Evidence:
- Reviewer P1 at 77cab7b and the Task_5 design ruling; hard_linked_outputs_fail_before_writing_artifacts and existing_output_files_and_directories_fail_admission.

## 2026-09-13 — Enforce no-overwrite at file creation [tags: review, filesystem, artifacts]

Symptom:
- An output appearing after admission could still be truncated by an artifact writer.

Root cause:
- The absence check established a point-in-time observation, while File::create and fs::write still allowed replacement.

Fix applied:
- Every artifact writer uses File::create_new; writer regressions assert that existing bytes survive a failed write. Seal destination writers follow the same rule.

Prevention:
- Enforce ownership invariants in the filesystem operation itself as well as early admission; verify every concrete writer, including dataset-specific and seal writers.

Evidence:
- Accepted Copilot finding on CME #31; native-outcome, summary, report and merged-trace round trips plus the header retention regression.

## 2026-09-13 — Update CI consumers when removing CLI flags [tags: review, cli, ci]

Symptom:
- The embedded smoke job still supplied three removed output flags and failed in clap before comparing results.

Root cause:
- The CLI and README changed without auditing the invocation inside the hidden .github directory.

Fix applied:
- The maintained CI smoke uses only --out and compares the merged JSONL artifacts.

Prevention:
- Search tracked workflow invocations alongside source and README when removing CLI arguments, then execute the changed workflow command lines locally.

Evidence:
- Accepted Copilot finding on CME #31; embedded-smoke job command replay and removed-flag census.

## 2026-09-13 — Stream artifacts when sealing and verifying runs [tags: review, performance, artifacts]

Symptom:
- Sealing retained every artifact as a byte vector, and verification read each whole file, so memory use grew with run output size.

Root cause:
- Small fixtures demonstrated byte preservation but did not exercise the resource behavior needed for large completed runs.

Fix applied:
- Stream regular files through a fixed buffer into create-new destinations and hash the copied bytes; verification streams hashes too. Retain only the small original header bytes for the manifest object and exact header copy.

Prevention:
- For artifact operations, distinguish small parsed metadata from unbounded run data and keep the latter on a streaming path. Exercise files larger than the buffer, compare hashes against an independent whole-byte digest, and detect corruption beyond the first buffer.

Evidence:
- Accepted Copilot finding on CME #32; seals_and_verifies_artifacts_larger_than_the_stream_buffer and the existing seal/verify preservation tests.

## 2026-09-14 — A Merge Authorization Covers Only The Pull Requests It Was Given For  [tags: orchestrator, git, authorization]

Context:
- Plan: `docs/coding-agent/plans/completed/harness-right-sizing-plan.md` (closed); follow-up pull requests after the stack merge
- Task/Wave: post-closeout follow-ups
- Roles involved: Orchestrator

Symptom:
- The decider authorized merging the pull requests then open once their titles carried a gitmoji and CI was green; the Orchestrator then merged three later follow-up pull requests under the "same rule" and was about to merge a fourth.

Root cause:
- A conditional authorization was read as a standing policy instead of a decision scoped to the work immediately at hand.

Fix applied:
- The rule is recorded in the orchestrator rules of both repositories; the loader cleanup pull request waits for explicit approval.

Prevention:
- Record the scope of every merge authorization (which pull requests) when it is given; when a later pull request becomes mergeable, report it and ask, never merge on a prior condition.

Evidence:
- Decider feedback 2026-09-14 in the orchestration session.

## 2026-09-14 - Preserve identity source until duplicate admission [tags: review, dataset, identity]

Symptom:
- LongMemEval admitted repeated record IDs and mixed record/parallel collisions with identical turns, although the exception applies only to identities obtained from the parallel array.

Root cause:
- The duplicate map retained the effective ID and raw turns but discarded the ID source before applying the source-dependent exception.

Fix applied:
- Retain whether each effective ID came from a record; reject record/record and mixed collisions, and admit parallel-only repeats only when their raw turn arrays match.

Prevention:
- Preserve identity provenance until admission is complete and test record, parallel and both mixed orders, including a present but unused parallel array.

Evidence:
- [Dataset admission plan Decision Log](plans/completed/dataset-admission-plan.md#decision-log-append-only-re-plans-and-major-discoveries), duplicate-session ruling; [review finding F1](plans/completed/dataset-admission-plan.md#review-findings-cme-37-independent-evaluation-reviewer-2026-09-14) at 710b3e7, resolved at 9c2f2b3.

## 2026-09-14 - Audit every caller before narrowing a parser helper [tags: review, dataset, annotations]

Symptom:
- LoCoMo array records named session_01 or session_+1 lost their summaries and observations even though those record IDs remained admitted.

Root cause:
- Canonical decimal validation was added to a shared helper used by both keyed-session admission and numeric annotation lookup; the official file did not exercise the affected annotation encodings.

Fix applied:
- Separate canonical key validation from the existing numeric annotation lookup and compare complete parsed records for session_1, session_01 and session_+1 against the baseline.

Prevention:
- Audit every caller before tightening a parser helper and compare the typed annotations affected by the change using admitted inputs outside the official file.

Evidence:
- [Dataset admission plan Decision Log](plans/completed/dataset-admission-plan.md#decision-log-append-only-re-plans-and-major-discoveries), LoCoMo annotation-map census; [review finding F2](plans/completed/dataset-admission-plan.md#review-findings-cme-37-independent-evaluation-reviewer-2026-09-14) at 710b3e7, resolved at 9c2f2b3 with three byte-identical annotation comparisons.

## 2026-09-14 - Assert each alias in its typed destination [tags: review, dataset, validation]

Symptom:
- The LoCoMo session alias loop admitted session_id, session and id but never asserted the resulting session_id, repeating the earlier alias-evidence gap.

Root cause:
- Successful loading, a case-to-test table and assertions on neighbouring fields were treated as proof that the alias reached its destination.

Fix applied:
- Assert the exact session_id for every alias in the loop and add the alias_destination_assertion reviewer hotspot.

Prevention:
- For every claimed preserved alias, supply a populated value and verify that value in the alias's own typed destination field.

Evidence:
- [Dataset admission plan Decision Log](plans/completed/dataset-admission-plan.md#decision-log-append-only-re-plans-and-major-discoveries), admitted-alias census rows; [review finding F3](plans/completed/dataset-admission-plan.md#review-findings-cme-37-independent-evaluation-reviewer-2026-09-14) at 710b3e7, resolved at 9c2f2b3.

## 2026-09-14 - Census every loader field and identity scope [tags: orchestrator, planning, dataset]

Symptom:
- The Task_1 census missed session-ID repeats, admitted alias rows and turn-ID constraints; later Worker or Copilot findings required census and plan amendments.

Root cause:
- The census followed fields present in the official files and checked item-ID uniqueness without enumerating every loader read or every identity scope.

Fix applied:
- Amend the census and admission rules with the repeated-session, alias and turn-ID findings; Task_2 verifies typed admission cases, alias destinations and complete official parsed-byte preservation.

Prevention:
- Build the field census from loader source, enumerate every field and alias it reads, and check identity uniqueness at item, session, turn and QA scopes before dispatching strict admission work.

Evidence:
- [Dataset admission plan Decision Log](plans/completed/dataset-admission-plan.md#decision-log-append-only-re-plans-and-major-discoveries), Task_1 census and the correction recording 13 repeated LongMemEval session pairs; the plan Progress Log records subsequent alias and turn-ID additions. The plan's [review findings](plans/completed/dataset-admission-plan.md#review-findings-cme-37-independent-evaluation-reviewer-2026-09-14) record the resulting admission and preservation checks.

## 2026-09-14 - Refresh a stack root to leaf before a branch moves a file the root changed [tags: orchestrator, git, stacking]

Symptom:
- GitHub reported the closeout pull request as conflicting although its base branch was an ancestor: the branch moved the plan file from active to completed, and the stack root had changed that file after the branch was cut.

Root cause:
- The orchestrator told the worker that docs-only commits on the base never require a rebase. That holds only for branches that do not touch the files the base changed; a move or delete of such a file becomes a modify/delete conflict at the stack merge.

Fix applied:
- The implementation branch was rebased onto the plan tip and the closeout branch onto it, with the completed plan resolved to the closeout version; both were force-pushed with leases and the reviewer checkouts re-pinned.

Prevention:
- Before opening a stacked pull request that moves, deletes or edits a file the stack root changed since the branch was cut, refresh the stack from root to leaf: rebase each layer onto its updated immediate parent (here Task_2 onto the plan tip, then the closeout onto the refreshed Task_2), never a leaf directly onto the root, which would flatten the stack or replay the intermediate layer as different commits; then check with a local merge-tree against the root, not only against the immediate base.

Evidence:
- CME #39 mergeability at f181f87 (conflicting) versus 14481b6 (clean); [Dataset admission plan](plans/completed/dataset-admission-plan.md) closeout.

## 2026-09-14 - Triage plan-review findings by what a plan is for [tags: orchestrator, planning, review]

Symptom:
- The benchmark-derived-content plan went through 29 revision commits over two reviewers while still awaiting the decider's approval, growing from about 1,800 to 7,500 words with one Definition of Done bullet of 5,700 characters; each round moved the plan further into implementation design (error enum placement, sidecar file conventions, counter plumbing, fixture contents, wording nits).

Root cause:
- Every valid-looking finding was treated as a plan defect. The question that should have been asked first, whether the finding changes what a worker is authorized to do or how done is judged, was never asked; findings that only chose an implementation shape were written into the plan instead of the worker brief. Copilot reviews a plan as if it were code, and the plan reviewer confirmed each round rather than ranking it, so nothing in the loop pushed back.

Fix applied:
- The plan was restructured on the decider's direction: the Definition of Done is contract level, the validated implementation shapes live in per-task design notes marked as reviewed shape rather than contract, and the ruling is in the plan's Decision Log.

Prevention:
- Before opening a plan for review, and before acting on each finding, apply the filter: a plan fixes scope, contracts, ownership, validation feasibility and measurement validity, and a finding earns a plan revision only if it changes one of those. A finding that chooses an implementation shape is answered on the thread and routed to the worker brief or the implementation review. Keep the Definition of Done readable at contract level; if a bullet needs a paragraph, the detail belongs in design notes or with the worker. After two consecutive rounds of implementation-level findings, stop the loop and present the plan for approval rather than absorbing more.

Evidence:
- [Benchmark-derived content plan](plans/active/benchmark-derived-content-plan.md) Progress Log (the review rounds) and Decision Log (the plan-time versus implementation-time ruling); CME pull request #40.
