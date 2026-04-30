# 接口文档

## 约定

- Base URL：`http://127.0.0.1:19280`
- Content-Type：除 `/frame` 和 `/sounds/upload` 外均为 `application/json`
- HTTP server 只绑定本机地址。
- 设备协议分为上行 `UplinkMessage` 和下行 `DownlinkMessage`。

## Daemon HTTP API

### 健康检查

| Method | Path | 说明 | 返回 |
|--------|------|------|------|
| GET | `/health` | daemon 存活检查 | `200 OK` |

### Session 和 Hook

| Method | Path | 说明 |
|--------|------|------|
| POST | `/event` | AI 工具 hook 事件入口 |
| GET | `/sessions` | 返回当前 session 列表 |

#### POST `/event`

请求示例：

```json
{
  "type": "SessionStart",
  "session_id": "abc-123",
  "name": "vibe-keyboard",
  "source": "claude-code",
  "cwd": "/Users/me/code/vibe-keyboard",
  "permission_mode": "default",
  "transcript_path": "/Users/me/.claude/projects/session.jsonl",
  "bundle_id": "com.googlecode.iterm2",
  "session_tty": "/dev/ttys042"
}
```

支持事件：

| `type` | 映射 |
|--------|------|
| `SessionStart`, `session_start`, `init` | 创建/更新 session |
| `SessionEnd`, `session_end`, `exit` | 结束 session |
| `PreToolUse` | `ToolUse`；同时视为权限检查 |
| `PostToolUse` | `Writing` |
| `Notification` | 有 `tool_input` 或 tool 名包含 permission 时为权限请求，否则为 `Thinking` |
| `UserPromptSubmit` | `Thinking` |
| `Stop` | `Done` |
| `SubagentStart` | `Thinking` |
| `SubagentStop` | `Writing` |
| `status`, `tool_use`, `message` | legacy 状态事件 |
| `permission`, `permission_request` | legacy 权限请求 |

权限请求返回示例：

```json
{
  "hookSpecificOutput": {
    "decision": {
      "behavior": "allow"
    }
  }
}
```

权限处理规则：

1. Always Allow 命中则直接 allow。
2. YOLO deny 优先于 allow。
3. YOLO 未命中则入队等待用户操作。
4. 用户 300 秒内无响应或内部错误，默认 deny。

#### GET `/sessions`

返回 `SessionInfo[]`：

```json
[
  {
    "id": 1,
    "name": "vibe-keyboard",
    "status": "thinking",
    "has_permission_request": false,
    "source": "claude-code",
    "cwd": "/Users/me/code/vibe-keyboard",
    "permission_mode": "default",
    "model": "claude-opus-4-6",
    "tokens_in": 12000,
    "tokens_out": 8200,
    "cost_usd": 0.47,
    "context_pct": 45,
    "last_message": "Implement feature",
    "last_ai_output": "Running tests",
    "bundle_id": "com.googlecode.iterm2",
    "session_tty": "/dev/ttys042",
    "started_at": 1777521600,
    "last_activity": 1777521700
  }
]
```

`status` 取值：`thinking`, `tool_use`, `writing`, `done`, `error`, `idle`, `permission_needed`。

### 控制接口

| Method | Path | 请求 | 说明 |
|--------|------|------|------|
| POST | `/button` | `{ "id": "send", "action": "click" }` | 虚拟按钮 |
| GET | `/button/state` | - | 返回按住中的键 |
| POST | `/knob` | `{ "action": "cw", "steps": 1 }` | 虚拟旋钮 |
| GET | `/frame` | - | 返回 LCD RGB565 原始帧 |
| GET | `/log` | - | 最近 50 条活动日志 |
| GET | `/yolo` | - | 当前 YOLO 状态 |

#### POST `/button`

`id` 取值：

| id | ButtonId | 默认行为 |
|----|----------|----------|
| `send` | `Send` | 发送/确认/Allow |
| `cancel` | `Cancel` | 返回/中断/Deny |
| `mode` | `Mode` | 模式切换 |
| `session` | `Session` | 通知/Session 相关操作 |
| `delete` | `Delete` | 宏，默认 `ctrl_u` |
| `voice` | `Voice` | 宏，默认 `fn` |

`action` 取值：

| action | 说明 |
|--------|------|
| `click` | 完整按下释放，默认值 |
| `down` | 按下并保持，主要用于宏 |
| `up` | 释放，主要用于宏 |
| `toggle` | 发送按钮按下事件 |

#### POST `/knob`

```json
{ "action": "cw", "steps": 1 }
```

| action | 说明 |
|--------|------|
| `cw` | 顺时针 |
| `ccw` | 逆时针 |
| `press` | 按下旋钮 |

#### GET `/frame`

返回 `application/octet-stream`，内容为 RGB565 原始字节。

Header：

| Header | 说明 |
|--------|------|
| `X-LCD-Width` | LCD 宽度，默认 `800` |
| `X-LCD-Height` | LCD 高度，默认 `340` |

### 配置接口

| Method | Path | 说明 |
|--------|------|
| GET | `/config` | 获取当前配置 |
| POST | `/config` | 设置单个配置项 |

POST 示例：

```json
{ "key": "yolo.active", "value": "true" }
```

支持 key：

| key | 类型 | 说明 |
|-----|------|------|
| `yolo.active` | bool | 启用 YOLO |
| `yolo.allow` | CSV string | allow 规则列表 |
| `yolo.deny` | CSV string | deny 规则列表 |
| `yolo.notify_auto_allow` | bool | 自动 allow 是否通知 |
| `general.hook_port` | u16 | hook 端口，保存后需重启生效 |
| `macros.delete` | string | DELETE 宏 |
| `macros.voice` | string | VOICE 宏 |
| `display.width` | u16 | LCD 宽 |
| `display.height` | u16 | LCD 高 |
| `sound.enabled` | bool | 音频开关 |
| `sound.volume` | u8 | 音量，代码内限制到 0-100 |
| `sound.muted` | bool | 静音 |
| `sound.mapping.permission_alert` | string | 权限提示音 |
| `sound.mapping.session_complete` | string | 完成提示音 |
| `sound.mapping.error` | string | 错误提示音 |
| `sound.mapping.click` | string | 点击提示音 |

### Setup 接口

| Method | Path | 说明 |
|--------|------|------|
| GET | `/setup/status` | 检测 AI 工具、推荐工具、系统状态 |
| POST | `/setup/install/{tool_id}` | 安装 hook |
| POST | `/setup/uninstall/{tool_id}` | 卸载 hook |
| POST | `/setup/brew-install/{package}` | 安装允许列表内 brew 包 |
| POST | `/setup/brew-uninstall/{package}` | 卸载允许列表内 brew 包 |

`tool_id`：

| tool_id | 状态 |
|---------|------|
| `claude-code` | hook 安装/卸载已实现 |
| `cursor` | 检测部分存在，hook 安装/卸载未实现 |
| `codex` | 检测部分存在，hook 安装/卸载未实现 |

`package` 允许列表：

| package | 用途 |
|---------|------|
| `iterm2` | 推荐终端 |
| `terminal-notifier` | macOS 通知 |

### 声音接口

| Method | Path | 请求 | 说明 |
|--------|------|------|------|
| GET | `/sounds` | - | 列出内置和自定义音效 |
| POST | `/sounds/play` | `{ "sound_type": "builtin:alert" }` | 播放音效 |
| POST | `/sounds/upload` | multipart `file` | 上传 WAV |

`/sounds/play` 支持：

- 直接 sound id：`builtin:alert`, `builtin:ding`, `builtin:buzz`, `builtin:click`, `custom:<name>`
- 事件 key：`permission_alert`, `session_complete`, `error`, `click`

上传限制：

- 文件名必须以 `.wav` 结尾。
- 最大 500 KB。
- 文件头必须为 `RIFF`。
- 保存到配置目录下 `vk-daemon/sounds/custom/`。

## 设备二进制协议

Wire format：

```text
[1B tag][fields...]
string = [2B little-endian length][utf8 bytes]
Vec<T> = [1B count][items...]
LedColor = [r, g, b]
```

### 上行 Keyboard -> Daemon

| Tag | Message | 字段 |
|-----|---------|------|
| `0x01` | `ButtonPress` | `button_id:u8` |
| `0x02` | `ButtonRelease` | `button_id:u8` |
| `0x03` | `KnobRotate` | `direction:u8`, `steps:u8` |
| `0x04` | `KnobPress` | - |
| `0x05` | `KnobRelease` | - |
| `0x06` | `PermissionResponse` | `session_id:u16`, `action:u8` |
| `0x07` | `SessionSwitch` | `session_id:u16` |

### 下行 Daemon -> Keyboard

| Tag | Message | 字段 |
|-----|---------|------|
| `0x81` | `SessionListUpdate` | `count:u8`, `SessionInfo[]`, `active_index:u8` |
| `0x82` | `SessionStatusChange` | `session_id:u16`, `status:u8` |
| `0x83` | `PermissionRequest` | `session_id:u16`, `action_desc:string` |
| `0x84` | `SetLed` | `button_id:u8`, `LedColor`, `blink:u8` |
| `0x85` | `SetKnobRing` | `LedColor` |
| `0x86` | `PlaySound` | `sound_type:u8` |
| `0x87` | `DismissPermission` | `session_id:u16` |
| `0x88` | `FrameData` | `width:u16`, `height:u16`, `pixel_len:u32`, `pixels` |
| `0x89` | `NotificationListUpdate` | `count:u8`, `NotificationInfo[]` |
| `0x8A` | `SetVolume` | `volume:u8` |
| `0x8B` | `SetMuted` | `muted:u8` |
| `0x8C` | `SetSoundMapping` | `sound_type:u8`, `sound_id:string` |

### 枚举编码

`ButtonId`：

| 值 | ButtonId |
|----|----------|
| 0 | `Delete` |
| 1 | `Cancel` |
| 2 | `Mode` |
| 3 | `Session` |
| 4 | `Send` |
| 5 | `Voice` |

`Direction`：`0 = Clockwise`, `1 = CounterClockwise`

`PermissionAction`：`0 = Allow`, `1 = Deny`, `2 = Always`

`SessionStatus`：

| 值 | 状态 |
|----|------|
| 0 | `Thinking` |
| 1 | `ToolUse` |
| 2 | `Writing` |
| 3 | `Done` |
| 4 | `Error` |
| 5 | `Idle` |
| 6 | `PermissionNeeded` |

`SoundType`：`0 = PermissionAlert`, `1 = SessionComplete`, `2 = Error`, `3 = Click`

## 结构体字段

### `SessionInfo`

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u16` | daemon 内部分配的数字 ID |
| `name` | `String` | 显示名 |
| `status` | `SessionStatus` | 状态 |
| `has_permission_request` | `bool` | 是否有待审批 |
| `source` | `String` | 来源，例如 `claude-code` |
| `cwd` | `String` | 工作目录 |
| `permission_mode` | `String` | AI 工具权限模式 |
| `model` | `String` | 模型名 |
| `tokens_in` | `u64` | 输入 token |
| `tokens_out` | `u64` | 输出 token |
| `cost_usd` | `f64` | 估算成本 |
| `context_pct` | `u8` | 上下文占用百分比 |
| `last_message` | `String` | 最近用户消息 |
| `last_ai_output` | `String` | 最近 AI 输出 |
| `bundle_id` | `String` | 终端/IDE bundle id |
| `session_tty` | `String` | TTY |
| `started_at` | `u64` | 启动时间戳 |
| `last_activity` | `u64` | 最近活动时间戳 |

### `NotificationInfo`

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u32` | 通知 ID |
| `session_id` | `u16` | 关联 session |
| `session_name` | `String` | session 名 |
| `status` | `SessionStatus` | 通知类型/状态 |
| `description` | `String` | 文案 |
| `timestamp` | `u64` | 时间戳 |
| `read` | `bool` | 是否已读 |
