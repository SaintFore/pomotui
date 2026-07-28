# Pomotui

Pomotui is a Linux Pomodoro timer with three frontends:

- a keyboard-first Ratatui dashboard;
- a scriptable CLI;
- a polling Waybar module.

One persistent Timer Service owns the Current Session, so closing the TUI or
restarting Waybar does not stop time progression. Tasks, Session History, daily
statistics, recovery after restart, desktop reminders, and completion sounds
are stored and coordinated centrally.

## Requirements

- Linux with systemd user services
- A Rust toolchain supporting edition 2024
- Optional: Waybar, `notify-send`, and `paplay`

## Install

Build and install for the current user; `sudo` is not required:

```sh
cd pomotui
cargo build --release --workspace
./packaging/install.sh
systemctl --user daemon-reload
systemctl --user enable --now pomotui.socket
```

To rebuild, reinstall, and restart after updating the source, run:

```sh
./packaging/rebuild-restart.sh
```

This preserves the existing configuration, Tasks, and Session History.

The default installation places executables in `~/.local/bin`. Ensure that
directory is in `PATH`, then verify the service:

```sh
pomotui status
systemctl --user status pomotui.socket
```

The installer preserves an existing user configuration.

After installation, desktop application launchers can find **Pomotui** (or
**Pomotui 番茄钟** in a Simplified Chinese locale). Opening it starts the TUI in
the desktop's configured terminal.

## Use the TUI

```sh
pomotui-tui
```

The Dashboard follows a Timer First layout and adapts to wide and narrow
terminals. Its primary controls are:

| Key | Action |
| --- | --- |
| `j`/`k`, `↑`/`↓` | Select a Task |
| `h`/`l`, `←`/`→` | Switch Dashboard, Today, and History |
| `Enter` | Start Focus with the selected Task |
| `Space` | Start, pause, or resume the Current Session |
| `X` / `K` | Stop / skip the Current Session |
| `n` / `r` | Create / rename a Task |
| `c` / `D` | Complete or reopen / delete a Task |
| `:` | Open the executable command palette |
| `?` / `s` | Open Help / Settings |
| `Esc` | Close an overlay |
| `q` | Close the TUI without stopping the Timer Service |

Task deletion requires confirmation and never deletes existing Session History.
In Settings, press `g` to switch between English and Simplified Chinese; the
selection is saved to the user configuration.

## Use the CLI

```sh
pomotui task create "Write release notes"
pomotui task list
pomotui start focus --task 1
pomotui pause
pomotui resume
pomotui stop
```

Other commands include `start short-break`, `start long-break`, `skip`,
`history`, `summary`, and the complete Task lifecycle:
`create/rename/complete/reopen/delete`. Use `--json` with status, history, and
other commands when integrating with scripts.

## Add Pomotui to Waybar

Add `"custom/pomotui"` to one of Waybar's `modules-left`, `modules-center`, or
`modules-right` arrays, then add this top-level module configuration:

```jsonc
"custom/pomotui": {
  "exec": "$HOME/.local/bin/pomotui waybar",
  "interval": 1,
  "return-type": "json",
  "tooltip": true,
  "on-click": "foot $HOME/.local/bin/pomotui-tui"
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

## Configuration and data

Pomotui follows the XDG base-directory conventions:

| Purpose | Default path |
| --- | --- |
| Configuration | `~/.config/pomotui/config.toml` |
| SQLite data and Session History | `~/.local/share/pomotui/pomotui.sqlite3` |
| Runtime socket | `$XDG_RUNTIME_DIR/pomotui/pomotui.sock` |

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

Run the same checks used by CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
tests/e2e.sh
```

The [domain language](CONTEXT.md), [accepted decisions](docs/adr/), [v1
specification](.scratch/pomotui-v1/spec.md), and
[crate-boundary policy](docs/architecture/crate-boundaries.md) explain the
product and architecture in more detail.
