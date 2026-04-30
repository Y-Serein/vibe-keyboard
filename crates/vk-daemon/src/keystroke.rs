//! Keystroke injection — sends real key events to the focused application.
//!
//! Defines [`KeystrokeInjector`] trait with platform-specific implementations:
//! - [`MacKeystrokeInjector`] — uses CGEvent on macOS
//! - [`NullKeystrokeInjector`] — stub for unsupported platforms / testing

#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Trait for platform-specific keystroke injection.
pub trait KeystrokeInjector: Send + Sync {
    /// Send a single key action (key down + key up).
    fn send_key(&self, action: &str) -> Result<(), String>;
    /// Hold a key down (for toggle mode).
    fn send_key_down(&self, action: &str) -> Result<(), String>;
    /// Release a held key.
    fn send_key_up(&self, action: &str) -> Result<(), String>;
    /// Platform name for logging.
    fn platform(&self) -> &str;
}

// ---------------------------------------------------------------------------
// MacKeystrokeInjector
// ---------------------------------------------------------------------------

/// macOS keystroke injector using CGEvent.
#[cfg(target_os = "macos")]
pub struct MacKeystrokeInjector;

#[cfg(target_os = "macos")]
impl MacKeystrokeInjector {
    /// Resolve action name to (keycode, flags).
    fn resolve_action(action: &str) -> Result<(CGKeyCode, CGEventFlags), String> {
        match action {
            "enter" => Ok((0x24, CGEventFlags::empty())),
            "escape" => Ok((0x35, CGEventFlags::empty())),
            "ctrl_u" => Ok((0x20, CGEventFlags::CGEventFlagControl)),
            "tab" => Ok((0x30, CGEventFlags::empty())),
            "space" => Ok((0x31, CGEventFlags::empty())),
            "backspace" => Ok((0x33, CGEventFlags::empty())),
            "ctrl_c" => Ok((0x08, CGEventFlags::CGEventFlagControl)),
            "ctrl_z" => Ok((0x06, CGEventFlags::CGEventFlagControl)),
            "ctrl_d" => Ok((0x02, CGEventFlags::CGEventFlagControl)),
            "ctrl_l" => Ok((0x25, CGEventFlags::CGEventFlagControl)),
            "fn" => return Err("fn uses send_fn_key(), not resolve_action".into()),
            "cmd_tab" => Ok((0x30, CGEventFlags::CGEventFlagCommand)),
            "ctrl_shift_space" => Ok((
                0x31,
                CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagShift,
            )),
            "cmd_space" => Ok((0x31, CGEventFlags::CGEventFlagCommand)),
            _ => Err(format!("unknown action: {action}")),
        }
    }

    /// Send a keystroke (key down + key up) to the active application.
    /// After key-up, sends FlagsChanged with empty flags to prevent modifier sticking.
    fn send_keystroke(keycode: CGKeyCode, flags: CGEventFlags) -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "failed to create event source".to_string())?;

        let key_down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
            .map_err(|_| "failed to create key-down event".to_string())?;
        key_down.set_flags(flags);
        key_down.post(CGEventTapLocation::HID);

        let key_up = CGEvent::new_keyboard_event(source.clone(), keycode, false)
            .map_err(|_| "failed to create key-up event".to_string())?;
        key_up.set_flags(CGEventFlags::empty());
        key_up.post(CGEventTapLocation::HID);

        // Clear any stuck modifiers
        if flags != CGEventFlags::empty() {
            Self::clear_modifier_flags(&source);
        }

        Ok(())
    }

    /// Post FlagsChanged with empty flags to clear any stuck modifier keys.
    fn clear_modifier_flags(source: &CGEventSource) {
        if let Ok(clear) = CGEvent::new_keyboard_event(source.clone(), 0xFF, true) {
            clear.set_flags(CGEventFlags::empty());
            clear.set_type(core_graphics::event::CGEventType::FlagsChanged);
            clear.post(CGEventTapLocation::HID);
        }
    }

    /// Send only key-down (press and hold).
    fn send_key_down_raw(keycode: CGKeyCode, flags: CGEventFlags) -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "failed to create event source".to_string())?;
        let event = CGEvent::new_keyboard_event(source, keycode, true)
            .map_err(|_| "failed to create key-down event".to_string())?;
        event.set_flags(flags);
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    /// Send only key-up (release) + clear modifiers.
    fn send_key_up_raw(keycode: CGKeyCode, flags: CGEventFlags) -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "failed to create event source".to_string())?;
        let event = CGEvent::new_keyboard_event(source.clone(), keycode, false)
            .map_err(|_| "failed to create key-up event".to_string())?;
        event.set_flags(CGEventFlags::empty());
        event.post(CGEventTapLocation::HID);
        if flags != CGEventFlags::empty() {
            Self::clear_modifier_flags(&source);
        }
        Ok(())
    }

    /// Send Fn key via CGEvent + immediate modifier clear.
    /// CGEvent 0x3F triggers Typeless/dictation but leaves SecondaryFn modifier stuck.
    /// Fix: after key-up, post a flagsChanged event with empty flags to clear.
    fn send_fn_key() -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "failed to create event source".to_string())?;

        // Key down
        let down = CGEvent::new_keyboard_event(source.clone(), 0x3F, true)
            .map_err(|_| "fn key-down failed".to_string())?;
        down.set_flags(CGEventFlags::CGEventFlagSecondaryFn);
        down.post(CGEventTapLocation::HID);

        // Key up — with empty flags to start clearing
        let up = CGEvent::new_keyboard_event(source.clone(), 0x3F, false)
            .map_err(|_| "fn key-up failed".to_string())?;
        up.set_flags(CGEventFlags::empty());
        up.post(CGEventTapLocation::HID);

        // Post flagsChanged with empty flags to force-clear any stuck modifiers
        let clear = CGEvent::new_keyboard_event(source, 0xFF, true)
            .map_err(|_| "flags clear failed".to_string())?;
        clear.set_flags(CGEventFlags::empty());
        clear.set_type(core_graphics::event::CGEventType::FlagsChanged);
        clear.post(CGEventTapLocation::HID);

        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl KeystrokeInjector for MacKeystrokeInjector {
    fn send_key(&self, action: &str) -> Result<(), String> {
        if action == "fn" {
            return Self::send_fn_key();
        }
        let (keycode, flags) = Self::resolve_action(action)?;
        Self::send_keystroke(keycode, flags)
    }

    fn send_key_down(&self, action: &str) -> Result<(), String> {
        if action == "fn" {
            return Self::send_fn_key();
        }
        let (keycode, flags) = Self::resolve_action(action)?;
        Self::send_key_down_raw(keycode, flags)
    }

    fn send_key_up(&self, action: &str) -> Result<(), String> {
        if action == "fn" {
            return Ok(()); // fn key-up is a no-op (fire-and-forget)
        }
        let (keycode, flags) = Self::resolve_action(action)?;
        Self::send_key_up_raw(keycode, flags)
    }

    fn platform(&self) -> &str {
        "macos"
    }
}

// ---------------------------------------------------------------------------
// NullKeystrokeInjector
// ---------------------------------------------------------------------------

/// Null injector for unsupported platforms and testing.
pub struct NullKeystrokeInjector;

impl KeystrokeInjector for NullKeystrokeInjector {
    fn send_key(&self, action: &str) -> Result<(), String> {
        Err(format!("keystroke injection not supported: {action}"))
    }

    fn send_key_down(&self, action: &str) -> Result<(), String> {
        Err(format!("keystroke injection not supported: {action}"))
    }

    fn send_key_up(&self, action: &str) -> Result<(), String> {
        Err(format!("keystroke injection not supported: {action}"))
    }

    fn platform(&self) -> &str {
        "null"
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create the default keystroke injector for the current platform.
pub fn default_injector() -> Box<dyn KeystrokeInjector> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacKeystrokeInjector)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(NullKeystrokeInjector)
    }
}

// ---------------------------------------------------------------------------
// Backward-compatible free functions (used by server.rs until T10.6)
// ---------------------------------------------------------------------------

/// Button action mapping — sends key down + key up.
pub fn execute_button_action(action: &str) -> Result<(), String> {
    default_injector().send_key(action)
}

/// Execute key-down only (for real-time hold mode).
pub fn execute_key_down(action: &str) -> Result<(), String> {
    default_injector().send_key_down(action)
}

/// Execute key-up only (for real-time hold mode).
pub fn execute_key_up(action: &str) -> Result<(), String> {
    default_injector().send_key_up(action)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_action_enter_returns_correct_keycode() {
        let (keycode, flags) = MacKeystrokeInjector::resolve_action("enter").unwrap();
        assert_eq!(keycode, 0x24);
        assert_eq!(flags, CGEventFlags::empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_action_unknown_returns_error() {
        let result = MacKeystrokeInjector::resolve_action("unknown");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown action"));
    }

    #[test]
    fn null_injector_send_key_returns_error() {
        let injector = NullKeystrokeInjector;
        let result = injector.send_key("enter");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not supported"));
    }

    #[test]
    fn null_injector_send_key_down_returns_error() {
        let injector = NullKeystrokeInjector;
        let result = injector.send_key_down("enter");
        assert!(result.is_err());
    }

    #[test]
    fn null_injector_send_key_up_returns_error() {
        let injector = NullKeystrokeInjector;
        let result = injector.send_key_up("enter");
        assert!(result.is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_injector_platform_returns_macos() {
        let injector = MacKeystrokeInjector;
        assert_eq!(injector.platform(), "macos");
    }

    #[test]
    fn null_injector_platform_returns_null() {
        let injector = NullKeystrokeInjector;
        assert_eq!(injector.platform(), "null");
    }

    #[test]
    fn execute_unknown_action_returns_error() {
        let result = execute_button_action("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn default_injector_returns_box() {
        let injector = default_injector();
        // On macOS it should be "macos", on other platforms "null"
        let platform = injector.platform();
        assert!(!platform.is_empty());
    }
}
