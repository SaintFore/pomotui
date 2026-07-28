# 02 — Enforce terminal-safe Task titles end to end

**What to build:** Give users one predictable Task title policy across create,
rename, start-by-title, persistence, CLI JSON, and every Timer Frontend. Valid
multilingual titles remain usable, while unbounded or terminal-hostile text is
rejected safely without silently damaging existing Tasks or Session History.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] A domain-owned Task title value defines deterministic normalization,
      UTF-8 byte, terminal display-width, whitespace, control-character, Bidi,
      and invisible-separator rules.
- [ ] Task creation, rename, start-by-title, and durable restoration apply the
      same policy.
- [ ] English, Simplified Chinese, emoji, combining text, and exact boundary
      cases have documented test coverage.
- [ ] Invalid titles return stable machine-readable errors without echoing
      unsafe raw input.
- [ ] Existing durable titles are not silently rewritten or discarded, and
      Session History title snapshots remain recoverable.
- [ ] TUI views use consistent display-width truncation for accepted titles.

