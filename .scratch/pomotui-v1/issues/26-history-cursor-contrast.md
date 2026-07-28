# Keep History cursor text readable in every theme

Status: resolved

## Classification

Accessibility and theme-rendering defect.

## Problem

The History cursor uses the theme accent as its row background while the
Session kind keeps its semantic accent foreground. A selected Focus row
therefore renders accent-on-accent and its kind disappears. Custom palettes can
create the same failure for other text.

## Scope

- Derive a high-contrast cursor foreground from the actual cursor background.
- Apply it to every span in the selected History row, including Session kind.
- Preserve semantic Session colors on rows outside the cursor.
- Cover every built-in theme and custom light/dark accent backgrounds.

## Acceptance

- No non-space cursor-row text has the same foreground and background.
- Focus, Break, outcome, Task, and duration remain readable under all themes.
- Unselected Session kinds retain their semantic colors.

## Comments

Created from a Ran Paper Light History screenshot on 2026-07-28.

Resolved 2026-07-28. Cursor text now switches to black or white from the actual
accent luminance, and the explicit Session-kind span uses the same cursor
foreground instead of its semantic accent. Tests inspect rendered cells across
all four presets plus custom light and dark accents to prevent foreground and
cursor background collisions.
