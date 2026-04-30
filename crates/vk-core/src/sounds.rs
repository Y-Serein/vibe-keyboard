//! Embedded WAV sound assets — single source of truth for all binaries.
//!
//! WAV files are compiled in via `include_bytes!` so every binary that links
//! `vk-core` shares the same data without duplicating the embed or the
//! match logic.

use crate::SoundType;

pub const WAV_PERMISSION_ALERT: &[u8] = include_bytes!("../assets/sounds/permission_alert.wav");
pub const WAV_SESSION_COMPLETE: &[u8] = include_bytes!("../assets/sounds/session_complete.wav");
pub const WAV_ERROR: &[u8] = include_bytes!("../assets/sounds/error.wav");
pub const WAV_CLICK: &[u8] = include_bytes!("../assets/sounds/click.wav");

/// Returns the embedded WAV data for a given [`SoundType`].
pub fn wav_data(sound: SoundType) -> &'static [u8] {
    match sound {
        SoundType::PermissionAlert => WAV_PERMISSION_ALERT,
        SoundType::SessionComplete => WAV_SESSION_COMPLETE,
        SoundType::Error => WAV_ERROR,
        SoundType::Click => WAV_CLICK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_data_returns_valid_riff() {
        for sound in [
            SoundType::PermissionAlert,
            SoundType::SessionComplete,
            SoundType::Error,
            SoundType::Click,
        ] {
            let data = wav_data(sound);
            assert!(data.len() > 44, "WAV too short for {sound:?}");
            assert_eq!(&data[0..4], b"RIFF", "Missing RIFF header for {sound:?}");
            assert_eq!(&data[8..12], b"WAVE", "Missing WAVE marker for {sound:?}");
        }
    }
}
