//! Speaker trait — audio feedback.

use vk_core::SoundType;

/// Speaker control trait.
///
/// Implementations:
/// - Piezo buzzer / DAC (ESP32)
/// - rodio audio playback (simulator)
/// - Web Audio API (Tauri UI)
pub trait Speaker {
    /// Play a predefined sound.
    fn play(&mut self, sound: SoundType);

    /// Set volume (0-100).
    fn set_volume(&mut self, volume: u8);

    /// Mute/unmute.
    fn set_muted(&mut self, muted: bool);
}
