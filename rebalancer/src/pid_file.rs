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
//! The guard holds an exclusive lock for its lifetime. A second start that
//! uses the same path fails while the lock is held. A crash releases the
//! lock. The next start then replaces the stale file.

use crate::error::{Error, Result};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
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

/// Write a PID file, hold its lock, and remove the file on drop.
#[derive(Debug)]
pub struct PidFileGuard {
    path: PathBuf,
    file: Option<File>,
}

impl PidFileGuard {
    /// Write `path` with the current process id and lock the file.
    ///
    /// The lock stays held until drop. A second guard for the same path
    /// fails. Drop removes the file before it releases the lock.
    pub fn create(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(Error::Audit)?;
        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(Error::Aborted(format!(
                    "PID file {} is in use by another rebalancer",
                    path.display()
                )));
            }
            Err(error) => return Err(Error::Audit(error)),
        }
        file.set_len(0).map_err(Error::Audit)?;
        writeln!(&file, "{}", std::process::id()).map_err(Error::Audit)?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        if self.file.is_none() {
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
        self.file.take();
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

        let error = PidFileGuard::create(path.clone()).unwrap_err();
        assert!(
            error.to_string().contains("in use by another rebalancer"),
            "{error}"
        );
        let _ = child.kill();
        let _ = child.wait();
        let guard = PidFileGuard::create(path.clone()).unwrap();
        assert_eq!(read_pid_file(&path).unwrap(), std::process::id());
        drop(guard);
        assert!(!pid_file_exists(&path));
    }
}
