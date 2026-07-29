# 19 — Edit any Chain Entry title and show its Chain position

**What to build:** Let users edit the display title of every current or
archived Chain Entry, not only Void entries, and show each entry's one-based
position in its Action Chain without exposing storage IDs.

**Blocked by:** 18 — Hide Chain IDs and provide Emacs-style text editing.

**Status:** resolved

- [x] The current Chain page prefixes every entry with its one-based Chain
  position (`1.`, `2.`, …) and does not render its database ID.
- [x] An Ended Chain detail prefixes every link and the final Chain Break with
  their one-based Chain positions and does not render database IDs.
- [x] `T` opens title editing for any selected current or archived Chain Entry,
  including Task-backed entries and the final Chain Break.
- [x] Title editing starts with the effective displayed title: the existing
  title override when present, otherwise the entry's Task title.
- [x] Saving creates or replaces the selected entry's title override without
  renaming the underlying Task or changing Session History.
- [x] Existing Void review requirements remain unchanged; this ticket broadens
  post-review title editing, not review-time attribution rules.
- [x] Reflection-only edits preserve the title, and title-only edits preserve
  the Reflection.
- [x] Empty title submission remains rejected by the text editor.
- [x] Current and archived title overrides survive the existing durable service
  persistence path.
- [x] English, Simplified Chinese, narrow/wide layouts, mixed Void/Task entries,
  and a long archived Chain render unambiguous positions.
- [x] Help, README, and the user guide describe `T` as editing any selected
  Chain Entry title.

## Proposed test seams

Confirmed by the user before the first TDD red cycle:

1. `Service::execute(Command::ChainEntryEdit { … })` verifies that a title
   override can be saved for Task-backed current links and archived
   links/Chain Breaks, while preserving Reflection and Task identity.
2. `App::handle_key` verifies that `T` opens the editor for any selected entry,
   prefills its effective displayed title, and emits the intended
   `ChainEntryEdit` command.
3. Ratatui `render` through `TestBackend` verifies one-based Chain positions,
   hidden database IDs, localization, and long/narrow rendering.

Tests will observe protocol commands, snapshots, and rendered text rather than
private helper functions or repository tables.

## Comments

Issue 18 correctly removed database IDs such as `#77`, but those IDs had also
been acting as accidental visual position labels. This ticket restores the
useful information as a stable one-based position within each Chain. Position
is presentation data and is intentionally distinct from storage identity.

The existing service permits `chain_entry_title` only for Void entries.
Post-review title editing is broadened to all Chain Entries; review-time Void
validation is left intact.
