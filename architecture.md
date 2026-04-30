# Vibe Keyboard — 软件架构设计

## 设计原则

1. **标准 Rust (std)** — 所有 crate 使用 std，ESP32 固件作为独立项目后续开发
2. **双进程架构** — simulator 进程(模拟键盘固件) + daemon 进程(桌面端 Tauri App)
3. **Transport trait 抽象** — async (tokio)，IPC(模拟) / USB HID(真硬件) / Channel(测试) 可插拔
4. **CLI 贯穿** — 所有模块都有 CLI 接口，方便独立调试和测试
5. **LCD 多后端渲染** — CLI(ratatui text widgets, debug console) / GUI(minifb framebuffer) / Tauri(Canvas) / SPI(真LCD)

## Monorepo 结构 (M18 重组后: 8 crates)

```
vibe-keyboard/
├── crates/
│   ├── vk-core/                  # 共享类型 (零外部依赖)
│   │   └── lib.rs                # ButtonId, SoundType, SessionStatus, SessionInfo, NotificationInfo, PermissionAction
│   │
│   ├── vk-protocol/              # 通信协议 (仅 codec, 无 tokio)
│   │   ├── message.rs            # re-exports from vk-core + UplinkMessage/DownlinkMessage
│   │   └── codec.rs              # 二进制编解码 (tag-based binary codec)
│   │
│   ├── vk-transport/             # 传输层 (async, tokio)
│   │   ├── transport.rs          # Transport trait (send/recv)
│   │   ├── ipc.rs                # IpcTransport (Unix socket)
│   │   └── channel.rs            # ChannelTransport (tokio mpsc, for testing)
│   │
│   ├── vk-display/               # 屏幕驱动抽象
│   │   ├── driver.rs             # DisplayDriver trait
│   │   ├── framebuffer.rs        # DynFramebuffer, 双缓冲, bytemuck zero-copy
│   │   └── color.rs              # RGB565 (Pod + Zeroable) / RGBA 互转
│   │
│   ├── vk-input/                 # 输入抽象 (依赖 vk-core, 非 vk-protocol)
│   │   ├── button.rs             # ButtonInput trait
│   │   ├── encoder.rs            # EncoderInput trait
│   │   ├── led.rs                # LedController trait
│   │   ├── speaker.rs            # Speaker trait
│   │   └── mock.rs               # Mock implementations for testing
│   │
│   ├── vk-ui/                    # UI 引擎
│   │   ├── screen.rs             # ScreenStateMachine (5 屏: Standby/Normal/Select/Allow/Notify)
│   │   ├── renderer.rs           # LCD 渲染 (unifont CJK, source badges, progress bars)
│   │   ├── widget.rs             # draw_text, draw_char_scaled, fill_rect
│   │   ├── event.rs              # UI 事件定义
│   │   └── animation.rs          # blink, pulse, slide, alternate
│   │
│   ├── vk-simulator/             # 模拟器进程 (模拟键盘固件)
│   │   ├── main.rs               # 入口: --cli / subcommands
│   │   ├── sim_display.rs        # DisplayDriver impl (half-block terminal rendering)
│   │   ├── sim_input.rs          # ButtonInput/EncoderInput impl (crossterm key mapping)
│   │   ├── sim_speaker.rs        # Speaker impl (rodio WAV playback)
│   │   └── sim_led.rs            # LedController impl (terminal ANSI colors)
│   │
│   └── vk-daemon/                # 桌面端 daemon (HTTP server + IPC + CLI)
│       ├── main.rs               # CLI 入口 (serve/session/focus/config/setup)
│       ├── server/               # HTTP server (拆分为 6 模块)
│       │   ├── mod.rs            # run_serve() 组合
│       │   ├── state.rs          # DaemonState 定义
│       │   ├── api.rs            # Axum router + HTTP handlers
│       │   ├── render.rs         # run_render_loop (dirty-flag)
│       │   ├── ipc_handler.rs    # IPC uplink/downlink + process_session_event
│       │   └── scanner.rs        # process_scanner + transcript_scanner
│       ├── session/              # Session 管理
│       │   ├── store.rs          # SessionStore + DaemonSession (composes SessionInfo)
│       │   └── monitor.rs        # Hook event parser
│       ├── focus/                # 窗口跳转策略
│       │   ├── mod.rs            # FocusStrategy trait + shared JXA helpers
│       │   ├── error.rs          # FocusError (thiserror)
│       │   ├── iterm.rs          # iTerm2 TTY 精确跳转
│       │   ├── generic.rs        # NSWorkspace 通用跳转
│       │   ├── vscode.rs         # VSCode/Cursor/Windsurf
│       │   ├── ghostty.rs        # Ghostty
│       │   ├── warp.rs           # Warp
│       │   └── macos.rs          # macOS 共享 JXA 辅助函数
│       ├── notification/         # 桌面通知
│       │   ├── mod.rs            # NotificationBackend trait + NotificationQueue
│       │   └── mac_native.rs     # terminal-notifier + osascript
│       ├── cesp.rs               # CESP 事件路由
│       ├── config.rs             # DaemonConfig (TOML, atomic write)
│       ├── discovery.rs          # SessionDiscovery trait + 实现
│       ├── keystroke.rs          # KeystrokeInjector trait (CGEvent)
│       ├── local_speaker.rs      # Desktop audio (rodio, Mutex<Sender>)
│       ├── permission.rs         # PermissionQueue + YOLO evaluation
│       ├── setup.rs              # Hook installer (Claude Code)
│       ├── terminal.rs           # TerminalDetector trait
│       ├── transcript.rs         # JSONL transcript parser
│       └── tests/
│           └── server_integration.rs
│
├── desktop/                      # Tauri v2 桌面应用
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── main.rs           # Tauri 入口
│   │   │   └── lib.rs            # Tauri lib
│   │   ├── build.rs
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   └── src/                      # React 前端
│       ├── App.tsx
│       ├── main.tsx
│       ├── tokens.css            # Design token CSS 变量
│       ├── types.ts              # TypeScript 类型定义
│       └── components/
│           ├── ActivityLog.tsx    # 活动日志面板
│           ├── BindingsPanel.tsx  # 按键绑定配置
│           ├── ConfigPanel.tsx    # 配置面板
│           ├── Screen.tsx        # LCD 屏幕镜像
│           ├── SetupPanel.tsx    # Setup/Onboarding 面板
│           ├── shared.tsx        # 共享组件 (Button/Card/Toggle 等)
│           ├── SoundPanel.tsx    # 音效配置面板
│           └── VirtualKeyboard.tsx # 虚拟按钮面板 (可点击)
│
├── doc/
│   ├── draft.md
│   ├── architecture.md           # 本文档
│   ├── button-mapping.md         # 按键映射方案 (待设计)
│   └── protocol-spec.md          # 通信协议规格 (待设计)
├── ideas/
└── Cargo.toml                    # workspace

```

## CLI 接口设计

### vk-simulator CLI

```bash
# 全模式
vk-simulator --cli                     # CLI 终端模式 (ratatui)
vk-simulator --gui                     # GUI 窗口模式

# 单模块调试
vk-simulator button press <name>       # 模拟按钮按下 (send/cancel/mode/...)
vk-simulator button release <name>     # 模拟按钮释放
vk-simulator knob rotate <steps>       # 模拟旋钮旋转 (+N/-N)
vk-simulator knob press                # 模拟旋钮按下
vk-simulator display test-pattern      # 显示测试图案
vk-simulator display show-frame        # 输出当前 framebuffer (PNG/ASCII)
vk-simulator speaker beep              # 测试喇叭
vk-simulator led set <name> <color>    # 测试 LED
```

### vk-daemon CLI

```bash
# 服务
vk-daemon serve                        # 启动 daemon (Tauri GUI)
vk-daemon serve --headless             # 无 GUI 启动 (纯后端)

# Session 管理
vk-daemon session list                 # 列出所有检测到的 session
vk-daemon session status <id>          # 查看 session 详情
vk-daemon session mock                 # 注入模拟 session 数据

# 窗口跳转
vk-daemon focus <session-id>           # 跳转到指定 session
vk-daemon focus test                   # 测试所有 session 的跳转能力

# 配置
vk-daemon config show                  # 显示当前配置
vk-daemon config set <key> <value>     # 修改配置项
vk-daemon config reset                 # 恢复默认配置

# 通信
vk-daemon transport status             # 查看设备连接状态
vk-daemon transport send <json-msg>    # 手动发送消息到设备
vk-daemon transport listen             # 监听设备消息 (调试)

# 通知
vk-daemon notify test                  # 发送测试通知到设备
vk-daemon notify list                  # 列出待处理通知
```

## 数据流

### 按钮事件链路 (上行)
```
物理按钮/模拟按钮
    → Button trait (vk-input)
    → ButtonEvent (vk-protocol)
    → Transport.send() (IPC / USB HID)
    → daemon 接收
    → 执行动作 (focus session / allow / cancel / ...)
```

### 显示更新链路 (下行)
```
AI tool hook / notification
    → daemon SessionMonitor
    → SessionState 变更
    → ScreenUpdate message (vk-protocol)
    → Transport.send() (IPC / USB HID)
    → simulator 接收
    → vk-ui 更新状态机
    → render to framebuffer (vk-display)
    → 显示到 CLI / GUI / Tauri Canvas / 真实 LCD
```

### LCD 镜像链路 (daemon Tauri UI)
```
simulator framebuffer
    → Transport.send(FrameData)
    → daemon 接收
    → Tauri emit("lcd-update", frame_data)
    → React Canvas 渲染
```

## Transport Trait

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send_uplink(&self, msg: &UplinkMessage) -> Result<(), TransportError>;
    async fn send_downlink(&self, msg: &DownlinkMessage) -> Result<(), TransportError>;
    async fn recv_uplink(&self) -> Result<UplinkMessage, TransportError>;
    async fn recv_downlink(&self) -> Result<DownlinkMessage, TransportError>;
    fn is_connected(&self) -> bool;
}

// 实现:
// 1. IpcTransport     — Unix domain socket (simulator ↔ daemon)
// 2. UsbHidTransport  — USB HID device    (真硬件 ↔ daemon)  [未来]
// 3. ChannelTransport — tokio mpsc         (单元测试)
```

> 详细 trait 设计和板块互通见 [trait-architecture.md](trait-architecture.md)

## 屏幕状态机

```
Standby ──[SessionUpdate]──► Normal ──[KnobRotate]──► Select
   ▲                           ▲  │                     │
   │                           │  │ [PermissionRequest] │ [KnobPress / 3s timeout]
   │ [AllSessionsGone]         │  ▼                     │
   │                           │ Allow                   │
   │                           │  │ [PermissionResolved] │
   │                           └──┘◄────────────────────┘
   │                           ▲                         │
   │                           │ [Timeout/Dismiss]       │
   │                           │                         │
   │                         Notify ◄─[Notification]─────┘
   └────────────────────────────────────────────────────┘
```

状态说明 (5 states)：
- **Standby**: 无活跃 session，显示 logo + 时间
- **Normal**: 显示当前 session 详情
- **Select**: 显示所有 session 列表，旋钮选择，按下确认切换
- **Allow**: 审批模式，显示 permission 请求，SEND=Allow, CANCEL=Deny, 旋钮选 Always
- **Notify**: 通知显示，NotificationQueue 驱动，超时/Dismiss 后回到 Normal
