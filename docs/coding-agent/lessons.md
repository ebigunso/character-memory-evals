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
