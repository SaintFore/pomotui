# 16 — Group TUI views into two Tab-switched areas

**What to build:** Replace the single seven-page TUI view cycle with two
top-level navigation areas. The Timer area contains Dashboard, Today, Review,
and History. The Work Chain area contains Chain, Chain Archive, and Rewards.
`Tab` switches areas directly while `h`/`l` and left/right arrows move only
within the current area.

**Blocked by:** 15 — Browse Ended Chain details and separate the Rewards page.

**Status:** ready-for-agent

- [x] The Timer area contains Dashboard, Today, Review, and History in that
  order.
- [x] The Work Chain area contains Chain, Chain Archive, and Rewards in that
  order.
- [x] `h`/`l` and left/right arrows wrap within the active area and never cross
  into the other area.
- [x] Physical `Tab` key input switches directly between the Timer and Work
  Chain areas.
- [x] Each area remembers its most recently visited page, so repeated `Tab`
  presses return the user to the page they left.
- [x] The initial remembered destinations are Dashboard for the Timer area and
  Chain for the Work Chain area.
- [x] An open overlay remains modal: `Tab` does not dismiss it or change the
  underlying page.
- [x] The footer visibly identifies both the active area and current page and
  advertises `Tab` area switching plus `h`/`l` page switching.
- [x] Help and the user guide document the two-area navigation in English and
  Simplified Chinese.
- [x] Existing page-specific keys, rendering, and Timer Service behavior remain
  unchanged.
- [x] Wide and narrow Ratatui rendering is covered.

## Test seams

Confirmed by the user before the first TDD red cycle:

1. `App::handle_key` is the public interaction seam for `Tab`, area-local
   `h`/`l` navigation, remembered destinations, wrapping, and modal behavior.
2. Ratatui `render` through `TestBackend` is the public presentation seam for
   the active-area/page indicator, discoverable shortcuts, localization, and
   responsive layouts.

The Crossterm event adapter will map physical `KeyCode::Tab` to the tested
`InputKey::Tab` interaction. Tests will not target private navigation helpers or
widget layout implementation.

## Comments

Reported after the Chain Archive and Rewards pages became first-class views.
The flat Dashboard → Chain → Chain Archive → Rewards → Today → Review → History
cycle makes common jumps require too many repeated `h`/`l` presses.

This ticket treats “番茄钟区域” as a Timer Frontend navigation area, not as a new
domain Timer. It does not change Current Session ownership or Timer Service
behavior.

Resolved on 2026-07-29. `Tab` now switches between the Timer and Work Chain
areas and restores the most recently visited page in each. Horizontal navigation
wraps within the active area, overlays remain modal, and the footer, help, and
user guide explain the interaction in both interface languages.
