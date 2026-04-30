# Changelog — Vibe Keyboard

## Session 1 (续) — 2026-03-31 — 按键/模拟器/流程设计

### 按键设计
- 确认沿用 V2 HTML 布局: DELETE/CANCEL/MODE/SESSION + SEND/VOICE + KNOB
- Allow 模式"快捷+完整"双路径确认保留
- LCD Normal 模式只显示当前 session 详情，旋钮进入 Select 页面浏览全部
- 多 permission 全部显示排队

### 模拟器架构
- **双进程架构**: simulator 进程(模拟键盘固件) + daemon 进程(Tauri App)
- **IPC 通信**: Unix domain socket，后续替换为 USB HID
- **Transport trait**: IPC / USB HID / Channel 三种实现
- **Tauri 一体化**: daemon = Tauri App，内嵌 LCD Canvas 镜像
- **CLI 贯穿**: 所有模块都有 CLI 调试接口
- **LCD 多后端**: CLI(ratatui) / GUI(minifb) / Tauri(Canvas) / SPI(真LCD)
- 详见 doc/architecture.md

### 流程优化
- **Onboarding**: brew install + 自动检测 AI 工具 + 自动配置 hook，Time to Value ~2-3 分钟
- **待机屏**: 品牌 logo + 时间
- **Session 溢出**: Normal 只显示当前，Select 页面可滚动全部
- **多审批冲突**: 全部排队，SESSION 按钮循环
- 详见 doc/interaction-flows.md

### 产出文档
- doc/architecture.md — 完整软件架构 + monorepo 结构 + CLI 接口
- doc/interaction-flows.md — 8 个交互流程 + 屏幕状态机 + onboarding + 边缘场景

---

## Session 1 — 2026-03-31

### Context
用户提供 draft.md，包含硬件布局、软件架构、通信协议设计思路。参考: super-controller (Tauri 桌面 AI agent 通知中心), binflow (自描述二进制协议), peon-ping (第三方 AI agent 通知系统)。

### Phase 1: 深度提问 (8 MECE 维度 + 硬件/混合补充)

**已覆盖维度:**
1. ✅ 问题定义 — 核心痛点: 多 session 中"找寻目标"
2. ✅ 用户画像 — Primary: AI 重度用户 (3+ session 并行)
3. ✅ 核心价值 — 消除"找寻"步骤, LCD 常驻状态显示
4. ✅ 商业模式 — $99 一次性, 软件免费
5. ✅ 技术可行性 — SC 插件系统已验证多平台适配
6. ✅ 竞争格局 — 品类真空, 12-18 月窗口
7. ✅ 规模潜力 — SAM $1-2.5 亿, SOM 5K-10K 台
8. ✅ 执行路径 — 模拟器先行, KS 众筹, 量产

**深度工具使用:**
- ✅ 5 Whys (2 层: 为什么物理设备? 为什么不 Stream Deck?)
- ✅ JTBD 四力分析 (推/拉/焦虑/惯性)
- ✅ 反画像法 (不用 AI coding agent 的人 = 不是用户)
- ✅ 用户旅程 5 问 (Q1-Q5 全部完成)
- ✅ 痛点量化 (最痛步骤: "找寻目标 session")

**用户旅程提取:**
- Q1 Trigger: 3+ session 并行, 频繁切换找不到目标
- Q2 Current flow: peon-ping 声音 → 手动找窗口/tab(最痛) → 审批 → 切回
- Q3 Pain step: "找寻目标" — 在多个 terminal tab/窗口中定位正确 session
- Q4 Future flow: LCD 提示 → 旋钮按下(LCD+桌面同步跳转) → 旋转选择 → 确认
- Q5 First experience: 插 USB → 安装 daemon → 配置各工具 hook → 开始使用

### Phase 2: 调研 (两轮, 6 个方向)

**第一轮:**
1. 竞品分析 → 品类真空, Stream Deck 最大间接威胁
2. 市场规模 → SAM $1-2.5 亿, 2026 年 2400 万+ AI 开发者
3. BOM 成本 → qty1000 整机 ~$11.5, $99 毛利 88%

**第二轮:**
4. GTM 策略 → KS + HN/Reddit/YouTube, CAC <$10 早期
5. 定价分析 → $99 黄金价位, 双 SKU 锚定
6. 发射案例 → Flipper Zero (社区), Elgato (品类), TE (设计)

### Phase 3: 专家挑战

| 挑战 | 结果 |
|------|------|
| Stream Deck 威胁 | "真实 vibecoding 感受"是不可复制的差异化 |
| 模拟器 vs 原型矛盾 | 模拟器验证软件逻辑, 触感由硬件原型验证 |
| 用户群体收敛 | 从"三类都要"收敛到"AI 重度用户优先" |
| **开源 vs 闭源矛盾** | **从"全部闭源"修正为"部分开源"(固件+SDK 开源, 硬件闭源)** |
| 单/双 SKU | 先 $99 单 SKU, 验证后出 $129 高配 |
| 品类命名 | "AI Controller" 倾向, 待最终确定 |

### Key Decisions (Session 1)
1. 商业化方向 — 产品化/商业
2. 核心用户 — AI 重度用户 (3+ 并行 session)
3. 定价 — $99 甜点价 (首批单 SKU)
4. 商业模式 — 硬件一次性购买, 软件免费
5. 平台 — macOS 优先
6. 开发策略 — 模拟器先行, 实现 draft 全部功能
7. 开源策略 — **部分开源** (固件+SDK 开源, 硬件闭源)
8. GTM — KS 众筹 + 社区口碑 + 内容营销
9. 声音 — 键盘内置小喇叭
10. Voice — MVP 不做, 预留接口
11. LCD — 混合显示 (概览↔详情)
12. SC 关系 — 完全独立, 大量参考复用
13. OTA — USB 有线更新
14. 认证 — 已规划 FCC/CE
15. 团队 — 用户+AI 为主, 硬件外包

### Open Questions (for next session)
1. LCD 标准屏 vs 定制屏最终决策
2. 品牌命名最终确定
3. 详细按键映射方案 (需专门文档)
4. Onboarding 流程优化 (简化 hook 配置)
5. 供应链、售后、物流、包装 (硬件运营维度)
6. 模拟器 MVP 的详细技术架构设计
7. Kickstarter 页面策划
