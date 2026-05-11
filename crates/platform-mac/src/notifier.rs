use linkpilot_core::platform::{Notifier, Result};

pub struct MacNotifier;

impl Notifier for MacNotifier {
    fn toast(&self, title: &str, body: &str) -> Result<()> {
        tracing::info!(%title, %body, "MacNotifier::toast (stub)");
        Ok(())
    }
}
