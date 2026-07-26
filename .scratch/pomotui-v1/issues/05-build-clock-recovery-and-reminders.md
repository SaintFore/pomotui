# Build clock recovery and reminders

Status: resolved
Blocked by: 04

## Objective

Make deadline progression reliable across suspension, clock changes, restarts,
and reboots, with once-only reminders.

## Scope

- Implement suspend-aware monotonic progression behind a clock port.
- Persist boot/recovery observations and recover same-boot/new-boot states.
- Apply overdue deadline transitions exactly once.
- Dispatch D-Bus desktop notification and optional audio through fallible ports.
- Persist reminder emission so restarts do not duplicate it.

## Acceptance

- Deterministic fake-clock tests cover every case in the spec's Time and
  recovery section.
- Timezone and manual wall-clock changes do not distort a live Session.
- Notification/audio failure never rolls back completion.
- Restart cannot duplicate history, cycle advancement, or reminder emission.

## Comments

Implemented 2026-07-26. Linux `/proc/uptime` supplies suspend-aware elapsed
time; boot identity plus monotonic/wall observations drive same-boot and reboot
recovery. A background ticker completes Sessions without frontends. Completion,
durable state, and reminder claim commit atomically before fallible
`notify-send`/`paplay` effects. Recovery, clock-change, failure-isolation,
restart, deadline, and once-only tests pass.
