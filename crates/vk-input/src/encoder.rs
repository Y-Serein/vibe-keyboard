//! Rotary encoder trait.

use vk_core::Direction;

/// Encoder event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderEvent {
    Rotate { direction: Direction, steps: u8 },
    Press,
    Release,
}

/// Rotary encoder input trait.
///
/// Implementations:
/// - EC11 encoder via GPIO (ESP32)
/// - Mouse scroll / arrow keys (simulator)
/// - Virtual knob (Tauri UI)
pub trait EncoderInput {
    /// Poll encoder, returning any pending event.
    fn poll(&mut self) -> Option<EncoderEvent>;
}
