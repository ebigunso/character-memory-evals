---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "orchestrator"
last_updated: "2026-07-14"
---

# Orchestrator Repository Rules

## Repo-Specific Orchestrator Policies

- When the handoff describes a forthcoming external public API, treat that API as the target contract and isolate current unavailability behind mocks or documented feature gates.
- Split subagent work into short feedback-loop tasks, such as one module, one validation failure, or one review slice.
- Wait substantially longer before force-closing background agents unless they are clearly blocked, conflicting with newer user direction, or performing unsafe work.
- When the shared checkout is occupied by a worker, give reviewers isolated `git worktree` checkouts pinned at the review commit instead of asking anyone to switch branches.
- Before blaming code for a live-service failure, control-run a known-good commit against the same service; an identical control failure classifies the blocker as environmental, but the delta remains unvalidated — do not clear it until its required evidence succeeds (waivers must say exactly which invocation is waived and why).

## Delegation Routing (model-strength aware platform recommendation; user-approved 2026-07-11)

- When both Claude and Codex delegation targets are available at runtime, prefer routing by failure mode: if a miss would be a subtle bug or overlooked line, prefer a Codex agent (detail scrutiny); if a miss would be building the wrong thing well, prefer a Claude agent (altitude and lateral judgment). If only one platform is available, any agent may take any role.
- Research: prefer Claude for exploratory research (design-space surveys, alternatives with tradeoffs, cross-repo implications); prefer Codex for forensic research (exhaustive inventories with file:line evidence, call-site censuses).
- Review tiers: Tier D defect/compliance review (post-implementation diff correctness, audits, serde/schema verification, determinism sweeps, acceptance-evidence checking) prefers a Codex reviewer — never the same agent identity that authored the diff, on any platform. Tier A altitude review (design/plan soundness, goal-achievement review) prefers a Claude reviewer. Routine impl diffs get Tier D only; design docs get Tier A only; milestone gates get both tiers in parallel.
- Implementation prefers Codex workers; give creative-design subtasks (scenario libraries, metric semantics, similarity-control schemes) a Claude design pass first and hand the implementing worker a spec.

## Repo-Specific Integration / Git Policy

- Keep git branch creation and commits in the orchestration thread unless the user explicitly delegates shared-state Git mutation.
- PR titles describe the change contents; never use bare version numbers or milestone labels as titles.

## Rule Suite Refresh Notes

- Suite migrated to rule schema v2 on 2026-07-14 (added reviewer.md and _lifecycle.json; front matter added to all role files) per character-memory-evals issue #10.
