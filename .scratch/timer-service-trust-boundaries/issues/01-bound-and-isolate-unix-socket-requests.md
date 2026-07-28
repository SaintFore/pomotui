# 01 — Bound and isolate Unix socket requests

**What to build:** Keep every Timer Frontend responsive when another local
client stalls, sends an incomplete frame, exceeds the request limit, or creates
connection pressure. Preserve the versioned one-request/one-response Unix
socket contract while making timeout, overload, and oversized-input behavior
bounded and machine-readable.

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] A client that sends an incomplete request cannot prevent another valid
      client from receiving a response within the documented deadline.
- [x] Requests at the frame-size boundary are handled, while oversized requests
      are rejected without unbounded allocation.
- [x] Socket reads have a finite deadline and do not hold mutable Timer Service
      state while waiting for input or output.
- [x] Simultaneously handled connections are bounded and overload behavior is
      deterministic.
- [x] Existing CLI, TUI, Waybar, reconnect, and simultaneous-client behavior
      remains compatible.
- [x] Protocol tests cover partial frames, timeout, malformed input, frame-size
      boundaries, concurrent valid requests, and overload.

## Comments

Implemented with a bounded synchronous worker queue while retaining the
one-request/one-response Unix socket contract. Requests have a 64 KiB frame
limit and two-second read deadline; parsing and socket I/O happen outside the
Timer Service state lock. Protocol tests cover isolation, exact and oversized
frame boundaries, unterminated and stalled frames, simultaneous clients, and a
deterministic busy response. Workspace formatting, lint, all tests, and the
process end-to-end smoke test pass.
