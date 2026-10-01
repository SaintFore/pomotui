<div align="center">

<img src="./favicon_io/pomotui-512x512.png" alt="Pomotui logo" align="center" height="96" />

# Pomotui

*一个终端番茄钟——无论你关掉哪个界面，计时都不会停止。*

[![Build Status](https://img.shields.io/github/actions/workflow/status/SaintFore/pomotui/ci.yml?style=flat-square&label=Build)](https://github.com/SaintFore/pomotui/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/Rust-edition%202024-000000?style=flat-square&logo=rust&logoColor=white)
[![English](https://img.shields.io/badge/English-README-blue?style=flat-square&logo=readthedocs&logoColor=white)](README.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

[概览](#概览) • [安装](#安装) • [TUI](#使用-tui) • [CLI](#使用-cli) • [Waybar](#添加到-waybar) • [同步](#在多台电脑之间同步共享活动) • [开发](#开发)

![Pomotui TUI 仪表盘](https://tree-1327913400.cos.ap-nanjing.myqcloud.com/world/20260731122427701.webp)

</div>

## 概览

Pomotui 是一个驻留在终端里的番茄钟，提供三种前端：

- 键盘优先的 Ratatui 仪表盘；
- 可脚本化的命令行工具；
- 轮询式 Waybar 模块。

一个持久化的**计时服务**拥有当前会话，因此关闭 TUI 或重启 Waybar 不会停止时间推进。任务、会话历史、每日统计、重启恢复、桌面提醒和完成音效都集中存储和协调。任务还可以通过一个文件在多台电脑之间收敛；文件传输继续使用你已有的复制工具。

## 特性

- **持久计时器** — 即使所有前端都已关闭，会话仍在计时服务中持续推进
- **三种前端** — TUI 仪表盘、CLI 和 Waybar 模块观察并控制同一个会话
- **任务与历史** — 任务生命周期、持久会话历史、每日统计和七日趋势
- **行动链条与奖励** — 复盘的会话累积成链条；奖励里程碑可以解锁并领取
- **跨设备收敛** — 通过一个与提供商无关的文件同步任务、历史、复盘和链条
- **桌面集成** — 通知、完成音效、systemd 用户 socket、应用启动器入口
- **双语界面** — 英文和简体中文，可在设置中切换

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
systemctl --user enable --now pomotui.socket
```

AUR 软件包会将可执行文件安装到 `/usr/bin`。上述命令会启用 systemd 用户
socket，并立即启动 Pomotui。

使用 `paru -Syu --devel` 更新 git 软件包及其他 VCS 依赖。
升级后，软件包会尽力重启计时服务。如果出现重启警告，请运行：

```sh
systemctl --user daemon-reload
systemctl --user restart pomotui.service
```

### macOS（Homebrew）

```sh
brew tap SaintFore/tap
brew install --HEAD pomotui
brew services start pomotui
pomotui-tray   # 可选：菜单栏计时器
```

### 从源码构建（Linux）

为当前用户构建和安装，无需 `sudo`：

```sh
cd pomotui
cargo build --release --workspace
./packaging/install.sh
systemctl --user daemon-reload
systemctl --user enable --now pomotui.socket
```

更新源码后，运行 `./packaging/rebuild-restart.sh` 重新构建、安装并重启。
这会保留现有配置、任务和会话历史。

默认安装将可执行文件放在 `~/.local/bin`。确保该目录在 `PATH` 中，然后验证服务：

```sh
pomotui status
systemctl --user status pomotui.socket
```

> [!NOTE]
> 安装程序会保留现有用户配置。安装后，桌面应用启动器可以找到
> **Pomotui**（简体中文语言环境下为 **Pomotui 番茄钟**）。打开它会在桌面
> 配置的终端中启动 TUI。

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

> [!TIP]
> 删除任务需要确认，且不会删除现有会话历史。单行编辑器预填现有值，支持
> 方向键、Home/End、Delete 和 Emacs/Readline 快捷键子集：`C-a/e/b/f`、
> `M-b/f`、`C-h/d/w`、`M-d`、`C-k/u/y`。在设置中按 `g` 可切换英文和简体
> 中文；选择会保存到用户配置。

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

其他命令包括 `start short-break`、`start long-break`、`skip`、`history`、
`summary` 以及完整的任务生命周期：`create/rename/complete/reopen/delete`。
与脚本集成时对 status、history 等命令使用 `--json`。

使用 `stop --review` 将提前结束的专注会话送去复盘，或 `stop --no-review`
记录但不影响行动链条。失败的复盘需要复盘内容：

```sh
pomotui review failure "被打断，丢失了思路"
pomotui chain archive
pomotui reward create 10 "吃肯德基" --budget 50
pomotui reward list
pomotui reward claim 1
```

当复盘的会话没有任务时，用 `--task ID` 分配，或用 `--void "链条条目标题"`。
使用 `--json` 获取稳定的内部标识符，用于编辑条目或领取奖励。

## 添加到 Waybar

在 Waybar 的 `modules-left`、`modules-center` 或 `modules-right` 数组中添加
`"custom/pomotui"`，然后添加顶层模块配置：

```jsonc
"custom/pomotui": {
  "exec": "/usr/bin/pomotui waybar",
  "interval": 1,
  "return-type": "json",
  "tooltip": true,
  "on-click": "foot /usr/bin/pomotui-tui"
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

## 在多台电脑之间同步共享活动

Pomotui 通过一个与提供商无关的文件，收敛任务、已结束的会话历史、已提交的
会话复盘、行动链条、奖励里程碑、解锁和领取记录。Pomotui 只读写本地文件；
Syncthing、Dropbox、iCloud Drive、Nextcloud 等文件复制工具负责在电脑之间
传输它。

首次建立一组新的同步数据时，先选择本地历史最完整的电脑，把同步路径指向
复制工具管理的目录，然后生成第一个有效的交换文档：

```sh
pomotui sync enable "$HOME/Sync/pomotui.sync"
pomotui sync rebuild
pomotui --json sync status
```

创建新同步集时只在一台电脑上执行一次 `sync rebuild`，并以这台电脑的数据库
作为初始数据来源。等复制工具把 `pomotui.sync` 传到另一台电脑后，再在那台
电脑上启用其本地路径并合并：

```sh
pomotui sync enable "$HOME/Sync/pomotui.sync"
pomotui sync now
pomotui --json sync status
```

不同电脑可以使用不同的本地路径。`sync now` 会在当前电脑上执行双向合并：
仅存在于本机 SQLite 数据库中的记录会写入本地 `.sync` 文件，仅存在于该文件
中的记录会导入 SQLite。它不会连接其他电脑，也不会上传文件；应先等待复制
工具传输更新后的文件，再由另一台电脑执行合并。

Timer Service 运行期间，Pomotui 会在服务启动、发生相关持久化变更后以及每
30 秒自动请求一次合并。`sync now` 只是立即请求一次后台合并；请通过
`pomotui --json sync status` 确认 `in_progress` 为 `false`、`last_success`
已有时间戳且 `last_error` 为 `null`。

离线时可以继续工作。标题相同的任务仍保持不同身份，并发编辑会确定性解决；
删除不会使本机当前会话正在使用的任务失效。

> [!IMPORTANT]
> 当前会话、专注循环进度、待复盘、会话时长设置和界面偏好仍保留在各自的
> 电脑上。同步文件是交换文档，不是复制完成的证明，也不能代替本地 SQLite
> 数据库备份。冲突、升级、路径变更、恢复和故障排查流程请参阅
> [用户指南](docs/user-guide.md#cross-device-synchronization)。

### 预发布升级与旧同步格式

稳定版发布前，Pomotui 的数据库和同步格式可能发生不兼容变更。恢复文件复制
前，请先升级所有参与同步的电脑。如果升级后出现 `missing field ...`、服务
反复退出，或 `pomotui.service` 进入 `start-limit-hit`，应重置预发布数据库并
重新启动用户 socket，无需重新安装软件：

```sh
systemctl --user stop pomotui.socket pomotui.service
pomotui reset --all-data --confirm
systemctl --user reset-failed pomotui.service pomotui.socket
systemctl --user restart pomotui.socket
pomotui status
```

重置会创建带时间戳的 SQLite 备份并保留配置，但会有意丢弃当前本地数据库。
它也不会删除外部 `.sync` 文件，因此重置后，旧交换文件仍可能验证失败：

```text
unsupported sync document version ...
```

如果要从零开始，请先暂停文件复制工具，避免其他电脑把旧文件传回来；可按需
复制一份旧文件作为备份，然后只在一台已重置的电脑上生成当前格式：

```sh
pomotui sync enable "$HOME/Sync/pomotui.sync"
pomotui sync rebuild
pomotui --json sync status
```

`sync rebuild` 在后台异步执行，命令刚返回时可能仍显示上一次错误。应反复检查
`sync status`，直到 `in_progress` 为 `false`、`last_success` 已设置且
`last_error` 为 `null`。`file records unknown` 表示文件在统计记录数之前就未
通过验证，并非当前格式版本未知。确认重建后的文件有效后再恢复文件复制；其他
已升级电脑应执行 `sync now`，不要再次执行 `sync rebuild`。

## 配置和数据

Pomotui 遵循 XDG 基目录规范：

| 用途 | 默认路径 |
| --- | --- |
| 配置 | `~/.config/pomotui/config.toml` |
| SQLite 数据和会话历史 | `~/.local/share/pomotui/pomotui.sqlite3` |
| 运行时 socket | `$XDG_RUNTIME_DIR/pomotui/pomotui.sock` |

配置涵盖会话时长、专注周期轮数、主题、界面语言（`en` 或 `zh-CN`）、通知、
声音、音量和完成动画。

备份和恢复指南参见[用户指南](docs/user-guide.md#backup-and-restore)。

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

Pomotui 是一个 Cargo 工作区，具有严格的 crate 分层：

| Crate | 职责 |
| --- | --- |
| `pomotui-domain` | 产品模型和与基础设施无关的端口 |
| `pomotui-service` | 拥有会话推进的持久计时服务 |
| `pomotui-tui` | Ratatui 仪表盘 |
| `pomotui-cli` | `pomotui` 命令行 |
| `pomotui-tray` | macOS 菜单栏计时器 |
| `pomotui-sync` | 与提供商无关的同步文档和收敛引擎 |
| `pomotui-protocol` | 计时前端共享的带版本换行分隔 JSON 协议 |
| `pomotui-platform` | SQLite、时钟、通知、音频和服务生命周期适配器 |

运行 CI 使用的相同检查：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
tests/e2e.sh
```

[领域语言](CONTEXT.md)、[已接受的决策](docs/adr/)和
[crate 边界策略](docs/architecture/crate-boundaries.md)更详细地解释了产品和架构。
