# Bootstrap the production workspace

Status: resolved

## Objective

Create the production Rust workspace and establish dependency direction without
copying the throwaway prototype or `tuxedo/`.

## Scope

- Confirm `docs/prototypes/pomotui-dashboard.md`, `CONTEXT.md`, and the v1 spec
  preserve all required reference findings.
- Remove the nested `tuxedo/` reference project.
- Create workspace crates/binaries for the domain core, Timer Service,
  protocol/client, CLI, TUI, and platform adapters.
- Add formatting, linting, test, and basic CI commands.
- Encode dependency boundaries so the domain core has no UI, storage, desktop,
  or systemd dependencies.

## Acceptance

- The clean workspace builds and tests.
- Dependency boundaries are documented and mechanically visible.
- No production source or distinctive asset is copied from the prototype or
  `tuxedo/`.

## Comments

Implemented on 2026-07-26. The retained context, ADRs, prototype verdict, and v1
spec were checked before removing `tuxedo/`. The production workspace now
contains six crates with documented inward-only dependencies plus shared
formatting, linting, testing, and CI configuration.
