//! LED trait — button indicator LEDs and knob ring.

use vk_core::{ButtonId, LedColor};

/// LED control trait.
///
/// Implementations:
/// - WS2812B / GPIO (ESP32)
/// - Terminal color codes (CLI simulator)
/// - CSS colors (Tauri UI)
pub trait LedController {
    /// Set button LED color.
    fn set_button_led(&mut self, button: ButtonId, color: LedColor);

    /// Set button LED blinking.
    fn set_button_blink(&mut self, button: ButtonId, on: bool);

    /// Set knob ring color.
    fn set_knob_ring(&mut self, color: LedColor);

    /// Turn off all LEDs.
    fn all_off(&mut self);
}
