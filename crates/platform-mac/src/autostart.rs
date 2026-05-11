use linkpilot_core::platform::{Autostart, PlatformError, Result};

pub struct MacAutostart;

impl Autostart for MacAutostart {
    fn is_enabled(&self) -> Result<bool> {
        Ok(false)
    }

    fn set_enabled(&self, _on: bool) -> Result<()> {
        Err(PlatformError::Other(
            "MacAutostart::set_enabled unimplemented".into(),
        ))
    }
}
