mod sim_display;
mod sim_input;
mod sim_led;
mod sim_speaker;

use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event};
use crossterm::terminal;
use tokio::sync::mpsc;

use vk_display::framebuffer::DynFramebuffer;
use vk_input::led::LedController;
use vk_input::speaker::Speaker;
use vk_transport::IpcTransport;
use vk_protocol::message::*;
use vk_transport::Transport;
use vk_ui::event::UiEvent;
use vk_ui::renderer::{self, RenderContext, LCD_H, LCD_W};
use vk_ui::screen::{ScreenStateMachine, UiAction};

use sim_display::framebuffer_to_terminal;
use sim_input::{map_key, sim_event_to_ui_event, SimEvent};
use sim_led::SimLed;
use sim_speaker::SimSpeaker;

#[derive(Parser)]
#[command(name = "vk-simulator")]
#[command(about = "Vibe Keyboard simulator — emulates keyboard firmware")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Run in CLI mode (terminal UI) — default
    #[arg(long)]
    cli: bool,

    /// Run in GUI mode (graphical window)
    #[arg(long)]
    gui: bool,

    /// Daemon IPC socket path (connects to vk-daemon serve)
    #[arg(long, default_value = "/tmp/vk-daemon.sock")]
    daemon: String,

    /// Run without connecting to daemon (demo mode with fake sessions)
    #[arg(long)]
    standalone: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Simulate a button press
    Button {
        /// Button action: press, release
        action: String,
        /// Button name: send, cancel, mode, session, delete, voice
        name: String,
    },
    /// Simulate knob action
    Knob {
        /// Knob action: rotate, press
        action: String,
        /// Steps for rotate (+N/-N)
        #[arg(default_value = "1")]
        steps: i8,
    },
    /// Display test commands
    Display {
        /// Display action: test-pattern, status
        action: String,
    },
    /// Speaker test
    Speaker {
        /// Sound: permission, complete, error, click
        sound: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => handle_subcommand(cmd),
        None => {
            if cli.gui {
                eprintln!("[sim] GUI mode not yet implemented");
                std::process::exit(1);
            } else {
                run_cli_mode(cli.standalone, &cli.daemon);
            }
        }
    }
}

fn handle_subcommand(cmd: Commands) {
    let output = execute_subcommand(&cmd);
    print!("{output}");
}

/// Execute a subcommand and return its output as a string (testable).
fn execute_subcommand(cmd: &Commands) -> String {
    match cmd {
        Commands::Button { action, name } => {
            let button_id = parse_button_name(name);
            match button_id {
                Some(id) => format!("[sim] button {action} {id:?}\n"),
                None => format!("[sim] unknown button: {name}\n"),
            }
        }
        Commands::Knob { action, steps } => {
            format!("[sim] knob {action} steps={steps}\n")
        }
        Commands::Display { action } => match action.as_str() {
            "test-pattern" => {
                let mut fb = DynFramebuffer::new(LCD_W, LCD_H);
                let bar_w = LCD_W / 6;
                let colors = [
                    vk_display::color::Rgb565::RED,
                    vk_display::color::Rgb565::GREEN,
                    vk_display::color::Rgb565::BLUE,
                    vk_display::color::Rgb565::WHITE,
                    vk_display::color::Rgb565::new(255, 255, 0),
                    vk_display::color::Rgb565::new(255, 0, 255),
                ];
                for (i, color) in colors.iter().enumerate() {
                    fb.fill_rect(i as u16 * bar_w, 0, bar_w, LCD_H, *color);
                }
                fb.swap();
                let lines = framebuffer_to_terminal(&fb);
                let mut out = String::new();
                for line in &lines {
                    out.push_str(line);
                    out.push('\n');
                }
                out.push_str(&format!(
                    "[sim] test-pattern rendered ({LCD_W}x{LCD_H} → {} terminal rows)\n",
                    lines.len()
                ));
                out
            }
            "status" => {
                format!("[sim] display: {LCD_W}x{LCD_H} LCD, half-block terminal rendering\n")
            }
            _ => format!("[sim] unknown display action: {action}\n"),
        },
        Commands::Speaker { sound } => {
            let mut speaker = SimSpeaker::new();
            speaker.set_muted(true);
            match sound.as_str() {
                "permission" => speaker.play(SoundType::PermissionAlert),
                "complete" => speaker.play(SoundType::SessionComplete),
                "error" => speaker.play(SoundType::Error),
                "click" => speaker.play(SoundType::Click),
                _ => return format!("[sim] unknown sound: {sound}\n"),
            }
            if let Some(s) = speaker.last_sound() {
                format!("[sim] played {s:?}\n")
            } else {
                String::new()
            }
        }
    }
}

fn run_cli_mode(standalone: bool, daemon_socket: &str) {
    // Setup terminal — requires a real TTY
    if let Err(e) = terminal::enable_raw_mode() {
        eprintln!("[sim] CLI mode requires a terminal (TTY). Error: {e}");
        eprintln!("[sim] Use subcommands (button/knob/display/speaker) for non-interactive mode.");
        std::process::exit(1);
    }
    let mut stdout = io::stdout();
    if let Err(e) = crossterm::execute!(stdout, terminal::EnterAlternateScreen) {
        terminal::disable_raw_mode().ok();
        eprintln!("[sim] Failed to setup terminal: {e}");
        std::process::exit(1);
    }

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let result = rt.block_on(cli_event_loop_async(&mut stdout, standalone, daemon_socket));

    // Restore terminal
    crossterm::execute!(stdout, terminal::LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();

    if let Err(e) = result {
        eprintln!("[sim] error: {e}");
    }
}

/// Connect to daemon with retry. Returns None if connection fails after retries.
async fn connect_to_daemon(
    socket: &str,
    downlink_tx: &mpsc::Sender<DownlinkMessage>,
    stdout: &mut io::Stdout,
) -> Option<Arc<IpcTransport>> {
    for attempt in 1..=3 {
        match IpcTransport::connect(socket).await {
            Ok(t) => {
                let t = Arc::new(t);
                let reader = Arc::clone(&t);
                let tx = downlink_tx.clone();
                tokio::spawn(async move {
                    while let Ok(msg) = reader.recv_downlink().await {
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                });
                write_status(stdout, "[sim] connected to daemon").ok();
                return Some(t);
            }
            Err(e) => {
                let msg = format!("[sim] waiting for daemon... (attempt {attempt}/3: {e:?})");
                write_status(stdout, &msg).ok();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    write_status(stdout, "[sim] daemon not available — waiting for connection... (press q to quit)").ok();
    None
}

async fn cli_event_loop_async(
    stdout: &mut io::Stdout,
    standalone: bool,
    daemon_socket: &str,
) -> io::Result<()> {
    let mut sm = ScreenStateMachine::new();
    let mut fb = DynFramebuffer::new(LCD_W, LCD_H);
    let mut led = SimLed::new();
    let mut speaker = SimSpeaker::new();
    // Speaker starts unmuted — daemon controls mute via SetMuted downlink.

    // Channel for downlink messages from IPC reader task
    let (downlink_tx, mut downlink_rx) = mpsc::channel::<DownlinkMessage>(64);

    // Try connecting to daemon (with reconnect support)
    let transport: Option<Arc<IpcTransport>> = if standalone {
        inject_demo_sessions(&mut sm);
        write_status(stdout, "[sim] standalone mode (demo sessions)")?;
        None
    } else {
        write_status(stdout, &format!("[sim] connecting to daemon at {daemon_socket}..."))?;
        stdout.flush()?;
        connect_to_daemon(daemon_socket, &downlink_tx, stdout).await
    };

    let ctx = RenderContext {
        time_str: "00:00".into(),
    };

    let connected = transport.is_some();

    loop {
        // Process any pending downlink messages from daemon
        while let Ok(msg) = downlink_rx.try_recv() {
            process_downlink(&mut sm, &mut led, &mut speaker, &msg);
        }

        // Render
        renderer::render(&sm, &mut fb, &ctx);
        fb.swap();
        let lines = framebuffer_to_terminal(&fb);

        // Draw to terminal
        crossterm::execute!(stdout, crossterm::cursor::MoveTo(0, 0))?;
        for line in &lines {
            write!(stdout, "{line}")?;
            crossterm::execute!(stdout, crossterm::cursor::MoveToNextLine(1))?;
        }

        // LED status line
        let led_line = led.status_line(sm.frame());
        write!(stdout, "\x1b[K{led_line}")?;
        crossterm::execute!(stdout, crossterm::cursor::MoveToNextLine(1))?;

        // Status + help line
        let mode = if connected { "daemon" } else { "standalone" };
        write!(
            stdout,
            "\x1b[K[{mode}] Enter=SEND Esc=CANCEL m=MODE s=SESSION d=DEL ↑↓=KNOB Space=Press q=Quit"
        )?;
        stdout.flush()?;

        // Tick
        sm.tick();

        // Poll input (33ms ≈ 30fps)
        if event::poll(Duration::from_millis(33))?
            && let Event::Key(key) = event::read()?
        {
            let sim_event = map_key(key);

            if sim_event == SimEvent::Quit {
                break;
            }

            if let Some(ui_event) = sim_event_to_ui_event(&sim_event) {
                let action = sm.handle_event(&ui_event);

                // Send uplink to daemon if connected
                if let Some(ref t) = transport {
                    send_uplink_for_event(t, &sim_event, &action).await;
                }

                handle_ui_action(&action, &mut led, &mut speaker);
            }
        }
    }

    Ok(())
}

/// Process a downlink message from daemon → update UI state.
fn process_downlink(
    sm: &mut ScreenStateMachine,
    led: &mut SimLed,
    speaker: &mut SimSpeaker,
    msg: &DownlinkMessage,
) {
    match msg {
        DownlinkMessage::SessionListUpdate { sessions, .. } => {
            // Full replacement with rich data — pass complete SessionInfo
            sm.handle_event(&UiEvent::SessionListReplace {
                sessions: sessions.clone(),
            });
        }
        DownlinkMessage::SessionStatusChange { session_id, status } => {
            sm.handle_event(&UiEvent::SessionUpdate {
                session_id: *session_id,
                name: String::new(), // keep existing name
                status: *status,
            });
        }
        DownlinkMessage::PermissionRequest {
            session_id,
            action_desc,
        } => {
            sm.handle_event(&UiEvent::PermissionRequest {
                session_id: *session_id,
                action_desc: action_desc.clone(),
            });
        }
        DownlinkMessage::DismissPermission { session_id } => {
            sm.handle_event(&UiEvent::PermissionResolved {
                session_id: *session_id,
                action: PermissionAction::Allow,
            });
        }
        DownlinkMessage::SetLed {
            button,
            color,
            blink,
        } => {
            led.set_button_led(*button, *color);
            led.set_button_blink(*button, *blink);
        }
        DownlinkMessage::SetKnobRing(color) => {
            led.set_knob_ring(*color);
        }
        DownlinkMessage::PlaySound(sound) => {
            speaker.play(*sound);
        }
        DownlinkMessage::NotificationListUpdate { notifications } => {
            // Convert protocol NotificationInfo to UI NotificationEntry
            let entries: Vec<vk_ui::screen::NotificationEntry> = notifications
                .iter()
                .map(|n| vk_ui::screen::NotificationEntry {
                    id: n.id,
                    session_id: n.session_id,
                    session_name: n.session_name.clone(),
                    status: n.status,
                    description: n.description.clone(),
                    timestamp: n.timestamp,
                    read: n.read,
                })
                .collect();
            sm.handle_event(&UiEvent::NotificationListUpdate {
                notifications: entries,
            });
        }
        DownlinkMessage::FrameData { .. } => {
            // FrameData is for GUI Canvas rendering, not CLI simulator
        }
        DownlinkMessage::SetVolume(vol) => {
            speaker.set_volume(*vol);
        }
        DownlinkMessage::SetMuted(muted) => {
            speaker.set_muted(*muted);
        }
        DownlinkMessage::SetSoundMapping { .. } => {
            // Sound mapping customization — not yet implemented in simulator
        }
    }
}

/// Send an uplink message to daemon based on user input.
async fn send_uplink_for_event(
    transport: &Arc<IpcTransport>,
    sim_event: &SimEvent,
    action: &UiAction,
) {
    let msg = match sim_event {
        SimEvent::Button(btn) => Some(UplinkMessage::ButtonPress(btn.id)),
        SimEvent::Encoder(vk_input::encoder::EncoderEvent::Press) => Some(UplinkMessage::KnobPress),
        SimEvent::Encoder(vk_input::encoder::EncoderEvent::Rotate { direction, steps }) => {
            Some(UplinkMessage::KnobRotate {
                direction: *direction,
                steps: *steps,
            })
        }
        _ => None,
    };

    // Also send semantic messages for UI actions
    match action {
        UiAction::PermissionResponse { session_id, action } => {
            let _ = transport
                .send_uplink(&UplinkMessage::PermissionResponse {
                    session_id: *session_id,
                    action: *action,
                })
                .await;
            return; // don't double-send
        }
        UiAction::SwitchSession { session_id } => {
            let _ = transport
                .send_uplink(&UplinkMessage::SessionSwitch {
                    session_id: *session_id,
                })
                .await;
            return;
        }
        UiAction::None => {}
    }

    if let Some(msg) = msg {
        let _ = transport.send_uplink(&msg).await;
    }
}

fn handle_ui_action(action: &UiAction, led: &mut SimLed, _speaker: &mut SimSpeaker) {
    match action {
        UiAction::SwitchSession { session_id } => {
            tracing::info!("switch to session {session_id}");
        }
        UiAction::PermissionResponse { session_id, action } => {
            tracing::info!("permission response: session={session_id} action={action:?}");
            led.set_knob_ring(LedColor::OFF);
        }
        UiAction::None => {}
    }
}

fn write_status(stdout: &mut io::Stdout, msg: &str) -> io::Result<()> {
    crossterm::execute!(stdout, crossterm::cursor::MoveTo(0, 0))?;
    write!(stdout, "\x1b[K{msg}")?;
    stdout.flush()
}

fn inject_demo_sessions(sm: &mut ScreenStateMachine) {
    sm.handle_event(&UiEvent::SessionUpdate {
        session_id: 1,
        name: "RustAgent".into(),
        status: SessionStatus::Thinking,
    });
    sm.handle_event(&UiEvent::SessionUpdate {
        session_id: 2,
        name: "FrontEnd".into(),
        status: SessionStatus::Idle,
    });
    sm.handle_event(&UiEvent::SessionUpdate {
        session_id: 3,
        name: "DevOps".into(),
        status: SessionStatus::PermissionNeeded,
    });
}

fn parse_button_name(name: &str) -> Option<ButtonId> {
    match name.to_lowercase().as_str() {
        "send" => Some(ButtonId::Send),
        "cancel" => Some(ButtonId::Cancel),
        "mode" => Some(ButtonId::Mode),
        "session" => Some(ButtonId::Session),
        "delete" => Some(ButtonId::Delete),
        "voice" => Some(ButtonId::Voice),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_button_name_all() {
        assert_eq!(parse_button_name("send"), Some(ButtonId::Send));
        assert_eq!(parse_button_name("cancel"), Some(ButtonId::Cancel));
        assert_eq!(parse_button_name("mode"), Some(ButtonId::Mode));
        assert_eq!(parse_button_name("session"), Some(ButtonId::Session));
        assert_eq!(parse_button_name("delete"), Some(ButtonId::Delete));
        assert_eq!(parse_button_name("voice"), Some(ButtonId::Voice));
        assert_eq!(parse_button_name("SEND"), Some(ButtonId::Send));
        assert_eq!(parse_button_name("unknown"), None);
    }

    #[test]
    fn inject_demo_creates_sessions() {
        let mut sm = ScreenStateMachine::new();
        inject_demo_sessions(&mut sm);
        assert_eq!(sm.sessions().len(), 3);
        assert_eq!(sm.sessions()[0].name, "RustAgent");
    }

    #[test]
    fn handle_ui_action_switch_no_panic() {
        let mut led = SimLed::new();
        let mut speaker = SimSpeaker::new();
        handle_ui_action(
            &UiAction::SwitchSession { session_id: 1 },
            &mut led,
            &mut speaker,
        );
    }

    #[test]
    fn handle_ui_action_permission_clears_knob() {
        let mut led = SimLed::new();
        led.set_knob_ring(LedColor::GREEN);
        let mut speaker = SimSpeaker::new();
        handle_ui_action(
            &UiAction::PermissionResponse {
                session_id: 1,
                action: vk_protocol::message::PermissionAction::Allow,
            },
            &mut led,
            &mut speaker,
        );
        assert_eq!(led.knob_ring_color(), LedColor::OFF);
    }

    #[test]
    fn subcommand_button_press_send() {
        let cmd = Commands::Button {
            action: "press".into(),
            name: "send".into(),
        };
        let output = execute_subcommand(&cmd);
        assert_eq!(output, "[sim] button press Send\n");
    }

    #[test]
    fn subcommand_button_unknown() {
        let cmd = Commands::Button {
            action: "press".into(),
            name: "xyz".into(),
        };
        let output = execute_subcommand(&cmd);
        assert!(output.contains("unknown button"), "should report unknown: {output}");
    }

    #[test]
    fn subcommand_display_test_pattern() {
        let cmd = Commands::Display {
            action: "test-pattern".into(),
        };
        let output = execute_subcommand(&cmd);
        assert!(output.contains("[sim] test-pattern rendered"));
        assert!(output.contains("960x412"));
        assert!(output.contains("\x1b[38;2;"));
    }

    #[test]
    fn subcommand_display_status() {
        let cmd = Commands::Display {
            action: "status".into(),
        };
        let output = execute_subcommand(&cmd);
        assert!(output.contains("960x412"));
        assert!(output.contains("half-block"));
    }

    #[test]
    fn cli_startup_renders_framebuffer() {
        let mut sm = ScreenStateMachine::new();
        inject_demo_sessions(&mut sm);
        let mut fb = DynFramebuffer::new(LCD_W, LCD_H);
        let ctx = RenderContext {
            time_str: "12:00".into(),
        };
        renderer::render(&sm, &mut fb, &ctx);
        fb.swap();
        let lines = framebuffer_to_terminal(&fb);
        assert!(!lines.is_empty());
        let has_color = lines.iter().any(|l| l.contains("\x1b[38;2;"));
        assert!(has_color);
    }

    #[test]
    fn subcommand_speaker_play() {
        let cmd = Commands::Speaker {
            sound: "permission".into(),
        };
        let output = execute_subcommand(&cmd);
        assert!(output.contains("[sim] played PermissionAlert"));
    }

    #[test]
    fn subcommand_knob_rotate() {
        let cmd = Commands::Knob {
            action: "rotate".into(),
            steps: 3,
        };
        let output = execute_subcommand(&cmd);
        assert_eq!(output, "[sim] knob rotate steps=3\n");
    }

    #[test]
    fn process_downlink_session_list() {
        let mut sm = ScreenStateMachine::new();
        let mut led = SimLed::new();
        let mut speaker = SimSpeaker::new();
        let msg = DownlinkMessage::SessionListUpdate {
            sessions: vec![SessionInfo {
                id: 1,
                name: "Test".into(),
                status: SessionStatus::Thinking,
                has_permission_request: false,
                ..Default::default()
            }],
            active_index: 0,
        };
        process_downlink(&mut sm, &mut led, &mut speaker, &msg);
        assert_eq!(sm.sessions().len(), 1);
        assert_eq!(sm.sessions()[0].name, "Test");
    }

    #[test]
    fn process_downlink_permission() {
        let mut sm = ScreenStateMachine::new();
        let mut led = SimLed::new();
        let mut speaker = SimSpeaker::new();
        // Add a session first
        sm.handle_event(&UiEvent::SessionUpdate {
            session_id: 1,
            name: "A".into(),
            status: SessionStatus::Idle,
        });
        let msg = DownlinkMessage::PermissionRequest {
            session_id: 1,
            action_desc: "Write main.rs".into(),
        };
        process_downlink(&mut sm, &mut led, &mut speaker, &msg);
        assert_eq!(sm.state(), vk_ui::screen::ScreenState::Allow);
    }
}
