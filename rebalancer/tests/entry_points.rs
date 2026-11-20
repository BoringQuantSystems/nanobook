//! G12: run, positions, status and reconcile against a closed port.

use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nanobook_rebalancer::config::Config;
use nanobook_rebalancer::error::Error;
use nanobook_rebalancer::execution::{self, RunOptions};
use nanobook_rebalancer::target::TargetSpec;

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn config_for_port(port: u16) -> Config {
    toml::from_str(&format!(
        r#"
[connection]
host = "127.0.0.1"
port = {port}
client_id = 1
[account]
id = "DU1"
type = "cash"
[execution]
[risk]
[cost]
[logging]
"#
    ))
    .unwrap()
}

fn spec() -> TargetSpec {
    TargetSpec::from_json(
        r#"{
            "timestamp": "2026-02-08T15:30:00Z",
            "targets": [{ "symbol": "AAPL", "weight": 0.5 }]
        }"#,
    )
    .unwrap()
}

fn call_with_timeout<T, F>(f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(8))
        .expect("IBKR connect to a closed port hung; G12 falls back to S1 for run")
}

fn assert_connection(result: nanobook_rebalancer::error::Result<()>) {
    match result {
        Err(Error::Connection(_)) => {}
        other => panic!("expected Connection, got {other:?}"),
    }
}

#[test]
fn run_against_closed_port_is_connection_error() {
    let config = config_for_port(closed_port());
    let spec = spec();
    let opts = RunOptions {
        dry_run: true,
        force: true,
        target_file: "t.json".into(),
        cron_mode: false,
    };
    assert_connection(call_with_timeout(move || {
        execution::run(&config, &spec, &opts)
    }));
}

#[test]
fn show_positions_against_closed_port_is_connection_error() {
    let config = config_for_port(closed_port());
    assert_connection(call_with_timeout(move || {
        execution::show_positions(&config)
    }));
}

#[test]
fn check_status_against_closed_port_is_connection_error() {
    let config = config_for_port(closed_port());
    assert_connection(call_with_timeout(move || execution::check_status(&config)));
}

#[test]
fn run_reconcile_against_closed_port_is_connection_error() {
    let config = config_for_port(closed_port());
    let spec = spec();
    assert_connection(call_with_timeout(move || {
        execution::run_reconcile(&config, &spec)
    }));
}
