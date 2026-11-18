//! Closing a `Commerce` and releasing its database file.

use super::Commerce;

use stateset_core::CommerceError;
use std::time::Duration;

/// How long [`Commerce::close`] waits for the SQLite file to be released.
/// Matches the pool's connection timeout, the longest a background
/// connection attempt can take.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(30);

impl Commerce {
    /// Close this instance and wait until its SQLite file is released.
    ///
    /// Dropping a `Commerce` closes its connection pool, but the file can
    /// stay open for a little longer: the pool grows lazily under concurrent
    /// load, a connection it is still opening in the background keeps the
    /// pool alive, and the pool's last connection closes only when that
    /// attempt finishes. `close` waits for it. When it returns `Ok`, every
    /// connection this instance opened has closed, so a WAL database has been
    /// checkpointed and its `-wal`/`-shm` files are gone, and the file can be
    /// copied, moved or reopened straight away.
    ///
    /// Anything still holding a connection from this instance delays the
    /// release: a repository or kernel executor taken from it, or a
    /// connection from [`Commerce::open_auxiliary_sqlite_connection`]. Drop
    /// those first. Non-SQLite backends have no file to wait for; `close`
    /// drops them and returns `Ok`.
    ///
    /// # Errors
    ///
    /// Returns [`CommerceError::DatabaseError`] if connections from this
    /// instance are still open 30 seconds after the drop. The instance is
    /// closed either way.
    pub fn close(self) -> Result<(), CommerceError> {
        self.close_within(CLOSE_TIMEOUT)
    }

    /// [`Commerce::close`] with a chosen wait, so tests need not wait 30s.
    pub(crate) fn close_within(self, timeout: Duration) -> Result<(), CommerceError> {
        #[cfg(feature = "sqlite")]
        let release = self.sqlite_file_release();
        drop(self);
        #[cfg(feature = "sqlite")]
        if let Some(release) = release {
            if !release.wait_for(timeout) {
                return Err(CommerceError::DatabaseError(format!(
                    "the SQLite file is still in use {timeout:?} after close: drop repositories, \
                     kernel executors and auxiliary connections taken from this Commerce first"
                )));
            }
        }
        #[cfg(not(feature = "sqlite"))]
        let _ = timeout;
        Ok(())
    }

    /// A watch that fires once every connection this instance opened has
    /// closed, so its SQLite file is released. `None` for a backend with no
    /// SQLite file. Take it before dropping the instance; see
    /// [`stateset_db::sqlite::SqliteFileRelease`].
    #[cfg(feature = "sqlite")]
    #[must_use]
    pub fn sqlite_file_release(&self) -> Option<stateset_db::sqlite::SqliteFileRelease> {
        self.sqlite_db.as_ref().map(|db| db.file_release())
    }
}
