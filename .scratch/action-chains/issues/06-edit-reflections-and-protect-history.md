# 06 — Edit reflections while protecting chain history

**What to build:** Let users improve the reflective text on current and Ended Chains without weakening historical integrity. Reflection and Void Chain Entry Title are editable; judgments, attribution, durations, snapshots, membership, and archived structure remain immutable even when related Tasks or Session History change.

**Blocked by:** 05 — Review failure and archive the Action Chain.

**Status:** ready-for-agent

- [x] A user can add or revise the Reflection on a Chain Link.
- [x] A user can revise the required Reflection on a Chain Break without making it empty.
- [x] A user can revise the Chain Entry Title of a Void Chain Link or Void Chain Break without changing its attribution.
- [x] Non-Void entries reject a separate Chain Entry Title.
- [x] Edit operations cannot change review judgment, source Session, Action Chain, Task attribution, Task title snapshot, duration, entry kind, or reward history.
- [x] Edits work for both the current Action Chain and Ended Chains.
- [x] Task rename does not rewrite stored Task title snapshots.
- [x] Deleting an eligible regular Task does not delete or alter Chain Links or Chain Breaks.
- [x] Deleting reviewed Session History does not cascade into Action Chain history, while a Session with Pending Review cannot be deleted.
- [x] Edit mutations are idempotent and expose stable validation or conflict errors.
- [x] Chain and Chain Archive TUI pages and CLI commands expose only the permitted edits.
- [x] English uses `Reflection`, Simplified Chinese uses `复盘`, and `Void` remains untranslated.
- [x] Service, protocol, CLI, Ratatui, localization, Task lifecycle, and archive immutability tests verify observable behavior.

## Comments

Issue 17 and ADR-0007 later permit deletion of a whole Ended Chain. Individual
Chain Links and the Chain Break remain non-deletable and retain the limited edit
rules established here.

Issue 19 supersedes the non-Void title restriction for post-review editing:
every Chain Entry may have an independent display title, while Void alone still
requires one during Session Review.
