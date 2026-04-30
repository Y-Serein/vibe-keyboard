# Vibe Keyboard

AI Agent 物理控制器 — 通过按钮/旋钮/LCD 管理多个 AI coding session。

```
┌─────────────────────────────────────────────────┐
│  Claude Code / Codex / Cursor ──hook──► daemon  │
│                                    ↕ IPC        │
│  Terminal ◄── LCD render ── simulator           │
│  Tauri Desktop GUI (Canvas LCD + 虚拟按钮)       │
└─────────────────────────────────────────────────┘
```

## Features

- **LCD 显示**: 800×340 RGB565 framebuffer, 中文 unifont 支持, 5 屏切换 (Standby/Normal/Select/Allow/Notify)
- **Session 管理**: 自动检测 Claude Code session, hook 事件实时状态, JSONL transcript 解析 (model/tokens/cost/context)
- **窗口跳转**: iTerm2 TTY 精确跳转, Ghostty/VSCode/Warp 通用跳转, tmux 多客户端支持
- **权限审批**: Permission 请求阻塞回复, Allow/Deny/Always 三选, YOLO 模式自动审批
- **按键宏**: 真实按键注入 (CGEvent), Fn/Enter/Escape/Ctrl+U 等, mouseDown/mouseUp 实时模式
- **音频系统**: rodio WAV 播放, 4 内置音效, 事件映射可配置, 音量/静音控制
- **桌面 GUI**: Tauri v2 + React, 5 tab (Keyboard/Bindings/Config/Sound/Setup), CSS token 设计系统

## Quick Start

```bash
# Build
cargo build --workspace

# 1. Start daemon
cargo run -p vk-daemon -- serve --headless

# 2. Start simulator (another terminal)
cargo run -p vk-simulator -- --cli

# 3. Install Claude Code hooks
cargo run -p vk-daemon -- setup claude-code

# 4. (Optional) Start Tauri desktop
cd desktop && npm install && cargo tauri dev
```

## Architecture

```
8 Rust crates + 1 Tauri desktop app:

vk-core       — 共享类型 (ButtonId, SessionStatus, SessionInfo 等), 零依赖
vk-protocol   — 消息枚举 (Uplink/Downlink) + binary codec, 依赖 vk-core
vk-transport  — Transport trait + IPC + Channel (async, tokio)
vk-display    — DynFramebuffer, RGB565, bytemuck zero-copy, 双缓冲
vk-input      — 按钮/旋钮/LED/喇叭 trait 抽象 + mock, 依赖 vk-core
vk-ui         — ScreenStateMachine (5 屏), renderer (unifont CJK), widget, animation
vk-simulator  — 终端 LCD 模拟器 (半角块字符渲染) + transport 客户端
vk-daemon     — HTTP hook server + IPC + session 管理 + 权限 + focus + 音频
desktop/      — Tauri v2 + React (Canvas LCD + 虚拟按钮 + 5 配置 tab)
```

### Key Design

- **vk-core 零依赖** — 共享类型独立 crate, 未来可用于 no_std (ESP32)
- **vk-protocol 无 tokio** — 纯类型+codec, transport 独立到 vk-transport
- **server/ 模块拆分** — state/api/render/ipc/scanner 5 模块
- **DaemonSession 组合 SessionInfo** — 单一类型来源, 无字段复制
- **FocusStrategy trait** — iTerm2/Ghostty/VSCode/Warp 可插拔策略
- **Dirty-flag 渲染** — generation counter + bytemuck zero-copy
- **原子配置写入** — temp + fsync + rename

## Usage

### Daemon

```bash
cargo run -p vk-daemon -- serve --headless        # Start daemon
cargo run -p vk-daemon -- session list             # List sessions (via HTTP)
cargo run -p vk-daemon -- focus 1                  # Focus session #1
cargo run -p vk-daemon -- config show              # Show config
cargo run -p vk-daemon -- config set yolo.active true
cargo run -p vk-daemon -- setup claude-code        # Install hooks
```

### Simulator

```bash
cargo run -p vk-simulator -- --cli                 # Interactive terminal UI
cargo run -p vk-simulator -- --cli --standalone    # Demo mode (no daemon)
cargo run -p vk-simulator -- button press send     # One-shot
cargo run -p vk-simulator -- knob rotate 3
```

### Hook API

```bash
# Session lifecycle (daemon listens on 127.0.0.1:19280)
curl -X POST http://127.0.0.1:19280/event \
  -H "Content-Type: application/json" \
  -d '{"type":"SessionStart","session_id":"abc","name":"MyAgent"}'

curl http://127.0.0.1:19280/sessions   # Query all sessions
curl http://127.0.0.1:19280/health     # Health check
```

### Keyboard Controls

| Key | Button | Action |
|-----|--------|--------|
| Enter | SEND | Allow permission / confirm |
| Esc | CANCEL | Deny permission / back |
| m | MODE | Toggle YOLO mode |
| s | SESSION | Notify screen / next alert |
| d | DELETE | Erase input (mouseDown/Up) |
| v | VOICE | Fn key toggle |
| ↑↓ | KNOB | Scroll sessions / rotate |
| Space | KNOB press | Select / enter Select mode |

### YOLO Mode

```toml
# ~/.config/vk-daemon/config.toml
[yolo]
active = true
allow = ["Read(*)", "Glob(*)", "Grep(*)"]
deny = ["Bash(git push*)", "Bash(rm -rf*)", "Bash(sudo*)"]
```

Deny > Allow > Ask User. Permission timeout = deny (fail-closed).

## Security

- HTTP server binds **127.0.0.1 only** (not 0.0.0.0)
- osascript inputs validated (bundle_id allowlist, TTY format check, JXA escaping)
- Transcript paths canonicalized (no path traversal, must be under `~/.claude/projects/`)
- Permission system fail-closed (timeout/error → deny)
- No `unsafe` code — `Mutex<Sender>` instead of `unsafe impl Sync`
- Config writes atomic (temp + fsync + rename)
- Lock ordering enforced (session_id_map → store)

## Development

```bash
cargo check              # Type check
cargo test               # 399 tests
cargo clippy --workspace # Lint
```

## Project Status

| Milestone | Tasks | Status |
|-----------|-------|--------|
| M1-M6 Core | 43 | ✅ Done |
| M7 真实可用 | 20 | ✅ Done |
| M8 GUI+配置 | 14 | ✅ Done |
| M9 完整修复 | 11 | ✅ Done |
| M10 Daemon Trait 化 | 11 | ✅ Done |
| M11 音频系统 | 9 | ✅ Done |
| M12 GUI 统一设计 | 11 | ✅ Done |
| M13 多平台 Hook | 10 | Pending |
| M14 安全加固 | 8 | ✅ Done |
| M15 架构重构 | 10 | ✅ Done |
| M16 性能优化 | 8 | ✅ Done |
| M17 代码质量 | 10 | ✅ Done |
| M18 架构纯净化 | 8 | ✅ Done |

**397 tests, 0 failures. 15 Codex checkpoints passed.**

## Documentation

### 整理后的开发文档（docs/）

| 文档 | 说明 |
|------|------|
| [docs/README.md](docs/README.md) | 文档索引和当前状态摘要 |
| [docs/development.md](docs/development.md) | 开发文档 — 工程结构、构建运行、调试流程、核心数据流 |
| [docs/api.md](docs/api.md) | 接口文档 — HTTP API、Hook 事件、设备协议、配置字段 |
| [docs/io-map.md](docs/io-map.md) | IO 资源映射表 — 按键、旋钮、LCD、LED、音频、通信 |
| [docs/todo.md](docs/todo.md) | TODO — 硬件适配、AI 工具集成、跨平台能力、验证清单 |

### 核心文档（根目录）

| 文档 | 说明 |
|------|------|
| [design-brief.md](design-brief.md) | 产品设计简报 — 交互逻辑、旋钮 UX、通知系统、事件集成、已知局限 |
| [architecture.md](architecture.md) | 软件架构 — 8 crate 分层、模块依赖、数据流 |
| [solution.md](solution.md) | 技术方案 — 双进程设计、trait 体系、设计决策 |
| [interaction-flows.md](interaction-flows.md) | 交互流程 — 5 屏状态机、按钮映射、导航逻辑 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 开发者指南 — 构建、API、数据流、代码规范 |

## License

MIT
