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

After a package upgrade, the package hook attempts to reload and restart an
already running Timer Service. If it warns that the user service manager was
unavailable, or if the CLI reports that an older Timer Service does not support
a newly installed command, run:

```sh
systemctl --user daemon-reload
systemctl --user restart pomotui.service
```

This restarts only the service process; durable local data and configuration
remain in place.

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

The TUI has two top-level areas. The Timer area contains Dashboard, Today,
Review, and History; the Work Chain area contains Chain, Chain Archive, and
Rewards. Press `Tab` to switch areas—the TUI remembers the last page visited in
each one. Use arrows or `h/l` to move only within the current area.

On Chain Archive, `j/k` selects an Ended Chain and shows its complete links,
Chain Break, and reward history. Press `Enter` to open the selected Ended Chain,
then use `j/k` to browse every archived entry; `E` edits its Reflection, `T`
edits any selected entry's display title, and `Esc` returns to the
archive list. Individual archived entries cannot be deleted. On the archive
list, uppercase `D` permanently deletes the whole selected Ended Chain after a
confirmation that shows its length and reward-history impact; Session History
and Tasks are preserved.

On Rewards, `j/k` selects a Reward Milestone, `n` creates one, `e` updates it,
uppercase `D` deletes it after confirmation, and uppercase `C` claims the first
unlocked reward. Use `gg`/`G` for the first/last item, `u`/`d` to move by a
page, and `v` for contiguous visual History selection. In History, `Space` or
`Alt+Space` toggles individual records so unrelated entries can be selected
together, and `D` deletes the marked records after confirmation. On the
Dashboard, press `Enter` to bind the selected Task to the Current Session and
`Space` to start/pause/resume; `X` stops and `K` skips.

Text editors open with the existing value and the cursor at its end. They are
single-line editors (not full Emacs), with arrows, Home/End, Backspace/Delete,
`C-a/e/b/f` and `M-b/f` for movement, `C-h/d/w` and `M-d` for deletion, plus
`C-k/u/y` for kill-to-end, kill-to-start, and yank.

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

## Cross-device synchronization

Pomotui exchanges Shared Activity through one ordinary file. First configure a
file-copy or file-replication tool of your choice to carry a file between your
Devices. Then point each Device at its local path for that file; paths do not
need to be identical:

```sh
pomotui sync enable "/path/replicated-by-your-tool/pomotui.sync"
pomotui sync now
pomotui sync status
```

On an existing installation, the Timer Service first validates the complete
sync document and creates a durable SQLite backup beside the database. Backup
names contain `initial-sync-export` and a timestamp; they are never overwritten
or removed automatically. It then exports existing Shared Activity and global
identity mappings in one transaction. If initial publication fails, the local
migration remains committed and `pomotui sync now` can safely retry.

Opening the unversioned schema-3 database shipped by Pomotui v0.1.0 and v0.2.0
also creates a separate `schema-migration` backup before upgrading its durable
state. To restore either backup, stop the Timer Service and replace the database
with the selected `.sqlite3` backup. The sync file is not required to open it.

Use `pomotui sync status --json` for automation (the global `--json` flag may
also come first):

```sh
pomotui --json sync status
pomotui --json sync now
```

Synchronization is eventually consistent. Each Device retains every valid
Synchronization Record it creates or observes, unions those records with the
file, applies new records transactionally, and atomically replaces the file.
Repeated imports are safe and do not double totals. Tasks, ended Session
History, submitted Session Reviews, Action Chains and their edits, Reward
Milestones, unlocks, claims, and synchronized deletions converge. Running or
Paused Current Sessions, Pending Reviews, Focus Cycle progress, Session
Durations, and interface settings remain local.

The Timer Service requests a merge when it starts, after relevant durable local
changes, and every 30 seconds while it is running. Overlapping requests are
coalesced into one later run. `pomotui sync now` requests the same merge
immediately when diagnosing or coordinating file transfer; it is not required
after every local command.

Timer Service also discovers Syncthing conflict siblings of the configured filename.
For `pomotui.sync`, the supported spelling is
`pomotui.sync-conflict-YYYYMMDD-HHMMSS-DEVICE.sync`; custom filenames use the
same stem and extension rule. Other neighboring files are ignored. Each attempt
scans at most 4096 directory entries and imports at most 32 candidates, rotating
through larger sets on subsequent attempts. Files are limited to 8 MiB; symbolic
links and nonregular objects are rejected. Valid copies are durably merged before
publishing the main exchange file. Copies remain available for recovery.

`sync status` distinguishes discovery failures (`discover`), invalid copies
(`candidate-validate`), contradictory record identities (`candidate-integrity`
or `import`), and publication failures (`replace`). A contradictory identity
reports the record ID and differing fields. Keep these artifacts and resolve the
reported invalid or contradictory input before retrying; local timer controls
remain available during transport failures.

Concurrent edits resolve without an interactive conflict screen. Immutable
facts from both sides are retained. Versions of mutable data use their UTC
mutation time and then a stable global identity as a deterministic tie-breaker;
the same records therefore produce the same result on every Device regardless
of arrival order. Submitted Session Reviews are ordered by the source Session's
end time and a stable identity tie-breaker. A later offline Session Review can
consequently revise old Action Chain boundaries and unclaimed reward
eligibility. A claimed reward remains claimed. A clock-skew warning reported by
`sync status` means a Device's clock may have produced a surprising order;
synchronization continues so the
record is not silently lost. Correct that Device's clock, make a new corrective
edit where applicable, and synchronize again.

### Upgrades, paths, and disabling

Back up the local SQLite database before first enabling synchronization and
before upgrades. After upgrading, restart the Timer Service if the package did
not do so automatically:

```sh
systemctl --user daemon-reload
systemctl --user restart pomotui.service
pomotui sync status
```

To move the exchange file, disable synchronization, arrange replication at the
new location, and enable the new local path. Disabling or changing a path does
not erase locally retained records:

```sh
pomotui sync disable
pomotui sync enable "/new/local/path/pomotui.sync"
pomotui sync now
```

Leave synchronization disabled with `pomotui sync disable`. Ordinary timer
commands and local data continue to work.

### Recovery and rebuild

An older copy overwriting `pomotui.sync` does not tell a Device to forget known
records. Run `pomotui sync now` on a Device that previously observed the missing
work, allow the transport tool to copy the repaired file, then run it on the
other Devices. Repeated merges restore the union.

If the file is deleted or cannot be parsed, choose a surviving Device and
rebuild from all records in that Device's database:

```sh
pomotui sync rebuild
pomotui sync status
pomotui --json sync status
```

Rebuild deliberately ignores an invalid target and replaces it with a valid
document. It can recover only records that the chosen database has observed.
Afterward, synchronize every other surviving Device: one that retained an
additional record will add it back to the union. Do not rebuild independently
on several Devices while the transport tool is actively copying the file;
rebuild once, let it replicate, then merge the others in turn.

Keep normal backups of each Device's SQLite database as described above.
`pomotui.sync` is a disposable exchange artifact: its presence only proves that
one local path exists. It does not prove the transport tool copied it anywhere,
does not acknowledge that another Device imported it, cannot recover unseen
records from a lost Device, and is not a replacement for a database backup.

### Troubleshooting

If changes have not appeared elsewhere:

1. Run `pomotui sync status` and `pomotui --json sync status` and check the
   configured path, enabled state, `in_progress`, `last_success`, warning, and
   stage-specific error fields.
2. Confirm independently that the transport tool copied the same
   `pomotui.sync` content to the other Device. Pomotui has no provider or Device
   registry and cannot report transport progress.
3. Run `pomotui sync now` on one Device, wait for success, let the file copy,
   and run it on the next Device. Repeat once if an older copy won a race.
4. Fix permission errors on the selected file and its parent directory. An
   invalid or inaccessible file is left untouched, local SQLite data is not
   imported or corrupted, and ordinary timer commands remain available.
5. Use `sync rebuild` only for a missing or invalid file and only after choosing
   the database with the broadest known history. Preserve database backups.
6. Restart the Timer Service if the CLI says the running service is older than
   the installed synchronization command.

### Reward Debt after synchronization

A claimed reward remains claimed when a late Session Review breaks its supporting
Action Chain. Each milestone keeps its own debt in missing successful reviews:
a threshold of seven supported by one success owes six. Future successes repay
one unit for every affected milestone before advancing that milestone's next
reward. Breaks preserve debt and repayment; actual chain length remains factual.
`pomotui reward list`, JSON status, human status, and the TUI expose debt and repaid
successes. Editing or deleting a milestone does not forgive its obligation.

New claims retain immutable support and observed-review identities in the same
portable activity file. Format 6 accepts formats 4 and 5 without changing old
Record IDs; persisted format 3 accepts the supported unversioned and format 2
states. Upgrade all Devices before synchronizing: older software rejects the new
format. Legacy claims without observed support remain claimed and accrue no
inferred debt. Missing evidence cannot be reconstructed from a claim timestamp.
