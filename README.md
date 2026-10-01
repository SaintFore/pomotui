<div align="center">

<img src="./favicon_io/pomotui-512x512.png" alt="Pomotui logo" align="center" height="96" />

# Pomotui

_A terminal Pomodoro timer that keeps running no matter which interface you close._

[![Build Status](https://img.shields.io/github/actions/workflow/status/SaintFore/pomotui/ci.yml?style=flat-square&label=Build)](https://github.com/SaintFore/pomotui/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/Rust-edition%202024-000000?style=flat-square&logo=rust&logoColor=white)
[![中文](https://img.shields.io/badge/中文-README-red?style=flat-square&logo=readthedocs&logoColor=white)](README_zh-CN.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

[Overview](#overview) • [Install](#install) • [TUI](#use-the-tui) • [CLI](#use-the-cli) • [Waybar](#add-pomotui-to-waybar) • [Sync](#sync-shared-activity-across-computers) • [Development](#development)

![Pomotui TUI Dashboard](https://tree-1327913400.cos.ap-nanjing.myqcloud.com/world/20260731122427701.webp)

</div>

## Overview

Pomotui is a Pomodoro timer that lives in your terminal, with three frontends:

- a keyboard-first Ratatui dashboard;
- a scriptable CLI;
- a polling Waybar module.

One persistent **Timer Service** owns the Current Session, so closing the TUI
or restarting Waybar does not stop time progression. Tasks, Session History,
daily statistics, recovery after restart, desktop reminders, and completion
sounds are stored and coordinated centrally. Tasks can also converge across
computers through one file managed by the file-replication tool you already
use.

## Features

- **Persistent timer** — sessions keep advancing in the Timer Service even with every frontend closed
- **Three frontends** — TUI dashboard, CLI, and Waybar module observe and control the same session
- **Tasks and history** — task lifecycle, durable Session History, daily statistics, and seven-day trends
- **Action Chains and rewards** — reviewed sessions build a chain; reward milestones can be unlocked and claimed
- **Cross-device convergence** — sync tasks, history, reviews, and chains through one provider-neutral file
- **Desktop integration** — notifications, completion sounds, systemd user socket, app launcher entry
- **Bilingual UI** — English and Simplified Chinese, switchable in Settings

## Requirements

**Linux:**

- systemd user services
- A Rust toolchain supporting edition 2024
- Optional: Waybar, `notify-send`, and `paplay`

**macOS:**

- A Rust toolchain supporting edition 2024
- Optional: `afplay` (built-in), `osascript` (built-in), Waybar (via Homebrew)

## Install

### Arch Linux (AUR)

```sh
paru -S pomotui
# or for latest git:
paru -S pomotui-git
systemctl --user enable --now pomotui.socket
```

The AUR packages install the executables in `/usr/bin`. The command above
enables the systemd user socket and starts Pomotui immediately.

Update the git package and its VCS dependencies with `paru -Syu --devel`.
The package makes a best-effort attempt to restart the Timer Service after an
upgrade. If it prints a restart warning, run:

```sh
systemctl --user daemon-reload
systemctl --user restart pomotui.service
```

### macOS (Homebrew)

```sh
brew tap SaintFore/tap
brew install --HEAD pomotui
brew services start pomotui
pomotui-tray   # optional: menu bar timer
```

### From source (Linux)

Build and install for the current user; `sudo` is not required:

```sh
cd pomotui
cargo build --release --workspace
./packaging/install.sh
systemctl --user daemon-reload
systemctl --user enable --now pomotui.socket
```

To rebuild, reinstall, and restart after updating the source, run
`./packaging/rebuild-restart.sh`. This preserves the existing configuration,
Tasks, and Session History.

The default installation places executables in `~/.local/bin`. Ensure that
directory is in `PATH`, then verify the service:

```sh
pomotui status
systemctl --user status pomotui.socket
```

> [!NOTE]
> The installer preserves an existing user configuration. After installation,
> desktop application launchers can find **Pomotui** (or **Pomotui 番茄钟** in a
> Simplified Chinese locale). Opening it starts the TUI in the desktop's
> configured terminal.

## Use the TUI

```sh
pomotui-tui
```

The Dashboard follows a Timer First layout and adapts to wide and narrow
terminals. Its primary controls are:

| Key                                | Action                                                                                          |
| ---------------------------------- | ----------------------------------------------------------------------------------------------- |
| `j`/`k`, `↑`/`↓`                   | Select a Task                                                                                   |
| `h`/`l`, `←`/`→`                   | Switch Dashboard, Chain, Chain Archive, Today, Review, and History                              |
| `Enter`                            | Start Focus with the selected Task                                                              |
| `Space`                            | Start, pause, or resume the Current Session                                                     |
| `X` / `K`                          | Choose how to stop / skip the Current Session                                                   |
| `S` / `Enter` in Pending Review    | Review as successful, using the existing or selected Task (`Void` asks for a Chain Entry Title) |
| `F` in Pending Review              | Review as failed                                                                                |
| `p`                                | Reopen the Pending Review dialog                                                                |
| `j` / `k` on Chain                 | Select a Chain Link                                                                             |
| `E` on Chain                       | Edit the selected Chain Link's Reflection                                                       |
| `T` on Chain or open Chain Archive | Edit the selected Chain Entry's display title                                                   |
| `E` on Chain Archive               | Edit the latest Chain Break Reflection                                                          |
| `R` on Chain                       | Open Reward Milestone manager (`n` create, `e` edit, `D` delete)                                |
| `C` on Chain                       | Claim the first unlocked reward                                                                 |
| `n` / `r`                          | Create / rename a Task                                                                          |
| `c` / `D`                          | Complete or reopen / delete a Task                                                              |
| `:`                                | Open the executable command palette                                                             |
| `?` / `s`                          | Open Help / Settings                                                                            |
| `Esc`                              | Close an overlay                                                                                |
| `q`                                | Close the TUI without stopping the Timer Service                                                |

> [!TIP]
> Task deletion requires confirmation and never deletes existing Session
> History. Single-line text editors prefill the existing value and support
> arrows, Home/End, Delete, and a compact Emacs/Readline subset: `C-a/e/b/f`,
> `M-b/f`, `C-h/d/w`, `M-d`, and `C-k/u/y`. In Settings, press `g` to switch
> between English and Simplified Chinese; the selection is saved to the user
> configuration.

## Use the CLI

```sh
pomotui task create "Write release notes"
pomotui task list
pomotui start focus --task 1
pomotui pause
pomotui resume
pomotui stop
pomotui review success --reflection "Finished the vertical slice"
pomotui chain
```

Other commands include `start short-break`, `start long-break`, `skip`,
`history`, `summary`, and the complete Task lifecycle:
`create/rename/complete/reopen/delete`. Use `--json` with status, history, and
other commands when integrating with scripts.

Use `stop --review` to send an early Focus Session to Session Review, or
`stop --no-review` to record it without affecting the Action Chain. A failed
review requires a Reflection:

```sh
pomotui review failure "Interrupted and lost the intended thread"
pomotui chain archive
pomotui reward create 10 "Eat KFC" --budget 50
pomotui reward list
pomotui reward claim 1
```

When a reviewed Session has no Task, assign one with `--task ID`, or use
`--void "Chain Entry Title"`. Run a command with `--json` to obtain stable
internal identities for editing entries or claiming rewards.

## Add Pomotui to Waybar

Add `"custom/pomotui"` to one of Waybar's `modules-left`, `modules-center`, or
`modules-right` arrays, then add this top-level module configuration:

```jsonc
"custom/pomotui": {
  "exec": "/usr/bin/pomotui waybar",
  "interval": 1,
  "return-type": "json",
  "tooltip": true,
  "on-click": "foot /usr/bin/pomotui-tui"
}
```

Replace `foot` with your terminal emulator. Reload Waybar after editing its
configuration:

```sh
pkill -SIGUSR2 waybar
```

The module exposes Session state and kind as CSS classes. Example styling:

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

## Sync Shared Activity across computers

Pomotui converges Tasks, ended Session History, submitted Session Reviews,
Action Chains, Reward Milestones, unlocks, and claims through one
provider-neutral file. Pomotui reads and writes the local file; a file-copy
tool such as Syncthing, Dropbox, iCloud Drive, or Nextcloud carries it between
computers.

To start a new synchronization set, choose the computer with the most complete
local history, enable a path inside the replicated directory, and build the
first valid exchange document:

```sh
pomotui sync enable "$HOME/Sync/pomotui.sync"
pomotui sync rebuild
pomotui --json sync status
```

Run `sync rebuild` only once when creating a new set, and only on the computer
whose database should seed it. After the copy tool delivers `pomotui.sync` to
another computer, enable that computer's local path and merge it:

```sh
pomotui sync enable "$HOME/Sync/pomotui.sync"
pomotui sync now
pomotui --json sync status
```

The local path may differ between computers. `sync now` performs a two-way
merge on the current computer: records found only in its SQLite database are
written to the local `.sync` file, while records found only in that file are
imported into SQLite. It does not contact another computer or upload the file.
Wait for the copy tool to transfer the updated file, then let the other
computer merge it.

While the Timer Service is running, Pomotui requests this merge automatically
at startup, after relevant durable changes, and every 30 seconds. `sync now`
requests one immediately; because the work runs in the background, use
`pomotui --json sync status` to confirm that `in_progress` is `false`,
`last_success` is set, and `last_error` is `null`.

Work can continue offline. Same-titled Tasks retain distinct identities,
concurrent edits resolve deterministically, and deletions do not invalidate a
Task used by the local Current Session.

> [!IMPORTANT]
> Current Sessions, Focus Cycle progress, Pending Reviews, Session Durations,
> and interface preferences remain local to each computer. The sync file is an
> exchange document, not proof of replication and not a replacement for backing
> up the local SQLite database. See the
> [user guide](docs/user-guide.md#cross-device-synchronization) for conflict,
> upgrade, path-change, recovery, and troubleshooting procedures.

## Configuration and data

Pomotui follows the XDG base-directory conventions:

| Purpose                         | Default path                             |
| ------------------------------- | ---------------------------------------- |
| Configuration                   | `~/.config/pomotui/config.toml`          |
| SQLite data and Session History | `~/.local/share/pomotui/pomotui.sqlite3` |
| Runtime socket                  | `$XDG_RUNTIME_DIR/pomotui/pomotui.sock`  |

Configuration covers Session durations, rounds per Focus Cycle, theme,
interface language (`en` or `zh-CN`), notifications, sound, volume, and
completion animation.

For backup and restore guidance, see the
[user guide](docs/user-guide.md#backup-and-restore).

## Uninstall

Remove the programs and systemd user units while preserving configuration and
Session History:

```sh
./packaging/uninstall.sh
```

Remove everything, including configuration and history:

```sh
./packaging/uninstall.sh --purge
```

When custom XDG or `PREFIX` values were used for installation, pass the same
values during uninstall.

## Development

Pomotui is a Cargo workspace with a strict crate layering:

| Crate              | Responsibility                                                       |
| ------------------ | -------------------------------------------------------------------- |
| `pomotui-domain`   | Product model and infrastructure-independent ports                   |
| `pomotui-service`  | The persistent Timer Service that owns session progression           |
| `pomotui-tui`      | The Ratatui dashboard                                                |
| `pomotui-cli`      | The `pomotui` command line                                           |
| `pomotui-tray`     | The macOS menu bar timer                                             |
| `pomotui-sync`     | Provider-neutral sync document and convergence engine                |
| `pomotui-protocol` | Versioned newline-delimited JSON protocol shared by Timer Frontends  |
| `pomotui-platform` | SQLite, clocks, notifications, audio, and service-lifecycle adapters |

Run the same checks used by CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
tests/e2e.sh
```

The [domain language](CONTEXT.md), [accepted decisions](docs/adr/),
and [crate-boundary policy](docs/architecture/crate-boundaries.md) explain the
product and architecture in more detail.
