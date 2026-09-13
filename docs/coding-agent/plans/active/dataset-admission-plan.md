# Plan: Dataset Admission (strict on identity and structure, lenient on annotations)

- status: draft
- generated: 2026-09-14
- last_updated: 2026-09-14
- work_type: code

## Goal
- An official dataset file that fails the strict set below is rejected before any embedding call or store mutation, with a typed error naming the item and the field, while every valid official record (including records that legitimately omit optional annotations) is admitted unchanged. No version discriminator exists in these files, so no version claim is made: the guarantee is exactly the strict set.

## Definition of Done
- Both dataset loaders (LoCoMo, LongMemEval) reject exactly this strict set, at load and before the pipeline creates a run root or calls a provider:
  - an unrecognized top-level shape (neither an array nor one of the admitted wrapper keys);
  - an item whose id is absent, null, not a string, or blank; duplicate item ids within one file;
  - a session in a shape the loader does not admit: the admitted shapes are the ones the official files use today (a LongMemEval session is an object or a turn array, with its id from the session record or from the parallel `haystack_session_ids` array; a LoCoMo session is either an entry of a conversation array that is an object with turns and its own id under the admitted aliases (session_id, session, id), or a keyed value under a key of the exact form `session_<N>` whose key is the session id and whose value is a turn array; the `session_<N>_date_time` keys are annotations, not sessions, and an annotation key without a matching session is ignored as it is today, since the official file carries such keys), and anything else (a scalar entry, a `session_<N>` value that is not a turn array, a session without an id from any of those sources) is rejected instead of being turned into a synthetic empty session, given an invented `session_N` id, or dropped while its siblings survive;
  - an item with zero sessions; a LongMemEval item without a question; a LoCoMo item whose QA array is absent, not an array, or empty, or any QA entry that is not an object carrying a question under one of the admitted aliases (question, q) that is absent, null, not a string, or blank (an item or QA entry that can produce no valid row is a structural defect, subject to the Task_1 census confirming no official item is like that).
- Identity rules: item ids come from the record under the admitted aliases; session ids come from the session record, the parallel `haystack_session_ids` array, or the keyed session's key, as each official encoding provides; duplicate ids are rejected within one file for items and for the effective QA ids (explicit or derived), and within one item for sessions whose ids are values (a session record id or the parallel array); for the keyed LoCoMo form, object-key uniqueness is the JSON parser's contract and a repeated key cannot be observed after parsing, so no duplicate-key promise is made for it and no source-text parser is added; a LoCoMo QA entry keeps an explicit id when it carries one under the admitted aliases (question_id, qid, id) and gets the deterministic id derived from the sample id and position only when it carries none, exactly as today; absent, null, wrong-type and blank are all "missing" for an identity or structural field.
- Optional annotations (question type, answer, question date, session dates, speaker labels) stay optional; the tested key aliases stay admitted. Whether an answer-less LongMemEval item is an abstention record is settled by Task_1 from the file's gold or has-answer fields, never inferred from a missing answer alone.
- The full official LoCoMo and LongMemEval-S files are admitted with the same item counts, ids and typed fields as before this plan (byte-level comparison of the parsed items), so no measurement changes.
- The decider ruling is recorded in the reviewer rules (dataset loaders join the fail-closed input surfaces) and the plan closes with censuses and the comparison evidence.

## Scope / Non-goals
- Scope: the two dataset loader crates, their typed errors, the runner's handling of a load failure (exit non-zero before admission of any output), tests, README dataset section, reviewer rule line. The converter is covered without a change of its own: its file entry point calls the same loader functions with error propagation, so it inherits the admission; its typed-object entry point is untouched.
- Non-goals: any change to scoring, ingest mapping, the converter's implementation, the continuity fixtures (already fail-closed), or the artifact contract; no dataset schema or version discriminator beyond the fields the loaders read today; no new dependency.

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: dataset loader admission (input contract) and the loaders' public parse functions returning typed errors
- stance: break
- justification: no external consumer of the loader crates exists (the runner and the converter are the only callers, both in this workspace); official dataset files that are valid today remain admitted by the definition of done.

## Context (workspace)
- Related files/areas: `crates/cmem-eval-longmemeval/src/loader.rs`, `crates/cmem-eval-locomo/src/loader.rs` (tolerant defaults: "unknown" ids, empty questions, empty sessions, synthetic session ids); `crates/cmem-eval-runner/src/pipeline.rs` (no post-load admission; only the empty-run guard); `crates/cmem-eval-benchmark-convert` (second caller of the loaders).
- Existing patterns or references: fixture and config admission fail closed (`crates/cmem-eval-continuity/src/fixture.rs`, `crates/cmem-eval/src/config.rs`); reviewer hotspot `admission_before_side_effect` in `docs/coding-agent/rules/reviewer.md`; ADR-I-0005 scopes derived-serde leniency to artifact readers only.
- Design record consulted and deviations from its acceptance: ADR-I-0004 (dataset crates own their loaders; no shared-crate edit in a dataset change) and ADR-I-0005 (input contracts are not artifact readers); no deviation.

## Open Questions (max 3)
- Q1 (settled 2026-09-14, see Decision Log): no answer-less LongMemEval item exists; abstention records carry a textual answer and an `_abs` id suffix, so a missing answer is never an abstention signal and stays an optional annotation.
- Q2 (settled 2026-09-14, see Decision Log): no official item has zero sessions or an empty QA array; the strict set rejects nothing in either official file.

## Assumptions
- A1: Both official files parse today with zero "unknown" ids and zero zero-session items — source: Task_1 census (Decision Log, 2026-09-14), confirmed.
- A2: The runner calls the loaders before creating the run root — source: `crates/cmem-eval-runner/src/pipeline.rs` (load before the run root and adapter construction), confirmed at review; Task_2 keeps it so.
- A3: The converter's file entry point delegates to the two loader load functions with error propagation, so file admission there inherits the loaders' strictness — source: `crates/cmem-eval-benchmark-convert/src/lib.rs` convert_paths, confirmed at review.

## Tasks

### Task_1: Dataset field census
- type: research
- owns:
  - docs/coding-agent/plans/active/dataset-admission-plan.md
- depends_on: []
- description: |
  For each official file (LoCoMo, LongMemEval-S) tabulate, per field the loaders read: present in every item, present in some, absent; and whether the loader defaults it today. Confirm A1, and answer Q1 (from the gold or has-answer fields and the official documentation) and Q2 (zero-session and empty-QA counts) from the files, not from memory. Record the table in this plan's Decision Log.
- acceptance:
  - The table names every field each loader reads with its presence class and today's default.
  - A1, Q1 and Q2 are answered with file evidence (item counts and example ids); if Q2 finds an official item the strict set would reject, the plan stops for a decider ruling before Task_2.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Plan Decision Log carries the census and the answers; the strict set in Task_2 follows from it"

### Task_2: Strict admission in both loaders
- type: impl
- owns:
  - crates/cmem-eval-longmemeval/**
  - crates/cmem-eval-locomo/**
  - crates/cmem-eval-runner/src/pipeline.rs
  - README.md
- depends_on: [Task_1]
- description: |
  Each loader returns a typed admission error (item index or id, field, reason) for the strict set in the Definition of Done; optional annotations and the tested key aliases stay as they are; the synthetic session id fallback and the silent drop of malformed sessions are removed in favour of rejection. The runner surfaces a load failure before creating the run root or calling any provider. README's dataset section states the contract.
- acceptance:
  - Every strict-set case has a rejection test with the named field; every optional-annotation case has an admission test.
  - The full official files are admitted with parsed items byte-identical to the pre-plan parse (comparison script retained as evidence).
  - A malformed file fails before any run root exists (census of the output directory after the failure).
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "fmt; clippy --workspace --all-targets -D warnings; embedded workspace suite; the pre/post parse comparison on both official files; the maintained continuity smoke pair"
  - kind: review
    required: true
    owner: reviewer
    detail: "Diff review against the strict set; independent parse comparison; rejection-before-side-effect probe with a malformed file"

### Task_3: Rule and closeout
- type: docs
- owns:
  - docs/coding-agent/rules/reviewer.md
  - docs/coding-agent/plans/active/dataset-admission-plan.md
  - docs/coding-agent/plans/completed/dataset-admission-plan.md
- depends_on: [Task_2]
- description: |
  Add dataset loaders to the fail-closed input surfaces in the reviewer hotspot; close the plan with the censuses and move it to completed.
- acceptance:
  - The reviewer rule names dataset loaders beside fixtures, configs and frozen stores.
  - The plan is in completed with its final progress entry.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "Rule wording and closeout entry"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2]
- Wave 3 (parallel): [Task_3]

## Rollback / Safety
- Loader changes are additive rejections; reverting the branch restores tolerant parsing. No persisted format changes.

## Progress Log (append-only)

- 2026-09-14 Plan drafted from the decider's ruling (option 3 of the dataset-handling question raised during the loader cleanup): strict on identity and structure, lenient on annotations, done now rather than folded into v0.2 planning.
- 2026-09-14 Copilot on the plan: effective LoCoMo QA ids (explicit or derived) must be unique across the file, since they become result identities the diff command indexes; the duplicate-session promise is scoped to value-sourced ids because serde collapses repeated object keys before the loader sees them, and a source-text parser for that case is not worth adding.
- 2026-09-14 Third pre-approval round (evals-reviewer): the LoCoMo object-session form (a conversation array of session objects with record ids) is named among the admitted shapes beside the keyed turn-array form, so the session rule matches every encoding the loader and its fixtures admit today.
- 2026-09-14 Second pre-approval round (evals-reviewer): the session rule preserves the admitted official encodings (LongMemEval object or turn-array sessions with ids from the record or the parallel array; LoCoMo keyed turn-array sessions with the key as id) and rejects only unsupported or malformed shapes; LoCoMo QA entries must be objects with a question under the admitted aliases; explicit LoCoMo QA ids are preserved and derivation applies only when an entry carries none.
- 2026-09-14 Pre-approval review (evals-reviewer, plan integrity) and Copilot: the goal no longer claims version rejection (no discriminator exists); the strict set now names identity semantics (absent, null, wrong type, blank are missing; duplicate scopes), rejects non-object and id-less sessions instead of synthesizing or dropping them, and treats an empty QA array as structural; the converter's inheritance of loader admission is recorded as a confirmed assumption instead of an open question; validation owners use the canonical role names (worker, reviewer, orchestrator; the evaluation worker and reviewer agents fill them); Task_3 owns the completed plan path.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-14 Task_1 census of the official files (`datasets/locomo10.json`, `datasets/longmemeval_s_cleaned.json`), read directly, not from memory.

  LoCoMo (10 items, sample ids conv-26, conv-30, conv-41, conv-42, conv-43, conv-44, conv-47, conv-48, conv-49, conv-50):

  | field the loader reads | presence | today's default |
  |---|---|---|
  | top-level shape | bare array | wrapper keys data/samples/items admitted, unused |
  | `sample_id` (alias `id`) | every item, unique, non-blank | "unknown" |
  | `conversation` (alias `conversations`) | every item, keyed object form | empty session list |
  | `conversation.speaker_a`, `speaker_b` | every item | absent |
  | `session_<N>` turn arrays | every item; 272 sessions in total, 19 to 32 per item, none empty | non-array value dropped silently |
  | `session_<N>_date_time` | 288 keys; 16 of them (conv-26, session_20 to session_35) have no matching `session_<N>` array | ignored when no session matches |
  | turn `speaker`, `dia_id`, `text` | every turn (5882) | speaker absent, dia_id derived, text empty |
  | turn `img_url`, `blip_caption` | some turns (910 and 1226) | absent |
  | `qa` | every item, 105 to 260 entries, 1986 in total | empty list |
  | qa `question`, `evidence`, `category` | every entry | question empty |
  | qa `answer` | 1542 entries; absent on 444 of the 446 category-5 (adversarial) entries, which carry `adversarial_answer` instead | absent |
  | qa `question_id` (aliases `qid`, `id`) | no entry | derived from sample id and position |

  LongMemEval-S (500 items, question ids unique and non-blank; every field below is present in every item, so no loader default is exercised by the official file):

  | field the loader reads | presence | today's default |
  |---|---|---|
  | top-level shape | bare array | wrapper keys data/instances/questions admitted, unused |
  | `question_id` (alias `id`) | every item, unique | "unknown" |
  | `question` | every item, non-blank | empty string |
  | `question_type` | every item (single-session-user 70, multi-session 133, single-session-preference 30, temporal-reasoning 133, knowledge-update 78, single-session-assistant 56) | absent |
  | `answer` | every item, non-blank | absent |
  | `question_date` | every item | absent |
  | `haystack_sessions` | every item, turn-array form only (23867 sessions), at least 38 per item | empty session list |
  | `haystack_session_ids`, `haystack_dates` | every item, lengths equal to the session count | synthetic `session_N` id; date absent |
  | `answer_session_ids` | every item, never empty | empty list |
  | turn `role`, `content` | every turn (246750) | speaker absent, text empty |
  | turn `has_answer` | some turns (10960) | false |

  Answers:
  - A1 confirmed: zero "unknown" ids and zero zero-session items in either file.
  - Q1: no LongMemEval item lacks an answer. The 30 abstention items (question ids ending in `_abs`, for example `0862e8bf_abs`) carry a textual answer ("The information provided is not enough." or "You did not mention this information...") and non-empty `answer_session_ids`. Abstention is signalled by the id suffix and the answer text, never by a missing answer, so `answer` stays an optional annotation and no abstention rule enters the strict set.
  - Q2: zero items with zero sessions, zero items with an absent or empty QA array, zero QA entries without a question, zero duplicate item ids, and the derived LoCoMo QA ids are unique. The strict set rejects nothing in either official file, so no decider ruling is needed before Task_2.
  - Discovery folded into the Definition of Done: conv-26 carries 16 `session_<N>_date_time` annotation keys without a session array. Today the loader ignores them because only keys that parse as `session_<N>` are session slots. The keyed-session rejection is therefore scoped to `session_<N>` keys, and an orphan date-time key stays ignored as an annotation, otherwise the strict set would reject an official file.
  - Discovery recorded, no rule change: 444 LoCoMo adversarial QA entries carry `adversarial_answer` and no `answer`; the loader reads `answer` (alias `a`) as an optional annotation and never reads `adversarial_answer`, so nothing changes.


## Notes
- The datasets are external official files cited by hash in the run header; admission protects paid provider calls and citable evidence from silently degraded inputs.
