# Implement the Timer Service protocol

Status: resolved
Blocked by: 04, 05

## Objective

Expose all reads and controls through a versioned Unix-socket protocol and run
the sole-owner Timer Service.

## Scope

- Define request, response, error, snapshot, and protocol-version types.
- Require idempotency keys on mutations.
- Implement Unix-socket server/client, reconnect behavior, and subscriptions or
  efficient snapshot updates.
- Add systemd socket activation support and graceful lifecycle handling.
- Surface disconnected, incompatible-version, and rejected-command states.

## Acceptance

- Concurrent clients observe one authoritative state.
- Protocol compatibility and malformed-message tests are deterministic.
- Socket activation starts the service on first access.
- No frontend reads or writes SQLite directly.

## Comments

Implemented 2026-07-26 as versioned newline-delimited JSON over Unix sockets.
Mutation keys are mandatory and durably idempotent; malformed/version errors
are explicit. Short-lived polling connections support concurrent frontends,
restart reconnection, and efficient snapshots. `listenfd` safely accepts
systemd socket activation; only the Timer Service depends on the SQLite adapter.
