use linkpilot_core::browser::{BrowserId, BrowserProfile, InstalledBrowser};
use linkpilot_core::platform::{BrowserInventory, Result};

pub struct MacInventory;

impl BrowserInventory for MacInventory {
    fn installed_browsers(&self) -> Result<Vec<InstalledBrowser>> {
        Ok(Vec::new())
    }

    fn profiles(&self, _browser: &BrowserId) -> Result<Vec<BrowserProfile>> {
        Ok(Vec::new())
    }
}
