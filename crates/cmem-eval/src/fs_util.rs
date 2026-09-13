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
