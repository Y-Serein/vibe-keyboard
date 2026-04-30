# Vibe Keyboard (工作代号)

## One-liner
全球首款 AI Agent 物理控制器 — 6 按钮 + 旋钮 + LCD + 内置喇叭，有线 USB，专为多 session 并行 vibecoding 设计。

## Product Type
Hybrid (hardware + embedded firmware + desktop software)

## Core Problem
同时运行 3+ AI coding session 的开发者，需要频繁切换窗口、处理 permission 请求、监控 agent 状态。最大痛点是**"找寻目标 session"** — 在多个 terminal tab/窗口中定位正确的 session，打断心流且浪费时间。

## Target User
**Primary**: AI 重度用户 — 同时运行 3+ AI agent session 的开发者
**Anti-Persona**: 不使用 AI coding agent 的开发者
**Secondary** (后续): 极客开发者、内容创作者

## Value Proposition
- **消除"找寻"步骤**: 旋钮按下 = LCD 定位 + 桌面窗口自动跳转，零认知负担
- **独立状态屏**: LCD 常驻显示所有 session 状态，不占主屏空间
- **一键操作**: Allow/Deny/YOLO/Cancel/Voice，减少鼠标操作
- **开箱即用**: 专为 vibecoding 设计的默认配置

## User Journey
**Trigger**: 3+ AI session 并行，频繁切换找不到目标
**As-Is**: peon-ping 声音 → 手动找窗口/tab(最痛) → 审批 → 切回
**To-Be**: LCD 提示 → 旋钮按下(LCD+桌面同步跳转) → 旋转选择 → 确认
**Aha Moment**: 第一次按旋钮看到桌面自动跳转到正确窗口

### 三大核心场景
1. **审批请求**: LCD闪烁 → 旋钮按下(自动跳转) → 旋转选择 → 确认
2. **完成通知**: 键盘喇叭响 + LCD变色 → 一眼看到
3. **状态概览**: LCD常亮显示 → 旋转浏览 → 按下跳转

## JTBD 四力分析
| 力 | 内容 | 强度 |
|---|---|---|
| 推力 | 找 session 太慢，状态丢失，频繁打断 | 中高 |
| 拉力 | 旋钮一按直达 + LCD 实时状态 + vibecoder 身份 | 高 |
| 焦虑 | $99 值不值？配置会不会很麻烦？ | 中 |
| 惯性 | 快捷键/Stream Deck 已经"够用了" | 中 |

## Competitive Position
- **品类**: 全新品类，零量产直接竞品（2026年3月调研）
- **间接竞品**: Stream Deck ($80-250), QMK 宏键盘 ($10-80)
- **护城河**: 深度 AI agent 集成 + 开箱即用 + 先发品牌 + 部分开源生态
- **窗口期**: ~12-18 个月
- **软件替代品**: agent-deck (1.8k stars), Conductor, agtx — 证明需求真实

## Pricing & Economics
- **定价策略**: $99 单 SKU (首批)，后续出 $129 CNC 铝壳高配版
- **BOM** (qty 1000): ~¥83.5 (~$11.5)
- **毛利率**: ~88%，净利率 ~60-65%
- **商业模式**: 硬件一次性购买，软件免费更新
- **中国市场**: ¥499/¥699 双轨定价
- **ROI 话术**: "每天省 30 分钟 × 250 天 = 125 小时/年，两周回本"

## Market
- **TAM**: 2026年 ~2400 万 AI 辅助开发者
- **SAM**: ~$1-2.5 亿
- **首年 SOM**: 5,000-10,000 台 ($50-100 万)
- **平台**: macOS 优先

## GTM Strategy
- **渠道**: HN Show HN + Reddit + Twitter/X + YouTube
- **路径**: Kickstarter (首选) + 同步独立站
- **CAC 目标**: 早期 <$10, 增长期 <$20
- **内容核心**: 30-90 秒痛点演示视频 ("切 session 前 vs 后")
- **参考案例**: Flipper Zero (社区驱动), Elgato (品类定义)

## Key Components
1. **硬件**: 6 按钮(机械轴) + 1 旋钮(带LED环) + LCD (128×480 或标准) + 小喇叭
2. **通信层**: USB HID，参考 binflow 协议
3. **屏幕系统**: trait 驱动模拟器 + 嵌入式可移植，混合显示(概览↔详情)
4. **桌面端 daemon**: 独立于 SC，但大量参考复用 SC 架构和代码
5. **工具集成**: 复用 SC 的多平台插件/适配器系统 (Claude Code, Cursor, Codex, etc.)
6. **Monorepo 结构**

## Hardware Design
- **MCU**: ESP32-S3 (USB HID 原生支持)
- **按钮**: 6x 机械轴(热插拔) + 键帽
- **旋钮**: EC11 编码器 + WS2812B LED 环 + 金属旋钮帽
- **屏幕**: 128×480 LCD 或标准 1.47" IPS
- **音频**: 小型蜂鸣器/喇叭
- **连接**: USB-C 有线
- **外壳**: 首批注塑，高配 CNC 铝壳

## Software Design
- **配置**: 默认一套 + 桌面端 GUI 可自定义
- **Voice**: 两种模式(设备麦+桌面麦)，MVP 不做，预留接口
- **固件更新**: USB 有线
- **开源策略**: 固件 + SDK 开源，硬件设计闭源

## Development Strategy
1. **模拟器 MVP**: 实现 draft.md 全部功能，验证完整软件架构
2. **硬件原型**: ESP32-S3 开发板 + 面包板验证物理交互
3. **Kickstarter 众筹**: 100-300 台 (3D打印外壳)
4. **首批量产**: 1000 台 (注塑)
5. **高配版**: $129 CNC 铝壳

## Team
- 创始人(用户) + AI 为主力
- 创始人负责框架/架构设计
- 硬件 PCB/结构设计外包
- FCC/CE 认证已规划

## Open Decisions
1. **LCD 选型**: 标准 1.47" 172×320 vs 定制 128×480
2. **品牌命名**: "Vibe Keyboard" / "AI Controller" / 其他 — 待定
3. **详细按键映射方案**: 需专门讨论并文档化
4. **Onboarding 优化**: 安装 daemon + 配置 hook 的体验简化

## Status
research-complete

## Created
2026-03-31
