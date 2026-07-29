# 15 — Browse Ended Chain details and separate the Rewards page

**What to build:** Make the Chain Archive a navigable master/detail view so a
user can inspect the complete contents of each Ended Chain, and move Reward
Milestone management out of the current Chain page into a dedicated Rewards
page in the normal TUI view cycle.

**Blocked by:** 13 — Manage Reward Milestones in the TUI.

**Status:** ready-for-agent

- [x] The normal TUI view cycle contains distinct Chain, Chain Archive, and
  Rewards pages in both directions.
- [x] The Chain Archive has a visible selection, and `j`/`k` plus arrow keys
  select an Ended Chain without changing Task selection.
- [x] The selected Ended Chain shows every Chain Link in stable order, followed
  by its terminal Chain Break.
- [x] Each archived entry shows its effective title, exact actual duration, and
  Reflection; the selected chain also shows its length.
- [x] The selected Ended Chain shows its claimed and unavailable reward history,
  including reward name, threshold, optional budget, and state.
- [x] Empty and populated archives remain usable in narrow terminals.
- [x] The Rewards page lists the complete Reward Milestone ladder in threshold
  order with a visible selection.
- [x] Creating, updating, and deleting Reward Milestones works from the Rewards
  page with the existing validation and confirmation behavior.
- [x] Claiming an unlocked reward works from the Rewards page, while the Chain
  page retains only a compact next-reward and earned-reward summary.
- [x] Normal TUI pages do not expose internal IDs.
- [x] English and Simplified Chinese rendering is covered for empty, populated,
  wide, and narrow states.
- [x] Existing service persistence, CLI archive output, reward semantics, and
  Action Chain editing behavior remain unchanged.

## Test seams

Confirmed by the user before the first TDD red cycle:

1. The Timer Service `Snapshot` is the public data-contract seam for the bounded
   archive detail and reward-history data required by the TUI.
2. `App::handle_key` is the public interaction seam for page navigation,
   archive selection, Reward Milestone actions, and reward claims.
3. Ratatui `render` through `TestBackend` is the public presentation seam for
   visible archive details, the dedicated Rewards page, localization, and
   responsive layouts.

Tests will not target private rendering helpers, internal service collections,
or widget layout implementation.

## Comments

Reported from direct use of the TUI: the Chain Archive currently renders each
Ended Chain as one summary line, so its Chain Links, detailed Chain Break, and
reward history cannot be inspected. Reward Milestone management currently opens
as an overlay from the Chain page, which makes rewards feel embedded in the
current chain instead of being a first-class destination.

The existing `ActionChainArchive` CLI/protocol query already returns full chain
entries, but the ordinary TUI consumes snapshots only. The snapshot contract
therefore needs a bounded, presentation-ready archive detail projection rather
than making the TUI infer domain state.

Resolved on 2026-07-29 with a dedicated Rewards page and a selectable
master/detail Chain Archive. The bounded snapshot projection now includes each
recent Ended Chain's complete links, terminal Chain Break title, and reward
history. TDD covered the confirmed service, interaction, and Ratatui rendering
seams; formatting, workspace tests, Clippy, and the end-to-end smoke test pass.
