# Scene reminder calibration, 2026-09-21

This directory retains the unsealed scene-reminder calibration captured on 2026-09-21 and promoted on 2026-09-22 for library-record citations. The measurements compare library `979643f` with `63f176f` using harness `97f63266b37722627f06a6a8c9ceecbfb3be7d81`. Each pin ran 515 conditions twice with byte-identical reports. The controlled vectors test retrieval behavior; they do not establish a production similarity threshold.

## Readings

- [Final before/after reading](after-the-fix.md): default results, floor sweeps, intermediate `b470b10`, distributions, and validation limitations. Its complete machine-readable comparison is [after-the-fix.json.gz](after-the-fix.json.gz).
- [Native-ID order control](id-order.md): both identity orders, recency expectations, and topic-score ties.
- [Unlived descriptions in populated stores](overlap-unlived.md): stranger/place probes, native occasions, cue membership, and counterfactual displacement.

The [manifest](artifact-manifest.json) records the SHA-256 of each exact JSON and compressed archive. The BEFORE pair is [before-the-fix-native.json.gz](before-the-fix-native.json.gz) and [before-the-fix-native-repeat.json.gz](before-the-fix-native-repeat.json.gz); both decompress to SHA-256 `2a89d0c1b062d6f321f6a8051c42741f220382da46b3dc2485e12f18ce5f0bad`. The AFTER pair is [after-the-fix-native.json.gz](after-the-fix-native.json.gz) and [after-the-fix-native-repeat.json.gz](after-the-fix-native-repeat.json.gz); both decompress to SHA-256 `294d4f09788709497a2cb178cf5b155bff27a3d9107fe10d79d4bf37b7296a51`. These hashes verify retained bytes; no evidence was sealed or added to the findings register.

The `fourth-pin-before-p2` and `fourth-pin-after` archives retain the approved earlier BEFORE capture and the `b470b10` intermediate capture at harness `8f49ddb`. New ID-order and unlived-description probes were not run at the intermediate pin. The [integrity audit](after-the-fix-audit.json) checks the common 270 original rows and six protected fixture hashes. The dated [worker report](Task_9-after-the-fix-report.yaml), [workspace log](final-workspace.log), and [owner disposition](owner-disposition.txt) retain the two deferred embedding-surface test migrations and the separate Windows OS1314 waiver. Independent review status in these historical artifacts is as recorded on 2026-09-21.

## Restore and verify the exact JSON

Run from the repository root with Python 3. This verifies archive and uncompressed hashes, restores all JSON into a new scratch directory, and checks the exact repeat aliases without altering the tracked evidence.

```python
import gzip
import hashlib
import json
from pathlib import Path

evidence = Path("docs/evidence/calibration/scene-reminders-2026-09-21")
scratch = Path(".agent-work/calibration/scene-reminders-restored")
scratch.mkdir(parents=True, exist_ok=False)
manifest = json.loads((evidence / "artifact-manifest.json").read_text(encoding="utf-8"))
for entry in manifest["json_archives"]:
    archive = (evidence / entry["archive"]).read_bytes()
    assert hashlib.sha256(archive).hexdigest() == entry["archive_sha256"]
    raw = gzip.decompress(archive)
    assert hashlib.sha256(raw).hexdigest() == entry["sha256"]
    (scratch / entry["path"]).write_bytes(raw)
    repeat = entry.get("byte_identical_repeat_alias")
    if repeat:
        repeat_archive = evidence / (repeat + ".gz")
        if repeat_archive.exists():
            assert gzip.decompress(repeat_archive.read_bytes()) == raw
        (scratch / repeat).write_bytes(raw)
```

## Reproduce the measurements

Use an isolated harness checkout at `97f63266b37722627f06a6a8c9ceecbfb3be7d81`, with an isolated sibling named `CharacterMemory` because the Cargo dependency resolves `../CharacterMemory`. Do not repin a shared checkout held by another task. Use the Rust/toolchain and embedded-store prerequisites documented in the [repository README](../../../../README.md).

1. Pin the isolated library to BEFORE `979643f662013a839f428451209cac70c3b42062`. Create a new `.agent-work/calibration` output directory, then run the commands below with `before-a.json` and `before-b.json`.
2. Pin the isolated library to AFTER `63f176fa6801a8268bb00e337267a0f8c334a681`. Repeat with `after-a.json` and `after-b.json`. Use `cargo run` after each pin change so the linked library rebuilds.
3. Compare each pair byte-for-byte, and compare each first run with its restored retained JSON. The runner refuses existing output/store paths and cleans its embedded stores. No paid calls or protected fixture changes are required.

```sh
cargo run --offline --release -p cmem-eval-continuity --bin calibrate_cue_floors -- .agent-work/calibration/before-a.json
cargo run --offline --release -p cmem-eval-continuity --bin calibrate_cue_floors -- .agent-work/calibration/before-b.json
git diff --no-index -- .agent-work/calibration/before-a.json .agent-work/calibration/before-b.json
```

The archived JSON records full inputs, source/configuration hashes, both revisions, native limits, and profile. A different harness revision changes the header even if the calibration source is unchanged. Exact retained files remain the historical evidence; reruns produce new files. Paths embedded in the archived JSON describe the original capture layout; the manifest maps each retained file to this directory. The original [comparison builder](build-fourth-after.py) and audit scripts are retained alongside their logs. To regenerate the comparison, restore the JSON and declared repeat aliases beside a scratch copy of the builder, both audit scripts, `fourth-protected-hashes.json`, and the matching repeat logs; invoke `audit-id-order.py` once per final native file, then `audit-after-comparison.py`, then `build-fourth-after.py before-the-fix-native.json after-the-fix-native.json after-the-fix` from that scratch directory. The audit uses repository-relative protected-file paths, so keep its scratch directory at `.agent-work/<role>/<task>/`, as in the original capture. The readable promotion adds dated context and navigation; it does not alter measured values.
