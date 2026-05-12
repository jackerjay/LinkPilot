//! Opener-app detection on macOS.
//!
//! Apple Events deliver only the URL string to `tauri-plugin-deep-link`; the
//! sender app is dropped. To still know which app opened a URL we keep a
//! small ring of recent foreground apps populated by polling
//! `[[NSWorkspace sharedWorkspace] frontmostApplication]` every 750 ms in a
//! background thread.
//!
//! When the URL handler runs LinkPilot is usually already frontmost (macOS
//! activated the .app to deliver the Apple Event), so we never trust the
//! current frontmost — we look at the most recent ring entry that **isn't**
//! LinkPilot itself, within the last 30 seconds. That recovers the actual
//! opener for the Slack / Terminal / VS Code → click-a-link flow.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use linkpilot_core::platform::{OpenEventHint, OpenerApp, OpenerDetector};

const POLL_INTERVAL: Duration = Duration::from_millis(750);
const STALE_AFTER: Duration = Duration::from_secs(30);
const RING_CAPACITY: usize = 8;

#[derive(Clone)]
struct RecentApp {
    seen: Instant,
    name: String,
    bundle_id: Option<String>,
    pid: Option<i32>,
}

pub struct MacOpenerDetector {
    ring: Arc<Mutex<VecDeque<RecentApp>>>,
}

impl MacOpenerDetector {
    /// Start the background poller. `own_bundle_id` is excluded from the
    /// ring so we never report LinkPilot as the opener.
    pub fn start(own_bundle_id: String) -> Self {
        let ring = Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAPACITY)));
        let ring_for_poll = Arc::clone(&ring);
        std::thread::Builder::new()
            .name("linkpilot-opener-poller".into())
            .spawn(move || poll_loop(ring_for_poll, own_bundle_id))
            .expect("spawn opener poller");
        Self { ring }
    }
}

impl OpenerDetector for MacOpenerDetector {
    fn detect(&self, _hint: &OpenEventHint) -> Option<OpenerApp> {
        let guard = self.ring.lock().ok()?;
        let now = Instant::now();
        let recent = guard
            .iter()
            .rev()
            .find(|a| now.duration_since(a.seen) < STALE_AFTER)?;
        Some(OpenerApp {
            name: recent.name.clone(),
            bundle_id: recent.bundle_id.clone(),
            executable: None,
            pid: recent.pid,
        })
    }
}

fn poll_loop(ring: Arc<Mutex<VecDeque<RecentApp>>>, own_bundle_id: String) {
    loop {
        if let Some(app) = read_frontmost() {
            let is_self = app
                .bundle_id
                .as_deref()
                .map(|b| b == own_bundle_id)
                .unwrap_or(false);
            if !is_self {
                push_unique(&ring, app);
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn push_unique(ring: &Mutex<VecDeque<RecentApp>>, app: RecentApp) {
    let Ok(mut guard) = ring.lock() else { return };
    let dup = guard
        .back()
        .map(|last| last.bundle_id == app.bundle_id && last.pid == app.pid)
        .unwrap_or(false);
    if dup {
        // Refresh the timestamp so a long-frontmost app remains "recent"
        // when we query, without bloating the ring.
        if let Some(last) = guard.back_mut() {
            last.seen = app.seen;
        }
        return;
    }
    if guard.len() == RING_CAPACITY {
        guard.pop_front();
    }
    guard.push_back(app);
}

fn read_frontmost() -> Option<RecentApp> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;

    // `frontmostApplication` queries LaunchServices' cached state — safe to
    // call from any thread per Apple's NSRunningApplication thread-safety
    // notes ("basic properties may be read from any thread"). Wrap in an
    // autoreleasepool because we're outside the main run loop.
    autoreleasepool(|_pool| unsafe {
        let workspace = NSWorkspace::sharedWorkspace();
        let app = workspace.frontmostApplication()?;
        let name = app
            .localizedName()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let bundle_id = app.bundleIdentifier().map(|s| s.to_string());
        // `processIdentifier` needs a libc feature; skipped — the bundle id
        // is what rules actually match on. `pid` stays None for now.
        Some(RecentApp {
            seen: Instant::now(),
            name,
            bundle_id,
            pid: None,
        })
    })
}
