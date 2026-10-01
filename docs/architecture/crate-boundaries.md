# Crate boundaries

Pomotui uses dependency direction to keep product rules independent of delivery
and infrastructure details.

```text
pomotui-domain
   ↑      ↑
   │  pomotui-sync
   │      ↑
pomotui-protocol
      ↑
  ┌───┴──────────────┐
  │                  │
pomotui-service   Timer Frontends
  ↑               (CLI and TUI)
pomotui-platform
```

## Crates

- `pomotui-domain` owns product types, invariants, transitions, and ports. It
  must not depend on serialization, async runtimes, terminal UI, SQLite,
  systemd, D-Bus, audio, or operating-system clock APIs.
- `pomotui-protocol` owns versioned wire DTOs and the reusable client. It may
  translate to and from public domain types but contains no business rules.
- `pomotui-sync` owns the provider-neutral synchronization document, validated
  global identities and mutation instants, set union, and deterministic
  projection plans. It depends only on domain vocabulary, not on the Timer
  Service, SQLite, or filesystem APIs.
- `pomotui-platform` implements domain/service ports for Linux facilities such
  as SQLite, clocks, notifications, audio, XDG paths, and systemd integration.
  It does not contain Timer Frontend behavior.
- `pomotui-service` composes the domain, protocol server, and platform adapters.
  It applies Sync Engine projection plans and is the sole process allowed to
  write the domain database.
- `pomotui-cli` and `pomotui-tui` are Timer Frontends. They depend on the
  protocol client and never access SQLite or own timer progression.

Cargo manifests mechanically expose these edges. A new edge toward the domain
requires an architectural review; adapters must point inward, never the reverse.

The two executable frontend crates remain separate initially so their dependency
sets and lifecycle behavior stay explicit. Packaging may later place them behind
one launcher without changing these boundaries.

## Reference-material policy

The disposable Dashboard prototype remains available only as findings and
snapshots on its prototype branch. The former `tuxedo/` reference checkout was
removed before this workspace was created. Production code and assets must be
authored from Pomotui's context, ADRs, specification, and prototype verdict—not
copied from either reference implementation.
