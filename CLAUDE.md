# CLAUDE.md

## Project Overview

Vibe Keyboard - AI Agent 物理控制器，通过按钮/旋钮/LCD 管理多个 AI coding session。

Tech stack: Rust 2024 + Tauri v2 + React (ESP32 firmware is a separate future project)

## Dev Flow

This project uses Dev Flow (strict preset) for constraint-driven AI development.
State: doc/state.json | Config: doc/flow-config.json | Collaboration: driver mode

### Commands

| Command | Purpose |
|---------|---------|
| /dev-flow:flow | Smart router — reads state, routes to correct phase |
| /dev-flow:dev | Execute development tasks |
| /dev-flow:testing | Run test gate |
| /dev-flow:checkpoint | Independent quality review |
| /dev-flow:task-done | Complete task, update docs |

### Project Documents

| File | Purpose |
|------|---------|
| doc/state.json | State machine (current phase + task) |
| doc/flow-config.json | Phase chain + depth config |
| doc/progress.md | Current task status |
| doc/roadmap.md | Task breakdown (18 milestones, 173 tasks) |
| doc/design/ | Milestone design docs |
| doc/requirements.md | BDD Scenarios (Gherkin) |
| doc/solution.md | Technical solution |
| doc/architecture.md | Full software architecture + monorepo structure |
| doc/interaction-flows.md | 8 interaction flows + screen states |
| ideas/ | Product research (competitive, market, BOM, GTM, pricing) |

### Architecture Key Points

- **8 crates**: vk-core (零依赖共享类型), vk-protocol (codec), vk-transport (async IPC/Channel), vk-display, vk-input, vk-ui, vk-simulator, vk-daemon (all std)
- **Dual process**: simulator (keyboard firmware) ↔ daemon (Tauri App) via IPC
- **Transport trait**: IPC / USB HID / Channel — pluggable backends (in vk-transport)
- **CLI everywhere**: every module has CLI debug interface
- **LCD multi-backend**: CLI (ratatui) / GUI (minifb) / Tauri (Canvas) / SPI (real LCD)

### Rules

- All crates use std (no_std is dropped; ESP32 firmware is a separate project)
- Transport trait is async (tokio)
- All public traits need unit tests
- CLI subcommands for every debuggable module
- Conventional commits (feat/fix/refactor/docs/test)
- 中文对话，英文代码/commit

### Build & Test

```bash
cargo check              # Type check
cargo test               # Run tests
cargo clippy --workspace # Lint
cargo run -p vk-simulator -- --cli  # Run simulator
```
