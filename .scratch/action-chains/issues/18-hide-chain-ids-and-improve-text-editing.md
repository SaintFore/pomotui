# 18 — Hide Chain IDs and provide Emacs-style text editing

**What to build:** Remove internal Action Chain and Chain Link IDs from the
normal Chain page, and make every TUI text-edit overlay preserve existing text
with a real cursor and a documented single-line Emacs/Readline editing subset.

**Blocked by:** 17 — Delete whole Ended Chains and open scrollable archive
details.

**Status:** resolved

- [x] The current Chain page renders `ROOT`, effective Chain Link titles, exact
  durations, and Reflections without Action Chain or Chain Link IDs.
- [x] Chain Archive list/detail and Rewards continue to hide internal IDs.
- [x] Opening Reflection editing loads the selected current or archived entry's
  complete existing Reflection and places the cursor at the end.
- [x] Opening Void Chain Entry Title editing loads the complete existing title
  and places the cursor at the end.
- [x] Task rename and Reward Milestone update retain their existing prefill and
  also place the cursor at the end.
- [x] Typing inserts at the cursor rather than always appending.
- [x] Left/Right, Home/End, Backspace, and Delete edit Unicode text without
  splitting a character.
- [x] `C-a`/`C-e` move to start/end; `C-b`/`C-f` move one character;
  `M-b`/`M-f` move one word.
- [x] `C-h` deletes backward; `C-d` deletes forward; `C-w` deletes the previous
  word; `M-d` deletes the next word.
- [x] `C-k` deletes from the cursor to the end, `C-u` deletes from the start to
  the cursor, and `C-y` restores the most recently killed text.
- [x] Enter saves and Esc cancels exactly as before; editing shortcuts never
  trigger page, Task, Session, or application commands.
- [x] The text overlay visibly renders the cursor at its logical position and
  keeps it visible when existing text is longer than the overlay width.
- [x] Empty and prefilled input, long text, mixed Chinese/ASCII text, English,
  Simplified Chinese, wide, and narrow terminals are covered.
- [x] Help and the user guide describe the supported editing subset without
  claiming full Emacs compatibility.

## Test seams

Confirmed by the user before the first TDD red cycle:

1. `App::handle_key` is the public editing seam for prefill, cursor movement,
   insertion/deletion, kill/yank, Unicode safety, cancellation, and the emitted
   protocol command.
2. The Crossterm key-event adapter is the physical-input seam for mapping
   Ctrl/Alt, Home/End, Delete, arrows, Enter, and Esc to the tested semantic
   `InputKey` actions.
3. Ratatui `render` through `TestBackend` is the public presentation seam for
   hidden IDs, cursor placement, long-text visibility, localization, and
   responsive layouts.

Tests will not target private string helpers or terminal buffer coordinates.

## Comments

Reported with screenshots from the current Chain and archived-entry Reflection
editor. The Chain page exposes `ROOT #3` and per-link `#x` values even though
normal TUI views are meant to hide storage identities. Reflection and Void-title
editing start with an empty input, forcing the user to retype long existing
text.

The current input model stores only a `String`, appends every character, and
removes only the final character. The physical input adapter also discards
modifier meaning for ordinary characters. This ticket introduces a Unicode-safe
cursor and a deliberately bounded single-line Emacs/Readline subset; multiline
editing, command history, selection, an Emacs kill ring, and arbitrary Emacs
keymaps remain out of scope.
