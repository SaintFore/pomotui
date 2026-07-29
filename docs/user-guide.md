# Pomotui user guide

## Install and first run

Build with `cargo build --release`, run `packaging/install.sh`, then activate:

```sh
systemctl --user daemon-reload
systemctl --user enable --now pomotui.socket
pomotui status
pomotui-tui
```

Pomotui uses `$XDG_RUNTIME_DIR/pomotui/pomotui.sock`,
`$XDG_DATA_HOME/pomotui/pomotui.sqlite3`, and
`$XDG_CONFIG_HOME/pomotui/config.toml` (with standard home-directory fallbacks).

## Uninstall

Run `packaging/uninstall.sh` from the source checkout. It stops and disables the
user service, removes installed programs and assets, and preserves configuration
and Session History by default:

```sh
packaging/uninstall.sh
```

To remove configuration and Session History as well:

```sh
packaging/uninstall.sh --purge
```

Use the same `PREFIX`, `XDG_CONFIG_HOME`, and `XDG_DATA_HOME` values that were
used during installation when they differ from their defaults.

## CLI and Waybar

Use `pomotui start focus`, `pause`, `resume`, `stop`, or `skip`. Task commands
are `task list/create/rename/complete/reopen/delete`; mutations by ID remain
unambiguous. `history`, `summary`, and `status --json` are scriptable.

Waybar custom-module example:

```json
{
  "custom/pomotui": {
    "exec": "pomotui waybar",
    "interval": 1,
    "return-type": "json",
    "on-click": "pomotui pause"
  }
}
```

## TUI

Press `g` in Settings to switch between English and Simplified Chinese and save
the choice. The equivalent configuration values are `language = "en"` and
`language = "zh-CN"`.

Built-in themes are `Vermilion Paper Light`, `Vermilion Paper Dark`,
`Ran Paper Light`, and `Ran Paper Dark`. Select one with `theme`:

```toml
theme = "Ran Paper Dark"
```

Every semantic color is an optional TOML interface. Values use strict
`#RRGGBB`; omitted fields continue to come from the selected built-in theme:

```toml
[colors]
background = "#11100F"
surface = "#25211C"
text = "#DDD2B9"
muted = "#9C9588"
accent = "#D64A3C"
gold = "#D8AD43"
good = "#6591AD"
border = "#564E43"
```

Use arrows or `h/l` to move through Dashboard, Chain, Chain Archive, Rewards,
Today, Review, and History. On Chain Archive, `j/k` selects an Ended Chain and
shows its complete links, Chain Break, and reward history. On Rewards, `j/k`
selects a Reward Milestone, `n` creates one, `e` updates it, uppercase `D`
deletes it after confirmation, and uppercase `C` claims the first unlocked
reward. Use `gg`/`G` for the first/last item, `u`/`d` to move by a page, and `v`
for contiguous visual History selection. In History, `Space` or
`Alt+Space` toggles individual records so unrelated entries can be selected
together, and `D` deletes the marked records after confirmation. On the
Dashboard, press `Enter` to bind the selected Task to the Current Session and
`Space` to start/pause/resume; `X` stops and `K` skips.

Task management is available without leaving the TUI: `n` creates, `r` renames,
`c` completes or reopens, and uppercase `D` deletes after confirmation. `:`
opens the executable command palette, `?` opens the complete key reference, `s`
opens settings, `Esc` closes any overlay, and `q` quits the TUI without stopping
the Timer Service.

Set `sound = "builtin:complete"` for the standard freedesktop completion sound,
or set `sound` to a local audio-file path. Sound and desktop-notification
failures are logged by their adapters and never undo Session completion.

## Backup and restore

Stop `pomotui.service`, copy the SQLite database and TOML configuration, then
restart the socket. Restore only into an empty data directory while the service
is stopped. Keep both files from the same backup point.
