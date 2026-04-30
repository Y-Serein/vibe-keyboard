//! SimLed — terminal LED controller.
//!
//! Stores LED states for display in the terminal status bar.

use std::collections::HashMap;
use vk_input::led::LedController;
use vk_protocol::message::{ButtonId, LedColor};

/// Terminal-based LED controller that stores state for rendering.
#[derive(Debug)]
pub struct SimLed {
    button_leds: HashMap<ButtonId, LedState>,
    knob_ring: LedColor,
}

#[derive(Debug, Clone, Copy)]
pub struct LedState {
    pub color: LedColor,
    pub blink: bool,
}

impl Default for SimLed {
    fn default() -> Self {
        Self::new()
    }
}

impl SimLed {
    pub fn new() -> Self {
        Self {
            button_leds: HashMap::new(),
            knob_ring: LedColor::OFF,
        }
    }

    pub fn get_button_led(&self, button: ButtonId) -> LedState {
        self.button_leds
            .get(&button)
            .copied()
            .unwrap_or(LedState {
                color: LedColor::OFF,
                blink: false,
            })
    }

    #[allow(dead_code)]
    pub fn knob_ring_color(&self) -> LedColor {
        self.knob_ring
    }

    /// Format LED status as a single-line string for terminal display.
    pub fn status_line(&self, frame: u32) -> String {
        let buttons = [
            ButtonId::Delete,
            ButtonId::Cancel,
            ButtonId::Mode,
            ButtonId::Session,
            ButtonId::Send,
            ButtonId::Voice,
        ];

        let mut parts = Vec::new();
        for btn in buttons {
            let state = self.get_button_led(btn);
            let visible = if state.blink {
                (frame / 15).is_multiple_of(2) // blink every 15 frames
            } else {
                true
            };
            let color_str = if visible && state.color != LedColor::OFF {
                format!(
                    "\x1b[38;2;{};{};{}m●\x1b[0m",
                    state.color.r, state.color.g, state.color.b
                )
            } else {
                "○".to_string()
            };
            parts.push(format!("{:?}:{color_str}", btn));
        }

        let knob = if self.knob_ring != LedColor::OFF {
            format!(
                "\x1b[38;2;{};{};{}m◉\x1b[0m",
                self.knob_ring.r, self.knob_ring.g, self.knob_ring.b
            )
        } else {
            "◎".to_string()
        };
        parts.push(format!("Knob:{knob}"));

        parts.join(" ")
    }
}

impl LedController for SimLed {
    fn set_button_led(&mut self, button: ButtonId, color: LedColor) {
        let entry = self.button_leds.entry(button).or_insert(LedState {
            color: LedColor::OFF,
            blink: false,
        });
        entry.color = color;
    }

    fn set_button_blink(&mut self, button: ButtonId, on: bool) {
        let entry = self.button_leds.entry(button).or_insert(LedState {
            color: LedColor::OFF,
            blink: false,
        });
        entry.blink = on;
    }

    fn set_knob_ring(&mut self, color: LedColor) {
        self.knob_ring = color;
    }

    fn all_off(&mut self) {
        self.button_leds.clear();
        self.knob_ring = LedColor::OFF;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_all_off() {
        let led = SimLed::new();
        assert_eq!(led.knob_ring_color(), LedColor::OFF);
        assert_eq!(led.get_button_led(ButtonId::Send).color, LedColor::OFF);
    }

    #[test]
    fn set_and_get_button_led() {
        let mut led = SimLed::new();
        led.set_button_led(ButtonId::Mode, LedColor::GREEN);
        assert_eq!(led.get_button_led(ButtonId::Mode).color, LedColor::GREEN);
    }

    #[test]
    fn set_blink() {
        let mut led = SimLed::new();
        led.set_button_blink(ButtonId::Session, true);
        assert!(led.get_button_led(ButtonId::Session).blink);
    }

    #[test]
    fn set_knob_ring() {
        let mut led = SimLed::new();
        led.set_knob_ring(LedColor::AMBER);
        assert_eq!(led.knob_ring_color(), LedColor::AMBER);
    }

    #[test]
    fn all_off_clears() {
        let mut led = SimLed::new();
        led.set_button_led(ButtonId::Send, LedColor::RED);
        led.set_knob_ring(LedColor::GREEN);
        led.all_off();
        assert_eq!(led.get_button_led(ButtonId::Send).color, LedColor::OFF);
        assert_eq!(led.knob_ring_color(), LedColor::OFF);
    }

    #[test]
    fn status_line_contains_all_buttons() {
        let led = SimLed::new();
        let line = led.status_line(0);
        assert!(line.contains("Delete"));
        assert!(line.contains("Send"));
        assert!(line.contains("Knob"));
    }

    #[test]
    fn status_line_colored_led() {
        let mut led = SimLed::new();
        led.set_button_led(ButtonId::Mode, LedColor::GREEN);
        let line = led.status_line(0);
        // Should contain ANSI color escape for green
        assert!(line.contains("\x1b[38;2;0;200;0m●"));
    }
}
