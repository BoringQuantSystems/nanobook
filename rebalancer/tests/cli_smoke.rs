//! G29 CLI entry and S2 exit codes.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rebalancer")
}

fn sample_config() -> &'static str {
    r#"
[connection]
host = "127.0.0.1"
port = 4002
client_id = 1
[account]
id = "DU1"
type = "cash"
[execution]
[risk]
[cost]
[logging]
dir = "./logs"
audit_file = "audit.jsonl"
"#
}

#[test]
fn missing_config_creates_log_dir_and_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .current_dir(dir.path())
        .args(["--config", "missing.toml", "status"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error loading config"),
        "stderr was: {stderr}"
    );
    assert!(dir.path().join("logs").is_dir());
}

#[test]
fn kill_without_pid_file_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), sample_config()).unwrap();
    let output = Command::new(bin())
        .current_dir(dir.path())
        .args(["--config", "config.toml", "--skip-validation", "kill"])
        .output()
        .unwrap();
    assert_ne!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PID file not found"),
        "stderr was: {stderr}"
    );
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn aborted_error_is_printed_as_its_bare_message() {
    // `Error::Aborted` has its own arm in main: the message is printed as is,
    // without the "Error: execution aborted:" prefix of the generic arm.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), sample_config()).unwrap();
    let output = Command::new(bin())
        .current_dir(dir.path())
        .args(["--config", "config.toml", "--skip-validation", "kill"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.lines().any(|l| l.starts_with("PID file not found")),
        "stderr was: {stderr}"
    );
    assert!(!stderr.contains("Error: "), "stderr was: {stderr}");
    assert!(
        !stderr.contains("execution aborted"),
        "stderr was: {stderr}"
    );
}
