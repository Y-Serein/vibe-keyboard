# Solution — Vibe Keyboard

## Overview

| Field | Value |
|-------|-------|
| Project | Vibe Keyboard |
| Based on | doc/requirements.md, doc/architecture.md |
| Date | 2026-03-31 |

## Architecture Decision: 设备端 vs 桌面端 (CR-006 更新)

### 双端职责分离

```
设备端 (SG2002 Linux / Simulator)       桌面端 (Daemon + Tauri)
┌─────────────────────────┐             ┌─────────────────────────┐
│ 硬件抽象 trait:           │             │ 业务逻辑:                │
│  ButtonInput             │             │  SessionMonitor          │
│  EncoderInput            │             │  SessionDiscovery        │
│  LedController           │ ←Transport→ │  FocusStrategy           │
│  Speaker                 │   trait     │  KeystrokeInjector       │
│  DisplayDriver           │  (IPC/USB)  │  NotificationBackend     │
│                          │             │  TerminalDetector        │
│ UI 引擎:                 │             │  CESP 事件路由           │
│  ScreenStateMachine      │             │  PermissionQueue         │
│  Renderer                │             │  ConfigManager           │
│  Notify/Toast            │             │                          │
└─────────────────────────┘             └─────────────────────────┘

Transport trait (vk-transport): IPC (Unix socket) / USB HID (未来) / Channel (测试)
共享类型 (vk-core): ButtonId, SessionStatus, SessionInfo 等，零依赖
```

| 能力 | 设备端 | 桌面端 | 说明 |
|------|:------:|:------:|------|
| 按钮/旋钮输入 | ✅ | — | ButtonInput/EncoderInput trait |
| LED 控制 | ✅ | — | LedController trait |
| 音频播放 | ✅ | — | Speaker trait (rodio / ALSA) |
| LCD 渲染 | ✅ | ✅(mirror) | DisplayDriver + Canvas |
| UI 状态机 | ✅ | ✅(mirror) | 设备主控, 桌面镜像 |
| Session 监控 | — | ✅ | Hook + JSONL + 进程扫描 |
| 窗口跳转 | — | ✅ | FocusStrategy trait |
| 按键注入 | — | ✅ | KeystrokeInjector trait |
| 桌面通知 | — | ✅ | NotificationBackend trait |
| 配置管理 | — | ✅ | TOML + GUI |

### 实现阶段

| 阶段 | 设备端实现 | 说明 |
|------|-----------|------|
| 当前 | Simulator (macOS) | trait 在桌面模拟: crossterm/rodio/ANSI |
| 未来 | SG2002 Linux | trait 真实实现: GPIO/ALSA/SPI LCD |

Simulator 保证了 trait 接口稳定 → SG2002 只需写 impl，不改协议。

## Architecture Decision: 双进程 + Trait 抽象

### Option Comparison

| Dimension | Option A: 双进程 IPC | Option B: 单进程 Channel |
|-----------|---------------------|------------------------|
| Logic | simulator 和 daemon 各自独立进程，Unix socket 通信 | 同一进程内 tokio mpsc channel |
| Pros | 1:1 映射真实架构；替换为硬件零改动；强制协议正确性 | 开发最快；调试简单 |
| Cons | IPC 增加复杂度；跨进程调试稍难 | 无法验证真实通信边界；替换硬件需重构 |
| Risk | IPC 实现 bug | 协议 bug 上硬件才暴露 |
| Effort | 中 | 低 |

### Recommendation: **Option A (双进程 IPC)**
理由：产品化目标要求真实验证通信协议，避免上硬件后返工。

## Crate 依赖图 (M18 重组后)

```
                    vk-core (零依赖)
                 /     |      \        \
          vk-display  vk-input  vk-protocol  desktop
          (bytemuck)  (core)    (core+codec)  (core)
               \       |        |
                vk-ui          vk-transport
         (display,input,       (tokio,protocol)
          protocol,core)          |
                \                 |
                 vk-daemon ──────┘
                (ui,transport,core,axum,rodio)
                      |
                 vk-simulator
              (ui,transport,input,core,crossterm)
```

## 核心数据流

```
按钮事件链路 (上行):
  物理按钮/模拟按钮
    → Button trait (vk-input)
    → ButtonEvent (vk-core → vk-protocol)
    → Transport.send() (vk-transport: IPC / USB HID)
    → daemon 接收
    → 执行动作 (focus / allow / cancel)

显示更新链路 (下行):
  AI tool hook → daemon SessionMonitor
    → SessionState 变更
    → ScreenUpdate message (vk-protocol)
    → Transport.send()
    → simulator → vk-ui 状态机
    → render to framebuffer
    → CLI / GUI / Tauri Canvas / 真实 LCD
```

## 模块依赖

| Module | Responsibility | Dependencies | Interface |
|--------|---------------|--------------|-----------|
| vk-protocol | 消息定义 + 编解码 + Transport trait (async) | tokio, async-trait | Message enum, Transport trait |
| vk-display | 屏幕驱动抽象 + 帧缓冲 | 无 | DisplayDriver trait, Framebuffer |
| vk-input | 输入设备抽象 | vk-protocol (message types) | Button/Encoder/LED/Speaker traits |
| vk-ui | UI 引擎 + 屏幕状态机 | vk-protocol, vk-display, vk-input | ScreenState, UiEvent, render() |
| vk-simulator | 模拟器进程 | 所有核心 crate | CLI/GUI binary |
| vk-daemon | 桌面端守护进程 | vk-protocol, tokio, tauri | Tauri App + CLI |

## 关键设计决策

| Decision | Choice | Reason |
|----------|--------|--------|
| 标准 Rust | 所有 crate 使用 std | 开发效率优先，ESP32 固件独立项目 |
| 通信 | 双进程 IPC | 1:1 映射真实架构 |
| 渲染 | Framebuffer → 多后端 | 一套代码多端显示 |
| UI | 状态机 (5 states: Standby/Normal/Select/Allow/Notify) | 简单可预测 |
| 配置 | TOML 文件 + GUI | 开发者友好 |
| CLI | 贯穿所有模块 | 调试和测试必需 |

## Architecture Decision: Daemon 侧 Trait 化 (CR-005)

> 参考: [peon-ping](https://github.com/PeonPing/peon-ping) 多终端/多平台架构

### 问题

daemon 中 focus、keystroke、notification 都是裸函数/硬编码 macOS 实现，无法扩展到其他终端和平台。

### 决策: 5 个可扩展 trait + 策略模式

```
daemon 侧 trait 全景 (7 个):

已有:
  SessionDiscovery       ← Session 发现 (hook / 文件系统 / 进程扫描)
  Speaker                ← 音频播放 (terminal bell / afplay / rodio)
  注: PlatformMonitor trait 已移除，功能由 SessionDiscovery + monitor.rs 替代

新增:
  FocusStrategy          ← 窗口跳转 (iTerm2 / Ghostty / Warp / VS Code / Generic)
  KeystrokeInjector      ← 按键注入 (macOS CGEvent / Linux xdotool / Win SendInput)
  NotificationBackend    ← 桌面通知 (macOS native / overlay / Linux / Win toast)
  SessionDiscovery       ← Session 发现 (hook / 文件系统 / 进程扫描)
  TerminalDetector       ← 终端识别 (TERM_PROGRAM + 回退检测)
```

### Trait 设计 (参考 peon-ping)

```rust
pub trait FocusStrategy: Send + Sync {
    fn can_focus(&self, session: &DaemonSession) -> bool;
    fn activate(&self, session: &DaemonSession) -> Result<(), String>;
    fn is_focused(&self, session: &DaemonSession) -> bool;  // 通知去重用
    fn name(&self) -> &str;
}

pub trait KeystrokeInjector: Send + Sync {
    fn send_key(&self, action: &str) -> Result<(), String>;
    fn platform(&self) -> &str;
}

pub trait NotificationBackend: Send + Sync {
    fn notify(&self, title: &str, body: &str, click_action: Option<&str>) -> Result<(), String>;
    fn name(&self) -> &str;
}

pub trait SessionDiscovery: Send + Sync {
    fn discover(&self) -> Vec<DiscoveredSession>;
    fn name(&self) -> &str;
}

pub trait TerminalDetector: Send + Sync {
    fn detect(&self, env: &HashMap<String, String>) -> TerminalInfo;
}
```

### 执行策略

daemon 持有 `Vec<Box<dyn FocusStrategy>>`，跳转时遍历找第一个 `can_focus() == true` 的执行。

macOS 先行，trait 接口预留多平台。后续 Linux/Windows 只需加 `impl`。

### peon-ping 参考要点

| 能力 | peon-ping 做法 | 我们的映射 |
|------|---------------|-----------|
| 终端检测 | `TERM_PROGRAM` + `ITERM_SESSION_ID` 回退 | TerminalDetector trait |
| iTerm2 焦点 | JXA: 遍历 window/tab/session by tty | ITermFocus.is_focused() |
| 点击跳转 | JXA `NSRunningApplication.activateWithOptions` | FocusStrategy.activate() |
| 桌面通知 | terminal-notifier / overlay / osascript | NotificationBackend trait |
| 平台分流 | `detect_platform()` → mac/linux/wsl/win | 编译时 `#[cfg(target_os)]` + 运行时检测 |

## Risks

| Risk | Level | Mitigation |
|------|-------|------------|
| ESP32 移植需额外工作 | L | ESP32 固件作为独立项目，核心逻辑可参考 |
| IPC 通信性能不足 | L | Unix socket 足够快，实测验证 |
| macOS Accessibility 权限用户体验差 | M | 首次启动引导 + 清晰提示 |
| 多 AI 工具适配器维护成本 | H | 先做 Claude Code，trait 抽象降低后续成本 |
