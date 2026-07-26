# Package and verify the Linux release

Status: resolved
Blocked by: 07, 08, 09

## Objective

Package Pomotui for a clean Linux/Wayland user environment and verify the v1
acceptance criteria end to end.

## Scope

- Install binaries, systemd user service/socket units, defaults/examples, and
  built-in animation using XDG paths.
- Document first run, TUI/CLI usage, Waybar configuration, settings, database
  location, and manual backup/import guidance.
- Add an isolated end-to-end harness with temporary XDG directories and fake
  external-effect adapters.
- Exercise multiple frontends, service restart, socket activation, recovery,
  Task/history behavior, and failure paths.

## Acceptance

- A clean install can run CLI, TUI, and Waybar flows via socket activation.
- The end-to-end suite verifies all eleven spec acceptance criteria or links
  each criterion to a lower-level automated test.
- Packaging does not depend on the prototype branch or `tuxedo/`.

## Comments

Implemented 2026-07-26. Packaging includes release binaries, systemd user
service/socket units, example/default TOML, built-in animation, an
XDG-respecting installer, Waybar/backup/user documentation, and CI. The
isolated E2E uses temporary XDG roots to verify service/CLI/Waybar flows,
restart recovery, Task/history preservation, disconnected JSON, installation,
assets, and non-overwrite of user configuration.
