# Time checkpoint, 2026-09-22

The [reading](time-d36d96d.md) and [44-row comparison](after-a.comparison.md) report frozen instrument `f3ebb4bf1125997281a940b9ef0252c257575b69` at library `f174f1b` and `d36d96d`, including the salient unshared-anniversary limitation.

Raw evidence is recoverable at `CharacterMemoryEvals@3506bc8090244e55ec6b12fef1b671ca40897fc5` under `refs/evidence/2026-09-22-time-checkpoint`. The [evidence tree](https://github.com/ebigunso/character-memory-evals/tree/3506bc8090244e55ec6b12fef1b671ca40897fc5/docs/evidence/calibration/time-checkpoint-2026-09-22) preserves the exact original readings, cited audits, comparison, execution record, validation evidence and lossless report archives. Its original [archive manifest](https://github.com/ebigunso/character-memory-evals/blob/3506bc8090244e55ec6b12fef1b671ca40897fc5/docs/evidence/calibration/time-checkpoint-2026-09-22/evidence/manifest.json) records compressed and original hashes; its absolute paths describe the capture location, while the archives are recovered under this folder's `evidence/` directory. The raw reports are stored only as lossless archives; repeat names share the same blobs. GitHub's not-on-a-branch banner is expected.

From the repository root:

```sh
git fetch origin refs/evidence/2026-09-22-time-checkpoint:refs/evidence/2026-09-22-time-checkpoint
git rev-parse refs/evidence/2026-09-22-time-checkpoint
git -c core.autocrlf=false restore --source=refs/evidence/2026-09-22-time-checkpoint --worktree -- docs/evidence/calibration/time-checkpoint-2026-09-22
```

The resolved hash must equal `3506bc8090244e55ec6b12fef1b671ca40897fc5`. Its tree lists every path and the commit binds every byte. The restore also restores the original readings; use a disposable checkout to keep the living copies unchanged. The explicit `core.autocrlf=false` preserves bytes even before the `/docs/evidence/** -text` rule. Cited data is recoverable; uncited scratch is discarded. This measurement is unsealed.
