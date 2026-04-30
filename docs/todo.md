# TODO

## P0：接 RV Nano/SG2002 前必须确认

| 项 | 状态 | 说明 |
|----|------|------|
| 原理图和 IO 表 | TODO | 需要确认按键、旋钮、LCD、背光、LED、喇叭、USB/串口连接到哪些 pad |
| LCD 分辨率统一 | TODO | 代码默认 `800 x 340`，交互草图出现 `128 x 480`；接真屏前必须统一 |
| LCD 接口类型 | TODO | 确认 SPI/MIPI DSI/RGB/fbdev，以及 U-Boot 是否要初始化 |
| 背光和 reset | TODO | `reset/backlight` 当前仍是待确认项 |
| SG2002 pinmux | TODO | 确认 pad 复用、GPIO number、pull 配置和冲突资源 |
| 真机通信方案 | TODO | 在 USB HID、串口、Unix socket over Linux 本机之间定案 |

## P1：硬件适配实现

| 项 | 状态 | 代码落点 |
|----|------|----------|
| GPIO ButtonInput | TODO | `vk-input` trait 的 SG2002 实现，新 crate 或 `vk-simulator` 外的新 binary |
| EC11 EncoderInput | TODO | 旋钮 A/B 相位解析、按压、防抖 |
| LCD DisplayDriver | TODO | RGB565 framebuffer 输出到真 LCD |
| LED Controller | TODO | 按钮 LED 和 knob ring 的具体器件驱动 |
| Speaker | TODO | ALSA/PWM/I2S 选型和实现 |
| UsbHidTransport | TODO | `vk-transport` 预留，当前未实现 |
| 真机启动脚本 | TODO | daemon/device client 随系统启动方式 |

## P1：AI 工具集成

| 项 | 状态 | 说明 |
|----|------|------|
| Cursor hook detection | TODO | `setup.rs` 里当前 `hook_active: false` |
| Cursor hook install/uninstall | TODO | 当前返回 not yet implemented |
| Codex hook detection | TODO | `setup.rs` 里当前 `hook_active: false` |
| Codex hook install/uninstall | TODO | 当前返回 not yet implemented |
| M13 多平台 Hook | TODO | README 标记 Pending |
| OpenCode/Gemini 等适配 | TODO | 仅设计层提到，未实现 |

## P2：跨平台桌面能力

| 项 | 状态 | 说明 |
|----|------|------|
| Linux focus strategy | TODO | 设计中提到 xdotool/平台分流，当前主实现为 macOS |
| Windows keystroke | TODO | 设计中提到 SendInput，当前未实现 |
| Linux notification | TODO | 当前主要为 macOS native/terminal-notifier 路径 |
| IDE 内终端深度跳转 | TODO | VS Code/Cursor 当前只能按窗口级策略，tab/pane 级需要 extension/API 研究 |

## P2：协议和产品完善

| 项 | 状态 | 说明 |
|----|------|------|
| `KnobRelease` 模拟器映射 | TODO | 协议有 tag `0x05`，CLI 模拟器当前未映射 |
| PermissionResponse 无 pending 反馈 | TODO | `ipc_handler.rs` 有 TODO：协议支持后给 hook caller 回 deny |
| FrameData 下行策略 | TODO | 当前 `/frame` 可取帧，真机是否需要 `FrameData` 下发需定案 |
| 配置热更新覆盖面 | TODO | 部分配置保存后需重启或未同步 runtime |
| 音效上传格式校验 | TODO | 当前只校验 `.wav`、大小和 RIFF header，可补采样率/时长限制 |
| 文档路径统一 | DONE | README 已加入 `docs/` 入口，旧根目录文档作为背景资料保留 |

## 已实现但需要持续验证

| 项 | 当前状态 | 验证方式 |
|----|----------|----------|
| Claude Code hook | 已实现 | `cargo run -p vk-daemon -- setup claude-code` 后发起 session |
| HTTP API | 已实现 | `GET /health`, `GET /sessions`, `POST /event` |
| 权限阻塞审批 | 已实现 | `PreToolUse` 触发，模拟器 SEND/CANCEL 响应 |
| YOLO deny/allow | 已实现 | 修改 `yolo.active/allow/deny` 后发权限事件 |
| Session 列表和状态机 | 已实现 | simulator CLI 和 Tauri GUI 观察 |
| 音频基础能力 | 已实现 | `POST /sounds/play` 和 `/notify/test` |
| 配置原子写入 | 已实现 | `config set` 和 `POST /config` |
| 安全边界 | 已实现部分 | 本地监听、路径 canonicalize、fail-closed、osascript 输入校验 |

## 验证清单

每次改协议或状态机后至少跑：

```bash
cargo test --workspace
cargo check --workspace
```

手工 smoke test：

```bash
cargo run -p vk-daemon -- serve --headless
cargo run -p vk-simulator -- --cli
curl http://127.0.0.1:19280/health
curl http://127.0.0.1:19280/sessions
```

权限事件 smoke test：

```bash
curl -X POST http://127.0.0.1:19280/event \
  -H "Content-Type: application/json" \
  -d '{
    "type":"SessionStart",
    "session_id":"demo-1",
    "name":"Demo",
    "source":"claude-code"
  }'

curl -X POST http://127.0.0.1:19280/event \
  -H "Content-Type: application/json" \
  -d '{
    "type":"PreToolUse",
    "session_id":"demo-1",
    "tool_name":"Write",
    "tool_input":"main.rs"
  }'
```
