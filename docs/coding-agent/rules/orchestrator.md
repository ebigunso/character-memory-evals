# Orchestrator Rules

## Repo-Specific Orchestrator Policies

- When the handoff describes a forthcoming external public API, treat that API as the target contract and isolate current unavailability behind mocks or documented feature gates.
- Split subagent work into short feedback-loop tasks, such as one module, one validation failure, or one review slice.
- Wait substantially longer before force-closing background agents unless they are clearly blocked, conflicting with newer user direction, or performing unsafe work.

## Delegation Routing (model-strength aware; user-approved 2026-07-11)

- Route by failure mode: if a miss would be a subtle bug or overlooked line, delegate to a Codex agent (detail scrutiny); if a miss would be building the wrong thing well, delegate to a Claude agent (altitude and lateral judgment).
- Research: exploratory research (design-space surveys, alternatives with tradeoffs, cross-repo implications) goes to Claude researcher subagents; forensic research (exhaustive inventories with file:line evidence, call-site censuses) goes to Codex agents via agmsg.
- Review tiers: Tier D defect/compliance review (post-implementation diff correctness, audits, serde/schema verification, determinism sweeps, acceptance-evidence checking) goes to the Codex `evals-reviewer` agent via agmsg — never to the Codex identity that authored the diff (`evals-worker` authored the architecture revision). Tier A altitude review (design/plan soundness, goal-achievement review) goes to Claude reviewer subagents. Routine impl diffs get Tier D only; design docs get Tier A only; milestone gates get both tiers in parallel.
- Workers stay Codex; give creative-design subtasks (scenario libraries, metric semantics, similarity-control schemes) a Claude design pass first and hand the Codex worker a spec.

## Repo-Specific Integration / Git Policy

- Keep git branch creation and commits in the orchestration thread unless the user explicitly delegates shared-state Git mutation.

## Global Migration Candidates (Placeholder)

- None.
