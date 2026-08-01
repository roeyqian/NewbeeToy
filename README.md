# Newbee Toy

[![License](https://img.shields.io/badge/License-Apache_2.0-orange.svg)](LICENSE)
[![Stars](https://img.shields.io/github/stars/roeyqian/NewbeeToy?style=flat&color=6366f1)](https://github.com/roeyqian/NewbeeToy/stargazers)

| [简体中文](#简体中文)       | [English](#English) | [日本語](#日本語) | [Español](#Español) |
|---------------------|---------------------|-------------|---------------------|

---

# 简体中文

| <span style="font-size: 16px;">[项目简介](#项目简介)</span> | <span style="font-size: 16px;">[已实现功能](#已实现功能)</span> | <span style="font-size: 16px;">[运行与构建](#运行与构建)</span> | <span style="font-size: 16px;">[运行时文件](#运行时文件)</span> | <span style="font-size: 16px;">[使用说明](#使用说明)</span> | <span style="font-size: 16px;">[许可证](#许可证)</span> |
|:---------------------------------------------------:|:-----------------------------------------------------:|:-----------------------------------------------------:|:-----------------------------------------------------:|:---------------------------------------------------:|:-------------------------------------------------:|

---

## 项目简介

`Newbee Toy` 是一个面向 Windows 桌面的轻量工具箱，使用 Rust + Slint 构建。当前项目不是通用跨平台工具，核心功能依赖 Windows API，主要用于 Windows 下的文件处理与系统辅助操作。

## 已实现功能

| 模块                        | 说明                                                                              |
|---------------------------|---------------------------------------------------------------------------------|
| Newbee Rename             | 批量重命名目录内文件和文件夹，支持普通替换、正则替换、大小写控制、计数语法、预览、移除预览行和撤销上次成功重命名。                       |
| Newbee Icon               | 从 `.exe`、`.dll`、`.icl`、`.ico` 扫描可提取项并导出 `.ico`；支持单文件或目录扫描，输出重名时自动追加序号。          |
| Newbee Unlock             | 使用 Windows Restart Manager 检测文件占用进程；也支持目录扫描，会递归检查最多 256 个文件并合并占用进程。可尝试结束普通占用进程。 |
| Newbee System Environment | 读取、编辑、保存、加载并应用 Windows 系统环境变量快照；支持独立变量值编辑窗口和二次确认应用。                             |

## 运行与构建

### 环境要求

- Windows
- Rust toolchain，包含 `cargo`

### 开发运行

```powershell
cargo run --release
```

### 构建发布版

```powershell
cargo build --release
```

构建产物通常位于：

```text
target/release/NewbeeToy.exe
```

## 运行时文件

程序会以可执行文件所在目录作为应用目录，并使用以下文件：

| 路径                   | 用途                                                             |
|----------------------|----------------------------------------------------------------|
| `config/base.toml`   | 文本 TOML 主配置，保存窗口状态、语言、最近路径等。                                     |
| `config/general.dat` | 二进制 DAT 配置，保存文件夹样式模块的分组路径列表。                                    |
| `config/system.dat`  | 二进制 DAT 配置，保存系统环境变量模块的预设快照。                                     |
| `assets/lang/*.toml` | 运行时语言表。程序只读取对应 TOML，不会自动生成或写入翻译文件。                             |
| `assets/fonts/*.otf` | 运行时字体资源。`icon.ico` 和 `icon.png` 会直接封装进 exe。                         |

`general.dat` 和 `system.dat` 由构建脚本生成二进制文件；如果构建目录中已有旧 TOML 文本 `.dat`，构建脚本会在可解析时迁移为二进制。运行时也会以同一二进制格式读写；旧版本生成的 TOML 文本 `.dat` 仍可被读取，之后保存会转换为二进制 DAT。

如果将 `NewbeeToy.exe` 单独复制到其他位置运行，建议同时准备可写目录，并带上 `assets` 资源目录。

## 使用说明

### Newbee Rename

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

### Newbee Icon

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

### Newbee Unlock

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

### Newbee System Environment

这个模块编辑系统或用户环境变量。系统变量对应注册表：

```text
HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
```

用户变量对应 `HKCU\Environment`。

通常需要管理员权限。

推荐流程：

1. 进入模块后先点击 `加载系统` 或 `加载用户`，把目标范围变量加载到预览表。
2. 如需新增变量，填写 `变量值路径` 和 `变量名`，点击 `新增变量`。
3. 如需编辑已有变量，点击表格行内的 `编辑`，在弹出的值编辑窗口中调整条目。
4. 如需删除变量，点击表格行内的 `移除`。
5. 可点击 `存储预设` 保存当前预览快照，也可用 `加载预设` 将预设变量合并到当前预览。
6. 点击 `应用系统` 或 `应用用户` 第一次只显示新增、更新、删除数量。
7. 再次点击同一个按钮才会真正写入对应范围的环境变量。

重要说明：

- 预览表会被视为所选应用范围的最终变量快照。
- 应用时，目标范围中存在但预览表中不存在的变量会被删除。
- 因此修改环境变量前，强烈建议先加载对应范围，再进行增删改。
- 变量值中包含 `%NAME%` 形式引用时会写为 `REG_EXPAND_SZ`，否则通常写为 `REG_SZ`。
- 写入后程序会广播 `WM_SETTINGCHANGE`，但部分已运行程序仍可能需要重启才能读取新环境变量。

### 注意事项

- 批量重命名、终止进程、修改系统环境变量都可能造成不可逆影响。
- 对正式数据操作前，建议先在测试目录验证规则。
- Sys Env 模块尤其需要谨慎：如果没有先加载系统变量，直接应用少量预览变量，可能导致未出现在预览中的系统变量被删除。

## 许可证

本项目采用 Apache License 2.0，详见 [LICENSE](LICENSE)。

---

# English

| <span style="font-size: 16px;">[Introduction](#introduction)</span>  | <span style="font-size: 16px;">[Features](#features)</span> | <span style="font-size: 16px;">[Build & Run](#build--run)</span> | <span style="font-size: 16px;">[Runtime Files](#runtime-files)</span> | <span style="font-size: 16px;">[Usage](#usage)</span> | <span style="font-size: 16px;">[License](#license)</span> |
|:--------------------------------------------------------------------:|:-----------------------------------------------------------:|:----------------------------------------------------------------:|:---------------------------------------------------------------------:|:-----------------------------------------------------:|:---------------------------------------------------------:|

---

## Introduction

`Newbee Toy` is a lightweight desktop toolbox for Windows, built with Rust + Slint. It is not a general cross-platform tool — the core features rely on Windows APIs, focusing on file processing and system utility operations.

## Features

| Module                      | Description                                                                                                                                                                                                            |
|-----------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Newbee Rename               | Batch rename files and folders in a directory. Supports plain replace, regex replace, case control, counter syntax, preview, removing preview rows, and undo last rename.                                              |
| Newbee Icon                 | Scan `.exe`, `.dll`, `.icl`, `.ico` for extractable icon resources and export as `.ico`. Supports single file or directory scanning; auto-appends sequence numbers on name conflicts.                                  |
| Newbee Unlock               | Detect processes locking a file via Windows Restart Manager. Also supports directory scanning, recursively checking up to 256 files and merging locking processes. Can attempt to terminate regular locking processes. |
| Newbee System Environment   | Read, edit, save, load, and apply Windows system environment variable snapshots. Supports a dedicated variable-value editor and two-step confirmation before applying.                                                 |

## Build & Run

### Requirements

- Windows
- Rust toolchain with `cargo`

### Development Run

```powershell
cargo run --release
```

### Build Release

```powershell
cargo build --release
```

The output is typically located at:

```text
target/release/NewbeeToy.exe
```

## Runtime Files

The program treats the executable's directory as the application directory and uses the following files:

| Path                   | Purpose                                                                                                                            |
|------------------------|------------------------------------------------------------------------------------------------------------------------------------|
| `config/base.toml`     | Text TOML main configuration — stores window state, language, recent paths, etc.                                                    |
| `config/general.dat`   | Binary DAT configuration — stores folder-style group path lists.                                                                    |
| `config/system.dat`    | Binary DAT configuration — stores system environment variable preset snapshots.                                                     |
| `assets/lang/*.toml`   | Runtime language tables. The app only reads the matching TOML files and does not generate or write translation files.               |
| `assets/fonts/*.otf`   | Runtime font resources. `icon.ico` and `icon.png` are bundled directly into the exe.                                                |

`general.dat` and `system.dat` are generated by the build script as binary files; if legacy TOML text `.dat` files already exist in the build directory, the build script migrates them when they can be parsed. Runtime reads and writes use the same binary format, and older TOML text `.dat` files can still be read and converted after saving.

If you copy `NewbeeToy.exe` to another location and run it standalone, ensure a writable directory is available and include the `assets` resource directory.

## Usage

### Newbee Rename

Basic workflow:

1. Select the target directory.
2. Enter `Find` and `Replace with`.
3. Optionally enable `Case Sensitive`, `Enable Regex`, `Enable Counter Syntax`.
4. Click `Generate Preview`.
5. Review the preview table and log; remove rows you do not want to process.
6. Click `Execute Rename`.
7. Click `Undo Last Rename` if you need to revert.

Limitations:

- Only direct children of the selected directory are processed — no recursion.
- Both files and folders appear in the preview.
- Empty find text produces no rename action.
- The target name must not contain Windows illegal characters `\ / : * ? " < > |`, and must not end with a space or dot.
- Errors or name conflicts in the preview will block execution.
- Undo only records the last successful rename plan.

Counter syntax:

```text
<IncNr[:start[:step[:pad]]]>
```

Examples:

```text
<IncNr:01>           -> 01, 02, 03, ...
<IncNr:10:-1:2>      -> 10, 09, 08, ...
File_<IncNr:1:1:3>   -> File_001, File_002, ...
```

### Newbee Icon

Basic workflow:

1. Select an input file or input directory.
2. Click `Scan Extractable Items`.
3. Review the preview; remove rows you don't want to export.
4. Select an output directory.
5. Click `Start Extraction`.

Implementation details:

- Supports `.exe`, `.dll`, `.icl`, `.ico`.
- When the input is a directory, only direct files are scanned — no recursion.
- `.ico` input files are copied directly.
- For `.exe`, `.dll`, `.icl` files, the first available icon group is read and written as `.ico`.
- On output filename conflicts, suffixes like `_2`, `_3` are automatically generated to avoid overwriting.

### Newbee Unlock

Basic workflow:

1. Select the target file or directory.
2. Click `Detect Locks`.
3. Review the locking process list; remove rows you don't want to handle.
4. Click `Remove Locks` — the program will attempt to terminate the remaining regular processes.

Safety boundaries:

- The target can be a file or directory.
- Directory scanning recursively collects up to 256 files and merges the detected locking processes.
- Targets under Windows system directories are blocked from being released.
- If a system process is detected holding a lock, forced removal is blocked.
- "Remove Locks" essentially terminates processes — use it only on known application processes from identifiable sources.

### Newbee System Environment

This module edits system or user environment variables. System variables correspond to:

```text
HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
```

User variables correspond to `HKCU\Environment`.

Administrator privileges are usually required.

Recommended workflow:

1. After entering the module, click `Load System` or `Load User` first to load the target variables into the preview table.
2. To add a new variable, fill in `Variable Value Path` and `Variable Name`, then click `Add Variable`.
3. To edit an existing variable, click `Edit` in the table row and adjust entries in the pop-up value editor.
4. To delete a variable, click `Remove` in the table row.
5. You can click `Save Preset` to save the current preview snapshot, or use `Load Preset` to merge preset variables into the current preview.
6. The first click of `Apply System` or `Apply User` only shows the counts of additions, updates, and deletions.
7. Click the same button again to write to the corresponding environment variables.

Important notes:

- The preview table is treated as the final snapshot for the selected target.
- When applying, variables that exist in the target but not in the preview table will be deleted.
- Therefore, before modifying environment variables, load the corresponding target first, then make additions, edits, and deletions.
- Variable values containing `%NAME%`-style references are written as `REG_EXPAND_SZ`; otherwise they are typically written as `REG_SZ`.
- After writing, the program broadcasts `WM_SETTINGCHANGE`, but some running applications may still need to be restarted to pick up the new environment variables.

### Cautions

- Batch renaming, terminating processes, and modifying system environment variables can all cause irreversible effects.
- Before operating on production data, test your rules in a test directory first.
- The Sys Env module requires extra care: if you apply a small set of preview variables without loading the system variables first, system variables not present in the preview may be deleted.

## License

This project is licensed under the Apache License 2.0. See [LICENSE](LICENSE).

---

# 日本語

|   <span style="font-size: 16px;">[概要](#概要)</span>    |   <span style="font-size: 16px;">[機能](#機能)</span>    |     <span style="font-size: 16px;">[ビルドと実行](#ビルドと実行)</span>      |      <span style="font-size: 16px;">[実行時ファイル](#実行時ファイル)</span>       |  <span style="font-size: 16px;">[使い方](#使い方)</span>   |     <span style="font-size: 16px;">[ライセンス](#ライセンス)</span>      |
|:----------------------------------------------------:|:----------------------------------------------------:|:----------------------------------------------------------------:|:--------------------------------------------------------------------:|:----------------------------------------------------:|:--------------------------------------------------------------:|

---

## 概要

`Newbee Toy` は、Rust + Slint で構築された Windows 向けの軽量デスクトップツールボックスです。汎用クロスプラットフォームツールではなく、コア機能は Windows API に依存しており、主にファイル処理とシステム補助操作に使用されます。

## 機能

| モジュール                         | 説明                                                                                                                                             |
|-------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------|
| Newbee Rename                 | ディレクトリ内のファイルとフォルダを一括リネーム。通常置換、正規表現置換、大文字小文字制御、カウンタ構文、プレビュー、プレビュー行の削除、および最後の成功リネームの取り消しをサポート。                                                   |
| Newbee Icon                   | `.exe`、`.dll`、`.icl`、`.ico` から抽出可能なアイコンリソースをスキャンし `.ico` としてエクスポート。単一ファイルまたはディレクトリスキャンに対応し、名前の重複時は自動で連番を追加。                                    |
| Newbee Unlock                 | Windows Restart Manager を使用してファイルをロックしているプロセスを検出。ディレクトリスキャンにも対応し、最大256ファイルを再帰的にチェックしてロックプロセスを統合。通常のロックプロセスの終了を試行可能。                            |
| Newbee System Environment     | Windows システム環境変数のスナップショットを読み取り、編集、保存、読み込み、適用。専用の変数値エディタと適用前の2段階確認をサポート。                                                                        |

## ビルドと実行

### 要件

- Windows
- `cargo` を含む Rust ツールチェーン

### 開発実行

```powershell
cargo run --release
```

### リリースビルド

```powershell
cargo build --release
```

ビルド成果物は通常以下に配置されます：

```text
target/release/NewbeeToy.exe
```

## 実行時ファイル

プログラムは実行可能ファイルの場所をアプリケーションディレクトリとして扱い、以下のファイルを使用します：

| パス                     | 用途                                                                                        |
|------------------------|-------------------------------------------------------------------------------------------|
| `config/base.toml`     | テキスト TOML のメイン設定 — ウィンドウ状態、言語、最近のパスなどを保存。                                                |
| `config/general.dat`   | バイナリ DAT 設定 — フォルダスタイルモジュールのグループ別パス一覧を保存。                                                |
| `config/system.dat`    | バイナリ DAT 設定 — システム環境変数モジュールのプリセットスナップショットを保存。                                             |
| `assets/lang/*.toml`   | 実行時言語テーブル。アプリは対応する TOML を読み取るだけで、翻訳ファイルの生成や書き込みは行いません。                                      |
| `assets/fonts/*.otf`   | 実行時フォントリソース。`icon.ico` と `icon.png` は exe に直接バンドルされます。                                         |

`general.dat` と `system.dat` はビルドスクリプトによってバイナリファイルとして生成されます。ビルドディレクトリに旧 TOML テキスト `.dat` が既にある場合、解析可能であればビルドスクリプトがバイナリへ移行します。実行時も同じバイナリ形式で読み書きされ、旧バージョンの TOML テキスト `.dat` は保存後にバイナリ DAT へ変換されます。

`NewbeeToy.exe` をスタンドアロンで別の場所にコピーして実行する場合は、書き込み可能なディレクトリを用意し、`assets` リソースディレクトリを同梱してください。

## 使い方

### Newbee Rename

基本ワークフロー：

1. 対象ディレクトリを選択。
2. `検索` と `置換` を入力。
3. 必要に応じて `大文字小文字を区別`、`正規表現を有効化`、`カウンタ構文を有効化` を有効に。
4. `プレビューを生成` をクリック。
5. プレビューテーブルとログを確認し、処理が不要な行を削除。
6. `リネームを実行` をクリック。
7. 元に戻す場合は `最後のリネームを取り消し` をクリック。

制限事項：

- 選択したディレクトリの直接の子のみ処理 — 再帰はしません。
- ファイルとフォルダの両方がプレビューに表示されます。
- 空の検索テキストはリネーム動作を生成しません。
- ターゲット名に Windows の禁止文字 `\ / : * ? " < > |` を含めることはできず、スペースまたはドットで終わることもできません。
- プレビューにエラーや名前の重複がある場合、実行はブロックされます。
- 取り消しは最後の成功したリネーム計画のみを記録します。

カウンタ構文：

```text
<IncNr[:start[:step[:pad]]]>
```

例：

```text
<IncNr:01>           -> 01, 02, 03, ...
<IncNr:10:-1:2>      -> 10, 09, 08, ...
File_<IncNr:1:1:3>   -> File_001, File_002, ...
```

### Newbee Icon

基本ワークフロー：

1. 入力ファイルまたは入力ディレクトリを選択。
2. `抽出可能項目をスキャン` をクリック。
3. プレビューを確認し、エクスポートが不要な行を削除。
4. 出力ディレクトリを選択。
5. `抽出を開始` をクリック。

実装の詳細：

- `.exe`、`.dll`、`.icl`、`.ico` に対応。
- 入力がディレクトリの場合、そのディレクトリ直下のファイルのみスキャン — 再帰はしません。
- `.ico` 入力はそのままコピーされます。
- `.exe`、`.dll`、`.icl` は最初の利用可能なアイコングループを読み取り `.ico` として出力します。
- 出力ファイル名が重複する場合、上書きを避けるために `_2`、`_3` などのサフィックスが自動生成されます。

### Newbee Unlock

基本ワークフロー：

1. 対象ファイルまたはディレクトリを選択。
2. `ロックを検出` をクリック。
3. ロックプロセスリストを確認し、処理が不要な行を削除。
4. `ロックを解除` をクリック — 残りの通常プロセスの終了を試行します。

安全境界：

- 対象はファイルまたはディレクトリです。
- ディレクトリスキャンは最大256ファイルを再帰的に収集し、検出されたロックプロセスを統合します。
- Windows システムディレクトリ配下の対象は解放がブロックされます。
- システムプロセスによるロックが検出された場合、強制解除はブロックされます。
- 「ロック解除」の本質はプロセスの終了です — 出所が明確な一般アプリケーションプロセスにのみ使用してください。

### Newbee System Environment

このモジュールはシステム環境変数を編集します（対応するレジストリキー）：

```text
HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
```

通常、管理者権限が必要です。

推奨ワークフロー：

1. モジュールに入ったら、まず `システムを読み込み` をクリックして現在のシステム変数をプレビューテーブルに読み込みます。
2. 新しい変数を追加するには、`変数値パス` と `変数名` を入力し、`変数を追加` をクリックします。
3. 既存の変数を編集するには、テーブル行の `編集` をクリックし、ポップアップの値エディタでエントリを調整します。
4. 変数を削除するには、テーブル行の `削除` をクリックします。
5. `プリセットを保存` をクリックして現在のプレビュースナップショットを保存したり、`プリセットを読み込み` でプリセット変数を現在のプレビューにマージできます。
6. 最初の `適用` クリックでは追加・更新・削除の数のみが表示されます。
7. 再度 `適用` をクリックすると、実際にシステム環境変数に書き込まれます。

重要事項：

- プレビューテーブルは最終的なシステム変数スナップショットとして扱われます。
- 適用時、システムに存在するがプレビューテーブルに存在しない変数は削除されます。
- そのため、システム環境変数を変更する前に、まず `システムを読み込み` をクリックし、その後に追加・編集・削除を行うことを強く推奨します。
- `%NAME%` 形式の参照を含む変数値は `REG_EXPAND_SZ` として書き込まれ、それ以外は通常 `REG_SZ` として書き込まれます。
- 書き込み後、プログラムは `WM_SETTINGCHANGE` をブロードキャストしますが、一部の実行中アプリケーションは新しい環境変数を読み取るために再起動が必要な場合があります。

### 注意事項

- 一括リネーム、プロセスの終了、システム環境変数の変更は、いずれも不可逆的な影響を及ぼす可能性があります。
- 本番データに対して操作する前に、テストディレクトリでルールを検証してください。
- Sys Env モジュールは特に注意が必要です：システム変数を先に読み込まずに少量のプレビュー変数を適用すると、プレビューに表示されていないシステム変数が削除される可能性があります。

## ライセンス

本プロジェクトは Apache License 2.0 の下でライセンスされています。詳細は [LICENSE](LICENSE) をご覧ください。

---

# Español

| <span style="font-size: 16px;">[Introducción](#introducción)</span>  | <span style="font-size: 16px;">[Funciones](#funciones)</span> |  <span style="font-size: 16px;">[Compilación y ejecución](#compilación-y-ejecución)</span>  |   <span style="font-size: 16px;">[Archivos en tiempo de ejecución](#archivos-en-tiempo-de-ejecución)</span>   | <span style="font-size: 16px;">[Guía de uso](#guía-de-uso)</span> | <span style="font-size: 16px;">[Licencia](#licencia)</span>  |
|:--------------------------------------------------------------------:|:-------------------------------------------------------------:|:-------------------------------------------------------------------------------------------:|:-------------------------------------------------------------------------------------------------------------:|:-----------------------------------------------------------------:|:------------------------------------------------------------:|

---

## Introducción

`Newbee Toy` es una caja de herramientas ligera para el escritorio de Windows, construida con Rust + Slint. No es una herramienta multiplataforma de uso general — las funciones principales dependen de las API de Windows, enfocándose en el procesamiento de archivos y operaciones auxiliares del sistema.

## Funciones

| Módulo                      | Descripción                                                                                                                                                                                                                                                                     |
|-----------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Newbee Rename               | Renombrado por lotes de archivos y carpetas en un directorio. Admite reemplazo simple, reemplazo con expresiones regulares, control de mayúsculas/minúsculas, sintaxis de contador, vista previa, eliminación de filas de vista previa y deshacer el último renombrado exitoso. |
| Newbee Icon                 | Escanea `.exe`, `.dll`, `.icl`, `.ico` en busca de recursos de iconos extraíbles y exporta como `.ico`. Admite escaneo de archivo único o directorio; agrega automáticamente números secuenciales en conflictos de nombre.                                                      |
| Newbee Unlock               | Detecta procesos que bloquean un archivo mediante Windows Restart Manager. También admite escaneo de directorios, verificando recursivamente hasta 256 archivos y fusionando los procesos de bloqueo. Puede intentar terminar procesos de bloqueo regulares.                    |
| Newbee System Environment   | Lee, edita, guarda, carga y aplica instantáneas de variables de entorno del sistema de Windows. Admite un editor dedicado de valores de variables y confirmación en dos pasos antes de aplicar.                                                                                 |

## Compilación y ejecución

### Requisitos

- Windows
- Toolchain de Rust con `cargo`

### Ejecución en desarrollo

```powershell
cargo run --release
```

### Compilación para publicación

```powershell
cargo build --release
```

El ejecutable se encuentra normalmente en:

```text
target/release/NewbeeToy.exe
```

## Archivos en tiempo de ejecución

El programa trata el directorio del ejecutable como el directorio de la aplicación y utiliza los siguientes archivos:

| Ruta                   | Propósito                                                                                                                                                              |
|------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `config/base.toml`     | Configuración principal en texto TOML — almacena el estado de la ventana, idioma, rutas recientes, etc.                                                                |
| `config/general.dat`   | Configuración DAT binaria — almacena las listas de rutas por grupo del módulo de estilo de carpetas.                                                                   |
| `config/system.dat`    | Configuración DAT binaria — almacena instantáneas preestablecidas de variables de entorno del sistema.                                                                 |
| `assets/lang/*.toml`   | Tablas de idioma en tiempo de ejecución. La app solo lee los TOML correspondientes y no genera ni escribe archivos de traducción.                                      |
| `assets/fonts/*.otf`   | Recursos de fuentes en tiempo de ejecución. `icon.ico` e `icon.png` se empaquetan directamente en el exe.                                                             |

`general.dat` y `system.dat` son generados por el script de compilación como archivos binarios; si ya existen `.dat` de texto TOML heredados en el directorio de compilación, el script los migra cuando puede analizarlos. La lectura y escritura en tiempo de ejecución usan el mismo formato binario, y los `.dat` TOML heredados aún pueden leerse y convertirse después de guardar.

Si copia `NewbeeToy.exe` a otra ubicación y lo ejecuta de forma independiente, asegúrese de que haya un directorio con permisos de escritura disponible e incluya el directorio de recursos `assets`.

## Guía de uso

### Newbee Rename

Flujo básico:

1. Seleccione el directorio de destino.
2. Ingrese `Buscar` y `Reemplazar con`.
3. Opcionalmente active `Distinguir mayúsculas/minúsculas`, `Habilitar regex`, `Habilitar sintaxis de contador`.
4. Haga clic en `Generar vista previa`.
5. Revise la tabla de vista previa y el registro; elimine las filas que no desee procesar.
6. Haga clic en `Ejecutar renombrado`.
7. Haga clic en `Deshacer último renombrado` si necesita revertir.

Limitaciones:

- Solo se procesan los elementos directamente dentro del directorio seleccionado — sin recursión.
- Tanto archivos como carpetas aparecen en la vista previa.
- Un texto de búsqueda vacío no produce ninguna acción de renombrado.
- El nombre de destino no debe contener los caracteres ilegales de Windows `\ / : * ? " < > |`, ni terminar con un espacio o punto.
- Los errores o conflictos de nombre en la vista previa bloquearán la ejecución.
- Deshacer solo registra el último plan de renombrado exitoso.

Sintaxis del contador:

```text
<IncNr[:inicio[:paso[:relleno]]]>
```

Ejemplos:

```text
<IncNr:01>           -> 01, 02, 03, ...
<IncNr:10:-1:2>      -> 10, 09, 08, ...
File_<IncNr:1:1:3>   -> File_001, File_002, ...
```

### Newbee Icon

Flujo básico:

1. Seleccione un archivo o directorio de entrada.
2. Haga clic en `Escanear elementos extraíbles`.
3. Revise la vista previa; elimine las filas que no desee exportar.
4. Seleccione un directorio de salida.
5. Haga clic en `Iniciar extracción`.

Detalles de implementación:

- Compatible con `.exe`, `.dll`, `.icl`, `.ico`.
- Cuando la entrada es un directorio, solo se escanean los archivos directos — sin recursión.
- Los archivos `.ico` de entrada se copian directamente.
- Para `.exe`, `.dll`, `.icl`, se lee el primer grupo de iconos disponible y se escribe como `.ico`.
- En conflictos de nombre de archivo de salida, se generan automáticamente sufijos como `_2`, `_3` para evitar sobrescrituras.

### Newbee Unlock

Flujo básico:

1. Seleccione el archivo o directorio de destino.
2. Haga clic en `Detectar bloqueos`.
3. Revise la lista de procesos bloqueantes; elimine las filas que no desee manejar.
4. Haga clic en `Eliminar bloqueos` — el programa intentará terminar los procesos regulares restantes.

Límites de seguridad:

- El objetivo puede ser un archivo o directorio.
- El escaneo de directorios recopila recursivamente hasta 256 archivos y fusiona los procesos bloqueantes detectados.
- Los objetivos bajo directorios del sistema de Windows están bloqueados para liberación.
- Si se detecta un proceso del sistema reteniendo un bloqueo, se bloquea la eliminación forzada.
- "Eliminar bloqueos" esencialmente termina procesos — utilícelo solo en procesos de aplicaciones conocidas de origen identificable.

### Newbee System Environment

Este módulo edita las variables de entorno del sistema, correspondientes a la clave de registro:

```text
HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
```

Normalmente se requieren privilegios de administrador.

Flujo recomendado:

1. Al entrar al módulo, haga clic primero en `Cargar sistema` para cargar las variables actuales del sistema en la tabla de vista previa.
2. Para agregar una nueva variable, complete `Ruta del valor` y `Nombre de variable`, luego haga clic en `Agregar variable`.
3. Para editar una variable existente, haga clic en `Editar` en la fila de la tabla y ajuste las entradas en el editor de valores emergente.
4. Para eliminar una variable, haga clic en `Eliminar` en la fila de la tabla.
5. Puede hacer clic en `Guardar preajuste` para guardar la instantánea actual de la vista previa, o usar `Cargar preajuste` para fusionar variables preajustadas en la vista previa actual.
6. El primer clic en `Aplicar` solo muestra los recuentos de adiciones, actualizaciones y eliminaciones.
7. Haga clic en `Aplicar` nuevamente para escribir realmente en las variables de entorno del sistema.

Notas importantes:

- La tabla de vista previa se trata como la instantánea final de variables del sistema.
- Al aplicar, las variables que existen en el sistema pero no en la tabla de vista previa serán eliminadas.
- Por lo tanto, antes de modificar las variables de entorno del sistema, se recomienda encarecidamente hacer clic primero en `Cargar sistema` y luego realizar adiciones, ediciones y eliminaciones.
- Los valores de variable que contienen referencias de estilo `%NOMBRE%` se escriben como `REG_EXPAND_SZ`; de lo contrario, normalmente se escriben como `REG_SZ`.
- Después de escribir, el programa transmite `WM_SETTINGCHANGE`, pero es posible que algunas aplicaciones en ejecución aún necesiten reiniciarse para leer las nuevas variables de entorno.

### Precauciones

- El renombrado por lotes, la terminación de procesos y la modificación de variables de entorno del sistema pueden causar efectos irreversibles.
- Antes de operar con datos de producción, pruebe sus reglas en un directorio de prueba.
- El módulo Sys Env requiere especial cuidado: si aplica un pequeño conjunto de variables de vista previa sin cargar primero las variables del sistema, las variables del sistema que no estén presentes en la vista previa podrían ser eliminadas.

## Licencia

Este proyecto está licenciado bajo la Apache License 2.0. Consulte [LICENSE](LICENSE).
