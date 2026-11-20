//! Helpers shared by the mutation-gap tests: capture of `tracing` output and
//! capture of the standard output of a test run in a child process.

#![allow(dead_code)]

use std::io;
use std::process::Command;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

/// Collects everything a scoped `tracing` subscriber writes.
#[derive(Clone, Default)]
pub struct LogCapture(Arc<Mutex<Vec<u8>>>);

pub struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogCapture {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(Arc::clone(&self.0))
    }
}

impl LogCapture {
    /// Run `f` with a subscriber that records every event at any level.
    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        // Enable every callsite for good. A callsite first reached by a test
        // without a capture, while no subscriber exists, is cached as
        // disabled, and a capturing test running at the same time then misses
        // its events.
        static ENABLE_ALL_CALLSITES: std::sync::Once = std::sync::Once::new();
        ENABLE_ALL_CALLSITES.call_once(|| {
            let _ = tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .with_max_level(tracing::Level::TRACE)
                    .with_writer(io::sink)
                    .finish(),
            );
        });
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .with_writer(self.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, f)
    }

    pub fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

const CHILD_ENV: &str = "NANOBOOK_TEST_CHILD_SCENARIO";

/// Inside a child process started by [`run_child`], the scenario name.
/// `None` in the normal (parent) run.
pub fn child_scenario() -> Option<String> {
    std::env::var(CHILD_ENV).ok()
}

/// `println!` output cannot be read back inside the test process, because the
/// test harness captures it. Run the test `test_name` of the current test
/// binary again in a child process with `--nocapture` and return its standard
/// output. The test body must call [`child_scenario`] and run the named
/// scenario when it returns `Some`.
pub fn run_child(test_name: &str, scenario: &str) -> String {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, scenario)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child {test_name}/{scenario} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
