//! Local speaker — plays sounds on the desktop when no device is connected.
//!
//! Uses a dedicated audio thread since rodio's OutputStream is !Send.
//! Commands are sent via a channel to the audio thread.

use std::sync::Mutex;

use tracing::warn;
use vk_core::sounds;
use vk_protocol::message::SoundType;

enum AudioCommand {
    Play(SoundType),
    PlayById(String), // "builtin:alert", "builtin:ding", etc.
    SetVolume(u8),
    SetMuted(bool),
}

/// Desktop-local sound player. Commands go to a dedicated audio thread.
pub struct LocalSpeaker {
    tx: Mutex<std::sync::mpsc::Sender<AudioCommand>>,
}

impl LocalSpeaker {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<AudioCommand>();
        std::thread::spawn(move || {
            audio_thread(rx);
        });
        Self { tx: Mutex::new(tx) }
    }

    pub fn play(&self, sound: SoundType) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::Play(sound));
        }
    }

    pub fn set_volume(&self, volume: u8) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::SetVolume(volume));
        }
    }

    pub fn set_muted(&self, muted: bool) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::SetMuted(muted));
        }
    }

    /// Play by sound_id (e.g. "builtin:alert", "builtin:ding", "builtin:buzz", "builtin:click").
    pub fn play_by_id(&self, sound_id: &str) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(AudioCommand::PlayById(sound_id.to_string()));
        }
    }
}

/// Resolve a sound_id string to WAV data.
fn resolve_sound_id(sound_id: &str) -> Option<&'static [u8]> {
    match sound_id {
        "builtin:alert" => Some(sounds::WAV_PERMISSION_ALERT),
        "builtin:ding" => Some(sounds::WAV_SESSION_COMPLETE),
        "builtin:buzz" => Some(sounds::WAV_ERROR),
        "builtin:click" => Some(sounds::WAV_CLICK),
        "builtin:none" => None,
        _ => None, // custom sounds not supported in local speaker yet
    }
}

fn audio_thread(rx: std::sync::mpsc::Receiver<AudioCommand>) {
    let (stream, handle) = match rodio::OutputStream::try_default() {
        Ok((s, h)) => (Some(s), Some(h)),
        Err(e) => {
            warn!("local speaker: audio init failed: {e}");
            (None, None)
        }
    };
    let _stream = stream; // Keep alive

    let mut volume: f32 = 0.8;
    let mut muted = false;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCommand::Play(sound) => {
                if muted {
                    continue;
                }
                let Some(ref h) = handle else { continue };
                let Ok(sink) = rodio::Sink::try_new(h) else { continue };
                sink.set_volume(volume);

                let wav_data = sounds::wav_data(sound);

                let Ok(source) = rodio::Decoder::new(std::io::Cursor::new(wav_data)) else {
                    continue;
                };
                sink.append(source);
                sink.detach();
            }
            AudioCommand::PlayById(ref sound_id) => {
                if muted {
                    continue;
                }
                let Some(wav_data) = resolve_sound_id(sound_id) else { continue };
                let Some(ref h) = handle else { continue };
                let Ok(sink) = rodio::Sink::try_new(h) else { continue };
                sink.set_volume(volume);
                let Ok(source) = rodio::Decoder::new(std::io::Cursor::new(wav_data)) else {
                    continue;
                };
                sink.append(source);
                sink.detach();
            }
            AudioCommand::SetVolume(v) => {
                volume = (v.min(100) as f32) / 100.0;
            }
            AudioCommand::SetMuted(m) => {
                muted = m;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_data_valid_riff() {
        for data in [
            sounds::WAV_PERMISSION_ALERT,
            sounds::WAV_SESSION_COMPLETE,
            sounds::WAV_ERROR,
            sounds::WAV_CLICK,
        ] {
            assert!(data.len() > 44);
            assert_eq!(&data[0..4], b"RIFF");
            assert_eq!(&data[8..12], b"WAVE");
        }
    }

    #[test]
    fn local_speaker_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LocalSpeaker>();
    }
}
