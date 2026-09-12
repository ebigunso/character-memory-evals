# Plan: Harness Right-Sizing (strictness follows the claim)

- status: approved
- generated: 2026-09-02
- last_updated: 2026-09-02
- work_type: mixed

## Goal
- Shrink the evaluation harness to a lab-notebook core sized to the decisions it informs: a deterministic runner, rows built on the library's public types, a diff tool, continuity metrics, and an on-demand sealing step, under the principle ruled 2026-09-02 that strictness follows the claim, not the code.

## Definition of Done
- Tiers 1 and 2 (inner loop; tuning and regression decisions) are served by `run` plus `diff` with no sealing, no schema versioning, and no two-run ritual.
- Tier 3 (durable claims) is served by one `seal` command that hashes a run directory, records the harness and library commits, and copies the traces and report into a tracked evidence directory; the findings register cites seal hashes from then on.
- Every retained feature names the decision it informs; the deletions listed in this plan are gone with zero-hit censuses.
- Process rules that encoded tier-3 rigor as a default are retired or narrowed as listed.
- The existing hash-cited evidence stays byte-identical and readable by hash; the current reference evidence is promoted into tracked storage.

## Scope / Non-goals
- Scope: the nine library-independent steps below now; the library-dependent steps after the library's embedded vector-store phase lands (tracked here, sequenced against that phase's merges).
- Non-goals: any change to what the library measures or decides (that is recorded in the library repository); changes to sealed bytes; new benchmark features.

## Context (workspace)
- Basis: the design-value audit at `docs/audits/2026-09-02-harness-design-value-audit.md`; verdict tally 14 delete, 18 oversized, 5 demote to diagnostic, 3 seal-on-demand, 12 earn their place, over ~21,300 production and ~13,000 test lines across seven crates.
- Prerequisite: the evidence-integrity fixes (PR #22) merged, since they already remove the shared graph-path fallback and the unhonored enrichment knobs and fix batch-outcome duplication.
- Sealed bytes that never change: every existing line of `reports/v0-1-5-findings-register.md` (new information is only ever appended as a dated addendum, the precedent set on 2026-07-29); the continuity fixtures and committed embedding manifests and stores it cites by hash; the 26 hash-cited configs. Deleting a config key that makes a cited config unparseable is acceptable (old artifacts are old); re-running a cited config is new evidence under a new hash.
- Constraint: the frozen-store file shape is kept as plain data while its guard logic is deleted, because the committed stores cost money to regenerate.
- Constraint: no run artifact is tracked today; the 885 MB of hash-cited evidence exists on one machine, so evidence promotion is the first sealing act.

## Open Questions (max 3)
- Q1: whether the conventional-benchmark pipelines (two dataset loaders and the converter) are re-audited at the default-flip decision that names them or earlier; default per the audit: at that decision.

## Assumptions
- A1: The library's outcome and telemetry types keep deriving serde, so rows can embed them directly once the harness is pinned to a library commit.
- A2: Mock deletion waits until the library runs service-free in CI (its embedded vector-store phase), or until CI gains a vector-service container.

## Tasks

### Task_1: Retire the rules that encode tier-3 rigor as a default
- type: docs
- owns:
  - docs/coding-agent/rules/common.md
  - docs/coding-agent/rules/reviewer.md
  - docs/coding-agent/rules/worker.md
  - docs/coding-agent/rules/orchestrator.md
  - docs/coding-agent/lessons.md
  - docs/coding-agent/plans/active/*.md
  - docs/decisions/**
- depends_on: []
- description: |
  Apply section 5 of the audit: retire the strict-reader and admission-strictness clauses and the ADR whose premise they rest on (with the archive move and reciprocal frontmatter the decisions README requires); retire the two-run, bijection, re-derivation, and reader-test clauses; narrow the field-trace, structured-error, canonical-hash, and design-consult clauses as listed; archive the lessons that teach two-run and canonicalization-literal discipline; close the stale active plans with a closeout note. Keep the gold-label, live-default, loud-mock, and fixture-field-consumer rules.
- acceptance:
  - Every clause quoted in section 5 of the audit is retired or narrowed exactly as listed; retained rules are unchanged.
  - No active plan remains that this plan supersedes.
- validation:
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Clause-by-clause check against audit section 5; decisions README lifecycle followed for the retired ADR"

### Task_2: Delete unclaimed features
- type: impl
- owns:
  - crates/cmem-eval-runner/**
  - crates/cmem-eval-core/**
  - crates/cmem-eval-adapter-cmem/src/lib.rs
  - configs/**
  - fixtures/synthetic_small.json
  - README.md
- depends_on: [Task_1]
- description: |
  Delete the official-export command, the summarize command with its identity invariants, the synthetic dataset and its smoke, the reserved and assertion-only config knobs, the repair and attempt counters as reported metrics (the library's telemetry stays on rows as diagnostic data), and the ten Task_7 sweep configs no decision cites. Nothing depends on them; each deletion ships with a zero-hit census.
- acceptance:
  - Zero-hit census per deleted feature; README updated; workspace builds and tests green.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; census commands recorded"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; confirm no cited config or sealed byte changed"

### Task_3: The diff subcommand
- type: impl
- owns:
  - crates/cmem-eval-runner/**
  - README.md
- depends_on: [Task_1]
- description: |
  Add `diff <run-a> <run-b>`: set every `latency_ms` to zero and replace every `run_id` with a placeholder (the run-level fields the README recipe normalises today; returned object identities are never touched, they are what the diff compares), then print per-query differences in returned identities, ranks, metrics, and degradation flags, plus a summary; retire the shell hashing recipe from the README. This restores tier-2 regression detection before any reader is removed.
- acceptance:
  - Two identical runs diff empty; a run pair with an intended rank change reports exactly that change.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "diff on the reviewer's two candidate runs is empty; diff candidate vs parent baseline is empty on identities, ranks, metrics, and degradation (parent already carries the rank fix), while removed-field absence is reported informationally"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; README recipe retired"

### Task_4: Derived readers, run header, no schema versioning
- type: impl
- owns:
  - crates/cmem-eval-core/**
  - crates/cmem-eval-continuity/**
  - crates/cmem-eval-runner/**
  - reports/v0-1-5-findings-register.md (append a dated addendum only; existing lines byte-identical)
- depends_on: [Task_2, Task_3]
- description: |
  Replace the strict fail-closed readers with derived serde, delete the schema-version constants and dispatch, delete summary and report normalization metadata, and add the run header (harness commit, library commit, config as read plus hash, fixture hash, store hash, embedding source, adapter mode, generation time). Record the resurrection pointer for the strict readers in the register addendum as the retired ADR's replacement note requires.
- acceptance:
  - No `required_option`-style strictness and no schema-version dispatch remain; run header present on every run; register addendum carries the pointer.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "workspace gates; a run produces the header; diff of a run before and after this task shows only the header and dropped metadata"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; sealed bytes untouched"

### Task_5: Rows into traces, report shrink, frozen store simplified
- type: impl
- owns:
  - crates/cmem-eval-core/**
  - crates/cmem-eval-continuity/**
  - crates/cmem-eval-runner/**
  - crates/cmem-eval-adapter-cmem/src/lib.rs
  - README.md
- depends_on: [Task_4]
- description: |
  Merge the rows file into the trace file and shrink the report; delete the payload-congruence validation that existed only because two artifacts carried one payload; simplify the frozen embedding store to cache plus ordering validator, deleting the bijection guard, reuse merge, dimension-policy logic, and live-run provenance rejection while keeping the persisted file shape.
- acceptance:
  - One artifact per run plus header and report; committed stores load unchanged; diff tool works on the merged shape.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "workspace gates; committed stores load byte-unchanged; continuity run + diff green"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review; store file shape byte-unchanged"

### Task_6: The seal subcommand and evidence promotion
- type: impl
- owns:
  - crates/cmem-eval-runner/**
  - evidence/**
  - reports/**
  - README.md
- depends_on: [Task_5]
- description: |
  Add `seal <run-dir>`: hash every artifact, write the hashes with the run header into a seal file, and copy the traces and report into a tracked evidence directory. Promote the current reference evidence (the register's reference pair, about 2.6 MB per run) into that directory, leaving every existing register line byte-identical and appending one dated addendum that links seal hashes to the register's cited hashes.
- acceptance:
  - A stranger can re-derive a cited number from tracked bytes; every existing register line is byte-identical and exactly one dated addendum is appended.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "seal on a fresh run; verify command recomputes hashes; promoted evidence hashes match the register's cited values"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Hash reconciliation against the register"

### Task_7: Honest CI for live tests
- type: test
- owns:
  - crates/cmem-eval-adapter-cmem/src/lib.rs
  - .github/workflows/ci.yml
- depends_on: [Task_2]
- description: |
  Mark the live adapter tests ignored so CI reports them honestly instead of passing by skip; keep the forced-live switch for local and service-backed runs.
- acceptance:
  - CI shows the live tests as ignored, not passed.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "cargo test --workspace output lists the live tests as ignored; forced-live run still executes them"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review"

### Task_8: Library-dependent shrink (after the library's embedded vector-store phase)
- type: impl
- owns:
  - crates/**
  - Cargo.toml
  - README.md
- depends_on: [Task_6, Task_7]
- description: |
  Sequenced against the library's phase merges: delete the mock adapter and its guard flags once the library runs service-free; embed the library's outcome and telemetry types on rows and delete the mirror vocabulary, the from-live projections, and the telemetry projection; move the vector-only baseline onto the retrieval trace as the library's record specifies (one singleton-scoped traced retrieval per kind with the multiplied limit and object-level deduplication) and drop the second vector-service client and payload constants; replace the string-keyed settings bridge with the library's typed construction; replace collection naming and the prefix cleanup guard with per-run directories; replace hand-built write plans with the library planner; delete the skip macros and run the adapter suite unconditionally in embedded mode; collapse the crates into one. Re-audit the conventional-benchmark loaders at the default-flip decision.
- acceptance:
  - One crate; rows carry library types; zero-hit census for the mirror, the mock, the direct search, and the skip macros; size within the estimate band in section 3 of the audit.
- validation:
  - kind: command
    required: true
    owner: evals-worker
    detail: "workspace gates in embedded mode with no service; continuity run + diff against the last sealed reference"
  - kind: review
    required: true
    owner: evals-reviewer
    detail: "Diff review per step; size and census evidence"

## Task Waves (explicit parallel dispatch sets)

- Wave 1 (parallel): [Task_1]
- Wave 2 (parallel): [Task_2, Task_3]
- Wave 3 (parallel): [Task_4, Task_7]
- Wave 4 (parallel): [Task_5]
- Wave 5 (parallel): [Task_6]
- Wave 6 (parallel): [Task_8] (gated on the library phase's merges; split into per-step PRs)

## Rollback / Safety
- Every wave is a separately revertible PR; sealed bytes are never edited; evidence promotion adds files only.
- The strict readers' resurrection pointer is recorded before they are deleted.

## Progress Log (append-only)

Append-only editing rule (applies to both logs below): when appending an entry, anchor the edit on the previous entry and reproduce it (or anchor on the section's tail marker) so the edit inserts rather than replaces, and verify afterward that the log grew.

- 2026-09-02 Plan authored from the design-value audit; direction approved by the decider with one correction: harness work is tracked here, not in the library repository.
- 2026-09-02 Waves 1 and 2 done (Task_1, Task_2, Task_3) with review fixes: lenient diff reader with an empty parent-versus-candidate proof, the maintained unsealed continuity smoke config with its README recipe, the reviewer rule narrowed to run plus diff, inbound links to the moved plans, and the continuity-smoke gate rule restored after Task_2 dropped it. Reviewer approved at d260193 with no open findings. Waves 3 and later remain.

- 2026-09-10 Task_8 step 2 done (CME #26, ec41de5, library main 7e6c898): the vector-only baseline reads scoped retrieval traces, the direct search and payload constants are deleted (service client kept for lifecycle only), telemetry carries one completeness verdict per retrieval at schema 2.2.0, vector-only is admitted in embedded mode, completed conventional namespaces are detached without deletion. Controlled evidence under deterministic vectors on the LoCoMo subset: trace reproduces the direct search exactly apart from per-kind cutoff ties, and service versus embedded is identical on every counter and score; the real-provider runs are retained with per-query explanations and make no parity claim of their own. Reviewer approved after independent regeneration and the forced-live suite.

- 2026-09-13 Task_8 step 3 worker implementation completed against library main 7e6c898: removed the mock adapter, selector flags, availability checks and skip macros; ordinary adapter and runner tests now use embedded stores, with two explicit service-mode tests. Merged only core and adapter into `cmem-eval`; the other dataset and runner crates remain. Rows and traces carry native retrieval, remember, link, and lifecycle outcomes at schema 3.0.0, with one degradation summary over native failures. Removed diagnostic metric projections while preserving character-facing metrics. The two BM25 configs remain byte-identical and run through the ingested-text baseline. Worker gates: fmt, strict workspace/all-target clippy, 272 service-free workspace tests and two service tests passed with zero ignored tests; the maintained native smoke pair has zero differences, and its pre-step comparison preserves all 60 retained metrics plus identity, rank, score, context and integrity. The 72-path preservation check found 71 unchanged files and the exact findings-register prefix plus the reader-pointer addendum. Physical Rust lines decreased by 4,397 production and 2,395 test lines; evidence and the module-aware counting method are under `.agent-work/evals-worker/task8-step3/`. External review remains the next gate.

- 2026-09-13 Task_8 step 3 done (CME #27, f687ed3, library main 7e6c898; audit steps 10, 11, 15): the mock memory adapter is deleted (BM25 stays as the harness's own ingest-text baseline selected by retrieval mode, configs byte-identical); the live-skip guard is deleted and the adapter suite runs unconditionally in embedded mode with two explicit service-mode tests that fail rather than skip; rows embed the library's native outcome and telemetry types in a minimal operation-id envelope, the mirror vocabulary and all projections are deleted, schema 3.0.0 with the 2.2.0 resurrection pointer at 89bc0a8; 34 telemetry-derived diagnostic keys leave `metrics` and the 60 retained metrics are proven unchanged; core and adapter collapse into `crates/cmem-eval`. Production −4,397 lines, tests −2,395. Reviewer approved with no revision requests.

- 2026-09-13 Task_8 step 4 P1 correction: service collection identities include the canonical run-root hash, recorded in the run header. Atomic run-root acquisition remains the ownership gate; different-root concurrent runs no longer share collections, and same-root duplicate admission fails before namespace setup. The library retains collection construction. Added concurrent service isolation and atomic root-admission regressions; validation evidence accompanies the Worker handoff.

- 2026-09-13 Task_8 step 4 Copilot corrections: named output paths are checked against the reserved run stores root before directory creation; retained namespace cleanup closes only that namespace and preserves open siblings and durable files. Added CLI admission, derived-output path, and two-namespace retention regressions. Task_5 is parked while this priority fix lands; Task_4 and Task_5 will be rebased onto the reviewed parent.

- 2026-09-13 Task_8 step 4 P1B correction: output admission resolves filesystem identities after creating required parents and atomically acquiring the reserved root. Removed lexical normalization and ASCII case folding. Unicode-case and junction aliases are rejected before artifact writes; failed admission removes only the acquired root. Added both Windows regressions and the reviewer path-identity hotspot.

- 2026-09-13 Task_8 step 4 P1C correction: every named output leaf is inspected without following links; live and dangling symlinks and non-file leaves are rejected by name. Ordinary file overwrite remains supported. Admission compares canonical created parents with the acquired stores root and has no canonicalize-NotFound fallback. The dangling-summary reproduction fails before artifact writes and removes only the newly acquired root.

- 2026-09-13 Task_4 implementation: rows, traces, summaries and reports use ordinary derived serde, without output schema dispatch, duplicate-key policing, required-option strictness or artifact-owned unknown-field rejection. Removed report normalization metadata and cross-artifact congruence checks. A single RunHeader owns identity, exact config/hash, input hash, root/hash, retention and scenario-keyed embedding bindings with known frozen store hash/source; rows retain run ID and measurements. Input fixture/config/frozen-store admission remains unchanged. README names `diff` as the comparison instrument. The branch records the strict-reader resurrection pointer in the findings register as the pull request whose squash commit last carries the readers (#28); the Orchestrator appends the hash after the stack merges.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-02 Decision: adopt "strictness follows the claim, not the code" as the harness's standard.
  - Trigger / new insight: the harness reached the library's size while serving a development aid's purpose; tier-3 machinery sat on every run and produced accounting defects in plumbing rather than measurements of characters.
  - Plan delta: notebook core with `diff` and `seal`; rules retired; unclaimed features deleted; mirror and mock deleted after the library is service-free.
  - Tradeoffs considered: section 6 of the audit (two-run gate replaced by a two-minute diff; strict readers replaced by seal hashes; compile-time breakage on library change is the desired signal; evidence credibility restored by promotion).
  - User approval: yes, 2026-09-02.
- 2026-09-02 Decision: harness work is planned and tracked in this repository; the library's records state only what these measurements allow it to decide and when.
  - Trigger / new insight: eval-side tasks had been mixed into the library's phase plan.
  - Plan delta: the two eval-side tasks were removed from the library plan and live here as Task_8's steps.
  - User approval: yes, 2026-09-02.
- 2026-09-02 Decision: BM25 baseline retained by decider ruling: it is the lexical hurdle recall must beat and the proof of improvement over prior versions; audit verdict DELETE overridden.
  - Plan delta: Task_2 retains the BM25 code path and the LongMemEval-S and LoCoMo configs; `synthetic_bm25.toml` stays deleted with the synthetic dataset because a config for a deleted dataset is not a usable baseline.
  - User approval: yes, 2026-09-02.
- 2026-09-04 Decision: Task_8 is re-sequenced into per-step PRs and its first step runs BEFORE the library's phase merge, by decider ruling: the library's default vector store is finalized only after this repository's evaluations run against the library's stacked pre-merge tip (CM 21d9786, PR stack #76).
  - Step 1 (this wave): compile adaptation to the pre-merge library (exhaustive conversion of the new vector-indexing failure kind; the completeness field on mirrored telemetry; the vector-store mode and path keys in the settings bridge with a per-run embedded store path); then the cross-mode evaluation: the continuity suite and the two conventional benchmarks run in service mode and in embedded mode against the same library tip, with `diff` between modes and against the last reference. The vector-only baseline stays on its direct search for this step; its trace migration and A/B proof are step 2.
  - Trigger: the library phase's completion evidence for the embedded default is in-library parity; the decider requires behaviour-level evidence from this repository before the default is final.
  - Tradeoffs: the mirror vocabulary is extended rather than deleted for this step (deletion remains Task_8's later step) so the evaluation runs on unchanged measurement code; the library pin is a pre-merge commit and the run is repeated on the merged tip only if the merge changes it.
  - User approval: yes, 2026-09-04.

- 2026-09-10 Decision: Task_8 step 1 closes on CME #25 with the schema advanced to 2.1.0 (telemetry's completeness field is required) and the vector-only-needs-service invariant enforced at config admission. Two low-severity review findings are deferred to step 2, where the full datasets run: embedded conventional runs retain one engine per namespace until the final cleanup pass (a resource ceiling for large datasets; remedy is a non-deleting namespace detach after each completed item), and the reattachment error message should name vector-store state in mode-neutral wording.

- 2026-09-13 Decision: Task_8 step 3 uses `RecordedOutcome<T> { operation_id, outcome }` with native `RememberOutcome`, `LinkOutcome`, and `LifecycleMutationOutcome`, and retains native `RetrieveOutcome` values directly. Telemetry-derived diagnostic metric families are deleted; rationale, recall, pollution, relevant-hit and lifecycle-safety metrics read native traces. BM25 remains a small index over ingested text. The live `diff` reads only native schema 3.0.0; the pre-step 2.2.0 comparison is retained as a separate evidence script. These choices follow the orchestrator's step 3 rulings; no compatibility reader is added.

- 2026-09-13 Task_8 step 4 rulings: one runner-owned root at `OUT_DIR/stores`, no UUID suffix, fail closed on an existing root, cleanup on success/error unless explicit retention includes a reason; service names derive from run ID and namespace and admission never deletes pre-existing collections. Selected-mode settings replace placeholders through the existing library Settings constructor (library PR #81 must land before harness CI, which resolves library main). A single RunHeader includes provenance, exact config/hash and retention/storage fields at schema 3.1.0; Task_4 retains ownership of deleting strict readers/schema dispatch.
- 2026-09-13 Task_8 loader re-audit proposal (no deletions in this step): retain both source loaders and the converter. LoCoMo and LongMemEval loaders still own official date/source parsing, typed ingest mapping and gold-label scoring; the runner consumes them for full-corpus evaluation and `cmem-eval-benchmark-convert/src/lib.rs` reuses them for curated conversion. Propose deleting only unused `LoCoMoSample.raw` and `LongMemEvalInstance.raw` fields plus their assignments (no workspace `.raw` consumer), and the infallible private LongMemEval `parse_instance` Result wrapper. Alternate source aliases are tested contracts, and LoCoMo enrichment flags remain consumed; no loader or crate is dead. Await the decider's deletion ruling.
- 2026-09-13 Task_8 planner census: candidate embedding_text is not the provider input; native commit reconstructs per-kind prefixed/cleaned surfaces from domain objects, matching the frozen stores. No sealed store needs regeneration for equal objects/targets. RememberInput prepare always emits exactly one episode and one observation and only reads the first explicit draft of each; arbitrary multi-episode/multi-observation, existing-episode observation-only, and zero-episode enrichment/link batches cannot preserve their topology through that helper. Item 3 code stays unchanged pending the design ruling; the Worker census records the gap rather than expanding the library surface locally.

- 2026-09-13 Decision (design scrutiny, decider-prompted): the harness's placeholder settings are removed by the library making settings required only by the selected mode (library PR #81, generalizing ADR-I-0023's service-string rule), not by a typed options constructor; the audit's "typed construction surface" wording assumed one existed. The unread candidate embedding text is deleted in the library (PR #82, stacked on #81); audit step 14 (replace hand-built plans with the library planner) is closed as not applicable: the library planner shapes one remembered turn by design and the typed batch builder is the loader's responsibility, composing public plan types and using native validation and commit.
- 2026-09-13 Reconciliation: the definition of done retires schema versioning (Task_4). While the strict readers exist, artifact constants move with the shape per ADR-I-0002; Task_8 steps 2-4 ran ahead of Waves 3-5 after the library merge by Orchestrator decision, and Task_4 follows step 4 as the next stacked step, with Tasks 5-7 after it.
- 2026-09-13 Proposed ADR-I-0004: one shared evaluation crate holds the contracts and the library integration; datasets own their crates. Constraint: no dataset dispatch in the shared crate and no shared-crate edit in a dataset addition. Why: the only second implementation of the contract was the deleted mock. Replaces ADR-I-0001; ADR-D-0001 is retired because nothing in it binds once the mock is gone. Status proposed until the decider accepts.
- 2026-09-13 Directive: sequential steps of one plan do not wait for the previous step's merge; each next step branches from the previous step's tip and its PR is linked into a GitHub stack; the decider merges the stack.

- 2026-09-13 ADR-I-0004 accepted by the decider (ebigunso); status flipped from proposed to accepted before merge.
- 2026-09-13 Proposed ADR-I-0005: sealed evidence is guaranteed as bytes by hash, never as parseability by the live binary; live readers deserialize only the current shape through derived serde, with no schema version and no unknown-field rejection, and every run states its provenance in one run header. Why: hashes protect bytes completely and the parser promise served no decision while costing a versioning ritual on every change. Replaces ADR-I-0002 in full (Task_4). Resurrection pointers inside a stack cite the pull request whose squash commit last carried the reader; the Orchestrator appends the hash after the stack merges. Status proposed until the decider accepts.

## Notes
- Risks and mitigations: section 6 of the audit.
- Edge cases: cited configs may become unparseable after key deletions; that is accepted, and re-runs are new evidence.
