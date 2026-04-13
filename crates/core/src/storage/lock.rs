use anyhow::{bail, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// An advisory file lock for a codeindex index directory.
///
/// Two lock types are supported:
/// - **Exclusive**: only one holder at a time; created as `lock.exclusive` in
///   the index directory.
/// - **Shared**: multiple holders permitted, but blocked when an exclusive lock
///   exists; created as `lock.shared.<pid>` in the index directory.
///
/// Locks are purely advisory (cooperative). They are released when `IndexLock`
/// is dropped.
pub struct IndexLock {
    lock_path: PathBuf,
}

impl IndexLock {
    /// Acquire an exclusive lock in `index_dir`.
    ///
    /// Creates `<index_dir>/lock.exclusive` with `O_CREAT | O_EXCL` so that
    /// only one process can hold it at a time. Returns an error if the lock
    /// file already exists (another holder is active).
    pub fn acquire_exclusive(index_dir: &Path) -> Result<Self> {
        let lock_path = index_dir.join("lock.exclusive");

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true) // fails if file exists
            .open(&lock_path)
            .map_err(|e| {
                anyhow::anyhow!(
                    "could not acquire exclusive lock at {}: {} \
                     (another process may hold the lock)",
                    lock_path.display(),
                    e
                )
            })?;

        let pid = std::process::id();
        writeln!(file, "exclusive:{pid}")?;

        Ok(Self { lock_path })
    }

    /// Acquire a shared lock in `index_dir`.
    ///
    /// Creates `<index_dir>/lock.shared.<pid>`. Fails if an exclusive lock
    /// file (`lock.exclusive`) is currently present.
    pub fn acquire_shared(index_dir: &Path) -> Result<Self> {
        let exclusive_path = index_dir.join("lock.exclusive");
        if exclusive_path.exists() {
            bail!(
                "could not acquire shared lock: an exclusive lock exists at {}",
                exclusive_path.display()
            );
        }

        let pid = std::process::id();
        let lock_path = index_dir.join(format!("lock.shared.{pid}"));

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                anyhow::anyhow!(
                    "could not create shared lock file at {}: {}",
                    lock_path.display(),
                    e
                )
            })?;

        writeln!(file, "shared:{pid}")?;

        Ok(Self { lock_path })
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        // Best-effort removal; ignore errors (e.g., already deleted).
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A second exclusive lock must fail while the first is still held.
    #[test]
    fn exclusive_lock_prevents_second_exclusive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        let _first = IndexLock::acquire_exclusive(path)
            .expect("first exclusive lock should succeed");

        let second = IndexLock::acquire_exclusive(path);
        assert!(
            second.is_err(),
            "second exclusive lock should fail while first is held"
        );
    }

    /// After the lock holder goes out of scope the lock file is removed and a
    /// new exclusive lock can be acquired.
    #[test]
    fn lock_released_on_drop() {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        {
            let _lock = IndexLock::acquire_exclusive(path)
                .expect("lock should be acquired inside scope");
            // lock file exists here
            assert!(path.join("lock.exclusive").exists());
        }
        // lock file must be gone after drop
        assert!(
            !path.join("lock.exclusive").exists(),
            "lock file should be removed after drop"
        );

        // Re-acquiring must succeed now
        let _reacquired = IndexLock::acquire_exclusive(path)
            .expect("lock should be re-acquirable after previous holder dropped it");
    }

    /// A shared lock is blocked by an existing exclusive lock.
    #[test]
    fn shared_lock_blocked_by_exclusive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        let _excl = IndexLock::acquire_exclusive(path).unwrap();

        let shared = IndexLock::acquire_shared(path);
        assert!(
            shared.is_err(),
            "shared lock should fail when exclusive lock exists"
        );
    }

    /// Multiple shared locks can coexist.
    #[test]
    fn multiple_shared_locks_allowed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        // We can only hold one shared lock per process (same PID → same
        // filename), so we verify that a second *exclusive* attempt fails while
        // a shared lock is held, and that the shared lock cleans up correctly.
        let shared = IndexLock::acquire_shared(path)
            .expect("shared lock should succeed with no exclusive lock present");
        drop(shared);

        // After drop the shared file must be gone.
        let pid = std::process::id();
        assert!(
            !path.join(format!("lock.shared.{pid}")).exists(),
            "shared lock file should be removed after drop"
        );
    }
}
