//! v0.2 follow-up #1: the IPC server's bind path must tolerate a
//! pre-existing socket file at the endpoint path. The headless daemon's
//! main() function does an explicit `remove_file` after the probe
//! succeeds (no daemon answering) and logs it; the IPC server itself
//! also does the unlink in `run()` as a belt-and-suspenders.
//!
//! This test exercises the IPC-server path directly: pre-create a
//! stale socket file at the target path, call `serve()`, and verify a
//! client can still connect + get a Pong.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use linkpilot_core::daemon::RequestHandler;
use linkpilot_core::protocol::{Request, Response};
use linkpilot_ipc::path::Endpoint;

struct PingHandler;

impl RequestHandler for PingHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::StatePing { request_id } => Response::Pong {
                request_id,
                daemon_version: "test".into(),
            },
            _ => Response::Error {
                request_id: String::new(),
                code: "unexpected".into(),
                message: "ping-only fixture".into(),
            },
        }
    }
}

fn tmp_socket() -> std::path::PathBuf {
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::env::temp_dir().join(format!("lp-stale-{}.sock", &id[..8]))
}

fn wait_for_socket(path: &std::path::Path, timeout: Duration) -> bool {
    use std::os::unix::fs::FileTypeExt;
    let start = Instant::now();
    while start.elapsed() < timeout {
        // Specifically check `is_socket()` — our test seeds a regular
        // file at the path, so a naive `.exists()` would return true
        // before the bind has actually swapped it out for a socket.
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.file_type().is_socket() {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn serve_succeeds_when_stale_socket_file_already_exists() {
    let path = tmp_socket();

    // Simulate a stale leftover: a regular file at the socket path.
    // `UnixListener::bind` would EADDRINUSE here without the cleanup.
    std::fs::write(&path, b"stale junk from a crashed daemon").expect("seed stale file");
    assert!(path.exists());

    let endpoint = Endpoint::UnixSocket(path.clone());
    let handle = linkpilot_ipc::server::serve(endpoint, Arc::new(PingHandler)).expect("serve");

    // Bind happens in a tokio task; wait specifically for a SOCKET to
    // replace the stale regular file we seeded. server.rs's run() should
    // remove our junk file and bind a fresh socket at the same path.
    assert!(
        wait_for_socket(&path, Duration::from_secs(3)),
        "socket {} never appeared after cleanup",
        path.display()
    );
    let mut stream = UnixStream::connect(&path).expect("connect post-cleanup");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    let req = Request::StatePing {
        request_id: "after-stale".into(),
    };
    let bytes = serde_json::to_vec(&req).unwrap();
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(&bytes).unwrap();
    stream.flush().unwrap();

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).unwrap();
    let resp: Response = serde_json::from_slice(&buf).unwrap();
    match resp {
        Response::Pong {
            request_id,
            daemon_version,
        } => {
            assert_eq!(request_id, "after-stale");
            assert_eq!(daemon_version, "test");
        }
        other => panic!("expected Pong, got {other:?}"),
    }

    drop(stream);
    drop(handle);
}
