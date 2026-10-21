//! Store restrictions are enforced at the native boundary as well as in the UI.
use linkpilot_core::browser::BrowserTarget;
use linkpilot_core::config::ConfigDocument;
#[cfg(all(target_os = "macos", feature = "app-store"))]
use linkpilot_core::rules::{Action, MatcherTree};

pub const APP_STORE: bool = cfg!(feature = "app-store");

pub fn require_direct(feature: &str) -> Result<(), String> {
    if APP_STORE {
        Err(format!("{feature} is unavailable in the App Store edition"))
    } else {
        Ok(())
    }
}

pub fn validate_target(target: &BrowserTarget) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "app-store"))]
    linkpilot_platform_mac::store::validate_target(target).map_err(|e| e.to_string())?;
    let _ = target;
    Ok(())
}

pub fn validate_document(doc: &ConfigDocument) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "app-store"))]
    {
        validate_target(&doc.default_target)?;
        if doc.settings.launch_at_login
            || doc.settings.auto_check_updates
            || !doc.settings.profile_orders.is_empty()
            || !doc.custom_browsers.is_empty()
        {
            return Err("App Store settings cannot enable LaunchAgents, external updates, profile ordering or custom browsers".into());
        }
        for rule in &doc.rules {
            validate_matcher(&rule.when)?;
            match &rule.then {
                Action::Open { target } => validate_target(target)?,
                Action::KeepSource => return Err("Keep-source requires browser integration and is unavailable in the App Store edition".into()),
                _ => {}
            }
        }
    }
    let _ = doc;
    Ok(())
}

#[cfg(all(target_os = "macos", feature = "app-store"))]
fn validate_matcher(matcher: &MatcherTree) -> Result<(), String> {
    match matcher {
        MatcherTree::SourceBrowser { .. } | MatcherTree::SourceProfile { .. } => Err("Source browser/profile matching requires browser integration and is unavailable in the App Store edition".into()),
        MatcherTree::All { of } | MatcherTree::Any { of } => of.iter().try_for_each(validate_matcher),
        MatcherTree::Not { of } => validate_matcher(of),
        _ => Ok(()),
    }
}

#[cfg(all(test, target_os = "macos", feature = "app-store"))]
mod tests {
    use super::*;
    #[test]
    fn incompatible_configuration_is_rejected_without_normalizing_it() {
        let mut doc = ConfigDocument::default();
        assert!(validate_document(&doc).is_err()); // Direct builds check GitHub by default.
        doc.settings.auto_check_updates = false;
        assert!(validate_document(&doc).is_ok());
        doc.default_target.profile = Some("Default".into());
        assert!(validate_document(&doc).is_err());
        assert_eq!(doc.default_target.profile.as_deref(), Some("Default"));
    }
    #[test]
    fn nested_extension_matchers_cannot_enter_store_configuration() {
        assert!(validate_matcher(&MatcherTree::Not {
            of: Box::new(MatcherTree::SourceBrowser {
                browser: "chrome".into()
            })
        })
        .is_err());
        assert!(validate_matcher(&MatcherTree::Always).is_ok());
        assert!(require_direct("CLI installation").is_err());
    }
}
