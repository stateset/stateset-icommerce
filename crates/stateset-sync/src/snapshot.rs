//! Atomic snapshot replacement and explicit cross-file durability barriers.

use std::{fs, io::Write, path::Path};

use serde::Serialize;

use crate::SyncError;

fn parent(path: &Path) -> &Path {
    path.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."))
}

/// Errors occur before replacement, so callers can safely restore old memory.
pub(crate) fn write(path: &Path, value: &impl Serialize) -> Result<(), SyncError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::create_dir_all(parent(path)).map_err(|error| {
        SyncError::Storage(format!("create snapshot directory failed: {error}"))
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent(path))
        .map_err(|error| SyncError::Storage(format!("create snapshot failed: {error}")))?;
    temporary
        .write_all(&bytes)
        .map_err(|error| SyncError::Storage(format!("write snapshot failed: {error}")))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| SyncError::Storage(format!("sync snapshot failed: {error}")))?;
    temporary
        .persist(path)
        .map_err(|error| SyncError::Storage(format!("replace snapshot failed: {error}")))?;
    Ok(())
}

/// A post-replacement barrier, kept separate because failure cannot undo rename.
pub(crate) fn sync_parent(path: &Path) -> Result<(), SyncError> {
    #[cfg(unix)]
    fs::File::open(parent(path))
        .and_then(|directory| directory.sync_all())
        .map_err(|error| SyncError::Storage(format!("sync snapshot directory failed: {error}")))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
