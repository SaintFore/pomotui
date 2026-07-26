# Install desktop application entry

Status: resolved

## Classification

New requirement.

## Problem

Pomotui installs command-line binaries but no freedesktop application entry.
Desktop application launchers therefore cannot discover or open the TUI.

## Scope

- Ship and install a freedesktop `.desktop` entry for Pomotui.
- Launch the TUI through the user's terminal using the desktop entry's terminal
  flag rather than assuming a particular terminal emulator.
- Include the entry in isolated install, reinstall, and uninstall verification.
- Document launcher use.

## Acceptance

- Installation places `pomotui.desktop` under the XDG applications directory.
- The entry has a Pomotui name, searchable Pomodoro/timer keywords, and launches
  `pomotui-tui` in a terminal.
- Reinstall is idempotent and uninstall removes the entry.
- The end-to-end packaging test covers installation and removal.

## Comments

Created from installed-product feedback on 2026-07-27.

Resolved 2026-07-27. Packaging now installs an XDG application entry with
localized launcher metadata, searchable keywords, `Terminal=true`, and an
absolute executable path generated from the active installation prefix.
Reinstall replaces the entry atomically and uninstall removes it. The isolated
end-to-end test verifies installation, launch metadata, and removal.
