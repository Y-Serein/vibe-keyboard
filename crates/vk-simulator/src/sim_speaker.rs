//! SimSpeaker — rodio-based speaker simulation with terminal bell fallback.
//!
//! Plays embedded WAV assets (from `vk_core::sounds`) through rodio.
//! Falls back to terminal bell (\x07) if audio output initialization fails.

use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use vk_input::speaker::Speaker;
use vk_protocol::message::SoundType;

/// Speaker implementation using rodio for real audio playback.
///
/// Falls back to terminal bell if rodio initialization fails.
pub struct SimSpeaker {
    volume: u8,
    muted: bool,
    last_sound: Option<SoundType>,
    /// rodio output stream (must stay alive for audio to play).
    _stream: Option<OutputStream>,
    /// rodio stream handle for creating sinks.
    stream_handle: Option<OutputStreamHandle>,
}

impl std::fmt::Debug for SimSpeaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimSpeaker")
            .field("volume", &self.volume)
            .field("muted", &self.muted)
            .field("last_sound", &self.last_sound)
            .field("has_audio", &self._stream.is_some())
            .finish()
    }
}

impl Default for SimSpeaker {
    fn default() -> Self {
        Self::new()
    }
}

impl SimSpeaker {
    pub fn new() -> Self {
        // Try to initialize rodio; fall back gracefully.
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                tracing::warn!("rodio init failed, falling back to terminal bell: {e}");
                (None, None)
            }
        };

        Self {
            volume: 80,
            muted: false,
            last_sound: None,
            _stream: stream,
            stream_handle: handle,
        }
    }

    /// Create a SimSpeaker without audio output (for testing).
    #[cfg(test)]
    fn new_silent() -> Self {
        Self {
            volume: 80,
            muted: false,
            last_sound: None,
            _stream: None,
            stream_handle: None,
        }
    }

    pub fn last_sound(&self) -> Option<SoundType> {
        self.last_sound
    }

    #[allow(dead_code)]
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    #[allow(dead_code)]
    pub fn volume(&self) -> u8 {
        self.volume
    }

    /// Play audio via rodio. Returns true if successful.
    fn play_rodio(&self, sound: SoundType) -> bool {
        let Some(handle) = &self.stream_handle else {
            return false;
        };

        let Ok(sink) = Sink::try_new(handle) else {
            return false;
        };

        // Set volume (0.0 - 1.0 from our 0-100 scale).
        sink.set_volume(self.volume as f32 / 100.0);

        let cursor = Cursor::new(vk_core::sounds::wav_data(sound));
        let Ok(source) = Decoder::new(cursor) else {
            return false;
        };

        sink.append(source);
        // Detach so playback continues without blocking.
        sink.detach();
        true
    }

    /// Terminal bell fallback.
    fn play_bell() {
        eprint!("\x07");
    }
}

impl Speaker for SimSpeaker {
    fn play(&mut self, sound: SoundType) {
        self.last_sound = Some(sound);
        if self.muted || self.volume == 0 {
            return;
        }

        if !self.play_rodio(sound) {
            Self::play_bell();
        }
    }

    fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let s = SimSpeaker::new_silent();
        assert_eq!(s.volume(), 80);
        assert!(!s.is_muted());
        assert!(s.last_sound().is_none());
    }

    #[test]
    fn play_records_sound() {
        let mut s = SimSpeaker::new_silent();
        s.set_muted(true); // mute to avoid any audio in tests
        s.play(SoundType::PermissionAlert);
        assert_eq!(s.last_sound(), Some(SoundType::PermissionAlert));
    }

    #[test]
    fn set_volume_clamps() {
        let mut s = SimSpeaker::new_silent();
        s.set_volume(150);
        assert_eq!(s.volume(), 100);
    }

    #[test]
    fn mute_unmute() {
        let mut s = SimSpeaker::new_silent();
        s.set_muted(true);
        assert!(s.is_muted());
        s.set_muted(false);
        assert!(!s.is_muted());
    }

    #[test]
    fn wav_data_returns_valid_riff() {
        // Verify all WAV data starts with RIFF header (via vk_core::sounds).
        for sound in [
            SoundType::PermissionAlert,
            SoundType::SessionComplete,
            SoundType::Error,
            SoundType::Click,
        ] {
            let data = vk_core::sounds::wav_data(sound);
            assert!(data.len() > 44, "WAV too short for {sound:?}");
            assert_eq!(&data[0..4], b"RIFF", "Missing RIFF header for {sound:?}");
            assert_eq!(&data[8..12], b"WAVE", "Missing WAVE marker for {sound:?}");
        }
    }

    #[test]
    fn play_when_muted_records_but_no_audio() {
        let mut s = SimSpeaker::new_silent();
        s.set_muted(true);
        s.play(SoundType::Click);
        assert_eq!(s.last_sound(), Some(SoundType::Click));
    }

    #[test]
    fn play_when_zero_volume_records_but_no_audio() {
        let mut s = SimSpeaker::new_silent();
        s.set_volume(0);
        s.play(SoundType::Error);
        assert_eq!(s.last_sound(), Some(SoundType::Error));
    }
}
