# Show Task attribution in Session History

Status: resolved

## Classification

Bug.

## Problem

Session History durably stores the optional Task identity and title snapshot,
and the CLI history response exposes them. The Dashboard snapshot instead
flattens recent records to strings such as `Focus Completed 1500s`, discarding
Task attribution before the TUI can render it.

## Scope

- Replace flattened recent-history strings with a structured protocol summary.
- Preserve Session kind, outcome, actual duration, and optional Task title.
- Render readable, localized History rows with Task attribution.
- Keep unattributed Focus Sessions and Break Sessions explicit.
- Retain the past/current/next hierarchy.

## Acceptance

- A recent Focus Session with a Task shows its historical title snapshot.
- A deleted or renamed Task cannot alter the displayed historical title.
- Break Sessions show that no Task attribution applies.
- English and Simplified Chinese History render tests cover Task, no-Task, and
  Break records.

## Comments

Created from installed-product feedback on 2026-07-27.

Resolved 2026-07-27. Protocol v2 replaces flattened recent-history strings
with structured Session kind, outcome, actual duration, and historical Task
title fields. The service projects the durable title snapshot, and the TUI
renders localized, semantically colored rows for attributed Focus Sessions,
unattributed Focus Sessions, and Break Sessions. Focused service-restart and
English/Chinese rendering tests pass.
