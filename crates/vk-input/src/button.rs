//! Button trait — abstraction over physical buttons and simulated inputs.

use vk_core::ButtonId;

/// Button state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
}

/// Button event with timestamp-like ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    pub id: ButtonId,
    pub state: ButtonState,
}

/// Button input trait.
///
/// Implementations:
/// - GPIO polling (ESP32 firmware)
/// - Keyboard shortcut mapping (simulator)
/// - Virtual click (Tauri UI)
pub trait ButtonInput {
    /// Poll all buttons, returning any pending events.
    fn poll(&mut self) -> Option<ButtonEvent>;
}
