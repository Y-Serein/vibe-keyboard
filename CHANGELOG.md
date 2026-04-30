# Changelog

## [Integration] - 2026-03-31

### Added — Real E2E Integration (post-M6)
- **vk-daemon serve mode**: `vk-daemon serve --headless` — persistent daemon with:
  - axum HTTP hook server on :3456 (POST /event, GET /health, GET /sessions)
  - IPC listener on Unix socket (auto-reconnect on simulator disconnect)
  - EventHandler: ButtonPress→action mapping (Send=Allow, Cancel=Deny, Mode=YOLO toggle)
  - Full session lifecycle via hooks (session_start/end/status/permission_request)
  - YOLO auto-allow/deny with glob pattern matching
  - Session backfill on simulator (re)connect
- **vk-simulator IPC client**: `--cli` connects to daemon, `--standalone` for demo mode
  - Receives DownlinkMessage → updates UI state machine in real-time
  - Sends UplinkMessage (button/knob/permission) → daemon handles
  - Automatic fallback to standalone if daemon unavailable
- **Real IPC E2E tests**: Unix socket session backfill + permission flow + concurrent bidirectional

### Verified (tmux real testing)
- daemon serve → curl hook inject → sessions API → YOLO auto-deny/allow
- simulator --cli connects to daemon → LCD shows `[daemon]` mode
- Permission flow: curl inject → daemon queues → simulator shows Allow → user presses Enter → daemon logs `PermissionResponse(Allow)`
- 222 tests, 0 clippy warnings

## [M6] - 2026-03-31

### Added
- Tauri v2 desktop app (desktop/src-tauri + desktop/src)
- LcdMirror.tsx — Canvas framebuffer rendering via Tauri events
- VirtualKeyboard.tsx — 6 buttons + knob with V2 physical layout
- SessionList.tsx — color-coded session status with permission markers
- Tauri commands: get_sessions, button_press (typed), knob_rotate, knob_press, inject_mock_sessions
- emit_lcd_frame + emit_session_update event pipeline to frontend
- Tauri v2 capabilities (core + event permissions)
- vk-daemon refactored to lib.rs (library usable from Tauri backend)

### Stats
- 218 tests (216 Rust + 2 Tauri), 0 clippy warnings, TS + Vite build clean
- Checkpoint: codex, 5 issues + 1 suggestion fixed
- Project complete: 6/6 milestones, 37/40 tasks (3 deferred: onboarding, cursor adapter, macro system)

## [M5] - 2026-03-31

### Added
- E2E integration tests: button round-trip, session push, permission complete flow, knob switch, bidirectional concurrent, invalid session_id, full message type coverage
- True E2E tests wiring ScreenStateMachine through ChannelTransport (permission + knob switch with UI state transitions)
- TOML config system (DaemonConfig: general/yolo/ipc sections, partial defaults, save/load)

### Fixed
- Config log_level default now correctly returns "info" (was empty string)

### Stats
- 216 tests, 0 clippy warnings
- Formal pipeline: testing (cargo test, 7 BDD) → test-review (10 AP, 0 P0) → checkpoint (codex, 3 fixed)
- Milestone 5/6 complete

## [M4] - 2026-03-31

### Added
- vk-daemon binary crate with CLI subcommands (session/focus/config)
- SessionStore: in-memory HashMap with protocol conversion
- HookEvent parser: Claude Code hook payload parsing with type aliases (init/exit/tool_use/permission_request)
- FocusManager: macOS osascript window activation with error propagation
- YoloConfig: deny > allow > ask permission auto-approval with glob matching
- PermissionQueue: FIFO queue with Always action persistence (always_allow list)
- Session mock injection for testing

### Fixed
- Focus script properly errors on missing windows (no more try swallowing)
- Glob matching upgrades single * to ** for path separator support

### Stats
- 200 tests passing (42 new), 0 clippy warnings
- Testing: gate PASS, 12 BDD scenarios covered (F7/F8/F9)
- Checkpoint: codex, 3 issues fixed + 2 by-design

## [M3] - 2026-03-31

### Added
- SimDisplay: framebuffer RGB565 → terminal half-block characters (▀) rendering
- SimInput: crossterm keyboard mapping (Enter/Esc/m/s/d/v → buttons, ↑↓ → knob, Space → press)
- SimLed: terminal LED status display with ANSI true-color output
- SimSpeaker: terminal bell audio feedback
- CLI event loop: 30fps render + crossterm input polling
- CLI subcommands: button, knob, display (test-pattern/status), speaker
- Demo sessions injected for standalone testing

### Changed
- CLI rendering: unified framebuffer pixel path (same as GUI/Tauri/hardware)
- ButtonEvent/EncoderEvent: added PartialEq derive for test assertions

### Fixed
- ANSI state leak: black pixels now reset terminal color before rendering space

### Stats
- 158 tests passing (40 new), 0 clippy warnings
- Testing: gate PASS, 5/6 M3 BDD scenarios (F5.5 deferred to M5)
- Checkpoint: codex, 2 issues fixed

## [M2] - 2026-03-31

### Added
- Widget system: Label, StatusBar, SessionCard with configurable alignment
- 6×10 bitmap font for LCD rendering (printable ASCII 0x20-0x7E)
- ScreenStateMachine: full Standby/Normal/Select/Allow state transitions
- Event handling: UiEvent → state transition → UiAction output
- Render engine: 4 screen states drawn to DynFramebuffer via RenderContext
- Tick-based idle timeout (90 frames = 3s at 30fps) for Select → Normal
- Animation primitives: blink, pulse, slide, alternate (frame-counter driven)
- Allow screen: knob rotation through Allow/Deny/Always, SEND confirms selection
- Multi-permission queue with SESSION button cycling
- Green border + accent highlighting for Allow screen

### Fixed
- UiEvent uses String (dropped old no_std byte arrays)
- SEND in Allow confirms current selection (not forced Allow)
- Standby screen renders time from RenderContext

### Stats
- 118 tests passing (54 new), 0 clippy warnings
- Testing: gate PASS, 11/11 M2 BDD scenarios mapped
- Test-review: self, 0 P0, 0 new P1
- Checkpoint: codex, 5 issues + 2 suggestions, all fixed

## [M1] - 2026-03-31

### Added
- Binary codec for all protocol messages (uplink + downlink), tag-based format
- ChannelTransport for in-process testing (tokio mpsc)
- IPC Transport over Unix domain sockets (length-prefixed framing)
- DynFramebuffer with double-buffering, write_pixels, clear, boundary clipping
- RGB565 ↔ RGBA color space conversion
- Mock implementations for ButtonInput, EncoderInput, LedController, Speaker
- Semantic message protocol: PermissionResponse, SessionSwitch
- Integration tests: 4 concurrent/bidirectional channel tests

### Fixed
- IPC decode errors now surface as TransportError::EncodingError (not silently dropped)
- Framebuffer saturating_add prevents u16 overflow on extreme coordinates

### Stats
- 64 tests passing, 0 clippy warnings
- Testing: gate PASS, 18/18 M1 BDD scenarios mapped
- Test-review: codex, 0 P0, 2 P1 deferred to M5
- Checkpoint: codex, 5 issues found and fixed
