use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const PERSIST_ATTEMPTS: usize = 4;
const PERSIST_BACKOFF_MS: u64 = 25;

pub fn atomic_replace(path: &Path, bytes: &[u8], artifact: &str) -> Result<()> {
    atomic_replace_with_before_persist(path, bytes, artifact, |_| Ok(()))
}

pub fn atomic_replace_with_before_persist<F>(
    path: &Path,
    bytes: &[u8],
    artifact: &str,
    before_persist: F,
) -> Result<()>
where
    F: FnOnce(&Path) -> Result<()>,
{
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
    before_persist(temporary.path())?;
    persist_with_retry(temporary, path, artifact, |temporary, path| {
        temporary.persist(path)
    })
}

pub fn persist_with_retry<F>(
    mut temporary: tempfile::NamedTempFile,
    path: &Path,
    artifact: &str,
    mut persist: F,
) -> Result<()>
where
    F: FnMut(
        tempfile::NamedTempFile,
        &Path,
    ) -> std::result::Result<std::fs::File, tempfile::PersistError>,
{
    for attempt in 1..=PERSIST_ATTEMPTS {
        match persist(temporary, path) {
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
    use crate::fs_util;
    use anyhow::bail;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn identity_registry_persist_retries_permission_denied_with_same_staged_bytes() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("identity.json");
        fs::write(&path, b"old complete registry\n").unwrap();
        let staged_bytes = b"new complete registry\n";
        let mut temporary = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        temporary.write_all(staged_bytes).unwrap();
        temporary.as_file().sync_all().unwrap();
        let mut attempts = 0;

        fs_util::persist_with_retry(temporary, &path, "identity registry", |temporary, path| {
            attempts += 1;
            assert_eq!(fs::read(temporary.path()).unwrap(), staged_bytes);
            if attempts == 1 {
                return Err(tempfile::PersistError {
                    error: std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected Windows replace contention",
                    ),
                    file: temporary,
                });
            }
            temporary.persist(path)
        })
        .unwrap();

        assert_eq!(attempts, 2);
        assert_eq!(fs::read(&path).unwrap(), staged_bytes);
    }

    #[test]
    fn failed_store_write_preserves_preexisting_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("store.json");
        let previous_bytes = b"previous complete store\n";
        let replacement_bytes = b"replacement complete store\n";
        fs::write(&path, previous_bytes).unwrap();
        let mut staged_path = None;

        let error = atomic_replace_with_before_persist(
            &path,
            replacement_bytes,
            "frozen embedding store",
            |temporary_path| {
                staged_path = Some(temporary_path.to_path_buf());
                assert_eq!(temporary_path.parent(), path.parent());
                assert_eq!(fs::read(temporary_path).unwrap(), replacement_bytes);
                assert_eq!(fs::read(&path).unwrap(), previous_bytes);
                bail!("simulated failure before atomic store replacement")
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("simulated failure"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), previous_bytes);
        assert!(!staged_path.unwrap().exists());
    }

    #[test]
    fn store_persist_retries_permission_denied_with_same_staged_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("store.json");
        fs::write(&path, b"previous complete store\n").unwrap();
        let staged_bytes = b"replacement complete store\n";
        let mut temporary = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        temporary.write_all(staged_bytes).unwrap();
        temporary.as_file().sync_all().unwrap();
        let mut attempts = 0;

        persist_with_retry(
            temporary,
            &path,
            "frozen embedding store",
            |temporary, path| {
                attempts += 1;
                assert_eq!(fs::read(temporary.path()).unwrap(), staged_bytes);
                if attempts == 1 {
                    return Err(tempfile::PersistError {
                        error: std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "injected Windows replace contention",
                        ),
                        file: temporary,
                    });
                }
                temporary.persist(path)
            },
        )
        .unwrap();

        assert_eq!(attempts, 2);
        assert_eq!(fs::read(&path).unwrap(), staged_bytes);
    }
}
