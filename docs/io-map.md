# IO 资源映射表

## 范围说明

当前工程已经定义软件层 IO 资源和模拟器映射，但尚未在 SG2002/RV Nano 上绑定实际 GPIO/pinmux。本文档把“已由源码确认”的逻辑映射和“真机待定”的硬件资源分开记录。

## 总体资源

| 类别 | 当前实现 | 真机目标 | 状态 |
|------|----------|----------|------|
| 按键 | `vk-input::ButtonInput` + simulator 键盘事件 | GPIO 输入或矩阵键盘 | 逻辑已定，GPIO 待定 |
| 旋钮 | `vk-input::EncoderInput` + simulator 方向键/空格 | EC11/同类编码器 GPIO A/B/SW | 逻辑已定，GPIO 待定 |
| LCD | `vk-display::DynFramebuffer`，RGB565 | SPI/DSI/RGB LCD 驱动 | framebuffer 已定，物理接口待定 |
| LED | `vk-input::LedController` + terminal ANSI | 独立 GPIO/PWM 或 WS2812/灯环 | 逻辑已定，器件待定 |
| 喇叭 | `vk-input::Speaker` + rodio/WAV | ALSA、PWM buzzer、I2S 或 DAC | 逻辑已定，硬件待定 |
| 设备通信 | Unix socket IPC | USB HID 或板端 socket/串口 | IPC 已实现，USB HID 待定 |
| 桌面控制 | HTTP API + CGEvent/JXA | macOS 当前实现 | 已实现 macOS 路径 |

## 物理布局逻辑

V2 布局中包含 6 个按钮、1 个带按压旋钮、LCD、按钮 LED/旋钮灯环、喇叭。

```text
LCD

DELETE   CANCEL        KNOB
MODE     SESSION
SEND     VOICE
```

## 按键资源映射

| 逻辑按键 | `ButtonId` | 协议编码 | HTTP id | 模拟器按键 | 默认动作 |
|----------|------------|----------|---------|------------|----------|
| 删除 | `Delete` | `0` | `delete` | `d` | 执行宏，默认 `ctrl_u` |
| 取消 | `Cancel` | `1` | `cancel` | `Esc` | Deny/返回/中断 |
| 模式 | `Mode` | `2` | `mode` | `m` | PLAN/YOLO 切换 |
| 通知/Session | `Session` | `3` | `session` | `s` | 通知中心或 session 相关操作 |
| 发送 | `Send` | `4` | `send` | `Enter` | 发送/确认/Allow |
| 语音 | `Voice` | `5` | `voice` | `v` | 执行宏，默认 `fn` |

### 真机 GPIO 分配占位

| 逻辑按键 | 建议电气 | SG2002 pad/GPIO | Linux 设备节点 | Debounce | 状态 |
|----------|----------|-----------------|-----------------|----------|------|
| Delete | 上拉输入，按下接地 | TBD | TBD | 5-20 ms | 待原理图 |
| Cancel | 上拉输入，按下接地 | TBD | TBD | 5-20 ms | 待原理图 |
| Mode | 上拉输入，按下接地 | TBD | TBD | 5-20 ms | 待原理图 |
| Session | 上拉输入，按下接地 | TBD | TBD | 5-20 ms | 待原理图 |
| Send | 上拉输入，按下接地 | TBD | TBD | 5-20 ms | 待原理图 |
| Voice | 上拉输入，按下接地 | TBD | TBD | 5-20 ms | 待原理图 |

## 旋钮资源映射

| 资源 | 逻辑事件 | 协议 | 模拟器 | 说明 |
|------|----------|------|--------|------|
| 旋钮 A/B | `EncoderEvent::Rotate` | `KnobRotate { direction, steps }`，tag `0x03` | `Down` = Clockwise，`Up` = CounterClockwise | 方向由 A/B 相位决定 |
| 旋钮按压 | `EncoderEvent::Press` | `KnobPress`，tag `0x04` | `Space` | 进入/确认 |
| 旋钮释放 | `EncoderEvent::Release` | `KnobRelease`，tag `0x05` | 当前模拟器未映射 | 真机可补释放事件 |

### 真机旋钮占位

| 信号 | 建议电气 | SG2002 pad/GPIO | Linux 设备节点 | 状态 |
|------|----------|-----------------|-----------------|------|
| Encoder A | 上拉输入，中断或轮询 | TBD | TBD | 待原理图 |
| Encoder B | 上拉输入，中断或轮询 | TBD | TBD | 待原理图 |
| Encoder SW | 上拉输入，按下接地 | TBD | TBD | 待原理图 |

## LCD 显示资源

| 项 | 当前值 | 来源/说明 |
|----|--------|-----------|
| framebuffer | RGB565 | `vk-display` |
| 默认宽高 | `800 x 340` | `DisplayConfig::default()` |
| 每帧大小 | `800 * 340 * 2 = 544000 bytes` | `/frame` 返回原始 RGB565 |
| UI 状态 | Standby/Normal/Select/Allow/Notify | `vk-ui::ScreenStateMachine` |
| 字体 | unifont CJK 支持 | renderer 文档描述 |

### 真机 LCD 占位

| 信号/资源 | 候选接口 | SG2002 pad/GPIO | Linux/U-Boot 节点 | 状态 |
|-----------|----------|-----------------|--------------------|------|
| Pixel data | SPI / MIPI DSI / RGB / framebuffer | TBD | TBD | 待屏幕资料和原理图 |
| Reset | GPIO output | TBD | TBD | 待确认 |
| Backlight | PWM 或 GPIO output | TBD | TBD | 待确认 |
| TE/IRQ | GPIO input，可选 | TBD | TBD | 待确认 |
| Power enable | GPIO output，可选 | TBD | TBD | 待确认 |

备注：根目录交互文档里出现过 `128 x 480` 的布局草图，当前源码默认配置为 `800 x 340`。接真屏前需要统一面板分辨率、旋转方向和 framebuffer 尺寸。

## LED 资源

| 逻辑资源 | 协议 | 当前颜色常量 | 用途 |
|----------|------|--------------|------|
| Button LED | `SetLed { button, color, blink }`，tag `0x84` | `OFF`, `GREEN`, `AMBER`, `RED`, `ORANGE` | 按键状态和通知提示 |
| Knob Ring | `SetKnobRing(LedColor)`，tag `0x85` | 同上 | 模式/权限/告警提示 |

### 真机 LED 占位

| 资源 | 候选实现 | SG2002 pad/GPIO | 状态 |
|------|----------|-----------------|------|
| Delete LED | GPIO/PWM/灯带通道 | TBD | 待原理图 |
| Cancel LED | GPIO/PWM/灯带通道 | TBD | 待原理图 |
| Mode LED | GPIO/PWM/灯带通道 | TBD | 待原理图 |
| Session LED | GPIO/PWM/灯带通道 | TBD | 待原理图 |
| Send LED | GPIO/PWM/灯带通道 | TBD | 待原理图 |
| Voice LED | GPIO/PWM/灯带通道 | TBD | 待原理图 |
| Knob ring | WS2812/PWM RGB/多 GPIO | TBD | 待原理图 |

## 音频资源

| 资源 | 当前实现 | 协议 | 说明 |
|------|----------|------|------|
| 播放音效 | `rodio` + 内置 WAV | `PlaySound(SoundType)`，tag `0x86` | 权限、完成、错误、点击 |
| 音量 | daemon 下发 | `SetVolume(u8)`，tag `0x8A` | 0-100 |
| 静音 | daemon 下发 | `SetMuted(bool)`，tag `0x8B` | true/false |
| 事件映射 | daemon 下发 | `SetSoundMapping`，tag `0x8C` | `builtin:*` 或 `custom:*` |

真机候选：

| 方案 | 优点 | 待确认 |
|------|------|--------|
| ALSA 声卡 | Linux 应用层最简单 | RV Nano 镜像是否启用音频设备 |
| PWM buzzer | IO 少，提示音足够 | PWM pin、驱动、音质 |
| I2S/DAC | 音质好 | 硬件 BOM 和驱动复杂度 |

## 通信资源

| 通道 | 当前实现 | 路径/端口 | 状态 |
|------|----------|-----------|------|
| HTTP hook/API | axum | `127.0.0.1:19280` | 已实现 |
| 模拟设备 IPC | Unix domain socket | `/tmp/vk-daemon.sock` | 已实现 |
| 测试通道 | tokio mpsc Channel | 内存 | 已实现 |
| 真机 USB HID | `UsbHidTransport` 预留 | TBD | 待实现 |
| 真机串口/网络 | 未定义 | TBD | 可作为调试或过渡方案 |

## SG2002/RV Nano pinmux 待确认清单

接硬件前必须确认：

1. 原理图中 6 个按键、旋钮 A/B/SW、LCD、背光、LED、喇叭分别接到哪些 pad。
2. 这些 pad 是否被 SD/eMMC、Wi-Fi、摄像头、屏幕、调试串口占用。
3. Linux 下对应 GPIO number、pinmux function、pull up/down、驱动节点。
4. U-Boot 阶段是否需要点亮屏幕或背光。
5. 应用层是直接访问 `/dev/gpiochip*`/`/dev/fb*`/ALSA，还是通过内核驱动暴露抽象设备。

## 建议落地顺序

1. 先用 1 个 GPIO 按键打通 `ButtonInput` 真机实现。
2. 再接旋钮 A/B/SW，验证方向和 debounce。
3. 接 LCD framebuffer 或屏幕驱动，先显示纯色/测试图，再接 renderer。
4. 接 LED 或灯环，验证 `SetLed`/`SetKnobRing`。
5. 接音频，先支持 click/alert 两个音效。
6. 最后把传输从 IPC 切到 USB HID 或实际选定通道。
