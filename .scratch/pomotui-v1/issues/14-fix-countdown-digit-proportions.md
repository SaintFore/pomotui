# Fix countdown digit proportions

Status: resolved

## Classification

Bug.

## Problem

The large countdown uses three-column by five-row block glyphs. In ordinary
terminal cells this makes digits, especially `1`, look unnaturally narrow and
the whole clock appear horizontally compressed.

## Scope

- Replace the countdown glyphs with proportions suited to terminal cell
  geometry.
- Preserve centered layout, semantic Session colors, and compact fallback at
  short or narrow sizes.
- Add a regression test for the exact `1`-digit width and clock geometry.

## Acceptance

- Every large digit occupies a consistent five-column canvas.
- `1` has visible structure across that canvas instead of a narrow vertical
  stroke.
- `25:00` remains centered without clipping in the supported wide layout.
- Existing wide/narrow and Session-state rendering tests pass.

## Comments

Created from installed-product feedback and screenshot evidence on 2026-07-27.

Resolved 2026-07-27. The countdown font was widened from a three-column canvas
to a consistent five-column canvas. The `1` now has a shoulder and full base,
and the colon shares the same canvas width. A focused regression test locks the
glyph proportions and complete `25:00` geometry; the existing responsive
Dashboard rendering test also passes.
