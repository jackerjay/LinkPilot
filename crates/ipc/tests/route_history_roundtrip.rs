//! End-to-end roundtrip for the v0.2 (M3) RouteHistory verb.
//!
//! Spins up a DaemonRuntime backed by an in-memory config + history
//! ring buffer, logs three fake routing decisions, sends a
//! `Request::RouteHistory` over a Unix socket and asserts the daemon
//! replies with `Response::RouteHistorySnapshot` containing those
//! three records, newest-first.
//!
//! Synchronous client (`std::os::unix::net::UnixStream`) on purpose:
//! the ServerHandle owns a tokio Runtime, and dropping it from inside
//! another runtime panics. See unknown_verb_fallback.rs for the same
//! reasoning.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use linkpilot_core::browser::{BrowserId, BrowserTarget};
use linkpilot_core::config::ConfigStore;
use linkpilot_core::daemon::DaemonRuntime;
use linkpilot_core::history::{RouteHistory, RouteRecord};
use linkpilot_core::platform::StubProvider;
use linkpilot_core::protocol::{Request, Response};
use linkpilot_core::routing::{RoutingContext, RoutingDecision, Source, SourceKind};
use linkpilot_ipc::path::Endpoint;

fn tmp_socket() -> PathBuf {
    // macOS sockaddr_un.sun_path is 104 bytes; keep the suffix small.
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::env::temp_dir().join(format!("lp-h-{}.sock", &id[..8]))
}

fn tmp_config() -> PathBuf {
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::env::temp_dir().join(format!("lp-cfg-{}.json", &id[..8]))
}

fn build_runtime() -> Arc<DaemonRuntime> {
    let cfg_path = tmp_config();
    let (config, _) = ConfigStore::load_or_init(cfg_path).expect("config init");
    Arc::new(DaemonRuntime::new(
        config,
        Arc::new(RouteHistory::new()),
        Arc::new(StubProvider),
        "test".to_string(),
    ))
}

fn fake_record(url: &str) -> RouteRecord {
    RouteRecord::new(
        RoutingContext {
            url: url.into(),
            source: Source {
                kind: SourceKind::Cli,
                app_name: None,
                bundle_id: None,
                browser: None,
                profile: None,
            },
            navigation: None,
            environment: None,
        },
        RoutingDecision::Open {
            target: BrowserTarget::new(BrowserId::new("chrome")),
            matched_rule: None,
            reason: "test".into(),
        },
    )
}

fn write_frame(stream: &mut UnixStream, payload: &[u8]) {
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(payload).unwrap();
    stream.flush().unwrap();
}

fn read_frame(stream: &mut UnixStream) -> Vec<u8> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).unwrap();
    buf
}

fn wait_for_path(path: &std::path::Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn route_history_returns_recent_records_newest_first() {
    let runtime = build_runtime();

    // Seed three records before serving so the test has something to
    // ask for. The handler reads the same Arc<RouteHistory>.
    for url in [
        "https://a.example.com",
        "https://b.example.com",
        "https://c.example.com",
    ] {
        runtime.history.log(fake_record(url));
    }

    let path = tmp_socket();
    let endpoint = Endpoint::UnixSocket(path.clone());
    let handle = linkpilot_ipc::server::serve(endpoint, runtime).expect("serve");

    assert!(
        wait_for_path(&path, Duration::from_secs(3)),
        "socket never appeared"
    );

    let mut stream = UnixStream::connect(&path).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    let req = Request::RouteHistory {
        request_id: "rt-1".into(),
        limit: Some(10),
    };
    write_frame(&mut stream, &serde_json::to_vec(&req).unwrap());

    let raw = read_frame(&mut stream);
    let resp: Response = serde_json::from_slice(&raw).expect("parse Response");
    match resp {
        Response::RouteHistorySnapshot {
            request_id,
            records,
        } => {
            assert_eq!(request_id, "rt-1");
            assert_eq!(records.len(), 3);
            // Newest first — `c` was logged last.
            assert_eq!(records[0].context.url, "https://c.example.com");
            assert_eq!(records[2].context.url, "https://a.example.com");
        }
        other => panic!("expected RouteHistorySnapshot, got {other:?}"),
    }

    drop(stream);
    drop(handle);
}

#[test]
fn route_history_with_limit_caps_results() {
    let runtime = build_runtime();
    for _ in 0..10 {
        runtime.history.log(fake_record("https://x.example.com"));
    }

    let path = tmp_socket();
    let endpoint = Endpoint::UnixSocket(path.clone());
    let handle = linkpilot_ipc::server::serve(endpoint, runtime).expect("serve");
    assert!(wait_for_path(&path, Duration::from_secs(3)));

    let mut stream = UnixStream::connect(&path).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    let req = Request::RouteHistory {
        request_id: "rt-2".into(),
        limit: Some(3),
    };
    write_frame(&mut stream, &serde_json::to_vec(&req).unwrap());

    let raw = read_frame(&mut stream);
    let resp: Response = serde_json::from_slice(&raw).expect("parse Response");
    match resp {
        Response::RouteHistorySnapshot { records, .. } => {
            assert_eq!(records.len(), 3, "limit should cap to 3");
        }
        other => panic!("unexpected: {other:?}"),
    }

    drop(stream);
    drop(handle);
}
