use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

fn file_name(name: &str) -> Result<()> {
    ensure!(
        !name.contains(['/', '\\', ':'])
            && matches!(
                Path::new(name).components().next(),
                Some(Component::Normal(_))
            ),
        "expected one filename component: {name:?}"
    );
    Ok(())
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    ensure!(
        metadata.is_file(),
        "expected a regular file: {}",
        path.display()
    );
    fs::read(path).with_context(|| format!("read {}", path.display()))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::File::create_new(path)
        .with_context(|| format!("create {} without overwrite", path.display()))?
        .write_all(bytes)
        .with_context(|| format!("write {}; partial file kept for inspection", path.display()))
}

pub(crate) fn seal(run_dir: &Path, evidence_root: &Path) -> Result<PathBuf> {
    let mut jsonl = fs::read_dir(run_dir)
        .with_context(|| format!("read run directory {}", run_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"));
    let artifact = jsonl
        .next()
        .context("run must contain one .jsonl artifact")?;
    ensure!(
        jsonl.next().is_none(),
        "run must contain only one .jsonl artifact"
    );
    let artifact_name = artifact
        .file_name()
        .into_string()
        .map_err(|_| anyhow::anyhow!("artifact filename must be UTF-8"))?;
    file_name(&artifact_name)?;
    let mut artifacts = BTreeMap::new();
    for name in [&*artifact_name, "header.json", "report.json"] {
        artifacts.insert(name, read_file(&run_dir.join(name))?);
    }
    let header: serde_json::Value =
        serde_json::from_slice(&artifacts["header.json"]).context("parse header.json")?;
    let run_id = header
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .context("header.json must be an object with a string run_id")?;
    file_name(run_id)?;
    let hashes: BTreeMap<_, _> = artifacts
        .iter()
        .map(|(name, bytes)| (*name, sha256(bytes)))
        .collect();
    let mut seal_bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "header": header,
        "files": hashes,
        "sealed_at": chrono::Utc::now(),
    }))?;
    seal_bytes.push(b'\n');

    fs::create_dir_all(evidence_root)?;
    ensure!(
        !fs::symlink_metadata(evidence_root)?
            .file_type()
            .is_symlink(),
        "evidence root must not be a link"
    );
    let destination = evidence_root.join(run_id);
    fs::create_dir(&destination).with_context(|| {
        format!(
            "create {}; sealed evidence must never be overwritten",
            destination.display()
        )
    })?;
    let mut written = Vec::new();
    let result = (|| -> Result<()> {
        for (name, bytes) in &artifacts {
            let path = destination.join(name);
            write_new(&path, bytes)?;
            written.push(path.display().to_string());
        }
        for path in [run_dir.join("seal.json"), destination.join("seal.json")] {
            write_new(&path, &seal_bytes)?;
            written.push(path.display().to_string());
        }
        Ok(())
    })();
    result.with_context(|| format!(
        "seal incomplete at {}; written files: [{}]; failed operation follows; all files kept for inspection",
        destination.display(), written.join(", ")
    ))?;
    Ok(destination)
}

pub(crate) fn verify(evidence_dir: &Path) -> Result<()> {
    #[derive(Deserialize)]
    struct Manifest {
        files: BTreeMap<String, String>,
    }
    let manifest: Manifest = serde_json::from_slice(&read_file(&evidence_dir.join("seal.json"))?)
        .context("parse seal.json")?;
    ensure!(!manifest.files.is_empty(), "seal contains no file hashes");
    let mut failures = Vec::new();
    for (name, expected) in manifest.files {
        file_name(&name)?;
        ensure!(name != "seal.json", "seal must not hash itself");
        match read_file(&evidence_dir.join(&name)) {
            Ok(bytes) => {
                let actual = sha256(&bytes);
                if actual != expected.to_ascii_lowercase() {
                    failures.push(format!("{name}: expected {expected}, got {actual}"));
                }
            }
            Err(error) => failures.push(format!("{name}: {error:#}")),
        }
    }
    if !failures.is_empty() {
        bail!("verification failed:\n{}", failures.join("\n"));
    }
    println!("verified {}", evidence_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> tempfile::TempDir {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker");
        fs::create_dir_all(&root).unwrap();
        tempfile::tempdir_in(root).unwrap()
    }

    #[test]
    fn seals_exact_bytes_and_detects_changes_without_overwriting() {
        let directory = tempdir();
        let run = directory.path().join("run");
        fs::create_dir(&run).unwrap();
        for (name, bytes) in [
            ("traces.jsonl", "{\"metric\":1}\n"),
            (
                "header.json",
                "{\r\n  \"run_id\": \"example\", \"extra\": null\r\n}\r\n",
            ),
            ("report.json", "{\"count\":1}\n"),
        ] {
            fs::write(run.join(name), bytes).unwrap();
        }
        let evidence_root = directory.path().join("evidence");
        let original_report = fs::read(run.join("report.json")).unwrap();
        assert!(write_new(&run.join("report.json"), b"replacement").is_err());
        assert_eq!(fs::read(run.join("report.json")).unwrap(), original_report);
        let evidence = seal(&run, &evidence_root).unwrap();
        for name in ["traces.jsonl", "header.json", "report.json", "seal.json"] {
            assert_eq!(
                fs::read(run.join(name)).unwrap(),
                fs::read(evidence.join(name)).unwrap()
            );
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(evidence.join("seal.json")).unwrap()).unwrap();
        assert_eq!(
            manifest["header"],
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(run.join("header.json")).unwrap()
            )
            .unwrap()
        );
        verify(&evidence).unwrap();
        assert!(seal(&run, &evidence_root).is_err());
        let other_root = directory.path().join("other-evidence");
        let error = format!("{:#}", seal(&run, &other_root).unwrap_err());
        assert!(error.contains("written files:"));
        assert!(error.contains("traces.jsonl"));
        assert!(error.contains(&format!("create {}", run.join("seal.json").display())));
        assert_eq!(fs::read_dir(other_root.join("example")).unwrap().count(), 3);
        for name in ["traces.jsonl", "header.json", "report.json"] {
            assert_eq!(
                fs::read(other_root.join("example").join(name)).unwrap(),
                fs::read(run.join(name)).unwrap()
            );
        }
        assert_eq!(
            fs::read(run.join("seal.json")).unwrap(),
            fs::read(evidence.join("seal.json")).unwrap()
        );
        fs::write(evidence.join("traces.jsonl"), "changed").unwrap();
        fs::remove_file(evidence.join("report.json")).unwrap();
        let error = verify(&evidence).unwrap_err().to_string();
        assert!(error.contains("traces.jsonl: expected"));
        assert!(error.contains("report.json: inspect"));
    }

    #[test]
    fn verifies_promotion_hashes_without_parsing_historical_artifacts() {
        let directory = tempdir();
        fs::write(directory.path().join("old.json"), [0xff, 0x00]).unwrap();
        let mut manifest = serde_json::json!({"files": {"old.json": sha256(&[0xff, 0x00])}, "promotion": "Historical bytes"});
        fs::write(
            directory.path().join("seal.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        verify(directory.path()).unwrap();
        for name in [
            "",
            "..",
            "../outside",
            "nested/file",
            "nested\\file",
            "C:outside",
            "seal.json",
        ] {
            manifest["files"] = serde_json::json!({name: sha256(&[])});
            fs::write(
                directory.path().join("seal.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            let error = verify(directory.path()).unwrap_err().to_string();
            assert!(
                error.contains("filename component") || error.contains("hash itself"),
                "{name}: {error}"
            );
        }
    }
}
