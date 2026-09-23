# Scene reminder calibration, 2026-09-21

Use the [corrected description reading](corrected-description-reading.md) for the current interpretation. The [historical reading](after-the-fix.md), [native-ID order control](id-order.md) and [unlived-description reading](overlap-unlived.md) remain for citation history; their original measured values are unchanged.

Raw evidence is recoverable at `CharacterMemoryEvals@784c2784260196352d7cc5184e0992260f0b1b0b` under `refs/evidence/2026-09-21-scene-reminders`. The [evidence tree](https://github.com/ebigunso/character-memory-evals/tree/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21) preserves the complete original folder, including repeat names, and the repository README. The original [folder README](https://github.com/ebigunso/character-memory-evals/blob/784c2784260196352d7cc5184e0992260f0b1b0b/docs/evidence/calibration/scene-reminders-2026-09-21/README.md) contains reproduction and archive-verification instructions. GitHub may show that the commit does not belong to a branch; this is expected.

From the repository root, fetch this evidence explicitly and restore its working-tree paths without line-ending conversion:

```sh
git fetch origin refs/evidence/2026-09-21-scene-reminders:refs/evidence/2026-09-21-scene-reminders
git rev-parse refs/evidence/2026-09-21-scene-reminders
git -c core.autocrlf=false restore --source=refs/evidence/2026-09-21-scene-reminders --worktree -- docs/evidence/calibration/scene-reminders-2026-09-21
```

The resolved hash must equal `784c2784260196352d7cc5184e0992260f0b1b0b`. The commit binds every stored byte and its tree lists every path. The restore also restores the original readings; run it in a disposable checkout to keep this living reading and manifest unchanged. With the current `/docs/evidence/** -text` attribute, plain `git restore` also preserves the bytes; the explicit setting above works on older checkouts. This is recoverable, unsealed evidence; register-cited sealed runs stay whole under `evidence/`.
