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
