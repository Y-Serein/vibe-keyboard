## 当前在做什么

把 vibe-keyboard 的 **LCD 内容 + 物理控制器布局** 重做。三大块改动都已落地：
1. **屏幕分辨率**：800×340 → **960×412**（匹配 3.4 寸 412W×960H 真实面板，横放使用）。
2. **物理控制器布局**（Tauri React UI）：参考 `output/image.jpg`，左 6 个**横向矩形按键**（3×2 网格），右上**可拖动旋钮**（CSS+JS 实现），右下**长条 SEND 键**。
3. **LCD 渲染**（`vk-ui` crate）：参考 `output/Image1.png`，多 session OVERVIEW 仪表盘——顶部横幅 + 8 列表格（SESSION/TOOL/MODE/MODEL/COST/PLAN USAGE LIMITS/RESETS IN/STATE）+ 底部 focus/task/cost/now 状态栏。Active session 垂直居中 + 琥珀色边框区分。

字体仍是位图（vk-display + unifont），不是真正矢量抗锯齿。给位图新增了 3 档 scale：
- `FONT_*_TINY` = 6×10 (1x)，底部状态栏专用
- `FONT_*_SM`   = 12×20 (2x)，**默认主字体**，绝大多数文字
- `FONT_*_MD`   = 18×30 (3x)，目前 vk-ui 里没有 caller，但常量+函数已就绪

当前 dirty（vs 上一轮 HANDOFF 起点累计）：
- `crates/vk-daemon/src/config.rs` — 960×412 默认值
- `crates/vk-daemon/src/transcript.rs` — 上轮的 mtime/length 修复
- `crates/vk-ui/src/renderer.rs` — `render_normal` 完全重写
- `crates/vk-ui/src/widget.rs` — 新增 SM/MD/TINY 字体 + `draw_text_at` 通用栅格化函数
- `crates/vk-simulator/src/main.rs` — 测试断言 800x340 → 960x412
- `crates/vk-simulator/src/sim_display.rs` — `terminal_rows_needed(340)` → `(412)`
- `crates/vk-ui/src/widget.rs` 顶部注释 (340 → 412)
- `desktop/src/components/Screen.tsx` — canvas 960×412，去 `pixelated`，原生 aspectRatio
- `desktop/src/components/VirtualKeyboard.tsx` — 全重写新布局
- `desktop/src/App.tsx` — 容器宽度 580 → 720
- `desktop/src-tauri/Cargo.toml` — 上轮的 indexmap 修复
- `desktop/src-tauri/tauri.conf.json` — 窗口 680×820 → 820×900
- `desktop/src-tauri/gen/schemas/linux-schema.json` — 上轮 build 副产物
- `desktop/vite.config.ts` — 加 `base: "./"`（其实当前 dev 流程下不影响，仅 release 有意义；保留）
- `start-dev.sh`（新文件）— 一行启动 daemon+vite+Tauri 的脚本
- `HANDOFF.md`

## 已经试过的方案和结果（含失败的）

**屏幕分辨率换成 960×412**：
- `crates/vk-daemon/src/config.rs` 默认值 + `crates/vk-ui/src/renderer.rs::LCD_W/LCD_H` 常量都改了。
- 简单替换没踩坑，因为渲染器里用 `fb.width()/height()` 居多，只有几处硬编码常量。
- 测试里有 `"800x340"` 字符串 + `terminal_rows_needed(340)` 也跟着改。
- React `Screen.tsx` 里 canvas 内部 width/height 改 960/412，CSS 用 `aspectRatio: 960/412` 自适应；`imageRendering` 从 `pixelated` 改成 `auto` → 浏览器自带降采样让小字看上去更柔和。

**LCD 显示比例反复试错**：
- 用户说"高是长的 2.29 倍" → 试过 CSS `aspectRatio: 2.29/1` + `maxWidth: 560`：被指出"长度也降低了"，回退。
- 几何事实：满宽下高度 = 容器宽 / 比值。要降低视觉高度只能加 maxWidth 或用更扁的比值。最终结论：**回到原生 960/412 比 + 满宽**，承认 LCD 区域就是 ~688×295 这个高度。

**物理控制器（VirtualKeyboard.tsx）布局**：
- 第一版：按键 1:1 正方形 + Tauri 窗口装不下底部 SEND → 用户截图 view.png 反馈。
- 改为 3:2 横向矩形（aspectRatio `3 / 2`），高度从 ~175px 降到 ~117px → 装得下了。
- 旋钮：rotary knob 实现 CSS+JS 拖动旋转 + 30° 步进 cw/ccw 事件 + 单击 = press。圆形径向金属渐变 + 锯齿纹（`repeating-conic-gradient` + `mask`）+ 顶部指示槽。

**LCD UI 设计反复迭代**（用户指了 `output/image1.png` 作为参考）：
- v3 第一版：单 session 详情 + Status/Model/Context/Cost/Tokens 多行列表（接近原版风格）。
- v4：改为 OVERVIEW 多 session 表格但用大 LG 字体 → 信息密度低、空白多。
- v5：active 行用 MD 字体 (18×30) → **列重叠**，"Claude" + "PLAN" + "opus-4.7" + "$" 全粘一起。
- v6：active 改两行卡片（行 1 MD: SESSION+STATE，行 2 SM: 其他）→ 用户驳回："我就是要一行显示完整！"
- **v7（当前）**：所有 row 单行 SM；active 行只靠琥珀边框 + ALERT 色区分；垂直居中 + 上下堆叠。
- 底部 focus/task/cost/now 一开始用 SM → CJK glyph 在 SM 下放大到 32px 高（unifont 16×16 × UNI_SCALE_SM=2），**溢出 LCD 底边**。改用 TINY（unifont 1× = 16px CJK）解决，同时正好满足"字体可以小一半"。

**编译错误踩坑**：
- 删 LG 字体 import 但 widget.rs 里没动 → 没问题，是 renderer.rs 自己的 import 块。
- 加 `FONT_W_MD` 时忘了 import → 单字段错误，加上即解。

**架构事实确认**：
- daemon 不会自动保存 config.toml 到磁盘（除非显式调用 `save_config`），所以改默认分辨率重启就生效，不用清缓存。
- daemon 支持的 button id：`send / cancel / mode / session / delete / voice` —— 6 个。`fn`（VirtualKeyboard 里那个 MULTI 占位按钮）daemon 不认，会被丢弃，目前无害。

## 下一步计划（3-5条actionable)

1. **看 v7 之后的截图确认 LCD 布局符合预期**。如果 OK，这一轮的视觉迭代就告一段落。
2. **真正抗锯齿字体**（用户提过"字体也需要优化"）：workspace 加 `ab_glyph = "0.2"`，仓库 vendored 一份 IBM Plex Mono / Cascadia Mono TTF（80–200KB），新写一个 grayscale 光栅器把 glyph alpha 跟背景 RGB565 alpha-blend。改动 200 行 + 影响 vk-ui 全部 5 个 render 函数。这是一段独立工作，不要和别的混做。
3. **给 ScreenStateMachine 加 `mode_yolo: bool`**，让 daemon 把 yolo 状态推到 sm，renderer 里 MODE 列才能真显示 PLAN/YOLO，不再硬编码 PLAN/AUTO/REVIEW 占位。这个改动跨 vk-protocol → vk-daemon → vk-ui 三层。
4. **接 RESETS IN 真数据**：Anthropic 5h rolling window 重置时间需要从 hooks/api 拿。现在全部 `—` 占位。如果暂时拿不到就换成展示别的有用信息（比如 "last activity 5m ago"）。
5. **清理上轮 + 这轮的 dirty file 一次性 commit**。本仓库累计 14+ 个 dirty 文件。建议拆 3 个 commit：
   a. 环境/build 修复（`transcript.rs`, `desktop/src-tauri/Cargo.toml`, `gen/schemas/linux-schema.json`, `vite.config.ts`, `start-dev.sh`）
   b. 屏幕分辨率（`config.rs`, `renderer.rs::LCD_W/H` 常量, simulator 测试断言, `widget.rs` 注释, `Screen.tsx`, `tauri.conf.json` 窗口大小）
   c. UI 重写（`renderer.rs::render_normal` + 辅助函数, `widget.rs` 新字体, `VirtualKeyboard.tsx`, `App.tsx` 容器宽度）

## 关键文件路径（相对路径，一行一个）

start-dev.sh
crates/vk-daemon/src/config.rs
crates/vk-daemon/src/transcript.rs
crates/vk-daemon/src/server/api.rs
crates/vk-ui/src/renderer.rs
crates/vk-ui/src/widget.rs
crates/vk-ui/src/screen.rs
crates/vk-protocol/src/message.rs
crates/vk-display/src/color.rs
crates/vk-simulator/src/main.rs
crates/vk-simulator/src/sim_display.rs
desktop/src/App.tsx
desktop/src/components/Screen.tsx
desktop/src/components/VirtualKeyboard.tsx
desktop/src-tauri/src/lib.rs
desktop/src-tauri/tauri.conf.json
desktop/src-tauri/Cargo.toml
desktop/vite.config.ts
output/image.jpg
output/Image1.png
HANDOFF.md
CLAUDE.md
docs/development.md

## 还没搞清楚的问题

- **`render_normal` 里 MODE 列硬编码 PLAN/AUTO/REVIEW 循环**——`ScreenStateMachine` 没有 yolo 字段，前端 `App.tsx` 里有 `modeYolo` 但只对前端 UI 状态用，没下发给 daemon。要真显示，需要给 sm 加字段 + daemon 里通过 IPC/HTTP 推过来。
- **RESETS IN 列**：5h rolling window 重置时间没接，全 `—`。Anthropic 的 hooks 里能不能拿到这个还没查。
- **顶栏 "0 sessions" vs LCD "1/2"** 数据源不一致（上一轮就有的 bug）：Tauri 端 `App.tsx::session-update` 事件首屏没收到，但 LCD 通过别的事件已经拿到 session 数据。`desktop/src-tauri/src/lib.rs` 里 emit 的事件名 + 前端 `listen` 对应关系还没逐一核对。
- **CSS `vite.config.ts::base: "./"`** 在 release build (frontendDist 走 dist) 是否真的让 Tauri custom scheme 加载成功？dev 流程下不影响，没在 release build 里实测。如果不需要，删掉。
- **WebKitGTK devtools 快捷键** Ctrl+Shift+I / F12 在 WSLg 下不响应。只能"右键 → Inspect Element"。是 WSLg 拦截还是 WebKit 没注册没查清。
- **`render_normal` 里几个 dead code 警告**（`format_k`, `context_bar_color`, `unread`）：旧 render_normal 留下的，要么删要么用。低优先级。

---

## 附：start-dev.sh 用法（不变，从上一轮带过来）

```bash
bash /home/slam/Sipeed/rv_nano/tools/vibe-keyboard/start-dev.sh
```

行为：
1. 杀掉残留 daemon / vite 进程 + 删 `/tmp/vk-daemon.sock`
2. 后台跑 daemon，等 `http://127.0.0.1:19280/health` 返回 200（最多 60s）
3. 后台跑 vite dev，等 `http://localhost:15173/` 通（最多 60s）
4. 前台跑 Tauri，关窗或 Ctrl+C → trap 清理 daemon、vite、socket

后台日志：`/tmp/vk-logs/daemon.log`、`/tmp/vk-logs/vite.log`。

任一步起不来时脚本会 tail 对应 log 末尾 30 行然后退出，直接看到原因。

**Vite dev server 必须开**（端口 15173）：Tauri v2 在 cargo run（debug profile）下走 `tauri.conf.json::devUrl`，不会自动 fallback 到 `frontendDist: ../dist`。这是上一轮花了一阵才定位到的根因——参考 v1.png 那个空白窗口截图。

**Tauri 编译目录必须在 ext4**：`CARGO_TARGET_DIR=/tmp/vk-target` 已写在 start-dev.sh 里。原仓库挂在 Windows DrvFS（`/home/slam/Sipeed → C:\Serein_Y\Sipeed`），Tauri build script 的 `fs::copy` 会在 DrvFS 下报 EPERM。

**前端依赖装在 `--no-bin-links` 模式**：`desktop/node_modules/.bin/` 不存在，所以 `npm run xxx` 流程不可用，必须用 `node node_modules/vite/bin/vite.js` 直接调入口。原仓库的 `run.sh` 是给 macOS 写的（最后还有 `osascript`），那个流程在这个环境下用不了。
