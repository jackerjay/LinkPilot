//! Integration test for the v0.2 (M3.2) IPC server's unknown-verb
//! fallback path: a client that sends a request type the daemon doesn't
//! recognise must get a structured `Error{code:"unknown-verb"}` reply
//! AND keep the connection alive for subsequent valid requests.
//!
//! Protocol v1 dropped the connection on a bad verb; v0.2+ daemons
//! soft-fail so forward-compat with future protocol versions is the
//! responsibility of the daemon, not the client. This test guards that
//! contract end-to-end.
//!
//! The client side is plain blocking `std::os::unix::net::UnixStream`
//! on purpose: the wire format is just length-prefixed JSON, and using
//! the std API keeps this test out of any tokio runtime so dropping
//! the ServerHandle (which itself owns a tokio Runtime) doesn't trip
//! "Cannot drop a runtime in a context where blocking is not allowed".

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use linkpilot_core::daemon::RequestHandler;
use linkpilot_core::protocol::{Request, Response, ERROR_UNKNOWN_VERB};
use linkpilot_ipc::path::Endpoint;

/// Trivial handler that only answers StatePing — every other verb
/// returns a generic Error. The test never reaches the catch-all
/// because it only sends StatePing on the typed path; unknown verbs
/// short-circuit inside the IPC server before reaching the handler.
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
                code: "unexpected-in-test".into(),
                message: "handler only knows StatePing".into(),
            },
        }
    }
}

fn tmp_socket() -> std::path::PathBuf {
    // macOS' sockaddr_un.sun_path is 104 bytes; TMPDIR alone is ~50
    // chars, so we keep the suffix tight (8-char uuid prefix).
    let short = uuid::Uuid::new_v4().simple().to_string();
    std::env::temp_dir().join(format!("lp-{}.sock", &short[..8]))
}

fn write_frame(stream: &mut UnixStream, payload: &[u8]) {
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .expect("write len");
    stream.write_all(payload).expect("write payload");
    stream.flush().expect("flush");
}

fn read_frame(stream: &mut UnixStream) -> Vec<u8> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).expect("read len");
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).expect("read payload");
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
fn unknown_verb_returns_error_and_keeps_connection_open() {
    let path = tmp_socket();
    let endpoint = Endpoint::UnixSocket(path.clone());

    let handle =
        linkpilot_ipc::server::serve(endpoint.clone(), Arc::new(PingHandler)).expect("serve");

    assert!(
        wait_for_path(&path, Duration::from_secs(3)),
        "socket {} never appeared",
        path.display()
    );

    let mut stream = UnixStream::connect(&path).expect("connect to test server");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .expect("write timeout");

    // ---- Stage 1: send a frame whose `type` is not in the Request enum.
    // Frame manually so we don't need a Rust enum variant for it; that's
    // the whole point of the test (simulate a future v0.3 verb).
    let bad = br#"{"type":"future-verb-from-v0.3","request_id":"bogus-1","whatever":true}"#;
    write_frame(&mut stream, bad);

    let raw = read_frame(&mut stream);
    let resp: Response = serde_json::from_slice(&raw).expect("parse Response");
    match resp {
        Response::Error {
            request_id,
            code,
            message,
        } => {
            assert_eq!(code, ERROR_UNKNOWN_VERB);
            assert_eq!(
                request_id, "bogus-1",
                "server should echo request_id even on unknown verb"
            );
            assert!(!message.is_empty(), "message should be human readable");
        }
        other => panic!("expected Error{{unknown-verb}}, got {other:?}"),
    }

    // ---- Stage 2: same connection must still work. Send a valid Ping.
    // Protocol v1 dropped the connection on bad verb; this assertion is
    // what makes the forward-compat contract observable.
    let ping = Request::StatePing {
        request_id: "ping-after-bad".into(),
    };
    let ping_bytes = serde_json::to_vec(&ping).expect("encode ping");
    write_frame(&mut stream, &ping_bytes);

    let raw = read_frame(&mut stream);
    let resp: Response = serde_json::from_slice(&raw).expect("parse Pong");
    match resp {
        Response::Pong {
            request_id,
            daemon_version,
        } => {
            assert_eq!(request_id, "ping-after-bad");
            assert_eq!(daemon_version, "test");
        }
        other => panic!("expected Pong, got {other:?}"),
    }

    // Close the client end first; let the server drain.
    drop(stream);
    drop(handle);
}
