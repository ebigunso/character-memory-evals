# Plan: Only evidence that must last is kept, and bulky evidence is recoverable without being downloaded

- status: in-progress
- generated: 2026-09-23
- last_updated: 2026-09-23
- work_type: docs

## Goal
- A measurement leaves behind only what something durable cites. The small human-readable reading of a durable claim lives on main. The raw traces that claim rests on stay recoverable from GitHub on demand, and a default clone or pull never downloads them. Everything else is deleted once read. The repository stops growing by tens of MiB per measurement.
- Decision this informs: the decider's request that only results which absolutely must be preserved reach a permanently saved state, and that the repository not bloat.

## Definition of Done
- The rules name three storage tiers chosen by what cites an artifact. They also state the size ceiling per promotion, the citation form, and the recovery command. Worker and reviewer rules and the README "Seal evidence" section follow them.
- CI fails when any path under `.agent-work/` is tracked, or anything but markdown is tracked under `docs/evidence/`.
- On the stack (PRs 53 to 66), the tip tree of every branch holds no `.agent-work` file and no uncited artifact. The scene-reminders raw archives exist once each under `refs/evidence/2026-09-21-scene-reminders` on GitHub. The text the stack adds under `docs/evidence/` is under the ceiling. A fresh default clone's size is recorded now and again after the stack lands and its branches are deleted.
- Every durable citation resolves: the v0.1.5 register's hash-cited evidence (untouched), and the library's two scene-reminders citations, re-pointed to a main path plus the evidence commit.
- A lesson records why this happened.

## Planner-added requirements
- The evidence commit carries the whole promoted folder exactly as the source commit has it: readings, raw files, manifests and the repeat names. Needed because: the readings and manifests link one another and name the repeat archives, so a complete, unedited tree keeps every internal link and every recovery instruction valid; the repeat names point to the same blobs as their natives, so keeping them stores nothing twice. The commit is also the one permalink target that resolves now and stays reachable after the stack lands, without resting on GitHub's pull request refs.
- The ref's tree uses the same paths the files would have in the working tree. Needed because: then `git restore --source=<ref> --worktree -- <path>` puts the raw files beside their reading. Nothing needs to be renamed on recovery.
- The evidence commit is built from the blobs already in the source commit (`git read-tree`), never re-added from the working tree, and `docs/evidence/**` is marked `-text` in `.gitattributes` like `/evidence/**`. Needed because: this machine has `core.autocrlf=true`, and a restore without the attribute rewrote a 1,444,397-byte JSON table to 1,496,987 bytes (LF to CRLF) in the Tier D replay.
- The 256 KiB ceiling per promotion. Needed because: reviewers need a number to hold a promotion to; Design gives its reason.

## Scope / Non-goals
- Scope: `docs/coding-agent/rules/{common,worker,reviewer}.md`, `.gitignore`, `.gitattributes` (one line), `.github/workflows/ci.yml` (one guard step), `docs/coding-agent/lessons.md`, and the plan file. On the stack: one cleanup commit each on PRs 63, 65 and 66, with no merge forward. One ref on GitHub, `refs/evidence/2026-09-21-scene-reminders`, plus a disposable test ref. In the library: the ADR-I-0036 citation and the scene-words plan permalink on every library branch head that carries them at execution time.
- Non-goals: rewriting history or force-pushing any branch (see Open Question 1). Moving or re-sealing `evidence/pr13r9ba` and `pr13r9bb` (register-sealed bytes stay where the closed v0.1.5 register links them). Changing `seal`/`verify` code. Git LFS, release assets or any storage outside git. Purging GitHub's server-side `refs/pull/*` copies (never downloaded by default). Any other change to the library.

## Design
- Chosen: storage follows citation, in three tiers.
  - DISCARDED (default): gate logs, intermediate and repeat captures, helper scripts, worker reports, worker YAMLs. Deleted once read. The numbers live in the worker report and the plan's Decision Log.
  - RECOVERABLE: raw traces a durable claim rests on, needed only to reproduce it. They go into one orphan commit (no parent, only those files, working-tree paths, one copy each) under `refs/evidence/<yyyy-mm-dd>-<slug>`. That ref sits outside `refs/heads/*` and `refs/tags/*`, so the default fetch refspec never downloads it. Recovery is `git fetch origin refs/evidence/<id>:refs/evidence/<id>` and then `git restore --source=refs/evidence/<id> --worktree -- <path>`. With `docs/evidence/** -text` in `.gitattributes`, the restore writes the stored bytes exactly; on a checkout that predates that line, `git -c core.autocrlf=false restore ...` does the same. Restored raw files are untracked; a `.gitignore` rule keeps anything but markdown under `docs/evidence/` from being added, and CI rejects it if it is force-added. GitHub shows its "does not belong to any branch" banner on the evidence commit's pages; that is expected.
  - PERMANENT on main: the reading (the markdown table a person reads) and a manifest, plus sealed evidence. The manifest is the folder's `README.md` and records the ref name, the evidence commit hash and the recovery commands; the commit hash binds every byte, and `/tree/<hash>` or `git ls-tree` lists the files, so neither a hash list nor a file list is kept. Sealed runs stay whole on main, run files included: a seal a register cites is exactly what must be preserved, `verify` hashes every listed file and fails when one is missing, and a custom ref is deletable by anyone with write access. Sealing is rare (the two sealed runs are about 3.2 MB each), so this costs little.
- Size ceiling: 256 KiB of new files per promotion on main, measured in bytes (`git ls-tree -r -l`), text only (markdown readings and the manifest). Outside sealed runs, raw data never goes on main at any size: no `.jsonl`, `.json.gz`, JSON tables or audits, logs or scripts. Why this number: the largest real reading is 28 KiB (`after-the-fix.md`), and the folder's five markdown files including its README come to 56,931 bytes, so 256 KiB leaves four times the largest real promotion. It is about 1/140 of the 35 MB bundle it replaces. Some raw JSON audits are smaller than the ceiling, so the text-only rule, not the number, is what keeps them off main.
- Citation form: a durable citation from the library or a register names the main path of the reading, then `CharacterMemoryEvals@<evidence commit>` for the raw data. Where a link is wanted, it is `https://github.com/ebigunso/CharacterMemoryEvals/tree/<evidence commit>/<folder>`, which resolves before and after the stack lands.
- Guard: one CI step in the existing format job, `test -z "$(git ls-files .agent-work; git ls-files docs/evidence | grep -v '\.md$')"`. It enforces both "`.agent-work` is never tracked" and "raw data never under `docs/evidence`" without relying on reviewers. It is the smallest form: this repository has no pre-commit framework, and a versioned hook needs per-clone setup. Its limit is that it catches the file at PR time, after the push (Open Question 2).
- Ref procedure: documented git commands, no code. Promotion to the ref is rare, and git plumbing in Rust would need process spawning, error paths and tests. Procedure: the additive form in `docs/coding-agent/rules/common.md`, which never removes index entries and so does not depend on any checkout: in a separate index file inside the git directory, `git read-tree --prefix=<folder>/ <source commit>:<folder>` for each kept folder, `git update-index --add --cacheinfo 100644,<blob>,<path>` for single files (`git rev-parse <commit>:<path>` gives the blob, `git hash-object -w --no-filters <file>` for never-committed files), then `commit-tree` and `update-ref`. The first form, which removed entries with `git rm --cached`, refused when the source differed from the current checkout. The orchestrator then pushes with `git push origin refs/evidence/<id>`.
- Alternative: Git LFS. Rejected: every checkout pulls the LFS objects by default, GitHub meters LFS bandwidth, and every clone needs LFS installed.
- Alternative: GitHub release assets. Rejected: they are not content-addressed by git, they need authenticated uploads, and they have a lifecycle apart from the repository.
- Alternative: a long-lived orphan branch. Rejected: branches are fetched by every default clone, which is what the request rules out.
- Alternative: a separate evidence repository. Rejected: it adds a second place to cite and to keep permissions on, and the ref gives the same isolation in one repository.
- Alternative: keep raw data on main and rely on shallow clones. Rejected: that pushes the burden onto every reader.
- Structure: no new code. There is one ignore rule, one CI line and the rule text. The tier is decided once, at promotion time, by the question "what cites this?".

## Compatibility stance (required if a contract/interface/persisted format is touched)
- surface: where promoted evidence lives, and how the library cites it.
- stance: break
- justification: nothing outside these two repositories cites the scene-reminders folder. Its two citations are re-pointed in this plan. Sealed evidence (`evidence/pr13r9ba`, `pr13r9bb`, and any future seal) stays whole on main, so ADR-I-0005 holds unchanged.

## Context (workspace)
- Related files/areas: `docs/coding-agent/rules/common.md` (rigor tiers, line 14; sealed evidence, lines 34, 45 and 52; the `.agent-work` DELETE/PROMOTE rule, lines 38 and 39); README "Seal evidence"; `.gitignore` (already ignores `.agent-work/`); `.gitattributes` (`/evidence/** -text`); `reports/v0-1-5-findings-register.md` (seals `pr13r9ba`/`pr13r9bb` by hash).
- Inventory (packed object bytes, from the Tier D reproduction): main's history is about 12.0 MiB; the local stack adds about 31.1 MiB beyond main, nearly all of it evidence, of which published PR 63 is about 16.4 MiB. `docs/evidence/calibration/scene-reminders-2026-09-21/` has 51 files and about 35 MB of unique gzip; each `*-repeat.json.gz` is byte-identical to its `*-native.json.gz`. `.agent-work/**` was force-added on PRs 63 (15.7 MiB packed, pushed), 65 and 66. `e06c0d7` is reachable only from `feature/2026-09-21/situated-floor-calibration` (PR 63), and none of PRs 64, 65 or 66 contains it. The local PR 63 branch is eight commits ahead of GitHub (`fdcdffe`..`f3ebb4b`, unpushed); one of them, `ce9233d`, adds the cited `corrected-description-reading.md` and about 10 MB of raw archives, so the library's ADR-I-0036 path does not resolve on GitHub today. `f3ebb4b` is the frozen instrument commit of the in-flight time measurement, so those commits are pushed as they are, not rewritten. Tip `.agent-work` size (uncompressed): PR 63 about 19.3 MB, PR 65 about 25.0 MB, PR 66 about 27.6 MB.
- Cited by something durable: the register's `pr13r9ba`/`pr13r9bb`. In scene-reminders, `corrected-description-reading.md` and `after-the-fix.md`, cited by the library's ADR-I-0036 by plain path; `after-the-fix.md` is also cited by a permalink to `e06c0d7` in the library's completed `v0-2-scene-words-plan.md`. At review time these library citations sit on `measured-floors`, `scene-trace-and-docs` (published), the three `time-*` branches and the local `phase-correctness`; the set is re-scanned just before execution. Transitively: `after-the-fix.md` cites `id-order.md`, `overlap-unlived.md`, its JSON archive and `artifact-manifest.json`; `corrected-description-reading.md` cites its three repeat archives, `corrected-description-table.json` and `corrected-description-manifest.json` (six archives listed); both link the folder README, and the corrected reading links the repository README. No `.agent-work` file is cited by anything.

## Open Questions (max 3)
- Q1: Without a force-push, a default clone keeps downloading the archives as long as any stack branch exists on GitHub, because the branch history still holds them. A cleanup commit fixes only the branch tips. There is no cheaper cure that does not rewrite published history: landing the stack sooner is the only lever. Clones made before landing shrink only after `git fetch --prune` and `git gc`. Recommended and accepted by the decider's approval of the direction: clones are cured when the stack lands squashed and its branches are deleted; no force-push.
- Q2: Is the CI guard enough, even though it fires after the push, or should a versioned pre-commit hook (`.githooks/pre-commit` plus `core.hooksPath`, set up in each clone and worktree) catch force-adds before they ever reach a branch? Recommended: CI plus the worker and reviewer rules. The offenders are agents, the rules reach them directly, and a hook that each worktree must opt into fails silently.

## Assumptions
- A1: GitHub accepts and keeps a push to `refs/evidence/*`, a fresh default clone does not fetch it, an explicit fetch does, and `/commit/<hash>` and `/tree/<hash>/<path>` URLs resolve for a commit reachable only from it. Source: verified by Task_1 on 2026-09-23 (Progress Log). If it fails, the plan stops before Task_2 and reports, because tags are fetched by default and are no substitute.
- A2: The stack lands on main by squash merge, as PRs 42 to 52 did, so main's history never holds the archives. Source: main's linear `(#NN)` history.
- A3: A ref in a custom namespace is protected by no branch rule, and anyone with write access can delete it. This is accepted because nothing sealed lives only there: the tier is "recoverable", not permanent, and the manifest on main names the commit, so any copy that survives can be verified by its hash.

## Tasks

### Task_1: The evidence ref mechanism works on GitHub
- type: research
- owns:
  - refs/evidence/0000-00-00-probe (GitHub, created and deleted within the task)
- depends_on: []
- description: |
  Create a one-file orphan commit with the procedure in Design and push it to `refs/evidence/0000-00-00-probe`. Then check four things. First, whether GitHub accepts it and `git ls-remote origin 'refs/evidence/*'` lists it. Second, whether a fresh `git clone` of the repository lacks the commit. Third, whether `git fetch origin refs/evidence/0000-00-00-probe:refs/evidence/0000-00-00-probe` in that clone retrieves it. Fourth, whether the `/commit/<hash>` and `/tree/<hash>/<path>` URLs resolve in a browser. Delete the ref with `git push origin :refs/evidence/0000-00-00-probe` and remove the clone. This is the plan's one outward step.
- acceptance:
  - All four checks are recorded in the Progress Log with the commands and their output, and the probe ref is gone from `ls-remote`.
  - If any check fails, the plan stops and the failure is reported to the decider.
- validation:
  - kind: command
    required: true
    owner: orchestrator
    detail: "the push, ls-remote, fresh clone with rev-parse --verify <hash> failing, explicit fetch succeeding, deletion and ls-remote empty"
  - kind: manual
    required: true
    owner: orchestrator
    detail: "the commit and tree URLs open on github.com"

### Task_2: The rules say what is kept, where, and how it is cited
- type: docs
- owns:
  - docs/coding-agent/rules/common.md
  - docs/coding-agent/rules/worker.md
  - docs/coding-agent/rules/reviewer.md
  - .gitignore
  - .gitattributes (one line)
  - .github/workflows/ci.yml (one guard step in the format job only, after the harness checkout)
- depends_on: [Task_1]
- description: |
  Write the three storage tiers, the 256 KiB text-only ceiling, the citation form, the recovery commands and the manual ref procedure from Design. In `common.md`, the DELETE/PROMOTE rule becomes DELETE (default) or PROMOTE into a tier chosen by what cites the artifact, and the rigor sentence at line 14 names where sealed evidence lives. `worker.md`: never `git add -f` under `.agent-work`, and a promotion states its tier and its size. `reviewer.md`: a promotion's size and tier are checked against the ceiling and the citation. Sealed runs stay whole on main, so the README "Seal evidence" section and `seal` are unchanged. `.gitignore`: ignore everything under `docs/evidence/` except directories and `*.md`. `.gitattributes`: `/docs/evidence/** -text`. CI: the guard step in Design, after the harness checkout.
- acceptance:
  - The four documents state one policy with no contradiction between them, as prose that is not hard-wrapped and does not depend on a version.
  - Run locally, the guard fails on a scratch commit that force-adds one `.agent-work` file, fails on one that force-adds a `.json.gz` under `docs/evidence/`, and passes on main.
  - A raw file restored under `docs/evidence/` does not show in `git status`, and a markdown file there does.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands; the guard run locally against both force-added files and against main; the ignore check above"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D review that the rules, .gitignore and CI agree with the Design section and with ADR-I-0005"

### Task_3: The stack drops its bulk without a force-push
- type: impl
- owns:
  - refs/evidence/2026-09-21-scene-reminders (GitHub)
  - docs/evidence/calibration/scene-reminders-2026-09-21/** (on feature/2026-09-21/situated-floor-calibration)
  - .agent-work/** (removal only, on the branches of PRs 63, 65 and 66)
  - refs/evidence/2026-09-22-time-checkpoint (GitHub) and docs/evidence/calibration/time-checkpoint-2026-09-22/** (on feature/2026-09-21/situated-floor-calibration)
  - library docs/decisions/implementation/ADR-I-0036-retain-measured-retrieval-bounds-with-one-reserved-slot-per-cue-kind.md (the citation line) and docs/coding-agent/plans/completed/v0-2-scene-words-plan.md (the permalink line), on every library branch head that carries either at execution time: today feature/2026-09-22/measured-floors, feature/2026-09-21/scene-trace-and-docs, the three feature/2026-09-22/time-* branches, and feature/2026-09-22/phase-correctness (local; its worker applies the same commit, or it inherits it by merging its fixed parent)
- depends_on: [Task_1, Task_2]
- description: |
  In this order. (0) Record `git count-objects -vH` of a fresh default clone as the baseline, and re-scan every evals and library branch head for citations into the folder and for `e06c0d7`. (1) Once the time measurement releases the PR 63 worktree, build the evidence commit from the local PR 63 tip (which includes the eight unpushed commits) with the Design procedure: its tree is the whole `docs/evidence/calibration/scene-reminders-2026-09-21/` folder exactly as that commit has it, repeat names included (they share blobs with their natives), plus the repository root `README.md`, which two files in the folder link for prerequisites. Push it and record its hash. (2) On PR 63, one commit removes every `.agent-work` file and every non-markdown file in the folder, keeps all five markdown files (both cited readings, `id-order.md` and `overlap-unlived.md`, which `after-the-fix.md` cites, and the folder README), and in the same commit rewrites every link from those files to a removed file as a link to the same path under `/blob/<evidence commit>/` or `/tree/<evidence commit>/`, and turns the folder README into the manifest (the ref, the commit, the byte-preserving recovery commands, and a note that the original README's reproduction steps are in the evidence commit). Push the branch with its eight earlier commits unchanged. (3) Re-point the library citations on every branch head found in step 0: ADR-I-0036 keeps the plain main paths and replaces "compressed raw measurements beside it" with "raw measurements recoverable at `CharacterMemoryEvals@<evidence commit>`"; the scene-words plan's `e06c0d7` permalink becomes the `/tree/<evidence commit>/...` link, since a citation should not rest on GitHub's pull request refs. One commit per library branch, pushed by the orchestrator; `phase-correctness` gets it through its worker. (4) On PRs 65 and 66, one commit each removes their own `.agent-work` and anything uncited they added. PR 63 is merged forward into 64, 65 and 66 only after its cleanup, as part of straightening the stack for GitHub stack tracking (Decision Log 2026-09-23). (4a) Promote the time slice checkpoint reading (`.agent-work/worker2/task9-time-checkpoint/`, cited by the library's time plan) by the same rule: its readings (`time-d36d96d.md` and the markdown comparison) go to `docs/evidence/calibration/time-checkpoint-2026-09-22/` on PR 63 with a manifest, links to raw files re-pointed; the raw reports, audits and archives go to `refs/evidence/2026-09-22-time-checkpoint` at the same folder path, together with the readings; since these files were never committed, the tree is built in a separate index with `git hash-object -w --no-filters <file>` and `git update-index --add --cacheinfo 100644,<blob>,<path>` so no line-ending filter touches them, and the restore check compares each restored file with its original bytes; everything else in that folder is discarded. (5) After the stack lands squashed and its branches are deleted, record `git count-objects -vH` of a fresh default clone again.
- acceptance:
  - On every stack branch tip, `git ls-tree -r <tip> -- .agent-work` is empty, and nothing under `docs/evidence/` is anything but markdown.
  - The files each of the three tips adds under `docs/evidence/`, summed from `git ls-tree -r -l`, come to under 256 KiB, and all are markdown.
  - After the explicit fetch, `git rev-parse` of the fetched ref equals the manifest's commit hash, and a restore of the whole folder writes every file byte-identical to its blob in the evidence commit (compared by SHA-256 of `git cat-file blob` against the restored file), JSON tables included.
  - Every link in the five markdown files on the PR 63 tip resolves: to a file on the tip, or to a path that exists in the evidence commit.
  - The baseline clone size is recorded now; the after-landing clone size is recorded by the orchestrator once the stack lands and its branches are deleted (Q1), and it is expected to be close to main's size today. Until then clones still download the archives through the branch histories, and the plan says so.
  - Every durable citation resolves: both paths ADR-I-0036 names exist on the pushed PR 63 tip, the evidence commit URL opens, the register's `pr13r9ba`/`pr13r9bb` seal hashes still verify (`verify`), and no library branch head found in step 0 still names `e06c0d7`. A branch created later from a fixed parent inherits the fix; one created from an unfixed parent is caught by the orchestrator's re-scan at closeout.
  - No branch was force-pushed.
- validation:
  - kind: command
    required: true
    owner: worker
    detail: "the three repository validation commands on each tip; the ls-tree checks and ls-tree -l byte sums; the manifest commit check and the byte-for-byte restore check; the link walk over the five markdown files; cargo run -p cmem-eval-runner -- verify on both register folders"
  - kind: command
    required: true
    owner: orchestrator
    detail: "the baseline clone size; the evidence ref push before the PR 63 removal commit; the library citation pushes; git grep e06c0d7 over the library branches empty; the after-landing clone size (pending until the stack lands)"
  - kind: review
    required: true
    owner: reviewer
    detail: "Tier D review that everything removed was uncited or re-pointed, that each kept artifact is in the right tier, an independent link walk and restore byte check, and that the citations resolve"

### Task_4: The lesson is recorded
- type: docs
- owns:
  - docs/coding-agent/lessons.md
- depends_on: [Task_3]
- description: |
  One entry: force-adding `.agent-work` and promoting raw archives into the tree made every clone of every branch download them, because a branch's history keeps what a later commit deletes. A second prevention from the plan review: a plan that includes git plumbing cites a dry run of the exact commands. Storage follows citation: raw data goes to an evidence ref, readings go to main, and everything else is deleted. The durable default change is the Task_2 rules and the CI guard.
- acceptance:
  - The entry names the cause, the default that changed, and where it is enforced, in one short paragraph.
- validation:
  - kind: review
    required: true
    owner: orchestrator
    detail: "the entry matches the Task_2 rules and the Task_3 outcome"

## Task Waves (explicit parallel dispatch sets)

- Wave 1: [Task_1] (orchestrator; the one outward probe)
- Wave 2: [Task_2]
- Wave 3: [Task_3] (after the time measurement releases the PR 63 worktree; the evidence ref is pushed before the PR 63 removal commit), then the stack straightening 63 -> 64 -> 65 -> 66
- Wave 4: [Task_4]

Task_2 is its own pull request on main, and it may land before the stack. Task_3 adds commits to the existing stack pull requests, and the library's citation edits are commits on its existing branches. Each of these merges only with the decider's approval for that pull request.

## Rollback / Safety
- The evidence ref is added and never rewritten. To undo it, delete the ref, but only after no citation names it. The removal commits on the stack are ordinary commits and can be reverted. The library's citation commits can be reverted, but reverting them brings back a citation to `e06c0d7`, which then rests on PR 63's branch or GitHub's `refs/pull/63/head`. No step force-pushes or rewrites history. The register-sealed bytes are verified by `verify` before and after Task_3.

## Progress Log (append-only)

- 2026-09-23 Task_2 done and approved by Tier D (`cf26fc9`, follow-up `e70303f` with the additive procedure, re-check approved).
- 2026-09-23 Task_3: baseline fresh default clone 2786 packed objects, one pack, 29.43 MiB. Evidence refs pushed after orchestrator verification: `refs/evidence/2026-09-21-scene-reminders` at `784c2784260196352d7cc5184e0992260f0b1b0b` (the folder's 51 files and the root README, byte-identical to `f3ebb4b`) and `refs/evidence/2026-09-22-time-checkpoint` at `3506bc8090244e55ec6b12fef1b671ca40897fc5` (16 cited files and the root README; archives only where a lossless archive exists). Restore from a fresh clone is byte-exact for every file. Cleanup commits: PR 63 `c849911` (eight markdown files, 74,573 bytes, 46 links resolve), PR 65 `8f6f85b`, PR 66 `976664d`; no tracked `.agent-work` and no raw data under `docs/evidence` on any tip; both seals verify; nothing force-pushed. The time-checkpoint scratch (72 files, 117 MB) was discarded after publication and restore. Library citations re-pointed on every library branch and merged up its stack; no library tip names `e06c0d7` or the scratch path.

- 2026-09-23 Task_1 done (orchestrator; decider approved the direction the same day). Probe commit `e4ce1c929d99a205e4b8cb5308c90d7225310aac` (one file, no parent) built with `hash-object -w`, `mktree`, `commit-tree`, pushed as `refs/evidence/0000-00-00-probe`: GitHub accepted it (`[new reference]`) and `git ls-remote origin 'refs/evidence/*'` listed it. A fresh default `git clone` lacked the commit (`git cat-file -e` failed), and still lacked it after `git pull`. `git fetch origin refs/evidence/0000-00-00-probe:refs/evidence/0000-00-00-probe` retrieved it and `git show` printed the file. `/tree/<hash>` and `/commit/<hash>` both open on github.com; the tree page shows GitHub's banner "This commit does not belong to any branch on this repository", so the manifest says that banner is expected. Probe deleted (`git push origin :refs/evidence/0000-00-00-probe`), `ls-remote` empty, clone removed. A1 holds.
- 2026-09-23 Task_3 approved by Tier D (fresh fetch of both refs, all 69 files restored byte-exact with `core.autocrlf=true`, link walks, ceilings, seals). Stack straightened 63 -> 64 -> 65 -> 66 by plain merges (tips `c849911`, `e0a7e7a`, `7e4f942`, `323351d`) and linked as GitHub stack #55; the one instrument change from the final merge (the overlap geometry audit following the separate surfaces) gets its own instrument review. Task_4 lesson done (`610a833`), reviewed by the orchestrator against the Task_2 rules and the Task_3 outcome. Open: the after-landing clone size, pending until the stack lands squashed and its branches are deleted; the plan stays in progress until then.

## Decision Log (append-only; re-plans and major discoveries)

- 2026-09-23 Decision: evidence storage follows citation, in three tiers, as decided by the decider.
  - Trigger / new insight: the unmerged stack would add about 32 MiB, nearly all of it uncited evidence and force-added `.agent-work`, and a default clone already downloads it through `refs/heads/*`.
  - Plan delta (what changed): initial draft.
  - Tradeoffs considered: a new ADR beside ADR-I-0005 was considered and declined. The tier rule is repository policy that lives in `common.md`, and ADR-I-0005's bytes-by-hash guarantee holds wherever bytes are stored. Teaching `seal` to write the ref was declined in favour of a five-command procedure.
  - User approval: pending.
  - Record proposed: none.
- 2026-09-23 Decision: orchestrator value audit of the draft. Every task earns its place (Task_1 gates an unverified assumption; Task_2 is the durable default; Task_3 is the cleanup the request is about; Task_4 is the lesson the repository requires). OVERSIZED and trimmed: the manifest's per-file SHA-256 list, because the evidence commit hash already binds every byte and a second hash list is one more thing to keep in step. Open questions stay the decider's: Q1 (a force-push is a history rewrite) and Q2 are presented with the recommendations above.
  - User approval: pending.
  - Record proposed: none.

- 2026-09-23 Decision: Tier A plan review (CHANGES REQUESTED, nine findings) applied in full. Sealed runs stay whole on main (the ref is deletable, and `verify` needs the run files), so the `.gitignore` seal rule and the README change are gone. The ref procedure uses an index file in the git directory and builds from existing blobs (the `mktemp` form failed in a dry run). The corrected reading ADR-I-0036 cites sits in an unpushed commit; it is pushed with the stack, and the ADR's "raw measurements beside it" is reworded. The clone check is now an honest before and after size, the after one pending until landing. A CI check keeps raw data out of `docs/evidence`. PR 63 is not merged forward. The manifest drops its file list too. The probe was already done (Task_1). The Tier A lesson candidate (a plan that includes git plumbing cites a dry run of the exact commands) goes into Task_4.
  - User approval: the decider approved the direction on 2026-09-23 ("Proceed with that direction, unless problems crop up"); these changes stay inside it.
  - Record proposed: none.

- 2026-09-23 Decision: Tier D plan review (CHANGES REQUESTED, three P2s and cleanup) applied. The evidence commit carries the whole folder unedited, repeat names included (same blobs), so its internal links and recovery steps stay valid; main keeps all five markdown files and re-points their links to removed files at the evidence commit in the same commit that removes them. `docs/evidence/** -text` is added so a restore writes the stored bytes (the replay showed LF to CRLF on a JSON table), and the restore check compares bytes with the blobs. The library citation edits cover every branch head carrying them at execution time, re-scanned first. Stale scope, assumption and validation text fixed; the inventory now names its metric.
  - User approval: within the direction the decider approved on 2026-09-23.
  - Record proposed: none.

- 2026-09-23 Decision: Tier D bounded re-check closed F2 and F3; the one open part of F1 (two links to the repository root README have no target in a folder-only tree) is closed by keeping the root `README.md` in the evidence commit.
  - User approval: within the direction the decider approved on 2026-09-23.
  - Record proposed: none.

- 2026-09-23 Decision: the time slice checkpoint reading, finished the same day and cited by the library's time plan, is promoted under this plan's rule in Task_3 rather than committed under `.agent-work`. First application of the rule to a new measurement.
  - User approval: within the direction the decider approved on 2026-09-23.
  - Record proposed: none.

- 2026-09-23 Decision: the decider asked for GitHub stack tracking of the stacked PRs. Stacks are linear, so PRs 64, 65 and 66, which all sat on 63, are straightened by plain merge commits into 63 -> 64 -> 65 -> 66 after the cleanup, and the stack is linked. This supersedes the Task_3 line that PR 63 is not merged forward: after cleanup, a merge brings no new objects into any clone, since git stores each object once.
  - User approval: the decider chose straightening on 2026-09-23.
  - Record proposed: none.

## Notes
- Risks: a custom ref is invisible in GitHub's branch list and unprotected (A3), which is why nothing sealed lives only there. A future promotion might skip the manifest, so the reviewer rule checks for it. A `--mirror` clone does fetch `refs/evidence/*`; that is a deliberate full copy.
- Edge cases: the evidence commit carries the cited readings as well as the raw files, so it is a complete permalink target even if main's copy of a reading is later edited. The main copy is the living one, and the ref copy is what the citation was made against.
