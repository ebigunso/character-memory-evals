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
