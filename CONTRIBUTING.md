# Contributing to Vibe Keyboard

## 项目概述

Vibe Keyboard 是一个 AI Agent 物理控制器，通过按钮/旋钮/LCD 管理多个 AI coding session。

- **Tech Stack**: Rust 2024 + Tauri v2 + React + TypeScript
- **代码量**: ~28K 行 (8 Rust crates + 1 Tauri desktop)
- **测试**: 397 tests, 0 failures
- **平台**: macOS (primary), Linux (planned)

---

## 快速上手

### 环境要求

```bash
# Rust (stable, edition 2024)
rustup update stable

# Node.js (for Tauri desktop)
node --version  # >= 18

# macOS: Xcode Command Line Tools
xcode-select --install
```

### 构建 & 运行

```bash
# 全量构建
cargo build --workspace

# 运行测试
cargo test --workspace

# Lint
cargo clippy --workspace

# 启动 daemon (HTTP server on 127.0.0.1:19280)
cargo run -p vk-daemon -- serve --headless

# 启动模拟器 (另一个终端)
cargo run -p vk-simulator -- --cli

# 启动 Tauri 桌面端 (另一个终端)
cd desktop && npm install && cargo tauri dev
```

### 安装 Claude Code Hook

```bash
cargo run -p vk-daemon -- setup claude-code
# 自动写入 ~/.claude/settings.json hooks 配置
```

---

## 架构总览

### 8 Crate 分层

```
                    vk-core (零依赖)
                 共享类型: ButtonId, SessionStatus, SessionInfo...
                /     |      \        \
         vk-display  vk-input  vk-protocol  desktop
         (bytemuck)  (core)    (core+codec)  (core)
              \       |        |
               vk-ui          vk-transport
        (display,input,       (tokio, async Transport)
         protocol,core)          |
               \                 |
                vk-daemon ──────┘
               (ui,transport,core,axum,rodio)
                     |
                vk-simulator
             (ui,transport,input,core,crossterm)
```

### Crate 职责

| Crate | 职责 | 依赖 | 关键 trait/struct |
|-------|------|------|-------------------|
| **vk-core** | 共享类型定义 | 无 (零依赖) | `ButtonId`, `SessionStatus`, `SessionInfo`, `NotificationInfo` |
| **vk-protocol** | 消息枚举 + 二进制编解码 | vk-core | `UplinkMessage`, `DownlinkMessage`, `encode/decode` |
| **vk-transport** | async 传输层 | tokio, vk-protocol | `Transport` trait, `IpcTransport`, `ChannelTransport` |
| **vk-display** | LCD 帧缓冲 | bytemuck | `DynFramebuffer`, `Rgb565`, `front_buffer_as_bytes()` |
| **vk-input** | 硬件输入抽象 | vk-core | `ButtonInput`, `EncoderInput`, `LedController`, `Speaker` |
| **vk-ui** | UI 状态机 + 渲染 | display, input, protocol | `ScreenStateMachine`, `renderer::render()`, unifont CJK |
| **vk-daemon** | 桌面端守护进程 | ui, transport, axum, rodio | `DaemonState`, HTTP server, Focus, Keystroke, Audio |
| **vk-simulator** | 终端模拟器 | ui, transport, input, crossterm | 半角块字符渲染, IPC 客户端 |

### 双进程架构

```
┌─────────────────┐         IPC (Unix socket)         ┌─────────────────┐
│   vk-simulator  │ ←─── UplinkMessage/DownlinkMessage ──→ │   vk-daemon     │
│   (键盘固件模拟) │         vk-transport                  │   (桌面端服务)   │
└─────────────────┘                                   └────────┬────────┘
                                                               │ HTTP hooks
                                                        POST /event
                                                               │
                                                      ┌────────▼────────┐
                                                      │  Claude Code    │
                                                      │  (AI 工具)      │
                                                      └─────────────────┘
```

---

## 核心数据流

### 1. Hook 事件 → Session 创建

```
Claude Code hook → POST /event { type: "SessionStart", session_id, name, ... }
  → server/api.rs::handle_hook_event()
    → session/monitor.rs::parse_hook_event() → SessionEvent::Started
    → server/ipc_handler.rs::process_session_event()
      → SessionStore::update(DaemonSession { info: SessionInfo { ... } })
      → DownlinkMessage::SessionListUpdate → IPC → simulator
      → transcript scanner 注册 (增量解析 JSONL)
```

### 2. Permission 审批流程

```
Claude Code hook → POST /event { type: "PreToolUse", tool_name: "Write", tool_input: "main.rs" }
  → 检查 YOLO (deny > allow > ask)
  → 如果 ask: 入队 PermissionQueue → 阻塞 HTTP 等待 (最长 300s)
  → DownlinkMessage::PermissionRequest → IPC → simulator LCD 显示 Allow 屏
  → 用户按 SEND (Allow) 或 CANCEL (Deny)
  → UplinkMessage::PermissionResponse → daemon
  → HTTP 返回 { hookSpecificOutput: { decision: { behavior: "allow/deny" } } }
  → 超时/错误 → 默认 deny (fail-closed)
```

### 3. LCD 渲染循环

```
run_render_loop (100ms tick):
  1. 检查 render_generation (dirty-flag) — 无变化则跳过
  2. 从 SessionStore 构建 session list
  3. 从 NotificationQueue 构建通知
  4. ui_state.handle_event() + tick()
  5. vk_ui::renderer::render() → DynFramebuffer (800×340 RGB565)
  6. framebuffer.swap() + front_buffer_as_bytes() (bytemuck 零拷贝)
  7. bytes::Bytes 存入 frame_buffer (Arc clone, 非数据拷贝)
  8. HTTP GET /frame → Tauri Canvas 绘制
```

### 4. 按键宏执行

```
GUI/Simulator 按键 → POST /button { button: "send", action: "click" }
  → 查找按键绑定 (config.macros: { "send": "enter" })
  → focus_active_then_keystroke()
    → FocusStrategy::activate() (iTerm2 JXA / NSWorkspace)
    → KeystrokeInjector::send_keystroke() (CGEvent)
```

---

## Daemon HTTP API

Base URL: `http://127.0.0.1:19280` (仅本地访问)

### Session & Hook

| Method | Path | 说明 |
|--------|------|------|
| POST | `/event` | Claude Code hook 事件入口 (SessionStart/Stop/PreToolUse/PostToolUse 等) |
| GET | `/sessions` | 获取所有 session 列表 (JSON) |
| GET | `/health` | 健康检查 |

**POST /event** 请求体:
```json
{
  "type": "SessionStart",
  "session_id": "abc-123",
  "name": "vibe-keyboard",
  "source": "claude-code",
  "cwd": "/Users/xxx/codes/vibe-keyboard",
  "session_tty": "/dev/ttys042",
  "bundle_id": "com.googlecode.iterm2",
  "transcript_path": "/Users/xxx/.claude/projects/.../session.jsonl"
}
```

### 控制

| Method | Path | 说明 |
|--------|------|------|
| POST | `/button` | 按钮操作 `{ "button": "send", "action": "click" }` |
| POST | `/knob` | 旋钮操作 `{ "action": "cw" / "ccw" / "press" }` |
| GET | `/frame` | 获取 LCD 帧数据 (bytes::Bytes, 544KB RGB565) |

### 配置

| Method | Path | 说明 |
|--------|------|------|
| GET | `/config` | 获取完整配置 (macros, yolo, sound) |
| POST | `/config` | 更新配置 |
| GET | `/yolo` | 获取 YOLO 模式状态 |
| GET | `/log` | 获取操作日志 (最近 50 条) |

### Setup

| Method | Path | 说明 |
|--------|------|------|
| GET | `/setup/status` | 检测 AI 工具安装状态 |
| POST | `/setup/install/{tool}` | 安装 hook (claude-code) |
| POST | `/setup/uninstall/{tool}` | 卸载 hook |
| POST | `/setup/brew-install/{pkg}` | brew install (限 iterm2, terminal-notifier) |
| POST | `/setup/brew-uninstall/{pkg}` | brew uninstall |

### 音频

| Method | Path | 说明 |
|--------|------|------|
| GET | `/sounds` | 列出可用音效 |
| POST | `/sounds/play` | 播放音效 `{ "sound_id": "builtin:alert" }` |
| POST | `/sounds/upload` | 上传自定义 WAV |

---

## 关键设计决策

### 1. DaemonSession 组合 SessionInfo

```rust
// vk-core: 单一类型来源
pub struct SessionInfo {
    pub id: u16,
    pub name: String,
    pub status: SessionStatus,
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub context_pct: u8,
    pub last_message: String,
    pub last_ai_output: String,
    // ... 共 16+ 字段
}

// vk-daemon: 组合而非复制
pub struct DaemonSession {
    pub info: SessionInfo,        // 协议层共享
    pub window_info: Option<WindowInfo>, // daemon 专有
}
```

**为什么**: 之前 SessionInfo/DaemonSession/UiSession 三重定义，新增字段要改 4 处。现在 `to_protocol()` 只需 `self.info.clone()`。

### 2. FocusStrategy trait + 策略链

```rust
pub trait FocusStrategy: Send + Sync {
    fn can_focus(&self, session: &DaemonSession) -> bool;
    fn activate(&self, session: &DaemonSession) -> Result<(), FocusError>;
    fn is_focused(&self, session: &DaemonSession) -> bool;
    fn name(&self) -> &str;
}

// 优先级顺序: iTerm2 > VSCode > Ghostty > Warp > Generic
let strategies = vec![
    Box::new(ITermFocus),      // TTY 精确跳转
    Box::new(VsCodeFocus),     // Cursor/Windsurf 检测
    Box::new(GhosttyFocus),
    Box::new(WarpFocus),
    Box::new(GenericMacFocus),  // NSWorkspace 通用回退
];
```

**为什么**: 不同终端跳转方式不同。iTerm2 需要 JXA 遍历 tab/session 匹配 TTY，VSCode 需要检测 bundle_id 变体。Strategy pattern 允许新增终端支持而不改核心逻辑。

### 3. Dirty-Flag 渲染 + 零拷贝

```rust
// 变化时 bump generation
state.render_generation.fetch_add(1, Ordering::Relaxed);

// 渲染循环检查
if current_gen == last_gen && !animating { continue; }

// framebuffer → HTTP: 零拷贝链路
bytemuck::cast_slice(&self.front)     // Rgb565 → &[u8], 零拷贝
bytes::Bytes::from(vec)               // Vec → Bytes (Arc)
body.clone()                          // Arc clone, 非数据拷贝
```

**为什么**: 之前 10fps 全量重绘消耗 5.44MB/s。dirty-flag 让空闲 CPU 接近 0%。bytemuck 消除了逐像素转换。

### 4. 安全设计

| 措施 | 说明 |
|------|------|
| HTTP 127.0.0.1 | 仅本地访问，LAN 不可达 |
| bundle_id 白名单 | `^[a-zA-Z0-9._-]+$`，防止 JXA 注入 |
| session_tty 验证 | `^/dev/ttys[0-9]+$`，防止 osascript 注入 |
| transcript_path canonicalize | 防止 `../../etc/passwd` 路径遍历 |
| Permission fail-closed | 超时/错误 → deny (非 allow) |
| 无 unsafe | `Mutex<Sender>` 替代 `unsafe impl Sync` |
| 原子配置写入 | temp + fsync + rename，防并发损坏 |
| 锁顺序 | session_id_map → store，防死锁 |

---

## 代码规范

### Git

- **Conventional Commits**: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `perf:`
- **分支**: 在 `main` 上直接开发 (当前阶段)
- **Co-Author**: AI 辅助代码标注 `Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>`

### Rust

- Edition 2024, stable toolchain
- `cargo clippy --workspace` 零新增 warning
- 公共 API 使用 `pub`，内部实现使用 `pub(crate)`
- 错误类型使用 `thiserror` (如 `FocusError`)，非 `Result<(), String>`
- 无 `unsafe` 代码
- 性能敏感路径使用 `bytemuck` 替代手动转换

### TypeScript (desktop)

- 严格模式 (`tsc --noEmit` 通过)
- Tauri invoke 使用类型化接口 (`invoke<DaemonConfig>("get_config")`)
- React 组件使用 `PressableButton` 而非直接 DOM 操作
- ARIA 无障碍属性 (`role="switch"`, `aria-checked`, `aria-label`)
- useEffect 正确清理 (debounce timer, setInterval)

### 测试

- 397 workspace tests
- 33 server 集成测试 (`crates/vk-daemon/tests/server_integration.rs`)
- Codec fuzz targets (`crates/vk-protocol/fuzz/`)
- `cargo test --workspace` 必须全部通过

---

## 项目状态

| Milestone | Tasks | 说明 |
|-----------|-------|------|
| M1-M6 | 43 | 核心基础 (protocol, display, input, ui, simulator, daemon) |
| M7-M9 | 45 | 真实可用 + GUI + 完整修复 |
| M10-M12 | 31 | Daemon trait 化 + 音频 + GUI 统一设计 |
| M13 | 10 | **Pending**: 多平台 Hook (Codex/OpenCode/Gemini/Cursor) |
| M14-M17 | 36 | 安全加固 + 架构重构 + 性能优化 + 代码质量 |
| M18 | 8 | 架构纯净化 (6 → 8 crate, server.rs 拆分) |

**下一步**: M13 多平台 Hook 适配

---

## 常用开发命令

```bash
# 检查编译
cargo check --workspace

# 运行特定 crate 测试
cargo test -p vk-daemon
cargo test -p vk-protocol

# 运行集成测试
cargo test -p vk-daemon --test server_integration

# Clippy lint
cargo clippy --workspace

# Daemon CLI
cargo run -p vk-daemon -- serve --headless      # 启动服务
cargo run -p vk-daemon -- session list           # 列出 session
cargo run -p vk-daemon -- config show            # 查看配置
cargo run -p vk-daemon -- setup claude-code      # 安装 hook

# Simulator CLI
cargo run -p vk-simulator -- --cli               # 交互模式
cargo run -p vk-simulator -- button press send   # 单次按钮

# Desktop
cd desktop && npm run dev                        # Vite dev server
cd desktop && cargo tauri dev                    # Tauri + React
```

## 文件导航

| 要找什么 | 看哪里 |
|---------|--------|
| 共享类型定义 | `crates/vk-core/src/lib.rs` |
| 消息编解码 | `crates/vk-protocol/src/codec.rs` |
| HTTP 路由 | `crates/vk-daemon/src/server/api.rs` |
| Hook 事件解析 | `crates/vk-daemon/src/session/monitor.rs` |
| Session 存储 | `crates/vk-daemon/src/session/store.rs` |
| 窗口跳转 | `crates/vk-daemon/src/focus/` (5 策略) |
| LCD 渲染 | `crates/vk-ui/src/renderer.rs` |
| 状态机 | `crates/vk-ui/src/screen.rs` |
| 配置文件 | `~/.config/vk-daemon/config.toml` |
| Hook 配置 | `~/.claude/settings.json` (hooks section) |
| 需求文档 | `doc/requirements.md` (BDD Gherkin) |
| 设计文档 | `doc/design/M{N}-*.md` (每个 milestone) |
| 变更记录 | `doc/change-record.md` (CR-001 ~ CR-009) |
