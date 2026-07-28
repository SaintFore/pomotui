# Add Ran themes and TOML color overrides

Status: resolved

## Classification

Theming and configuration feature.

## Problem

TOML can select a built-in theme name, but every RGB value is compiled into
the TUI. Users cannot add small personal adjustments, and the palette inspired
by Akira Kurosawa's *Ran* is not available.

## Scope

- Add Ran Paper Light and Ran Paper Dark presets from the supplied palette.
- Make every semantic TUI color optionally overridable in `[colors]`.
- Accept strict `#RRGGBB` values with actionable validation errors.
- Keep missing `[colors]` fully backward compatible.
- Cycle all built-in themes in Settings and show their names.
- Document the semantic fields and example TOML.
- Add configuration and rendering regression tests.

## Acceptance

- `theme = "Ran Paper Light"` and `"Ran Paper Dark"` load the supplied colors.
- `[colors]` may override any subset of background, surface, text, muted,
  accent, gold, good, and border.
- Invalid or unknown color fields fail with a precise diagnostic.
- Existing configuration files render exactly as before.
- Settings cycles through all four built-in themes.

## Comments

Created from the user-provided palette reference on 2026-07-28.

Resolved 2026-07-28. Ran Paper Light and Ran Paper Dark use the reference
palette's blood red, banner gold, warrior blue, straw, smoke, and fortress
colors. All eight semantic TUI colors accept independent strict `#RRGGBB`
overrides through `[colors]`; omitted values retain their selected preset.
Configuration validation, all-theme rendering, defaults, and documentation
were updated.
