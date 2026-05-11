use linkpilot_core::platform::{OpenEventHint, OpenerApp, OpenerDetector};

pub struct MacOpenerDetector;

impl OpenerDetector for MacOpenerDetector {
    fn detect(&self, _hint: &OpenEventHint) -> Option<OpenerApp> {
        None
    }
}
