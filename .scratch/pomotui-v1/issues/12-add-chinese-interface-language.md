# Add Chinese interface language

Status: resolved

## Classification

New requirement.

## Problem

The installed TUI is English-only and exposes no language control, so Chinese
users cannot discover or select a Chinese interface.

## Scope

- Add Simplified Chinese and English interface languages.
- Make language selectable from the TUI Settings overlay.
- Persist the selection in the user configuration.
- Localize the Dashboard, Today, History, Help, Settings, command palette,
  task-editing overlays, state labels, and footer feedback.
- Keep Task titles and protocol data unchanged.

## Acceptance

- The default configuration documents the language setting.
- Settings visibly shows how to switch between English and Simplified Chinese.
- A deterministic render test verifies representative Chinese Dashboard,
  Settings, and Help text.
- English remains available and existing user configurations remain valid.

## Comments

Created from installed-product feedback on 2026-07-27.

Resolved 2026-07-27. The TUI now supports English and Simplified Chinese across
the main views, state labels, command palette, Help, Settings, task-editing
overlays, and footer guidance. Settings exposes `g` to switch languages and
persists the choice without discarding unrelated configuration. The default
config and user documentation describe `language = "en"` / `"zh-CN"`.
Deterministic rendering tests cover the Chinese Dashboard, Settings, and Help.
