---
rule_schema_version: 2
suite_id: "rules-cme-20260714"
rule_file: "orchestrator"
last_updated: "2026-09-22"
---

# Orchestrator Repository Rules

## Repo-Specific Orchestrator Policies

- A merge authorization from the decider, conditional or not, covers only the pull requests named or in flight when it was given; every later pull request, however small, waits for its own explicit approval before merging (decider feedback 2026-09-14).
- Sequential steps of one plan do not wait for the previous step's merge: the next step branches from the previous step's tip and its PR is linked into a GitHub stack on the previous PR (`gh stack link`); the stack decides merge order and the decider merges the stack (user directive 2026-09-13).
- Every ruling or brief that authorizes a change, of any size and in any layer, is decided on what is best for the overall product and architectural design: state the design intent it serves (decision records, philosophy, README consumer path, phase documents), what it would make worse, and the alternative rejected; audit text, plan text and worker findings are inputs, not decisions, and a worker's "blocked, need X" is a symptom to diagnose, not a specification to forward (decider feedback 2026-09-13; lessons.md same date).
- When the handoff describes a forthcoming external public API, treat that API as the target contract and isolate current unavailability behind mocks or documented feature gates.
- When the shared checkout is occupied by a worker, give reviewers isolated `git worktree` checkouts pinned at the review commit instead of asking anyone to switch branches.

## Delegation Routing (repo scheduling; model-strength routing is harness-owned)

- Model-strength/platform routing is harness-owned: subagent-strategy `model-routing.md`. Repo policy retained: a reviewer is never the same agent identity that authored the diff, on any platform; routine impl diffs get Tier D only; design docs get Tier A only; milestone gates get both tiers in parallel.

## Push Sequencing (internal review before external review)

- A push that triggers external review (opening a PR, or pushing commits to a branch with an open PR) is the promotion step from internally-approved to externally-visible: it happens only AFTER the internal Tier D verdict covering those commits is APPROVED (user-directed 2026-07-22).
- Workers commit locally and do not push; internal reviewers pin worktrees from the local repository, so review never requires the remote. Dispatch prompts must not pair "push" with "reviewer bounce follows".
- Exceptions, each explicit per instance: docs-only commits with no review obligation; a CI-environment behavior that genuinely cannot be reproduced locally (orchestrator-ruled, with the reason recorded).
- Rationale: external reviewers (Copilot) should spend their rounds on internally-approved code, not re-discover defects the internal pass was already catching; overlapping the two layers wastes external rounds and creates thread churn.

## Value-Audit Triggers (design-value review scheduling)

- Design-value audit verdict mechanics and triggers are harness-owned (long-horizon-audit appendix; third-bounce and pre-merge-churn triggers). Repo policy retained: the audit is judged against this repo's roadmap deliverables and philosophy — does it serve a meaningful purpose NOW — and is assigned to a Claude Tier A agent.

## Measurement Ownership (library behavior changes)

- This repository owns the measurement of library behavior changes at the checkpoints the library plans name (a slice's completion; a task introducing a new road or a design rule flagged high-risk; a review that changes a design rule), not at every step or fix tip (decider refinement 2026-09-22): each measurement runs before and after on the same generated family and the same inputs, with identifiers ordered against time, run twice, and the numbers are handed back for the library worker report and the plan's Decision Log (decider ruling 2026-09-22).
- A library plan names the instrument in its `Design` section as an existing generated family here or the family it needs; when the family does not exist yet, building it is planned and tracked in this repository before the library task is dispatched, and the plan is not approvable until the instrument is named.
- An instrument's validity is reviewed before its numbers count; numbers from an unreviewed instrument are not evidence and are not handed back.
- Numbers handed back carry a reading of what they mean for the character against the behaviors the library's philosophy illustrates, and an explicit verdict, direction confirmed or direction questioned; a bare table is not a handoff (decider instruction 2026-09-22).

## Design-Consult Threshold (coordination/advice separation)

- Escalation-ruling tiers and the blast-radius obligation are harness-owned (lifecycle-gates Escalation Ruling). Repo policy retained, stronger than the harness default: contract-shape escalations — public API surfaces, cross-repo obligations, deferral-boundary questions — REQUIRE a design consult before ruling.
- The design consult is a dispatched Claude design/Tier-A agent holding the design doc and its amendments as resident context, asked what the proposed shape implies for the whole contract; when genuine urgency forbids the round trip, the orchestrator runs the full blast-radius checklist itself and records in the ruling that the consult was skipped and why.
- Rationale: coordination runs at interrupt tempo and biases rulings toward the proposal's local elegance; the phase's defective rulings were all contract-shape decisions made at coordination tempo, while every altitude decision routed through a dedicated design agent held up.

## Repo-Specific Integration / Git Policy

- PR titles state the point of the change in one short clause, not a list of its contents and not an explanation of the evidence; the contents and qualifications go in the body. Never use bare version numbers or milestone labels as titles.

## Rule Suite Refresh Notes

- Suite migrated to rule schema v2 on 2026-07-14 (added reviewer.md and _lifecycle.json; front matter added to all role files) per character-memory-evals issue #10.
