//! Mock implementations of input traits for testing.

use std::collections::{HashMap, VecDeque};
use vk_core::{ButtonId, LedColor, SoundType};

use crate::button::{ButtonEvent, ButtonInput};
use crate::encoder::EncoderEvent;
use crate::encoder::EncoderInput;
use crate::led::LedController;
use crate::speaker::Speaker;

// ── MockButtonInput ──

/// Mock button input that returns pre-queued events.
pub struct MockButtonInput {
    events: VecDeque<ButtonEvent>,
}

impl MockButtonInput {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Queue a button event to be returned by the next `poll()` call.
    pub fn push(&mut self, event: ButtonEvent) {
        self.events.push_back(event);
    }

    /// Number of queued events remaining.
    pub fn pending(&self) -> usize {
        self.events.len()
    }
}

impl Default for MockButtonInput {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonInput for MockButtonInput {
    fn poll(&mut self) -> Option<ButtonEvent> {
        self.events.pop_front()
    }
}

// ── MockEncoderInput ──

/// Mock encoder input that returns pre-queued events.
pub struct MockEncoderInput {
    events: VecDeque<EncoderEvent>,
}

impl MockEncoderInput {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Queue an encoder event.
    pub fn push(&mut self, event: EncoderEvent) {
        self.events.push_back(event);
    }

    /// Number of queued events remaining.
    pub fn pending(&self) -> usize {
        self.events.len()
    }
}

impl Default for MockEncoderInput {
    fn default() -> Self {
        Self::new()
    }
}

impl EncoderInput for MockEncoderInput {
    fn poll(&mut self) -> Option<EncoderEvent> {
        self.events.pop_front()
    }
}

// ── MockLedController ──

/// Mock LED controller that records all calls.
pub struct MockLedController {
    /// Current state: (color, blink) per button.
    pub button_leds: HashMap<ButtonIdKey, (LedColor, bool)>,
    /// Current knob ring color.
    pub knob_ring: LedColor,
}

/// Wrapper for ButtonId to use as HashMap key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ButtonIdKey(pub ButtonId);

impl MockLedController {
    pub fn new() -> Self {
        Self {
            button_leds: HashMap::new(),
            knob_ring: LedColor::OFF,
        }
    }

    /// Get the current LED state for a button.
    pub fn get_button(&self, button: ButtonId) -> Option<&(LedColor, bool)> {
        self.button_leds.get(&ButtonIdKey(button))
    }
}

impl Default for MockLedController {
    fn default() -> Self {
        Self::new()
    }
}

impl LedController for MockLedController {
    fn set_button_led(&mut self, button: ButtonId, color: LedColor) {
        let entry = self
            .button_leds
            .entry(ButtonIdKey(button))
            .or_insert((LedColor::OFF, false));
        entry.0 = color;
    }

    fn set_button_blink(&mut self, button: ButtonId, on: bool) {
        let entry = self
            .button_leds
            .entry(ButtonIdKey(button))
            .or_insert((LedColor::OFF, false));
        entry.1 = on;
    }

    fn set_knob_ring(&mut self, color: LedColor) {
        self.knob_ring = color;
    }

    fn all_off(&mut self) {
        self.button_leds.clear();
        self.knob_ring = LedColor::OFF;
    }
}

// ── MockSpeaker ──

/// Mock speaker that records all played sounds.
pub struct MockSpeaker {
    /// History of played sounds.
    pub played: Vec<SoundType>,
    /// Current volume (0-100).
    pub volume: u8,
    /// Whether muted.
    pub muted: bool,
}

impl MockSpeaker {
    pub fn new() -> Self {
        Self {
            played: Vec::new(),
            volume: 50,
            muted: false,
        }
    }
}

impl Default for MockSpeaker {
    fn default() -> Self {
        Self::new()
    }
}

impl Speaker for MockSpeaker {
    fn play(&mut self, sound: SoundType) {
        self.played.push(sound);
    }

    fn set_volume(&mut self, volume: u8) {
        self.volume = volume;
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::button::ButtonState;
    use vk_core::Direction;

    #[test]
    fn mock_button_input_empty() {
        let mut input = MockButtonInput::new();
        assert_eq!(input.pending(), 0);
        assert!(input.poll().is_none());
    }

    #[test]
    fn mock_button_input_fifo() {
        let mut input = MockButtonInput::new();
        input.push(ButtonEvent {
            id: ButtonId::Send,
            state: ButtonState::Pressed,
        });
        input.push(ButtonEvent {
            id: ButtonId::Send,
            state: ButtonState::Released,
        });
        assert_eq!(input.pending(), 2);

        let e1 = input.poll().unwrap();
        assert_eq!(e1.id, ButtonId::Send);
        assert_eq!(e1.state, ButtonState::Pressed);

        let e2 = input.poll().unwrap();
        assert_eq!(e2.state, ButtonState::Released);

        assert!(input.poll().is_none());
    }

    #[test]
    fn mock_encoder_input_empty() {
        let mut input = MockEncoderInput::new();
        assert_eq!(input.pending(), 0);
        assert!(input.poll().is_none());
    }

    #[test]
    fn mock_encoder_input_fifo() {
        let mut input = MockEncoderInput::new();
        input.push(EncoderEvent::Rotate {
            direction: Direction::Clockwise,
            steps: 3,
        });
        input.push(EncoderEvent::Press);
        input.push(EncoderEvent::Release);

        assert_eq!(input.pending(), 3);

        let e1 = input.poll().unwrap();
        assert!(matches!(
            e1,
            EncoderEvent::Rotate {
                direction: Direction::Clockwise,
                steps: 3
            }
        ));

        assert!(matches!(input.poll(), Some(EncoderEvent::Press)));
        assert!(matches!(input.poll(), Some(EncoderEvent::Release)));
        assert!(input.poll().is_none());
    }

    #[test]
    fn mock_led_controller_set_and_get() {
        let mut led = MockLedController::new();
        led.set_button_led(ButtonId::Send, LedColor::GREEN);
        led.set_button_blink(ButtonId::Send, true);

        let (color, blink) = led.get_button(ButtonId::Send).unwrap();
        assert_eq!(*color, LedColor::GREEN);
        assert!(*blink);
    }

    #[test]
    fn mock_led_controller_knob_ring() {
        let mut led = MockLedController::new();
        assert_eq!(led.knob_ring, LedColor::OFF);
        led.set_knob_ring(LedColor::AMBER);
        assert_eq!(led.knob_ring, LedColor::AMBER);
    }

    #[test]
    fn mock_led_controller_all_off() {
        let mut led = MockLedController::new();
        led.set_button_led(ButtonId::Send, LedColor::RED);
        led.set_knob_ring(LedColor::GREEN);
        led.all_off();
        assert!(led.button_leds.is_empty());
        assert_eq!(led.knob_ring, LedColor::OFF);
    }

    #[test]
    fn mock_speaker_play_records() {
        let mut speaker = MockSpeaker::new();
        speaker.play(SoundType::Click);
        speaker.play(SoundType::Error);
        assert_eq!(speaker.played, vec![SoundType::Click, SoundType::Error]);
    }

    #[test]
    fn mock_speaker_volume_and_mute() {
        let mut speaker = MockSpeaker::new();
        assert_eq!(speaker.volume, 50);
        assert!(!speaker.muted);

        speaker.set_volume(80);
        speaker.set_muted(true);
        assert_eq!(speaker.volume, 80);
        assert!(speaker.muted);
    }
}
