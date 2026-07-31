# Pomotui

[![English](https://img.shields.io/badge/English-README-blue?style=flat&logo=readthedocs&logoColor=white)](README.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

Pomotui 是一个终端番茄钟计时器，提供三种前端：

- 键盘优先的 Ratatui 仪表盘；
- 可脚本化的命令行工具；
- 轮询式 Waybar 模块。

一个持久化的计时服务拥有当前会话，关闭 TUI 或重启 Waybar 不会停止计时。任务、会话历史、每日统计、重启恢复、桌面提醒和完成音效都集中存储和协调。

![Pomotui TUI 仪表盘](https://tree-1327913400.cos.ap-nanjing.myqcloud.com/world/20260731122427701.webp)

## 系统要求

**Linux：**
- systemd 用户服务
- 支持 edition 2024 的 Rust 工具链
- 可选：Waybar、`notify-send`、`paplay`

**macOS：**
- 支持 edition 2024 的 Rust 工具链
- 可选：`afplay`（内置）、`osascript`（内置）、Waybar（通过 Homebrew）

## 安装

### Arch Linux（AUR）

```sh
paru -S pomotui
# 或安装最新 git 版：
paru -S pomotui-git
```

### macOS（Homebrew）

```sh
brew tap SaintFore/tap
brew install --HEAD pomotui
brew services start pomotui
pomotui-tray   # 可选：菜单栏计时器
```

### Linux（从源码构建）

为当前用户构建和安装，无需 `sudo`：

```sh
cd pomotui
cargo build --release --workspace
./packaging/install.sh
systemctl --user daemon-reload
systemctl --user enable --now pomotui.socket
```

重新构建、安装并重启：

```sh
./packaging/rebuild-restart.sh
```

这会保留现有配置、任务和会话历史。

默认安装将可执行文件放在 `~/.local/bin`。确保该目录在 `PATH` 中，然后验证服务：

```sh
pomotui status
systemctl --user status pomotui.socket
```

安装程序会保留现有用户配置。

安装后，桌面应用启动器可以找到 **Pomotui 番茄钟**。打开它会在桌面配置的终端中启动 TUI。

## 使用 TUI

```sh
pomotui-tui
```

仪表盘采用计时器优先布局，适配宽屏和窄屏终端。主要操作：

| 按键 | 操作 |
| --- | --- |
| `j`/`k`、`↑`/`↓` | 选择任务 |
| `h`/`l`、`←`/`→` | 切换仪表盘、链条、链条归档、今日、复盘和历史 |
| `Enter` | 用选中的任务开始专注 |
| `Space` | 开始、暂停或恢复当前会话 |
| `X` / `K` | 停止 / 跳过当前会话 |
| `S` / 待复盘时 `Enter` | 复盘为成功，使用现有或选中的任务（`Void` 需要输入链条条目标题） |
| `F` | 复盘为失败 |
| `p` | 重新打开复盘对话框 |
| `j` / `k`（链条页面） | 选择链条链接 |
| `E`（链条页面） | 编辑选中链条链接的复盘内容 |
| `T`（链条页面或归档） | 编辑选中链条条目的显示标题 |
| `E`（链条归档） | 编辑最近的链条断裂复盘 |
| `R`（链条页面） | 打开奖励里程碑管理器（`n` 创建、`e` 编辑、`D` 删除） |
| `C`（链条页面） | 领取第一个已解锁的奖励 |
| `n` / `r` | 创建 / 重命名任务 |
| `c` / `D` | 完成或重新打开 / 删除任务 |
| `:` | 打开命令面板 |
| `?` / `s` | 打开帮助 / 设置 |
| `Esc` | 关闭覆盖层 |
| `q` | 关闭 TUI（不停止计时服务） |

删除任务需要确认，且不会删除现有会话历史。单行编辑器预填现有值，支持方向键、Home/End、Delete 和 Emacs/Readline 快捷键子集：`C-a/e/b/f`、`M-b/f`、`C-h/d/w`、`M-d`、`C-k/u/y`。
在设置中按 `g` 可切换英文和简体中文；选择会保存到用户配置。

## 使用 CLI

```sh
pomotui task create "写发布说明"
pomotui task list
pomotui start focus --task 1
pomotui pause
pomotui resume
pomotui stop
pomotui review success --reflection "完成了垂直切片"
pomotui chain
```

其他命令包括 `start short-break`、`start long-break`、`skip`、`history`、`summary` 以及完整的任务生命周期：`create/rename/complete/reopen/delete`。与脚本集成时使用 `--json`。

使用 `stop --review` 将提前结束的专注会话送去复盘，或 `stop --no-review` 记录但不影响行动链条。失败的复盘需要复盘内容：

```sh
pomotui review failure "被打断，丢失了思路"
pomotui chain archive
pomotui reward create 10 "吃肯德基" --budget 50
pomotui reward list
pomotui reward claim 1
```

当复盘的会话没有任务时，用 `--task ID` 分配，或用 `--void "链条条目标题"`。使用 `--json` 获取稳定的内部标识符。

## 添加到 Waybar

在 Waybar 的 `modules-left`、`modules-center` 或 `modules-right` 数组中添加 `"custom/pomotui"`，然后添加模块配置：

```jsonc
"custom/pomotui": {
  "exec": "$HOME/.local/bin/pomotui waybar",
  "interval": 1,
  "return-type": "json",
  "tooltip": true,
  "on-click": "foot $HOME/.local/bin/pomotui-tui"
}
```

将 `foot` 替换为你的终端模拟器。编辑配置后重载 Waybar：

```sh
pkill -SIGUSR2 waybar
```

模块将会话状态和类型暴露为 CSS 类。样式示例：

```css
#custom-pomotui {
  color: #d66b5f;
  padding: 0 8px;
}

#custom-pomotui.paused,
#custom-pomotui.pending {
  color: #c9a66b;
}

#custom-pomotui.shortbreak,
#custom-pomotui.longbreak {
  color: #70b184;
}
```

## 配置和数据

Pomotui 遵循 XDG 基目录规范：

| 用途 | 默认路径 |
| --- | --- |
| 配置 | `~/.config/pomotui/config.toml` |
| SQLite 数据和会话历史 | `~/.local/share/pomotui/pomotui.sqlite3` |
| 运行时 socket | `$XDG_RUNTIME_DIR/pomotui/pomotui.sock` |

配置涵盖会话时长、专注周期轮数、主题、界面语言（`en` 或 `zh-CN`）、通知、声音、音量和完成动画。

备份和恢复指南参见[用户手册](docs/user-guide.md#backup-and-restore)。

## 卸载

移除程序和 systemd 用户单元，保留配置和会话历史：

```sh
./packaging/uninstall.sh
```

移除所有内容，包括配置和历史：

```sh
./packaging/uninstall.sh --purge
```

使用自定义 XDG 或 `PREFIX` 值安装时，卸载时需传入相同的值。

## 开发

运行 CI 使用的相同检查：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
tests/e2e.sh
```

[领域语言](CONTEXT.md)、[已接受的决策](docs/adr/)、[v1 规范](.scratch/pomotui-v1/spec.md)和[crate 边界策略](docs/architecture/crate-boundaries.md)更详细地解释了产品和架构。
