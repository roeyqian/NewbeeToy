# Newbee Toy

Windows desktop toolbox built with Rust and Slint.

`Newbee Toy` focuses on small, high-frequency Windows utilities: batch rename, icon extraction, file lock inspection/release, and system environment variable management.

## Languages

- [简体中文](#简体中文)
- [English](#english)

---

## 简体中文

### 项目简介

`Newbee Toy` 是一个面向 Windows 桌面的轻量工具箱，使用 Rust + Slint 构建。当前项目不是通用跨平台工具，核心功能依赖 Windows API，主要用于 Windows 下的文件处理与系统辅助操作。

### 已实现功能

| 模块 | 说明 |
| --- | --- |
| Newbee Rename | 批量重命名目录内文件和文件夹，支持普通替换、正则替换、大小写控制、计数语法、预览、移除预览行和撤销上次成功重命名。 |
| Newbee Icon | 从 `.exe`、`.dll`、`.icl`、`.ico` 扫描可提取项并导出 `.ico`；支持单文件或目录扫描，输出重名时自动追加序号。 |
| Newbee Unlock | 使用 Windows Restart Manager 检测文件占用进程；也支持目录扫描，会递归检查最多 256 个文件并合并占用进程。可尝试结束普通占用进程。 |
| Newbee System Environment | 读取、编辑、保存、加载并应用 Windows 系统环境变量快照；支持独立变量值编辑窗口和二次确认应用。 |

### 当前特性

- 原生 Windows 桌面窗口，UI 使用 Slint。
- 首页按 `通用 / 媒体 / 系统` 分类进入工具。
- 支持中文、英文、日文、西班牙文界面。
- 保存窗口尺寸、位置、锁定状态、语言和最近路径。
- 日志区会滚动保留最近内容，便于检查执行结果。
- 运行时配置默认保存在可执行文件所在目录。

### 运行与构建

#### 环境要求

- Windows
- Rust toolchain，包含 `cargo`

#### 开发运行

```powershell
cargo run --release
```

#### 构建发布版

```powershell
cargo build --release
```

构建产物通常位于：

```text
target/release/NewbeeToy.exe
```

### 运行时文件

程序会以可执行文件所在目录作为应用目录，并使用以下文件：

| 路径 | 用途 |
| --- | --- |
| `config/base.toml` | 主配置，保存窗口状态、语言、最近路径等。 |
| `config/sysenv.toml` | 系统环境变量模块的默认预设文件路径。 |
| `lang.toml` | 运行时语言表。不存在或无效时会从 `lang/*.json`、`assets/lang/*.json` 或开发目录资源重建。 |

如果将 `NewbeeToy.exe` 单独复制到其他位置运行，建议同时准备可写目录，并带上语言资源或首次运行生成的 `lang.toml`。

### 使用说明

#### Newbee Rename

基本流程：

1. 选择目标目录。
2. 输入 `查找` 和 `替换为`。
3. 按需启用 `区分大小写`、`启用正则表达式`、`启用计数语法`。
4. 点击 `生成预览`。
5. 检查预览表和日志；不需要处理的行可以移除。
6. 点击 `执行重命名`。
7. 需要回退时，点击 `撤销上次重命名`。

实现限制：

- 只处理所选目录的直接子项，不递归。
- 文件和文件夹都会进入预览。
- 空查找文本不会产生重命名动作。
- 目标名称不能包含 Windows 非法字符 `\ / : * ? " < > |`，也不能以空格或点结尾。
- 预览中存在错误或重名冲突时会阻止执行。
- 撤销只记录最近一次成功执行的重命名计划。

计数语法：

```text
<IncNr[:start[:step[:pad]]]>
```

示例：

```text
<IncNr:01>        -> 01, 02, 03, ...
<IncNr:10:-1:2>  -> 10, 09, 08, ...
File_<IncNr:1:1:3> -> File_001, File_002, ...
```

#### Newbee Icon

基本流程：

1. 选择输入文件或输入目录。
2. 点击 `扫描可提取项`。
3. 检查预览，按需移除不需要导出的行。
4. 选择输出目录。
5. 点击 `开始提取`。

实现细节：

- 支持 `.exe`、`.dll`、`.icl`、`.ico`。
- 输入为目录时只扫描该目录下的文件，不递归。
- `.ico` 输入会直接复制。
- `.exe`、`.dll`、`.icl` 会读取第一个可用的图标组并写出 `.ico`。
- 输出文件重名时会自动生成 `_2`、`_3` 等后缀，避免覆盖。

#### Newbee Unlock

基本流程：

1. 选择目标文件或目录。
2. 点击 `检测占用`。
3. 检查占用进程列表；不希望处理的行可以移除。
4. 点击 `移除占用`，程序会尝试结束剩余普通进程。

安全边界：

- 目标可以是文件或目录。
- 目录扫描会递归收集最多 256 个文件，并合并检测到的占用进程。
- Windows 系统目录下的目标会被阻止释放。
- 检测到系统进程占用时会阻止强制移除。
- “移除占用”的本质是终止进程，请只对明确来源的普通应用进程使用。

#### Newbee System Environment

这个模块编辑的是系统环境变量，对应注册表：

```text
HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
```

通常需要管理员权限。

推荐流程：

1. 进入模块后先点击 `加载系统`，把当前系统变量加载到预览表。
2. 如需新增变量，填写 `变量值路径` 和 `变量名`，点击 `新增变量`。
3. 如需编辑已有变量，点击表格行内的 `编辑`，在弹出的值编辑窗口中调整条目。
4. 如需删除变量，点击表格行内的 `移除`。
5. 可点击 `存储预设` 保存当前预览快照，也可用 `加载预设` 将预设变量合并到当前预览。
6. 点击 `应用` 第一次只显示新增、更新、删除数量。
7. 再次点击 `应用` 才会真正写入系统环境变量。

重要说明：

- 预览表会被视为最终系统变量快照。
- 应用时，系统中存在但预览表中不存在的变量会被删除。
- 因此修改系统环境变量前，强烈建议先点击 `加载系统`，再进行增删改。
- 变量值中包含 `%NAME%` 形式引用时会写为 `REG_EXPAND_SZ`，否则通常写为 `REG_SZ`。
- 写入后程序会广播 `WM_SETTINGCHANGE`，但部分已运行程序仍可能需要重启才能读取新环境变量。

### 开发结构

```text
src/main.rs                 程序入口、窗口状态、页面初始化
src/main.slint              主界面
src/core/config.rs          配置文件读写
src/core/lang.rs            多语言加载
src/core/general/rename.rs  批量重命名逻辑
src/core/general/unlock.rs  文件占用检测与释放
src/core/media/icon.rs      图标扫描与提取
src/core/system/sysenv.rs   系统环境变量管理
assets/lang/*.json          内置语言资源
assets/fonts/               UI 字体
assets/icon.*               应用图标
```

### 注意事项

- 批量重命名、终止进程、修改系统环境变量都可能造成不可逆影响。
- 对正式数据操作前，建议先在测试目录验证规则。
- Sys Env 模块尤其需要谨慎：如果没有先加载系统变量，直接应用少量预览变量，可能导致未出现在预览中的系统变量被删除。

### 许可证

本项目采用 Apache License 2.0，详见 [LICENSE](LICENSE)。

---

## English

### Overview

`Newbee Toy` is a Windows desktop toolbox built with Rust and Slint. It currently targets Windows-specific workflows and uses Windows APIs for several core features.

### Features

| Module | Description |
| --- | --- |
| Newbee Rename | Batch rename files and folders in one directory with plain replacement, regex, case sensitivity, counter syntax, preview, row exclusion, and one-step undo. |
| Newbee Icon | Scan `.exe`, `.dll`, `.icl`, and `.ico` files, then export `.ico` files. Supports single file or non-recursive directory input. |
| Newbee Unlock | Detect locking processes with Windows Restart Manager. Supports files and directories; directory scans inspect up to 256 files recursively. |
| Newbee System Environment | Manage Windows system environment variable snapshots with preset save/load, row editing, and two-step apply confirmation. |

### Quick Start

Requirements:

- Windows
- Rust toolchain with `cargo`

Run:

```powershell
cargo run --release
```

Build:

```powershell
cargo build --release
```

Release executable:

```text
target/release/NewbeeToy.exe
```

### Runtime Files

| Path | Purpose |
| --- | --- |
| `config/base.toml` | Window state, language, and recent paths. |
| `config/sysenv.toml` | Default preset path for the system environment module. |
| `lang.toml` | Runtime language table, generated from bundled language JSON files when needed. |

### Important Notes

- The app is intended for Windows.
- `Newbee Unlock` terminates processes when releasing locks. Use it only for known non-system processes.
- `Newbee System Environment` writes to `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` and usually requires administrator privileges.
- In the Sys Env module, the preview table is treated as the final system environment snapshot. Load the current system variables first before editing; variables missing from the preview are deleted on apply.

### License

Apache License 2.0. See [LICENSE](LICENSE).
