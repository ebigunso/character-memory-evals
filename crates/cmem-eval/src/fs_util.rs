use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const PERSIST_ATTEMPTS: usize = 4;
const PERSIST_BACKOFF_MS: u64 = 25;

pub fn atomic_replace(path: &Path, bytes: &[u8], artifact: &str) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create {artifact} directory {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary {artifact} beside {}", path.display()))?;
    temporary
        .write_all(bytes)
        .with_context(|| format!("write temporary {artifact} for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync temporary {artifact} for {}", path.display()))?;
    for attempt in 1..=PERSIST_ATTEMPTS {
        match temporary.persist(path) {
            Ok(_) => return Ok(()),
            Err(error) => {
                let retryable = error.error.kind() == std::io::ErrorKind::PermissionDenied
                    && attempt < PERSIST_ATTEMPTS;
                if !retryable {
                    return Err(error.error).with_context(|| {
                        format!("atomically replace {artifact} {}", path.display())
                    });
                }
                temporary = error.file;
                std::thread::sleep(Duration::from_millis(PERSIST_BACKOFF_MS * attempt as u64));
            }
        }
    }
    unreachable!("atomic replacement loop always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn failed_store_write_preserves_preexisting_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("store.json");
        fs::create_dir(&path).unwrap();
        let sentinel = path.join("previous.json");
        let previous_bytes = b"previous complete store\n";
        fs::write(&sentinel, previous_bytes).unwrap();

        // A directory cannot be atomically replaced with a file.
        let error = atomic_replace(
            &path,
            b"replacement complete store\n",
            "frozen embedding store",
        )
        .unwrap_err();

        assert!(error.to_string().contains("atomically replace"), "{error}");
        assert_eq!(fs::read(&sentinel).unwrap(), previous_bytes);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
