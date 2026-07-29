# 12 — Browse and edit the current Action Chain

**What to build:** Turn the Chain page into a navigable chain instead of a flat recent-items summary. Show every current Chain Link as a connected sequence with its ID, effective title, exact duration, and Reflection; let the user select a link and edit the fields that the domain permits.

**Blocked by:** 11 — Infer Void during successful Session Review.

**Status:** ready-for-agent

- [x] The Timer Service snapshot exposes every current Chain Link in stable chain order instead of truncating it to five.
- [x] The Chain page visibly connects the root and Chain Links as one sequence.
- [x] Every Chain Link shows its stable ID, effective title, exact actual duration, and Reflection.
- [x] `j`/`k` (and arrow keys) move a visible Chain Link selection without changing Task selection.
- [x] `E` edits the selected Chain Link's Reflection rather than always editing the newest link.
- [x] A selected Void Chain Link offers an edit action for its Chain Entry Title; non-Void titles remain immutable.
- [x] Empty, populated, narrow, English, and Simplified Chinese Ratatui rendering is covered.

## Comments

The current implementation displays at most five links as unconnected text and binds `E` to the newest link. That makes the durable chain and its existing edit capability effectively undiscoverable.

Resolved with a complete root-to-tail snapshot and a connected, selectable Chain page. `E` edits the selected Reflection and `T` is available only for the selected Void link's Chain Entry Title.

Issue 18 later removes the stable IDs from normal Chain rendering to align with
the product-wide rule that internal identities remain available only through
detailed or JSON interfaces.

Issue 19 later broadens `T` to every selected Chain Entry and uses the Task
title snapshot as the initial editable display title when no override exists.
