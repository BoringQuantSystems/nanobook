//! Mutation-gap tests for `PidFileGuard`: dropping the guard warns when the
//! PID file cannot be removed, and stays quiet when it is already gone.

mod common;

use common::LogCapture;
use nanobook_rebalancer::pid_file::PidFileGuard;

const WARNING: &str = "failed to remove PID file";

#[test]
fn drop_is_quiet_when_the_pid_file_is_already_gone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rebalancer.pid");
    let guard = PidFileGuard::create(path.clone()).unwrap();
    std::fs::remove_file(&path).unwrap();

    let capture = LogCapture::default();
    capture.run(|| drop(guard));

    let text = capture.text();
    assert!(!text.contains(WARNING), "log was: {text}");
}

#[test]
fn drop_warns_when_the_pid_file_cannot_be_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rebalancer.pid");
    let guard = PidFileGuard::create(path.clone()).unwrap();
    // A directory in place of the file: removing it fails, and not with "not found".
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();

    let capture = LogCapture::default();
    capture.run(|| drop(guard));

    let text = capture.text();
    assert!(text.contains(WARNING), "log was: {text}");
    assert!(path.is_dir());
}
