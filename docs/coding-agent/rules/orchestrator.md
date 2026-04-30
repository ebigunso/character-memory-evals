# Orchestrator Rules

## Repo-Specific Orchestrator Policies

- When the handoff describes a forthcoming external public API, treat that API as the target contract and isolate current unavailability behind mocks or documented feature gates.
- Split subagent work into short feedback-loop tasks, such as one module, one validation failure, or one review slice.
- Wait substantially longer before force-closing background agents unless they are clearly blocked, conflicting with newer user direction, or performing unsafe work.

## Repo-Specific Integration / Git Policy

- Keep git branch creation and commits in the orchestration thread unless the user explicitly delegates shared-state Git mutation.

## Global Migration Candidates (Placeholder)

- None.
