# Vibe Keyboard — 交互流程设计

## 按键布局 (V2 确认版)

```
┌─── LCD (128×480) ──────────────────┐
│                                     │
│  (Normal: 当前 session 详情)         │
│  (Select: session 列表滚动)          │
│  (Allow: permission 审批界面)        │
│  (Standby: 品牌 logo + 时间)        │
│                                     │
└─────────────────────────────────────┘

┌──────────┬──────────┬──────────────┐
│ DELETE ⊗ │ CANCEL ✘ │              │
│ erase    │ stop AI  │    KNOB      │
├──────────┼──────────┤  (旋转+按下)  │
│ MODE ⚡  │ NOTIFY🔔 │   🟠/🟢     │
│ PLAN/YOLO│ 通知中心  │               │
├──────────┴──────────┴──────────────┤
│    SEND ↵      │     VOICE 🎤      │
│   (大按钮)      │    (大按钮)        │
└────────────────┴───────────────────┘
```

## LCD 屏幕状态机 (5 状态, CR-005 新增 Notify)

```
                    ┌──────────┐
          ┌────────►│ Standby  │◄── 无 session 时
          │         │ logo+时间│
          │         └────┬─────┘
          │              │ session 出现
          │              ▼
          │         ┌──────────┐   旋转旋钮    ┌──────────┐
          │    ┌───►│  Normal  │ ────────────► │  Select  │
          │    │    │ 当前详情  │ ◄──────────── │ session列表│
          │    │    └──┬───┬───┘  按下旋钮/超时  └──────────┘
          │    │       │   │
 session  │    │ allow │   │ NOTIFY 按钮
 全部结束  │    │ /deny │   │
          │    │       │   ▼
          │    │  ┌────┴───────┐    ┌──────────┐
          │    │  │   Allow    │    │  Notify  │
          │    │  │  审批选择   │    │ 通知列表  │
          │    │  └────────────┘    └────┬─────┘
          │    │       │                 │ 选择跳转/超时
          │    └───────┴─────────────────┘
          └────────────────────────────────────┘
```

### Standby (待机)
- **触发**: daemon 运行但无活跃 session
- **显示**: 品牌 logo + 当前时间 + daemon 连接状态
- **退出**: 检测到任何 session → 自动切换到 Normal

### Normal (当前 session 详情)
- **显示**: 当前聚焦 session 的完整信息
  ```
  ┌────────────────────────────────────────┐
  │ RustAgent                    ● active  │
  │ Claude Code · opus-4-6          🔔3    │  ← 通知徽章在第2行末尾
  │ ────────────────────────────           │
  │ Status: Thinking...                    │
  │ Branch: feat/agent-loop                │
  │ Tokens: 12.5k in / 8.2k out           │
  │ Cost:   $0.47                          │
  │ Last:   "Implementing the agent..."    │
  │ ────────────────────────────           │
  │ Mode: PLAN          Context: 45%       │
  └────────────────────────────────────────┘
  ```
- **通知徽章**: 第 2 行末尾显示 "🔔N"（不遮挡第 1 行 session 名和状态）
  - 红色 = 有紧急通知 (permission/error)
  - 蓝色 = 只有普通通知 (complete/question)
  - 无通知时不显示
- **操作**: 旋转旋钮 → Select / 按 NOTIFY → Notify

### Toast 卡片 (右侧弹出, 覆盖在任意屏幕上)
- **触发**: SessionStatus 变化产生通知 (Done/Error/PermissionNeeded) 或 context_pct > 90%
- **显示**: 从屏幕右边缘滑入，覆盖在右侧空白区域（Normal 屏数据行在左半部分，右侧有空间）
  ```
  ┌────────────────────────────────────────┐
  │ vibe-keyboard  CC             1/32     │
  │ Status    done                         │
  │ Model     claude-opus-4-6  ┌─────────┐│
  │ Context   45% of 1M ██████ │✓FrontEnd││ ← 右侧弹入
  │ Cost      $1054.26         │ complete ││
  │ Tokens    70.1M in  32.2k  └─────────┘│
  │ ...                                    │
  └────────────────────────────────────────┘
  ```
- **边框颜色**: 红=PermissionNeeded/Error, 蓝=Done, 琥珀=ResourceLimit
- **自动消失**: 5 秒后向右滑出消失
- **用户操作**: 任何按钮/旋钮 → 立即消失
- **不打断**: 不改变屏幕状态，不影响当前操作
- **堆叠**: 多个通知同时到达时，新卡片在旧卡片下方（最多 2 个）

### Select (Session 列表)
- **触发**: 在 Normal 模式下旋转旋钮
- **显示**: 所有 session 的列表，当前选中高亮
  ```
  ┌────────────────────────────────────────┐
  │ ▸ RustAgent        ● thinking    $0.47 │
  │   FrontEnd         ○ idle        $0.12 │
  │   DevOps           ⚠ permission  $0.33 │
  │   TestRunner       ✓ done        $0.08 │
  │   DataPipeline     ● writing     $1.20 │
  │ ────────────────────────────           │
  │ ▲▼ browse   ⏎ switch   ← back        │
  └────────────────────────────────────────┘
  ```
- **操作**:
  - 旋转旋钮: 上下滚动高亮
  - 按下旋钮: 切换到选中 session (桌面端同步跳转)
  - 超时 3 秒不操作: 自动返回 Normal
- **优先级排序**: 需审批 > 运行中 > 完成 > 空闲

### Allow (审批选择)
- **触发**: session 发出 permission 请求时自动弹出
- **显示**:
  ```
  ┌────────────────────────────────────────┐
  │ ⚠ PERMISSION REQUEST                   │
  │ ────────────────────────────           │
  │ Session: RustAgent                     │
  │ Action:  Write main.rs                 │
  │ ────────────────────────────           │
  │ Knob → ● ALLOW  ○ DENY  ○ ALWAYS      │
  │                                        │
  │ SEND = Quick Allow  CANCEL = Quick Deny│
  └────────────────────────────────────────┘
  ```
- **多审批**: 全部显示，底部显示 "1/3 pending"，旋钮可切换
- **视觉**: LCD 边框绿色，旋钮        变绿
- **操作**:
  - 快捷: SEND = 立即 Allow, CANCEL = 立即 Deny
  - 完整: 旋转旋钮选 Allow/Deny/Always → SEND 确认
  - NOTIFY 按钮: 打开 Notify 列表

### Notify (通知中心) — CR-005 新增
- **触发**: 在任意状态下按 NOTIFY 按钮
- **显示**: 通知列表 = 未读 (置顶) + 历史已读，按时间和紧急度排列
  ```
  ┌────────────────────────────────────────┐
  │ 📋 NOTIFICATIONS (2 new · 5 total)    │
  │ ────────────────────────────           │
  │ ▸ ⚠ RustAgent    Permission: Write..  │  ← 未读, 红色, 粗体
  │   ✘ DevOps       Bash exit code 1     │  ← 未读, 红色, 粗体
  │ ── history ─────────────────           │
  │   ✓ FrontEnd     Task complete  10:32 │  ← 已读, 蓝色, 暗色
  │   ✓ DataPipe     Task complete  10:28 │  ← 已读
  │   ⚡ RustAgent    Context 92%   10:15  │  ← 已读
  │ ────────────────────────────           │
  │ ▲▼ browse  ⏎ jump  ✘ back            │
  └────────────────────────────────────────┘
  ```
- **分区**: 上方=未读 (粗体, 按紧急度排序) / 下方=历史已读 (暗色, 按时间倒序)
- **排序**: 未读区: Permission > Error > ResourceLimit > Done / 已读区: 最近的在前
- **历史上限**: 保留最近 20 条已读通知，超过自动清理最旧的
- **操作**:
  - 旋钮旋转: 上下选择通知 (可滚动到历史区)
  - 旋钮按下/SEND: 跳转到对应 session 窗口
    - 未读 permission → 进入 Allow 状态 + 标记已读
    - 未读 complete/error → 进入 Normal + 标记已读
    - 已读通知 → 仅跳转到 session (不改变通知状态)
  - CANCEL: 返回 Normal
  - 5 秒无操作: 自动返回 Normal
- **无通知**: 按 NOTIFY 显示空列表 "No notifications yet"

## 8 个核心交互流程

### Flow 1: 发送消息 (SEND)
```
前置: 用户在 terminal 编辑器里已输入文字
动作: 按 SEND 按钮
效果: 触发 Enter 键 → 消息发送
备注: SEND 在 Allow 模式下 = Quick Allow
```

### Flow 2: 语音输入 (VOICE)
```
动作: 长按 VOICE → 说话 → 释放
效果: PTT 录音 → STT 转文字 → 发送到当前 session
备注: MVP 不实现，按钮预留；先期可配置为键盘宏
```

### Flow 3: 删除输入 (DELETE)
```
动作: 按 DELETE
效果: 清空当前输入缓冲区 (Ctrl+U 或类似)
备注: 不影响 AI 执行历史
```

### Flow 4: 中断 AI (CANCEL)
```
前置: AI 正在执行 (Thinking/Tool Use)
动作: 按 CANCEL
效果: 发送 Escape 键 → 中断 AI 执行
Allow 模式: CANCEL = Quick Deny
```

### Flow 5: 切换模式 (MODE)
```
动作: 按 MODE
效果: PLAN ↔ YOLO 切换 (软切换，通过 daemon 发送命令到 AI 工具)
视觉: LED 绿色(PLAN) ↔ 琥珀色(YOLO)
LCD: 底部 Mode 字段更新
```

### Flow 6: Session 浏览与切换 (KNOB)
```
Normal 模式:
  旋转旋钮 → 进入 Select 页面，浏览 session 列表
  按下旋钮 → 切换到高亮 session + 桌面窗口跳转
  不操作 3 秒 → 自动返回 Normal

Allow 模式:
  旋转旋钮 → 切换 Allow / Deny / Always 选项
  按下旋钮 → 确认选择 (或用 SEND 确认)
```

### Flow 7: 通知中心 (NOTIFY) — CR-005 重新设计
```
前置: 有未读通知 (permission/error/complete/resource_limit)
视觉: NOTIFY 按钮 LED 闪烁 (红=紧急, 蓝=普通)
      Normal 屏右上角显示通知数量徽章 "🔴3"

动作: 按 NOTIFY
效果: LCD 切换到 Notify 屏幕 (第 5 个屏幕状态)

Notify 屏幕:
  ┌────────────────────────────────────────┐
  │ 📋 NOTIFICATIONS (3)                   │
  │ ────────────────────────────           │
  │ ▸ ⚠ RustAgent    Permission: Write..  │  ← 红色
  │   ✘ DevOps       Bash exit code 1     │  ← 红色
  │   ✓ FrontEnd     Task complete        │  ← 蓝色
  │ ────────────────────────────           │
  │ ▲▼ browse  ⏎ jump  ✘ back            │
  └────────────────────────────────────────┘

操作:
  旋钮旋转 → 上下滚动通知列表
  旋钮按下/SEND → 跳转到该 session 的终端窗口
    - permission 类型 → 跳转后进入 Allow 状态
    - complete/error → 跳转后进入 Normal 状态
    - 通知标记为已读/移除
  CANCEL → 返回 Normal
  3 秒无操作 → 自动返回 Normal
  无通知时按 NOTIFY → 短暂显示 "No notifications"

排序: permission > error > resource_limit > complete > info
```

### Flow 8: 审批处理 (ALLOW MODE)
```
触发: daemon 检测到 permission 请求
自动:
  1. LCD 自动切换到 Allow 界面 (绿色边框)
  2. 旋钮        变绿
  3. 喇叭发出提示音
  4. 桌面同步跳转到对应 session 窗口

用户操作 (快捷路径):
  SEND → 立即 Allow → 返回 Normal
  CANCEL → 立即 Deny → 返回 Normal

用户操作 (完整路径):
  旋转旋钮 → 选择 Allow/Deny/Always
  SEND → 确认选择 → 返回 Normal

多审批:
  LCD 显示 "1/3 pending"
  处理完当前 → 自动弹出下一个
  NOTIFY 按钮 → 切换到其他待审批
```

## Onboarding 流程

### 安装
```bash
# macOS
brew install vk-daemon

# 或一键脚本
curl -fsSL https://get.vibekeyboard.dev | sh
```

### 首次启动
```
1. 启动 daemon
   $ vk-daemon serve

2. 自动检测
   daemon 扫描已安装的 AI 工具:
   ✓ Claude Code 检测到 (v1.x)
   ✓ Cursor 检测到 (v0.x)
   ✗ Codex 未安装
   ✗ Amp 未安装

3. 自动配置 hook
   → Claude Code: 自动安装 hook (类似 SC 的 hooks/install.js)
   → Cursor: 自动安装 extension
   → 需要用户确认 Accessibility 权限 (macOS)

4. 连接设备
   → 检测 USB 设备 / 启动 simulator
   → LCD 显示 "Connected ✓"

5. 引导完成
   → LCD 显示简短教程:
     "旋转旋钮浏览 session"
     "按下跳转"
     "SEND = 发送/Allow"
```

### Time to Value: 约 2-3 分钟
- brew install: 30 秒
- 首次启动 + 自动配置: 60 秒
- 权限确认: 30 秒
- 第一次旋钮切换 session: 即时 aha moment

## 边缘场景

| 场景 | 处理方式 |
|------|---------|
| daemon 未启动 | LCD 显示 "Waiting for daemon..." |
| USB 断开 (真硬件) | daemon 显示警告，设备 LCD 显示 "Disconnected" |
| session 崩溃 | LCD 显示 ✘ 状态，红色高亮 |
| 喇叭被关闭 | 仅视觉通知 (LCD + LED) |
| 用户在 Allow 模式下长时间不操作 | 30 秒后 LCD 降低亮度，但不自动退出 Allow |
| 8+ session 同时运行 | Normal 只显示当前 session，Select 页面可滚动浏览全部 |
| 多个 permission 同时到达 | 全部排队显示，NOTIFY 按钮循环切换 |
