//! SimInput — keyboard shortcut mapping to button/knob events.
//!
//! Maps terminal key events (crossterm) to vk-input trait events.

use crossterm::event::{KeyCode, KeyEvent};
use vk_input::button::{ButtonEvent, ButtonState};
use vk_input::encoder::EncoderEvent;
use vk_protocol::message::{ButtonId, Direction};

/// Result of mapping a terminal key event.
#[derive(Debug, Clone, PartialEq)]
pub enum SimEvent {
    Button(ButtonEvent),
    Encoder(EncoderEvent),
    Quit,
    Unknown,
}

/// Map a crossterm KeyEvent to a simulator event.
pub fn map_key(key: KeyEvent) -> SimEvent {
    match key.code {
        KeyCode::Enter => SimEvent::Button(ButtonEvent {
            id: ButtonId::Send,
            state: ButtonState::Pressed,
        }),
        KeyCode::Esc => SimEvent::Button(ButtonEvent {
            id: ButtonId::Cancel,
            state: ButtonState::Pressed,
        }),
        KeyCode::Char('m') => SimEvent::Button(ButtonEvent {
            id: ButtonId::Mode,
            state: ButtonState::Pressed,
        }),
        KeyCode::Char('s') => SimEvent::Button(ButtonEvent {
            id: ButtonId::Session,
            state: ButtonState::Pressed,
        }),
        KeyCode::Char('d') => SimEvent::Button(ButtonEvent {
            id: ButtonId::Delete,
            state: ButtonState::Pressed,
        }),
        KeyCode::Char('v') => SimEvent::Button(ButtonEvent {
            id: ButtonId::Voice,
            state: ButtonState::Pressed,
        }),
        KeyCode::Up => SimEvent::Encoder(EncoderEvent::Rotate {
            direction: Direction::CounterClockwise,
            steps: 1,
        }),
        KeyCode::Down => SimEvent::Encoder(EncoderEvent::Rotate {
            direction: Direction::Clockwise,
            steps: 1,
        }),
        KeyCode::Char(' ') => SimEvent::Encoder(EncoderEvent::Press),
        KeyCode::Char('q') => SimEvent::Quit,
        _ => SimEvent::Unknown,
    }
}

/// Convert a SimEvent into a UiEvent for the state machine.
pub fn sim_event_to_ui_event(event: &SimEvent) -> Option<vk_ui::event::UiEvent> {
    match event {
        SimEvent::Button(btn) => Some(vk_ui::event::UiEvent::ButtonPress(btn.id)),
        SimEvent::Encoder(EncoderEvent::Rotate { direction, steps }) => {
            let s = match direction {
                Direction::Clockwise => *steps as i8,
                Direction::CounterClockwise => -(*steps as i8),
            };
            Some(vk_ui::event::UiEvent::KnobRotate { steps: s })
        }
        SimEvent::Encoder(EncoderEvent::Press) => Some(vk_ui::event::UiEvent::KnobPress),
        SimEvent::Encoder(EncoderEvent::Release) => None,
        SimEvent::Quit | SimEvent::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn enter_maps_to_send() {
        let event = map_key(make_key(KeyCode::Enter));
        assert_eq!(
            event,
            SimEvent::Button(ButtonEvent {
                id: ButtonId::Send,
                state: ButtonState::Pressed,
            })
        );
    }

    #[test]
    fn esc_maps_to_cancel() {
        let event = map_key(make_key(KeyCode::Esc));
        assert_eq!(
            event,
            SimEvent::Button(ButtonEvent {
                id: ButtonId::Cancel,
                state: ButtonState::Pressed,
            })
        );
    }

    #[test]
    fn m_maps_to_mode() {
        let event = map_key(make_key(KeyCode::Char('m')));
        assert_eq!(
            event,
            SimEvent::Button(ButtonEvent {
                id: ButtonId::Mode,
                state: ButtonState::Pressed,
            })
        );
    }

    #[test]
    fn up_maps_to_knob_ccw() {
        let event = map_key(make_key(KeyCode::Up));
        assert_eq!(
            event,
            SimEvent::Encoder(EncoderEvent::Rotate {
                direction: Direction::CounterClockwise,
                steps: 1,
            })
        );
    }

    #[test]
    fn down_maps_to_knob_cw() {
        let event = map_key(make_key(KeyCode::Down));
        assert_eq!(
            event,
            SimEvent::Encoder(EncoderEvent::Rotate {
                direction: Direction::Clockwise,
                steps: 1,
            })
        );
    }

    #[test]
    fn space_maps_to_knob_press() {
        let event = map_key(make_key(KeyCode::Char(' ')));
        assert_eq!(event, SimEvent::Encoder(EncoderEvent::Press));
    }

    #[test]
    fn q_maps_to_quit() {
        assert_eq!(map_key(make_key(KeyCode::Char('q'))), SimEvent::Quit);
    }

    #[test]
    fn unknown_key_returns_unknown() {
        assert_eq!(map_key(make_key(KeyCode::F(1))), SimEvent::Unknown);
    }

    #[test]
    fn sim_event_to_ui_button() {
        let event = SimEvent::Button(ButtonEvent {
            id: ButtonId::Send,
            state: ButtonState::Pressed,
        });
        let ui = sim_event_to_ui_event(&event);
        assert_eq!(ui, Some(vk_ui::event::UiEvent::ButtonPress(ButtonId::Send)));
    }

    #[test]
    fn sim_event_to_ui_knob_rotate() {
        let event = SimEvent::Encoder(EncoderEvent::Rotate {
            direction: Direction::Clockwise,
            steps: 3,
        });
        let ui = sim_event_to_ui_event(&event);
        assert_eq!(ui, Some(vk_ui::event::UiEvent::KnobRotate { steps: 3 }));
    }

    #[test]
    fn sim_event_to_ui_quit_is_none() {
        assert_eq!(sim_event_to_ui_event(&SimEvent::Quit), None);
    }

    #[test]
    fn all_buttons_mapped() {
        let mappings = [
            (KeyCode::Enter, ButtonId::Send),
            (KeyCode::Esc, ButtonId::Cancel),
            (KeyCode::Char('m'), ButtonId::Mode),
            (KeyCode::Char('s'), ButtonId::Session),
            (KeyCode::Char('d'), ButtonId::Delete),
            (KeyCode::Char('v'), ButtonId::Voice),
        ];
        for (key, expected_id) in mappings {
            if let SimEvent::Button(btn) = map_key(make_key(key)) {
                assert_eq!(btn.id, expected_id, "key {key:?} should map to {expected_id:?}");
            } else {
                panic!("key {key:?} should map to button");
            }
        }
    }
}
