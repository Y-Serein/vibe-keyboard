# Vibe Keyboard 文档索引

本文档目录整理当前 `vibe-keyboard` 工程的开发资料、接口说明、IO 资源映射和待办事项。内容以当前源码为准，根目录下的历史方案文档仍保留作为背景资料。

## 文档列表

| 文档 | 内容 |
|------|------|
| [development.md](./development.md) | 工程结构、构建运行、调试流程、核心数据流 |
| [api.md](./api.md) | Daemon HTTP API、Hook 事件、设备协议、配置字段 |
| [io-map.md](./io-map.md) | 物理按键/旋钮/LCD/LED/音频/通信资源映射 |
| [todo.md](./todo.md) | 已确认待办、风险、验证清单 |

## 当前状态摘要

- 当前可运行形态：macOS/桌面模拟器 + `vk-daemon` + Tauri GUI。
- 当前硬件适配状态：SG2002/RV Nano 真机 IO 驱动尚未落地，已有 Rust trait 边界可承接 GPIO、编码器、LED、喇叭、LCD 和通信后端。
- 当前 AI 工具集成：Claude Code hook 已实现；Cursor/Codex hook 检测和安装仍待实现。
- 默认 daemon 地址：`127.0.0.1:19280`。
- 默认 IPC socket：`/tmp/vk-daemon.sock`。

## 相关历史文档

| 文件 | 说明 |
|------|------|
| [../README.md](../README.md) | 项目总览和快速启动 |
| [../architecture.md](../architecture.md) | 8 crate 软件架构 |
| [../solution.md](../solution.md) | 双端职责、trait 抽象、设计决策 |
| [../interaction-flows.md](../interaction-flows.md) | 屏幕状态机和按键交互流程 |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | 开发者指南和 API 概览 |
