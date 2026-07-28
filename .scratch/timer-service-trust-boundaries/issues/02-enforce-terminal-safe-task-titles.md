# 02 — Enforce terminal-safe Task titles end to end

**What to build:** Give users one predictable Task title policy across create,
rename, start-by-title, persistence, CLI JSON, and every Timer Frontend. Valid
multilingual titles remain usable, while unbounded or terminal-hostile text is
rejected safely without silently damaging existing Tasks or Session History.

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] A domain-owned Task title value defines deterministic normalization,
      UTF-8 byte, terminal display-width, whitespace, control-character, Bidi,
      and invisible-separator rules.
- [x] Task creation, rename, start-by-title, and durable restoration apply the
      same policy.
- [x] English, Simplified Chinese, emoji, combining text, and exact boundary
      cases have documented test coverage.
- [x] Invalid titles return stable machine-readable errors without echoing
      unsafe raw input.
- [x] Existing durable titles are not silently rewritten or discarded, and
      Session History title snapshots remain recoverable.
- [x] TUI views use consistent display-width truncation for accepted titles.

## Comments

Implemented a domain-owned Task title value using NFC normalization, a 256-byte
storage limit, a 120-column terminal limit, and explicit rejection of terminal
controls, Bidi formatting controls, and dangerous invisible separators. New
titles normalize consistently while restoration validates legacy titles
without silently rewriting them. Create, rename, and start-by-title return
stable protocol rules for unsafe input, and the existing shared
display-width-aware TUI rendering remains the presentation seam. Domain,
Service, workspace, lint, and process end-to-end tests pass.
