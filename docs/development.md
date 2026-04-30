# 开发文档

## 项目定位

`vibe-keyboard` 是 AI coding session 的物理控制器原型：设备端负责按钮、旋钮、LCD、LED、喇叭和本地 UI 状态机；桌面端 daemon 负责 AI 工具 hook、session 管理、权限审批、窗口跳转、按键注入、通知、配置和音频。

当前工程以模拟器验证设备协议和交互逻辑，后续接 SG2002/RV Nano 时优先补硬件 trait 实现，不改上层协议。

## 工程结构

```text
vibe-keyboard/
├── Cargo.toml
├── crates/
│   ├── vk-core        # 共享类型，零外部依赖
│   ├── vk-protocol    # Uplink/Downlink 消息和二进制 codec
│   ├── vk-transport   # Transport trait，IPC 和 Channel 实现
│   ├── vk-display     # RGB565 framebuffer 和显示驱动抽象
│   ├── vk-input       # Button/Encoder/LED/Speaker trait
│   ├── vk-ui          # 5 屏状态机和 LCD renderer
│   ├── vk-daemon      # HTTP hook server、session、focus、permission、setup、sound
│   └── vk-simulator   # CLI 设备模拟器
├── desktop/           # Tauri v2 + React 桌面 GUI
├── docs/              # 当前整理后的开发文档
└── scripts/           # tmux/demo 启动脚本
```

## 环境要求

- Rust stable，edition 2024。
- Node.js 18+，用于 `desktop/` Tauri 前端。
- macOS 为当前主开发平台；Linux/Windows 只保留接口和部分策略预留。
- macOS 真按键注入/窗口跳转需要 Accessibility 权限。

## 构建和运行

```bash
# 全量构建
cargo build --workspace

# 运行测试
cargo test --workspace

# Lint
cargo clippy --workspace

# 启动 daemon，监听 127.0.0.1:19280
cargo run -p vk-daemon -- serve --headless

# 另一个终端启动设备模拟器
cargo run -p vk-simulator -- --cli

# 安装 Claude Code hook
cargo run -p vk-daemon -- setup claude-code

# 启动 Tauri 桌面 GUI
cd desktop
npm install
cargo tauri dev
```

## 常用 CLI

### Daemon

```bash
cargo run -p vk-daemon -- serve --headless
cargo run -p vk-daemon -- session list
cargo run -p vk-daemon -- session status <id>
cargo run -p vk-daemon -- focus <id>
cargo run -p vk-daemon -- config show
cargo run -p vk-daemon -- config set yolo.active true
cargo run -p vk-daemon -- setup claude-code --port 19280
```

### Simulator

```bash
cargo run -p vk-simulator -- --cli
cargo run -p vk-simulator -- --cli --standalone
cargo run -p vk-simulator -- button press send
cargo run -p vk-simulator -- knob rotate 3
```

## 核心数据流

### Hook 到 LCD

```text
Claude Code hook
  -> POST /event
  -> parse_hook_event()
  -> process_session_event()
  -> SessionStore / PermissionQueue / NotificationQueue
  -> DownlinkMessage
  -> Transport
  -> simulator / future hardware
  -> ScreenStateMachine
  -> RGB565 framebuffer
```

### 按键到桌面动作

```text
ButtonInput / EncoderInput
  -> UiEvent
  -> ScreenStateMachine
  -> UplinkMessage
  -> daemon handle_uplink()
  -> focus / permission response / macro / yolo toggle
```

### 帧渲染

```text
daemon render loop
  -> dirty generation check
  -> build session list + notifications + permissions
  -> vk_ui::renderer::render()
  -> 800 x 340 RGB565 framebuffer
  -> GET /frame or FrameData downlink
```

## 关键模块入口

| 模块 | 入口文件 | 关注点 |
|------|----------|--------|
| 共享类型 | `crates/vk-core/src/lib.rs` | `ButtonId`, `SessionInfo`, `SessionStatus`, `NotificationInfo` |
| 设备协议 | `crates/vk-protocol/src/message.rs`, `codec.rs` | 消息枚举、tag、字段编码 |
| 传输 | `crates/vk-transport/src/transport.rs`, `ipc.rs` | Unix socket、Channel 测试后端 |
| 输入抽象 | `crates/vk-input/src/*.rs` | 按钮、旋钮、LED、喇叭 trait |
| UI 状态机 | `crates/vk-ui/src/screen.rs` | Standby/Normal/Select/Allow/Notify |
| HTTP API | `crates/vk-daemon/src/server/api.rs` | REST API、hook、配置、setup、sound |
| Hook 解析 | `crates/vk-daemon/src/session/monitor.rs` | Claude Code/兼容事件映射 |
| 权限队列 | `crates/vk-daemon/src/permission.rs` | YOLO、Always Allow、阻塞审批 |
| 窗口跳转 | `crates/vk-daemon/src/focus/` | iTerm2/Ghostty/VSCode/Warp/macOS |
| 模拟输入 | `crates/vk-simulator/src/sim_input.rs` | 终端按键到设备事件映射 |

## 屏幕状态机

| 状态 | 触发 | 说明 |
|------|------|------|
| `Standby` | 无 session | 显示待机信息 |
| `Normal` | 有 session | 显示当前 session 详情 |
| `Select` | Normal 下旋钮旋转 | 浏览和切换 session |
| `Allow` | 权限请求 | 选择 Allow/Deny/Always |
| `Notify` | NOTIFY/SESSION 按钮 | 查看通知并跳转 |

## 配置文件

默认路径来自 `dirs::config_dir()/vk-daemon/config.toml`。常用字段：

```toml
[general]
hook_port = 19280
log_level = "info"

[ipc]
socket_path = "/tmp/vk-daemon.sock"

[display]
width = 800
height = 340

[macros]
delete = "ctrl_u"
voice = "fn"

[yolo]
active = false
allow = ["Read(*)", "Glob(*)", "Grep(*)"]
deny = ["Bash(git push*)", "Bash(rm -rf*)", "Bash(sudo*)"]
notify_auto_allow = true
auto_allow_log = true

[sound]
enabled = true
volume = 80
muted = false
```

## 开发原则

- SG2002 真机适配优先写 trait 实现，不把平台细节塞进 `vk-ui` 或 `vk-protocol`。
- 协议字段变更必须同步更新 `message.rs`、`codec.rs`、测试和本文档。
- HTTP API 默认只绑定 `127.0.0.1`，不要改成公网监听。
- 权限审批保持 fail-closed：超时、错误、断连默认 deny。
