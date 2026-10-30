//! Read modifier state through public AppKit APIs, without a global key monitor.

use linkpilot_core::config::AskQuickOpenModifier;
use objc2::{class, msg_send};

/// Snapshot the selected modifier as soon as a URL handoff arrives.
pub fn is_modifier_pressed(modifier: AskQuickOpenModifier) -> bool {
    let mask = match modifier {
        AskQuickOpenModifier::Off => return false,
        AskQuickOpenModifier::Command => 1 << 20,
        AskQuickOpenModifier::Option => 1 << 19,
        AskQuickOpenModifier::Control => 1 << 18,
        AskQuickOpenModifier::Shift => 1 << 17,
    };
    // +modifierFlags reports keyboard state independently of the focused app.
    let flags: usize = unsafe { msg_send![class!(NSEvent), modifierFlags] };
    flags & mask != 0
}
