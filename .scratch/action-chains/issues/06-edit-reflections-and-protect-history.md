# 06 — Edit reflections while protecting chain history

**What to build:** Let users improve the reflective text on current and Ended Chains without weakening historical integrity. Reflection and Void Chain Entry Title are editable; judgments, attribution, durations, snapshots, membership, and archived structure remain immutable even when related Tasks or Session History change.

**Blocked by:** 05 — Review failure and archive the Action Chain.

**Status:** ready-for-agent

- [ ] A user can add or revise the Reflection on a Chain Link.
- [ ] A user can revise the required Reflection on a Chain Break without making it empty.
- [ ] A user can revise the Chain Entry Title of a Void Chain Link or Void Chain Break without changing its attribution.
- [ ] Non-Void entries reject a separate Chain Entry Title.
- [ ] Edit operations cannot change review judgment, source Session, Action Chain, Task attribution, Task title snapshot, duration, entry kind, or reward history.
- [ ] Edits work for both the current Action Chain and Ended Chains.
- [ ] Task rename does not rewrite stored Task title snapshots.
- [ ] Deleting an eligible regular Task does not delete or alter Chain Links or Chain Breaks.
- [ ] Deleting reviewed Session History does not cascade into Action Chain history, while a Session with Pending Review cannot be deleted.
- [ ] Edit mutations are idempotent and expose stable validation or conflict errors.
- [ ] Chain and Chain Archive TUI pages and CLI commands expose only the permitted edits.
- [ ] English uses `Reflection`, Simplified Chinese uses `复盘`, and `Void` remains untranslated.
- [ ] Service, protocol, CLI, Ratatui, localization, Task lifecycle, and archive immutability tests verify observable behavior.

