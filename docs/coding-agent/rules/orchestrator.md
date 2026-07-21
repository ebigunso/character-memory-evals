---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "orchestrator"
last_updated: "2026-07-22"
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

## Push Sequencing (internal review before external review)

- A push that triggers external review (opening a PR, or pushing commits to a branch with an open PR) is the promotion step from internally-approved to externally-visible: it happens only AFTER the internal Tier D verdict covering those commits is APPROVED (user-directed 2026-07-22).
- Workers commit locally and do not push; internal reviewers pin worktrees from the local repository, so review never requires the remote. Dispatch prompts must not pair "push" with "reviewer bounce follows".
- Exceptions, each explicit per instance: docs-only commits with no review obligation; a CI-environment behavior that genuinely cannot be reproduced locally (orchestrator-ruled, with the reason recorded).
- Rationale: external reviewers (Copilot) should spend their rounds on internally-approved code, not re-discover defects the internal pass was already catching; overlapping the two layers wastes external rounds and creates thread churn.

## Design-Consult Threshold (coordination/advice separation)

- Escalation rulings split into two tiers (user-directed 2026-07-22, from the project-manager/product-manager analysis): routine escalations (naming, single-variant additions, test shapes, mechanical sequencing) are ruled fast-path WITH the blast-radius checklist; contract-shape escalations — public API surfaces, serialization schemas, cross-repo obligations, deferral-boundary questions — additionally require a design consult BEFORE ruling.
- The design consult is a dispatched Claude design/Tier-A agent holding the design doc and its amendments as resident context, asked what the proposed shape implies for the whole contract; when genuine urgency forbids the round trip, the orchestrator runs the full blast-radius checklist itself and records in the ruling that the consult was skipped and why.
- Rationale: coordination runs at interrupt tempo and biases rulings toward the proposal's local elegance; the phase's defective rulings were all contract-shape decisions made at coordination tempo, while every altitude decision routed through a dedicated design agent held up.

## Workaround Tripwire Obligations

- Treat Worker or Reviewer tripwire escalations (see common.md) as replan triggers: record the ruling in the plan Decision Log before the affected chunk resumes (user-directed 2026-07-21).
- When framing dispatches, do not attach surface-minimizing constraints ("minimal diff", "no new public types", "keep the signature") to contract, diagnostics, or schema work without also stating that preserving existing structure outranks the constraint; a constraint that forces a workaround is the Orchestrator's framing defect, not the Worker's implementation choice.
- Before dispatching a fix for a reported finding, check the proposed fix shape against the owning types and design record; a fix that works around a type it could change is itself a tripwire.
- Ruling scope is the blast radius, not the patch (user-directed 2026-07-22): workers and reviewers legitimately see only the local code they are working on; the Orchestrator's assessment of every escalation is what the change implies for the entirety of what it affects — every consumer (both repos), serialization/schema surfaces, deferred or coordinated scopes, and existing owned contracts. When the Orchestrator's own verification cannot cover that radius quickly, dispatch a researcher subagent (Codex forensic for consumer/call-site censuses, Claude for design-implication surveys) BEFORE ruling, not after the break.

## Repo-Specific Integration / Git Policy

- Keep git branch creation and commits in the orchestration thread unless the user explicitly delegates shared-state Git mutation.
- PR titles describe the change contents; never use bare version numbers or milestone labels as titles.

## Rule Suite Refresh Notes

- Suite migrated to rule schema v2 on 2026-07-14 (added reviewer.md and _lifecycle.json; front matter added to all role files) per character-memory-evals issue #10.
