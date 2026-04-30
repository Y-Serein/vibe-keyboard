## 当前在做什么

把 `vibe-keyboard` 在 WSL2 (Ubuntu 24.04, WSLg) 桌面端真正跑通，让 Tauri 窗口能加载 React UI 并连上 daemon。当前状态：**完全跑通**，从 daemon → Vite dev → Tauri webview → 显示 session 列表 / LCD / 虚拟键盘 全链路验证 ok（绿点 "Connected to daemon"，LCD 显示真实 session 数据，Cost/Token/Model 都对）。

为了让以后一行命令就能起整套环境，新写了 `start-dev.sh`。详见下文。

本轮代码改动：

- `desktop/vite.config.ts`：加了一行 `base: "./"`。**注意**：这其实是排错时的误诊修复——dev profile 下 Tauri 走 `devUrl`，根本不读 `dist/`，所以 base 改与不改对当前 dev 流程都没影响；但对未来 release build（用 `frontendDist`）是有意义的，留着。
- `start-dev.sh`（新文件）：一行启动 daemon + vite dev + Tauri 的脚本，trap 关窗自动清理。

当前 dirty 文件：

- `desktop/vite.config.ts`
- `start-dev.sh`（新增）
- `HANDOFF.md`

（上一轮的 dirty 文件 `crates/vk-daemon/src/transcript.rs`、`desktop/src-tauri/Cargo.toml`、`desktop/src-tauri/gen/schemas/linux-schema.json` 还在，未提交。）

## 已经试过的方案和结果（含失败的）

- 顺着上轮 HANDOFF 起：daemon (`cargo run -p vk-daemon -- serve --headless`) 启动正常，2 个 session 被扫到，HTTP 19280 + IPC `/tmp/vk-daemon.sock` 都能用。
- ALSA 那一坨 warning 仍在，HANDOFF 里点过名，无影响。
- `curl --noproxy '*' http://127.0.0.1:19280/{health,sessions,config}` 全绿。
- 第一次启动 Tauri：`CARGO_TARGET_DIR=/tmp/vk-target cargo run -p vk-desktop` 能起窗口，但**只有标题栏，body 全空**。
  - WebKitGTK 的 EGL/MESA warning（`failed to get driver name for fd -1` 之类）是 WSLg 软渲染的预期噪音，不是问题。
  - WebView devtools 用 `Ctrl+Shift+I` / `F12` 都开不出来（WSLg 键盘转发不到），**只能用「右键 → Inspect Element」**。
  - 开了 devtools 后 Console 是空的、`<body>` 也空，但 inspector 顶部 URL 显示 **`about:blank`**——这是关键线索。
- 误诊一：以为是 Tauri 自定义 scheme 不喜欢绝对路径 `/assets/...`，给 `vite.config.ts` 加 `base: "./"` 然后 `vite build`。dist 路径改对了，但窗口仍空白。
- 真正根因：**Tauri v2 在 debug build (`cargo run`) 下优先用 `tauri.conf.json` 里的 `devUrl: http://localhost:15173`**。它不会自动启动 Vite，也不会 fallback 到 `frontendDist`，连不上就停在 `about:blank`。
- 用户原本只跑了 `vite build`（一次性产 dist 然后退出），没起 dev server，所以 15173 没人监听。
- 修复：**第三个终端**起 `node node_modules/vite/bin/vite.js`（不带 build），让它常驻；再启动 Tauri。窗口立刻有内容：5 个 Tab、LCD 渲染 session #1 (slam, claude-opus-4-7, Context 1%, Cost $63.62, Tokens 3.9M/59.1k)、虚拟键盘、旋钮、Activity Log 全在。
- 写了 `start-dev.sh` 自动化三件套：先杀残留进程 + 删 socket → 后台起 daemon、轮询 `/health` 直到 200 → 后台起 vite、轮询 `:15173` 直到通 → 前台跑 Tauri。EXIT/INT/TERM trap 关窗清理。`chmod +x` 在 DrvFS 上 EPERM，所以必须用 `bash start-dev.sh` 调用。
- 项目里原本就有的 `run.sh` **不能用**：是给 macOS 写的（最后还有 `osascript`），用 `npm run tauri dev`，但本机 `node_modules/.bin/` 不存在（`npm ci --no-bin-links`），那条路走不通。

## 下一步计划（3-5条actionable)

1. **日常启动一行命令**：`bash /home/slam/Sipeed/rv_nano/tools/vibe-keyboard/start-dev.sh`。日志在 `/tmp/vk-logs/{daemon,vite}.log`，关窗或 Ctrl+C 自动停所有进程。
2. **小 bug 待查**：UI 顶栏写 "0 sessions" 但 LCD 已经显示 "1/2"。两边数据源不一致——LCD 直接拿到了 session 数据，但 React 顶栏依赖的 Tauri `session-update` 事件首屏没收到。看 `desktop/src-tauri/src/lib.rs` 的事件 emit 逻辑 + `App.tsx:47` 的 `listen("session-update")`，可能 daemon 没在启动时主动 push 一次，要轮询才会推。
3. **验证物理键盘 IPC 流**：开第四个终端 `cargo run -p vk-simulator -- --cli`，看 daemon 能否打 `IPC: simulator connected`，按键事件能否走通。当前只用了 Tauri 内的虚拟键盘。
4. **验证 release build 能脱离 vite dev**：`CARGO_TARGET_DIR=/tmp/vk-target cargo run --release -p vk-desktop`。如果能直接从 `dist/` 加载（依赖那个 `base: "./"` 改动），可以加一个 `start-dev.sh` 的 `--release` 模式，省掉 vite dev 终端。
5. **决定改动是否要 commit**：本轮 + 上轮累计 dirty 5 个文件 + 1 个新文件 (`start-dev.sh`)。`base: "./"` 改动是误诊但保留有价值；`start-dev.sh` 是 WSL2 专用，要不要进 git 自己判断。

## 关键文件路径（相对路径，一行一个）

start-dev.sh
desktop/vite.config.ts
desktop/src-tauri/tauri.conf.json
desktop/src-tauri/src/lib.rs
desktop/src-tauri/Cargo.toml
desktop/src-tauri/gen/schemas/linux-schema.json
desktop/src/App.tsx
desktop/dist/index.html
crates/vk-daemon/src/transcript.rs
crates/vk-daemon/src/server/api.rs
crates/vk-simulator/src/main.rs
run.sh
HANDOFF.md
CLAUDE.md
docs/development.md

## 还没搞清楚的问题

- 顶栏 "0 sessions" vs LCD "1/2" 的数据源不一致。LCD 是怎么拿到的？是 Tauri backend 主动 poll daemon 然后通过别的事件推过来的吗？需要看 `lib.rs` 里 emit 的所有事件名以及前端 `listen` 对应关系。
- `vite.config.ts` 加 `base: "./"` 在 release/production 流程里是否真的让 Tauri custom scheme 加载成功？dev 流程下不影响，但没在 release build 里实测。
- WebKitGTK 在 WSLg 上的 `Ctrl+Shift+I` / `F12` 为什么不响应？是 WSLg 的全局快捷键拦截，还是 WebKit 没注册？右键能用就先这样。
- 上轮 HANDOFF 留下的：是否要把仓库迁到 WSL ext4。当前 `CARGO_TARGET_DIR=/tmp/vk-target` 已经能绕过 Tauri build script EPERM 和 `chmod +x` EPERM，能用就先不动。
- `npm ci --no-bin-links` 装的依赖在普通 Linux 文件系统上是否还需要 no-bin-links。如果未来迁到 ext4，可以重装试试，恢复 `npm run` 流程，那 `start-dev.sh` 也能简化（直接 `npm run dev` 而不是手写 `node node_modules/vite/bin/vite.js`）。
- 真机/桌面音频未验证（daemon 启动时 ALSA `Unknown PCM default`，按钮 click/buzz 等内置音效在桌面上是否能出声没试）。

---

## 附：start-dev.sh 用法

每次启动**一行命令**：

```bash
bash /home/slam/Sipeed/rv_nano/tools/vibe-keyboard/start-dev.sh
```

行为：
1. 杀掉残留 daemon / vite 进程 + 删 `/tmp/vk-daemon.sock`
2. 后台跑 daemon，等 `http://127.0.0.1:19280/health` 返回 200（最多 60s）
3. 后台跑 vite dev，等 `http://localhost:15173/` 通（最多 60s）
4. 前台跑 Tauri，窗口出来即可用
5. **关窗或 Ctrl+C** → trap 清理 daemon、vite、socket

后台日志：`/tmp/vk-logs/daemon.log`、`/tmp/vk-logs/vite.log`。要看实时 daemon 事件：`tail -f /tmp/vk-logs/daemon.log`。

任一步起不来时脚本会 tail 对应 log 末尾 30 行然后退出，直接看到原因。
