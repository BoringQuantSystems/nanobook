//! PID file management for the kill switch.
//!
//! The PID file stores the process ID of a running rebalancer instance.
//! This allows the --kill subcommand to locate and signal the running process.
//!
//! # PID File Format
//!
//! The PID file is a plain text file containing a single integer: the process ID
//! of the running rebalancer. For example:
//!
//! ```text
//! 12345
//! ```
//!
//! # Default Location
//!
//! By default, the PID file is located at `rebalancer.pid` in the working directory.
//! This can be customized by changing the `DEFAULT_PID_FILE` constant.
//!
//! # Lifecycle
//!
//! The PID file should be:
//! 1. Written when the rebalancer starts (before any orders are submitted)
//! 2. Removed when the rebalancer exits cleanly (after all orders are filled/cancelled)
//! 3. Used by the --kill subcommand to locate and signal the running process
//!
//! # Cleanup
//!
//! The guard holds an exclusive lock on `<pid file>.lock` for its lifetime.
//! A second start that uses the same path fails while the lock is held. A
//! crash releases the lock. The next start then replaces the stale PID file.

use crate::error::{Error, Result};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

/// Default PID file path.
pub const DEFAULT_PID_FILE: &str = "rebalancer.pid";

/// Write the current process ID to a PID file.
///
/// # Errors
///
/// Returns an error if the file cannot be written or if the PID cannot be obtained.
pub fn write_pid_file(path: &Path) -> Result<()> {
    let pid = std::process::id();
    fs::write(path, pid.to_string()).map_err(Error::Audit)?;
    Ok(())
}

/// Read the PID from a PID file.
///
/// # Errors
///
/// Returns an error if the file does not exist, cannot be read, or does not contain a valid PID.
pub fn read_pid_file(path: &Path) -> Result<u32> {
    let contents = fs::read_to_string(path).map_err(Error::Audit)?;
    let pid: u32 = contents
        .trim()
        .parse()
        .map_err(|_| Error::Aborted(format!("Invalid PID in file: {}", contents)))?;
    Ok(pid)
}

/// Check if a PID file exists.
pub fn pid_file_exists(path: &Path) -> bool {
    path.exists()
}

/// Remove the PID file.
///
/// # Errors
///
/// Returns an error if the file cannot be removed.
pub fn remove_pid_file(path: &Path) -> Result<()> {
    fs::remove_file(path).map_err(Error::Audit)?;
    Ok(())
}

/// The lock file that guards `pid_path`: the same path with `.lock` appended.
///
/// The lock sits on a separate file because a Windows lock also blocks reads,
/// and `--kill` must read the PID file while the rebalancer runs.
pub fn lock_path(pid_path: &Path) -> PathBuf {
    let mut name = pid_path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Write a PID file, hold the lock on its `.lock` file, and remove the PID file on drop.
#[derive(Debug)]
pub struct PidFileGuard {
    path: PathBuf,
    lock: Option<File>,
}

impl PidFileGuard {
    /// Lock `<path>.lock`, then write `path` with the current process id.
    ///
    /// The lock stays held until drop. A second guard for the same path
    /// fails. Drop removes the PID file before it releases the lock. The
    /// empty lock file stays on disk.
    pub fn create(path: PathBuf) -> Result<Self> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path(&path))
            .map_err(Error::Audit)?;
        if let Err(error) = lock.try_lock_exclusive() {
            return Err(lock_error(error, &path));
        }
        write_pid_file(&path)?;
        Ok(Self {
            path,
            lock: Some(lock),
        })
    }
}

/// The error `PidFileGuard::create` returns when locking `<path>.lock` fails:
/// a held lock is `Aborted`, any other failure is `Audit`.
fn lock_error(error: std::io::Error, path: &Path) -> Error {
    // Unix reports a held lock as WouldBlock; Windows as ERROR_LOCK_VIOLATION (33).
    if error.kind() == std::io::ErrorKind::WouldBlock
        || error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
    {
        Error::Aborted(format!(
            "PID file {} is in use by another rebalancer",
            path.display()
        ))
    } else {
        Error::Audit(error)
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        if self.lock.is_none() {
            return;
        }
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %error,
                    "failed to remove PID file"
                );
            }
        }
        self.lock.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_and_read_pid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        write_pid_file(path).unwrap();
        assert!(pid_file_exists(path));

        let pid = read_pid_file(path).unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn test_pid_file_exists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.pid");

        assert!(!pid_file_exists(&path));
        write_pid_file(&path).unwrap();
        assert!(pid_file_exists(&path));
    }

    #[test]
    fn test_remove_pid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        write_pid_file(path).unwrap();
        assert!(pid_file_exists(path));

        remove_pid_file(path).unwrap();
        assert!(!pid_file_exists(path));
    }

    #[test]
    fn test_read_invalid_pid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        fs::write(path, "not-a-number").unwrap();
        let result = read_pid_file(path);
        assert!(result.is_err());
    }

    #[test]
    fn a_second_guard_cannot_replace_a_live_pid_file() {
        if std::env::var_os("NANOBOOK_PID_HOLDER").is_some() {
            let path = PathBuf::from(std::env::var("NANOBOOK_PID_PATH").unwrap());
            let guard = PidFileGuard::create(path.clone()).unwrap();
            fs::write(path.with_extension("ready"), b"1").unwrap();
            std::thread::sleep(std::time::Duration::from_secs(30));
            drop(guard);
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rebalancer.pid");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("pid_file::tests::a_second_guard_cannot_replace_a_live_pid_file")
            .arg("--exact")
            .env("NANOBOOK_PID_HOLDER", "1")
            .env("NANOBOOK_PID_PATH", &path)
            .spawn()
            .unwrap();
        let ready = path.with_extension("ready");
        let started = std::time::Instant::now();
        while !ready.exists() {
            if started.elapsed() > std::time::Duration::from_secs(10) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("holder did not lock the PID file");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        // --kill reads the PID file while the holder runs.
        assert_eq!(read_pid_file(&path).unwrap(), child.id());
        let error = PidFileGuard::create(path.clone()).unwrap_err();
        assert!(
            error.to_string().contains("in use by another rebalancer"),
            "{error}"
        );
        let _ = child.kill();
        let _ = child.wait();
        // Windows may release a killed process's lock a moment after the process exits.
        let started = std::time::Instant::now();
        let guard = loop {
            match PidFileGuard::create(path.clone()) {
                Ok(guard) => break guard,
                Err(_) if started.elapsed() < std::time::Duration::from_secs(5) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(error) => panic!("lock not released after the holder exited: {error}"),
            }
        };
        assert_eq!(read_pid_file(&path).unwrap(), std::process::id());
        drop(guard);
        assert!(!pid_file_exists(&path));
    }
}
