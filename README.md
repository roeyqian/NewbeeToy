# Newbee Toy

## 中文版

### 项目简介

`Newbee Toy` 是一个面向 Windows 桌面的轻量工具箱，使用 Rust + Slint 构建，聚焦高频文件与系统辅助操作。

### 核心功能

- `Newbee Rename`：批量重命名文件/文件夹，支持普通替换、正则替换、计数语法、预览、排除行、撤销最近一次执行。
- `Newbee Icon`：从 `exe/dll/icl/lnk/ico` 提取或复制图标，支持扫描预览、排除条目、批量导出。
- `Newbee Unlock`：扫描占用目标文件的进程，并尝试结束可释放的占用进程。
- `Newbee Env`：设置/编辑系统环境变量，支持预设路径、预设保存/加载与双确认应用。

### 项目特点

- 原生桌面 UI，分类入口清晰（通用 / 媒体 / 系统）。
- 各模块均提供预览区与日志区，便于先检查后执行。
- 自动持久化窗口状态、语言和最近路径。
- 每个模块日志默认最多保留 100 行。

### 快速开始

#### 运行环境

- Windows（推荐）
- Rust toolchain（含 `cargo`）

#### 开发运行

```powershell
cargo run --release
```

#### 构建发布

```powershell
cargo build --release
```

### 配置与文件

首次运行会在可执行文件目录生成或使用：

- `config/base.toml`：主配置（窗口状态、语言、最近路径）
- `config/env.toml`：环境变量模块默认预设文件（可在界面中改为其他预设路径）
- `lang.toml`：语言覆盖文件（可选）

### 平台与权限说明

- 项目主要面向 Windows。
- `Newbee Unlock` 依赖 Windows Restart Manager。
- `Newbee Env` 写入系统环境变量（注册表 `HKLM`）通常需要管理员权限。

### 安全提示

- 批量重命名、文件解锁、环境变量写入都可能影响系统或工程状态。
- 建议先在测试目录验证流程，再对正式数据执行操作。

### 文档

- 使用教程：`USAGE.md`
- 资源目录：`assets/`

### 许可证

本项目采用 Apache License 2.0 许可证，详见 `LICENSE`。

---

## English Version

### Overview

`Newbee Toy` is a lightweight toolbox for Windows desktop workflows, built with Rust + Slint, focused on practical file and system utilities.

### Core Features

- `Newbee Rename`: Batch rename files/folders with plain text or regex replacement, counter syntax, preview, row exclusion, and one-step undo for the latest run.
- `Newbee Icon`: Extract or copy icons from `exe/dll/icl/lnk/ico` with scan preview, row exclusion, and batch export.
- `Newbee Unlock`: Scan processes locking a target file and attempt to terminate releasable lockers.
- `Newbee Env`: Set/edit system environment variables with configurable preset path, preset save/load, and two-step apply confirmation.

### Highlights

- Native desktop UI with clear category entry points (General / Media / System).
- Each module includes preview and log panels for safer operations.
- Automatic persistence for window state, language, and recent paths.
- Each module keeps up to 100 log lines by default.

### Quick Start

#### Requirements

- Windows (recommended)
- Rust toolchain with `cargo`

#### Run in Development

```powershell
cargo run --release
```

#### Build Release

```powershell
cargo build --release
```

### Configuration Files

On first run, the app creates or uses:

- `config/base.toml`: main app config (window state, language, recent paths)
- `config/env.toml`: default preset file for the environment module (can be changed via the preset-path field)
- `lang.toml`: optional language override file

### Platform and Permission Notes

- This project primarily targets Windows.
- `Newbee Unlock` depends on Windows Restart Manager.
- `Newbee Env` writes system environment variables in registry `HKLM`, which usually requires administrator privileges.

### Safety Notes

- Rename, unlock, and environment writes can affect system/project state.
- Validate workflows in a test folder before running on production data.

### Documentation

- Usage guide: `USAGE.md`
- Asset directory: `assets/`

### License

This project is licensed under the Apache License 2.0. See `LICENSE` for details.
