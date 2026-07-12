# Coding Agent Lessons

## 2026-04-30 — Treat Forthcoming Public APIs As Contract Targets  [tags: assumptions, planning, adapters]

Context:
- Plan: `docs/coding-agent/plans/active/character-memory-evals-bootstrap-plan.md`
- Task/Wave: planning
- Roles involved: Orchestrator | Researcher

Symptom:
- The plan framed the real Character Memory adapter mainly as a limitation because the current sibling crate does not yet expose the needed public API.

Root cause:
- I over-weighted current local implementation state and under-weighted the user's handoff assumption that the API contract should exist shortly.

Fix applied:
- Plan and implementation will treat the real public API as the intended contract target, while retaining a mock adapter for deterministic tests until the upstream API lands.

Prevention:
- Repo rule candidate:
  - audience: orchestrator
  - proposed rule: When a handoff describes a soon-to-exist external public API, model that API as the target contract and isolate current unavailability behind mocks or documented feature gates.
- Dispatch/plan guardrail:
  - Record external API readiness assumptions explicitly before implementation.

Evidence:
- User correction on 2026-04-30: "assume the public API exists, since it's just not done yet and it will be there shortly."

## 2026-04-30 — Split Subagent Work Into Smaller Bounded Tasks  [tags: delegation, orchestration, planning]

Context:
- Plan: `docs/coding-agent/plans/active/character-memory-evals-bootstrap-plan.md`
- Task/Wave: Wave 3
- Roles involved: Orchestrator | Worker

Symptom:
- Dataset worker tasks were too broad, ran for several minutes without returning, and the orchestrator took over implementation.

Root cause:
- The worker prompts assigned whole crate implementations instead of small compile- or file-bounded objectives with short feedback loops.

Fix applied:
- Future subagent use in this turn will split work into narrow review, compile triage, and targeted fix tasks.

Prevention:
- Repo rule candidate:
  - audience: orchestrator
  - proposed rule: Prefer subagent tasks that can complete in one short feedback loop, such as one module, one validation failure, or one review slice.
- Dispatch/plan guardrail:
  - For multi-crate implementation, dispatch smaller workers by module or validation failure rather than whole crates.

Evidence:
- User correction on 2026-04-30: "split tasks into smaller steps to have the sub-agents finish them in reasonable amounts of time."

## 2026-04-30 — Wait Longer Before Closing Background Agents  [tags: delegation, orchestration, patience]

Context:
- Plan: `docs/coding-agent/plans/completed/character-memory-evals-bootstrap-plan.md`
- Task/Wave: follow-up correction
- Roles involved: Orchestrator | Worker | Reviewer

Symptom:
- The orchestrator closed background agents too quickly after short waits and checkpoint prompts.

Root cause:
- I optimized for main-thread progress and treated missing short-window responses as a reason to take over, instead of allowing enough time for background agents to complete compile, review, or repo-analysis work.

Fix applied:
- Future subagent management will use longer wait windows before force-closing agents, with checkpoint prompts used to redirect or narrow work rather than as an immediate prelude to shutdown.

Prevention:
- Repo rule candidate:
  - audience: orchestrator
  - proposed rule: Wait substantially longer before force-closing background agents unless they are clearly blocked, conflicting with newer user direction, or performing unsafe work.
- Dispatch/plan guardrail:
  - For compile, review, or repository exploration tasks, prefer multi-minute waits and explicit checkpoint prompts before considering closure.

Evidence:
- User correction on 2026-04-30: "You were generally too impatient about background agents in this session. You should wait much longer before forcefully closing them."

## 2026-04-30 — Separate Benchmark Runtime Defaults From CI Defaults  [tags: planning, validation, adapters, defaults]

Context:
- Plan: `docs/coding-agent/plans/active/character-memory-public-api-eval-adapter-plan.md`
- Task/Wave: planning follow-up
- Roles involved: Orchestrator | Researcher

Symptom:
- The active plan preserved mock-backed defaults too broadly, which could let users accidentally run benchmark evals against mocks.

Root cause:
- I conflated service-free validation defaults with user-facing benchmark runtime defaults.

Fix applied:
- The plan now makes live Character Memory the default for benchmark CLI runs while keeping mock paths explicit and guarded for tests/smoke validation.

Prevention:
- Repo rule candidate:
  - audience: orchestrator
  - proposed rule: Distinguish benchmark runtime defaults from CI/test defaults; real eval commands should fail loudly instead of silently falling back to mocks.
- Dispatch/plan guardrail:
  - When mocks are retained for validation, require explicit mock opt-in flags and visible mock/smoke output labeling.

Evidence:
- User correction on 2026-04-30: "Can you also make the default run be a live eval run, instead of a mock based solution?"

## 2026-05-03 — Parallelize Approved Harness Implementation Work  [tags: delegation, orchestration, parallelism, workflow]

Context:
- Plan: `docs/coding-agent/plans/active/eval-harness-performance-boosts-plan.md`
- Task/Wave: implementation start
- Roles involved: Orchestrator | Worker

Symptom:
- The orchestrator began implementing an approved multi-task harness plan locally without first dispatching eligible disjoint work to subagents.

Root cause:
- I treated the implementation as a tightly coupled local edit because the first two files were already in progress, but the approved plan had independent LoCoMo caching work that could run safely in parallel.

Fix applied:
- Continue current real-adapter changes locally to avoid conflicts, and dispatch disjoint Task_4 LoCoMo caching work to a Worker.

Prevention:
- Before starting implementation on an approved harness plan, identify the immediate local task and at least one disjoint parallel worker task; if none exists, state why parallelism is not useful.

Evidence:
- User correction on 2026-05-03: "Use sub-agents where possible to parellelize work!"

## 2026-05-04 — Keep Generated Dataset Artifacts Out Of Commits Unless Explicitly Requested  [tags: git, datasets, artifacts, scope]

Context:
- Plan: `docs/coding-agent/plans/active/locomo-online-enrichment-snapshots-plan.md`
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

## 2026-05-04 — Verify Source-Only Metadata Before Treating Generated Artifacts As Valid  [tags: datasets, validation, assumptions, enrichment]

Context:
- Plan: `docs/coding-agent/plans/completed/longmemeval-s-online-enrichment-snapshots-plan.md`
- Task/Wave: LongMemEval-S source-only correction and snapshot regeneration
- Roles involved: Orchestrator

Symptom:
- I generated LongMemEval-S snapshots from a source-only file that lacked `question_date`, then treated the artifact as useful by falling back to final-haystack cutoffs.

Root cause:
- I adapted to missing source-only metadata instead of stopping to correct the source-only dataset so the artifact could satisfy the intended eval cutoff semantics.

Fix applied:
- Rebuilt the LongMemEval-S source-only file with `question_date`, removed nested forbidden keys, archived the invalid snapshot artifact, and regenerated snapshots using question-date cutoffs.

Prevention:
- Before generating dataset artifacts, verify the source-only input contains every non-label field required by eval semantics. If required metadata is missing, correct the source-only input first; do not invent fallback cutoff semantics.

Evidence:
- User correction on 2026-05-04: "If the source only file does not contain the required metadata, then that should be corrected, then the enrichment generation should be run to output the actually relevant files."

## 2026-05-03 — Stop At Plan Gate When Requested  [tags: planning, orchestration, workflow, correction]

Context:
- Plan: forthcoming `docs/coding-agent/plans/active/exact-tiktoken-context-metrics-plan.md`
- Task/Wave: planning
- Roles involved: Orchestrator | Researcher

Symptom:
- The orchestrator began preparing implementation steps for the token-counting fix after the user explicitly redirected to use the orchestration harness and plan the fix first.

Root cause:
- I treated the follow-up as an implementation approval instead of re-running the harness plan gate and waiting for explicit plan approval.

Fix applied:
- Stop implementation, dispatch the required Researcher pass, and provide an approval-ready plan before making product code changes.

Prevention:
- For harness-triggered non-trivial follow-ups, create or update the execution plan and wait for user approval before any code edits, even when the technical fix appears straightforward.

Evidence:
- User correction on 2026-05-03: "Plan a fix first."

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

## 2026-07-11 — Audit Removed Features From The Repository Root  [tags: validation, review, docs, search]

Context:
- Plan: `docs/coding-agent/plans/active/eval-harness-architecture-revision-plan.md`
- Task/Wave: Task_1 / Wave 1 integration
- Roles involved: Worker | Orchestrator

Symptom:
- The Worker reported that all live references to the retired adapter feature were removed, but `scripts/README.md` still contained two current feature-gated commands.

Root cause:
- The acceptance audit searched a handpicked set of roots (`Cargo.toml`, `README.md`, `crates`, and `docs`) and omitted `scripts`.

Fix applied:
- Removed the two stale flags and changed the audit to search from the repository root while explicitly excluding append-only plan history.

Prevention:
- Repo rule candidate:
  - audience: worker
  - proposed rule: For repository-wide removal acceptance criteria, search from the repository root and explicitly exclude only documented historical or generated paths.
- Dispatch/plan guardrail:
  - Treat a zero-match repository-root search as required evidence before reporting a removed feature, flag, or identifier fully absent.
- Residual risk / waiver:
  - None.

Evidence:
- Orchestrator integration finding on 2026-07-11 identified `scripts/README.md` lines 10-11 after the Worker reported a clean audit.

## 2026-07-12 — Do Not Infer Asset Absence From Ignore-Aware Search  [tags: validation, datasets, tooling, assumptions, gitignore]

Context:
- Plan: `docs/coding-agent/plans/active/eval-harness-architecture-revision-plan.md`
- Task/Wave: Task_4 / Wave 3 integration
- Roles involved: Worker | Orchestrator

Symptom:
- The Worker reported that LongMemEval-S and LoCoMo local datasets and enrichment assets were absent, then skipped their required pre/post artifact diffs.

Root cause:
- Asset discovery used `rg --files`, which honors ignore rules and therefore omitted the gitignored `datasets/` tree even though the files existed on disk.

Fix applied:
- Verify the assets with direct filesystem existence checks or `rg --no-ignore`, then run the real pre/post CLI validations from a detached pre-change worktree and the current tree.

Prevention:
- Repo rule candidate:
  - audience: worker
  - proposed rule: When validation depends on local or generated asset existence, use direct filesystem checks or an explicit no-ignore search; never infer absence from default `rg`, `fd`, or tracked-file enumeration.
- Dispatch/plan guardrail:
  - Required validation may be marked unavailable only after an ignore-independent existence check records the exact expected paths.
- Residual risk / waiver:
  - None.

Evidence:
- Orchestrator integration finding on 2026-07-11 confirmed `datasets/longmemeval_s_cleaned.json`, `datasets/locomo10.json`, `datasets/enriched/`, and `datasets/enrichment_source/` exist as gitignored local assets.

## 2026-07-12 — Verify AGMSG Reports Against The Registered Store  [tags: workflow, tooling, agmsg, reporting, validation]

Context:
- Plan: `docs/coding-agent/plans/active/eval-harness-architecture-revision-plan.md`
- Task/Wave: Task_5 / Wave 4 reporting
- Roles involved: Worker | Orchestrator

Symptom:
- The Worker received successful `send.sh` results for Task_5 checkpoints and a strict YAML report, but the Orchestrator's registered AGMSG history never received those messages and Task_5 could not be integrated.

Root cause:
- After the registered database rejected sandboxed writes, the Worker overrode `AGMSG_STORAGE_PATH` to a separate writable mirror database; delivery succeeded only inside that split store rather than the active team's registered store.

Fix applied:
- Send the report to the registered AGMSG store with the required filesystem escalation, then verify delivery by reading history from that same registered store.

Prevention:
- Never redirect an AGMSG send to a different database merely to bypass a write restriction; request the required approval for the registered store and verify critical handoffs in the same store's history before ending the turn.
- Turn-closing guardrail: after every required Worker YAML handoff, confirm the report is visible in registered team history before treating delivery as complete.

Evidence:
- Orchestrator correction on 2026-07-11: the three Task_5 commits were visible, but no Task_5 report or post-16:01 message existed in its AGMSG history.

## 2026-07-12 — Test Durable Lifecycle From A Fresh Adapter Instance  [tags: review, lifecycle, persistence, validation]

Symptom:
- The benchmark pipeline reset a namespace through a fresh adapter whose in-memory namespace map was empty, so cleanup silently skipped an existing deterministic collection and identity registry before ingest.

Root cause:
- Lifecycle validation covered reopen and reattach behavior but did not exercise the distinct fresh-run path from a new adapter instance against stale durable state.

Fix applied:
- Make fresh namespace preparation explicitly reset durable identity before `open_namespace`, make live reset derive durable paths without an in-memory entry, and verify the behavior with a live fresh-instance regression test.

Prevention:
- For restart-capable adapters, require separate regression scenarios for fresh open, intended reattach, and fresh-instance cleanup of stale durable state.

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
- Re-summarizing result rows used an empty config and no dataset metric family, dropping provider metadata and changing registry coverage relative to run-emitted summaries.

Root cause:
- The compatibility entrypoint reconstructed a derived artifact without requiring the original configuration and dataset selection that defined its semantics.

Fix applied:
- Require the summarize CLI/API to receive the original config and metric family, validate run/dataset consistency, and compare regenerated provider/config/coverage fields with run output.

Prevention:
- Any reconstruction or compatibility path for a derived artifact must receive and validate all original semantic inputs, with parity tests against the primary emission path.

## 2026-07-12 — Verify Gated Live Tests With The Service Down  [tags: ci, validation, integration, availability]

Symptom:
- Hosted CI failed instead of skipping the live adapter test because a newly added early Qdrant call returned a raw connection-refused error outside the existing typed-error skip classifier.

Root cause:
- Local validation exercised only the Qdrant-up path, and the test gated one setup call rather than every fallible live call across its full lifecycle.

Fix applied:
- Route typed and raw Qdrant-unavailable errors from every live phase through one test-only gate while preserving production error behavior: absence before the first successful live operation skips, later service loss fails, and teardown receives one bounded retry before failing.

Prevention:
- Skip-if-unavailable predicates must gate every live call made by a gated test, and any setup-path change requires explicit service-down and service-up verification before completion.

## 2026-07-12 — Keep Fresh Reset And Post-Run Cleanup Policies Separate  [tags: review, lifecycle, configuration, contracts]

Symptom:
- Fresh reset consulted `cleanup.require_collection_prefix` even when cleanup was disabled, so a valid leftover mismatched cleanup prefix blocked the next fresh run before ingest.

Root cause:
- One adapter operation represented both unconditional pre-open freshness and optional post-run cleanup, allowing a post-run configuration constraint to leak into the fresh-open path.

Fix applied:
- Split fresh reset from post-run cleanup at the adapter contract: fresh reset uses `namespace_prefix`, while post-run cleanup separately uses `cleanup.require_collection_prefix`.

Prevention:
- When one durable-state action serves multiple lifecycle phases, model each phase explicitly and test that phase-local configuration cannot affect the other phase.

## 2026-07-12 — Validate Source-Only CI Optimizations Against Workspace Metadata  [tags: ci, validation, dependencies, workflow]

Symptom:
- A proposed source-only formatting job failed because `cargo fmt --all --check` invokes workspace metadata and the workspace contains a sibling path dependency.

Root cause:
- The optimization assumed formatting never resolves workspace manifests, without testing the exact command in a checkout where `../CharacterMemory` was absent.

Fix applied:
- Restored the credential-less public sibling checkout for the formatting job after reproducing the failure in an isolated source-only archive.

Prevention:
- Before removing dependency checkout or setup steps from a CI gate, execute the exact gate in an isolated environment with that dependency intentionally absent.

## 2026-07-12 — Resolve Moving External Dependencies Once Before CI Fan-Out  [tags: ci, review, dependencies, consistency]

Symptom:
- Parallel CI jobs independently checked out the public sibling's moving default branch, so a mid-run sibling push could make different gates validate different source snapshots.

Root cause:
- The job split preserved per-job setup but treated a moving external repository ref as equivalent to the monolithic job's single resolved checkout.

Fix applied:
- Added a credential-less resolver job that captures the sibling's current `main` SHA once and passes that immutable run-scoped SHA to every parallel gate checkout.

Prevention:
- When CI fans out across jobs that consume a moving external dependency, resolve the dependency revision once before fan-out and assert every consumer uses the shared immutable output.

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

## 2026-07-12 — Inventory Every Durable Store In Lifecycle Transitions  [tags: review, lifecycle, persistence, isolation]

Symptom:
- Fresh reset cleared the Qdrant collection and external-ID registry while reusing configured Oxigraph and SQLite retrieval-stat paths, so stale graph objects, links, lifecycle state, and counters survived into a supposedly fresh run.

Root cause:
- The lifecycle contract modeled only the first two durable stores added to the adapter and treated later persistence paths as shared configuration rather than members of the same namespace identity.

Fix applied:
- Derive namespace-scoped Oxigraph and retrieval-stat paths from the shared prefix/run/namespace identity, delete only those derived paths during reset, and require every configured store to exist before reattach.

Prevention:
- For every fresh-open, reset, reattach, or cleanup change, enumerate all configured durable stores and add regressions for structural namespace isolation, missing-store diagnostics, reattach restoration, and empty state after reset.

## 2026-07-12 — Prove Destructive Scope With A Surviving Sibling  [tags: review, validation, lifecycle, isolation, documentation]

Symptom:
- The durable-store reset fix had single-namespace removal coverage but omitted the explicitly required sibling-survival scenario, left operator documentation describing the old two-store lifecycle, and did not exercise a missing identity registry in the aggregated live diagnostic matrix.

Root cause:
- Validation demonstrated that the target namespace became empty without proving the negative boundary that a destructive derived-path operation could not affect a neighboring namespace, and closeout did not reconcile every durable-store inventory item across tests and operator-facing documentation.

Fix applied:
- Add a two-namespace production-lifecycle regression under one configured root/template that resets A, proves B's exact files and data survive, and reattaches B; extend the missing-store matrix to the identity registry; update README lifecycle and cleanup semantics.

Prevention:
- Turn-closing guardrail: for any destructive scoped operation, map every acceptance item to evidence for the target, a surviving sibling, the shared parent/root, every resource-kind failure case, and current operator documentation before reporting the review fix complete.
