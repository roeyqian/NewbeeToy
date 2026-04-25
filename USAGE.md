# Newbee Toy Usage Guide

## 中文版

本文档按“首页通用 -> 各功能模块 -> 常见问题”组织，覆盖当前项目已实现能力。

### 1. 启动与通用操作

#### 1.1 启动程序

```powershell
cargo run --release
```

#### 1.2 首页导航

- 分类入口：`General`、`Media`、`System`
- 模块入口：`Newbee Rename`、`Newbee Icon`、`Newbee Unlock`、`Newbee Env`
- 通用按钮：语言切换、锁定窗口、清空日志、帮助

#### 1.3 配置持久化

程序退出时会保存状态到可执行文件目录附近：

- `config/base.toml`：窗口大小/位置、语言、最近路径
- `config/env.toml`：环境变量模块默认预设（可在界面预设路径中改为其他文件）
- `lang.toml`：语言覆盖文件（可选）

---

### 2. Newbee Rename（批量重命名）

#### 2.1 功能说明

- 对选定目录内条目进行重命名（文件 + 文件夹，非递归）
- 支持普通替换 / 正则替换 / 计数语法
- 支持预览、删除预览行、执行、撤销最近一次成功执行

#### 2.2 基本流程

1. 选择目标目录。
2. 输入 `Find`（查找）与 `Replace`（替换为）。
3. 视需要勾选：`Case Sensitive`、`Use Regex`、`Count Syntax`。
4. 点击 `Preview` 生成预览。
5. 检查预览与日志；必要时移除不想处理的行。
6. 点击 `Apply` 执行。
7. 需要回退时点击 `Undo`（仅最近一次成功执行）。

#### 2.3 正则模式

- 启用 `Use Regex` 后，`Find` 按正则表达式解析。
- 关闭 `Case Sensitive` 时按忽略大小写匹配。
- 正则非法会在预览阶段报错，并阻止执行。

示例：

- `img_001.png` -> `photo_001.png`
  - Find: `^img_(\d+)\.png$`
  - Replace: `photo_$1.png`
- `test` -> `demo`（忽略大小写）
  - Find: `test`
  - Replace: `demo`
  - 取消勾选 `Case Sensitive`

#### 2.4 计数语法

启用 `Count Syntax` 后，`Replace` 支持：

- `<IncNr[:start[:step[:pad]]]>`

参数：

- `start`：起始值，默认 `1`
- `step`：步长，默认 `1`（可为负）
- `pad`：补零宽度，默认 `0`

示例：

- `<IncNr:01>` -> `01, 02, 03, ...`
- `<IncNr:10:-1:2>` -> `10, 09, 08, ...`
- `File_<IncNr:1:1:3>` -> `File_001, File_002, ...`

规则：

- 仅命中查找条件的条目会推进计数。

#### 2.5 重要限制

- 目标文件名不能包含 Windows 非法字符 `\\ / : * ? " < > |`。
- 名称冲突、非法名、预览错误都会阻止执行。
- 日志最多保留 100 行。

---

### 3. Newbee Icon（图标提取）

#### 3.1 支持输入

- 单文件：`exe` / `dll` / `icl` / `lnk` / `ico`
- 文件夹：扫描当前目录内文件（不递归）

#### 3.2 基本流程

1. 选择输入文件或输入目录。
2. 点击 `Scan` 扫描候选项。
3. 在预览中移除不想导出的行（可选）。
4. 选择输出目录。
5. 点击 `Extract` 批量导出。

#### 3.3 输出规则

- 输入是 `ico`：直接复制。
- 输入是 `exe/dll/icl/lnk`：提取关联图标写为 `.ico`。
- 输出重名时自动加后缀（如 `_2`, `_3`）避免覆盖。

#### 3.4 常见问题定位

- 扫描后为空：路径不存在或目录中无可处理后缀。
- 部分条目失败：预览状态会标记不可提取，最终日志会给出失败统计。

---

### 4. Newbee Unlock（文件解锁）

#### 4.1 功能说明

- 扫描“哪些进程占用了目标文件”。
- 尝试结束占用进程以释放文件。

#### 4.2 基本流程

1. 选择目标文件（仅文件，不支持目录）。
2. 点击 `Scan` 获取占用列表。
3. 可在预览中移除不希望处理的进程行。
4. 点击 `Release` 执行释放。

#### 4.3 安全限制

- `C:\Windows` 路径下文件会被阻止释放。
- 检测到系统进程占用时会告警，并阻止释放。
- 释放动作本质是终止进程，请谨慎使用。

---

### 5. Newbee Env（系统环境变量）

#### 5.1 功能说明

- 读取系统环境变量并在预览表展示。
- 可新增/修改变量，也可移除预览行标记删除。
- 支持预设路径输入，并按该路径保存/加载预设（默认 `config/env.toml`）。
- 应用系统变更使用“双确认”机制。

#### 5.2 基本流程

1. 进入模块后会自动加载系统环境变量到预览区。
2. 在第一行 `Preset Path` 设置预设文件路径（可选，默认 `config/env.toml`）。
3. 在第二行 `Value Path` 与第三行 `Variable Name` 输入待设置项。
4. 需要重新读取系统变量时，点击 `Variable Name` 右侧 `Load System`。
5. 点击 `Set Variable` 将键值加入预览。
6. 可点击行内 `Remove` 标记删除项。
7. 点击 `Apply` 第一次：显示将新增/修改/删除数量（确认提示）。
8. 再次点击 `Apply`：真正写入系统环境变量。

#### 5.3 预设文件操作

- `Store`：按当前 `Preset Path` 保存 `Value Path`、`Variable Name` 与预览变量，并回填预设路径框
- `Load Preset`：按当前 `Preset Path` 读取并恢复；若失败，日志会带上对应路径
- `Load System`：重新读取系统环境变量（位于 `Variable Name` 右侧）

#### 5.4 权限说明

- 模块会写入系统注册表环境变量项（`HKLM`），通常需要管理员权限。
- 若权限不足，日志中会显示失败项。

---

### 6. 常见问题

#### Q1: 为什么点击执行后没有动作？
- 先检查预览区是否为空、是否有错误、或是否全部被移除。
- Rename/Unlock/Icon 都要求先扫描或预览再执行。

#### Q2: 为什么日志里只有最近内容？
- 每个模块日志默认最多保留 100 行，旧日志会滚动丢弃。

#### Q3: Unlock 为什么提示系统进程并阻止释放？
- 这是内置安全策略，防止误杀关键进程导致系统不稳定。

#### Q4: Env 点击一次 Apply 没有立即写入？
- 这是双确认机制：第一次仅展示变更统计，第二次才提交。

---

### 7. 使用建议

- 先在测试目录验证 Rename 与 Icon 规则，再处理正式数据。
- Unlock 仅对你明确来源的普通应用文件执行。
- Env 变更前建议先 `Store` 一份预设，便于回看与恢复。

---

## English Version

This guide is organized as "Home/Common -> Feature Modules -> FAQ" and covers currently implemented functionality.

### 1. Launch and Common Operations

#### 1.1 Start the app

```powershell
cargo run --release
```

#### 1.2 Home navigation

- Category entry: `General`, `Media`, `System`
- Tool entry: `Newbee Rename`, `Newbee Icon`, `Newbee Unlock`, `Newbee Env`
- Common controls: language switch, window lock, clear logs, help

#### 1.3 Config persistence

State is saved on app exit near the executable:

- `config/base.toml`: window size/position, language, recent paths
- `config/env.toml`: default env-module preset (can be changed via the preset-path field)
- `lang.toml`: optional language override

---

### 2. Newbee Rename (Batch Rename)

#### 2.1 What it does

- Renames entries in the selected folder (files + directories, non-recursive)
- Supports plain replacement / regex replacement / counter syntax
- Supports preview, row removal, apply, and undo of the latest successful run

#### 2.2 Basic flow

1. Select a folder.
2. Fill `Find` and `Replace`.
3. Optionally enable `Case Sensitive`, `Use Regex`, `Count Syntax`.
4. Click `Preview`.
5. Review preview and logs; remove rows if needed.
6. Click `Apply`.
7. Click `Undo` to revert the last successful apply only.

#### 2.3 Regex mode

- With `Use Regex`, `Find` is interpreted as regex.
- If `Case Sensitive` is off, matching is case-insensitive.
- Invalid regex fails at preview and blocks apply.

Examples:

- `img_001.png` -> `photo_001.png`
  - Find: `^img_(\d+)\.png$`
  - Replace: `photo_$1.png`
- `test` -> `demo` (case-insensitive)
  - Find: `test`
  - Replace: `demo`
  - Turn off `Case Sensitive`

#### 2.4 Counter syntax

With `Count Syntax`, `Replace` supports:

- `<IncNr[:start[:step[:pad]]]>`

Parameters:

- `start`: start value, default `1`
- `step`: increment step, default `1` (can be negative)
- `pad`: zero-pad width, default `0`

Examples:

- `<IncNr:01>` -> `01, 02, 03, ...`
- `<IncNr:10:-1:2>` -> `10, 09, 08, ...`
- `File_<IncNr:1:1:3>` -> `File_001, File_002, ...`

Rule:

- Counter increments only for entries that match `Find`.

#### 2.5 Important limits

- Target names cannot contain invalid Windows characters `\\ / : * ? " < > |`.
- Name collisions, invalid names, or preview errors block apply.
- Logs keep up to 100 lines.

---

### 3. Newbee Icon (Icon Extraction)

#### 3.1 Supported inputs

- Single file: `exe` / `dll` / `icl` / `lnk` / `ico`
- Folder: scans files in current folder (non-recursive)

#### 3.2 Basic flow

1. Pick an input file or folder.
2. Click `Scan`.
3. Optionally remove rows from preview.
4. Choose an output folder.
5. Click `Extract` for batch export.

#### 3.3 Output rules

- `ico` input is copied directly.
- `exe/dll/icl/lnk` input is extracted into `.ico`.
- Duplicate output names get automatic suffixes (for example `_2`, `_3`).

#### 3.4 Troubleshooting hints

- Empty scan result: invalid path or no supported extensions in folder.
- Partial failures: preview marks unextractable rows and logs show failure count.

---

### 4. Newbee Unlock (File Unlock)

#### 4.1 What it does

- Scans which processes lock a target file.
- Attempts to terminate locking processes to release the file.

#### 4.2 Basic flow

1. Select a target file (file only, no directory).
2. Click `Scan` to list lockers.
3. Optionally remove rows you do not want to handle.
4. Click `Release`.

#### 4.3 Safety guards

- Files under `C:\Windows` are blocked from release.
- If system processes are detected, release is blocked.
- Release is process termination under the hood; use carefully.

---

### 5. Newbee Env (System Environment Variables)

#### 5.1 What it does

- Loads system environment variables into preview.
- Supports add/update variables and deletion via row removal.
- Supports a preset-path field and save/load by that path (default `config/env.toml`).
- Uses a two-step confirmation before applying system changes.

#### 5.2 Basic flow

1. On entering the module, system variables are loaded into preview.
2. Set `Preset Path` on the first row (optional, default `config/env.toml`).
3. Fill `Value Path` on the second row and `Variable Name` on the third row.
4. To reload from system variables, click `Load System` on the right of `Variable Name`.
5. Click `Set Variable` to stage the key/value.
6. Use row `Remove` to mark deletion.
7. First `Apply`: shows add/change/delete counts.
8. Second `Apply`: commits changes to system environment variables.

#### 5.3 Preset file operations

- `Store`: save current `Value Path`, `Variable Name`, and preview variables to current `Preset Path`, then update the preset-path field
- `Load Preset`: load from current `Preset Path`; if it fails, logs include the related path
- `Load System`: reload from system environment variables (button is on the right of `Variable Name`)

#### 5.4 Permission note

- This module writes system env values in registry (`HKLM`) and usually needs administrator rights.
- If privileges are insufficient, failed items are logged.

---

### 6. FAQ

#### Q1: Why does apply do nothing?
- Check whether preview is empty, has errors, or all rows were removed.
- Rename/Unlock/Icon need scan or preview before apply.

#### Q2: Why are only recent logs visible?
- Each module keeps up to 100 log lines; older lines are trimmed.

#### Q3: Why does Unlock block release for system processes?
- This is a built-in safety policy to avoid terminating critical processes.

#### Q4: Why does Env not write on first Apply click?
- Two-step confirmation is required; first click shows change counts, second click commits.

---

### 7. Recommended Practice

- Validate Rename/Icon rules in a test folder first.
- Use Unlock only for files from known, non-critical applications.
- Store an Env preset before applying changes for safer rollback planning.
