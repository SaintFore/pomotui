# Pomotui v1

Status: resolved

## Summary

Pomotui is a reliable Pomodoro timer for Linux/Wayland. A persistent Timer
Service owns one shared Current Session; a Ratatui TUI, CLI, and Waybar module
observe and control it. Tasks and Session History are durable, preferences are
editable, and session deadlines remain meaningful across closed frontends,
service restarts, system suspension, timezone changes, and wall-clock changes.

## Goals

- Provide one authoritative timer shared by every Timer Frontend.
- Support Focus, Short Break, and Long Break Sessions with explicit start,
  pause, resume, stop, and skip controls.
- Track Tasks, actual focus time, Completed Rounds, daily totals, seven-day
  trends, and durable Session History.
- Provide a usable keyboard-first TUI at wide and narrow terminal sizes.
- Expose scriptable CLI output and a Waybar-compatible status stream.
- Emit a once-only desktop notification and optional sound at a deadline.
- Recover safely from service and frontend restarts without duplicate
  transitions or reminders.

## Non-goals

- macOS, Windows, non-systemd Linux, or non-Wayland desktop integration.
- Multiple simultaneous timers, per-window timers, or per-Task timers.
- Projects, nested Tasks, due dates, task synchronization, or todo.txt.
- Accounts, cloud synchronization, collaboration, or mobile clients.
- Automatic start of the next Pending Session.
- Treating the throwaway prototype or `tuxedo/` as production source.

## Domain rules

`CONTEXT.md` is authoritative for product language. In particular:

- There is at most one Current Session. It is Running, Paused, or Pending.
- A Focus Session may have no Current Task.
- A Running Session advances by real elapsed time. Suspension counts; timezone
  and manual wall-clock changes do not change its planned duration.
- Changed duration settings affect only newly started Sessions.
- Reaching a deadline records a completed Session Outcome. Only a completed
  Focus Session advances the Focus Cycle and counts as a Completed Round.
- Stopping records actual elapsed time, does not advance the Focus Cycle, and
  leaves the same recommended Session type Pending.
- Skipping records a skipped Session Outcome and advances to the following
  recommendation without starting it. A skipped Focus round remains due after
  the intervening break.
- Completing a Session reveals, but never starts, the next Pending Session.
- Completing a Focus Session neither completes nor detaches its Current Task.
- Task titles are non-unique. Ambiguous title-based commands fail with enough
  identities to choose explicitly.
- A referenced Task cannot be deleted. Otherwise deletion preserves the title
  snapshot and identity already stored in Session History.
- Each completed deadline emits at most one Session Reminder, including across
  Timer Service restarts. Reminder or sound failure cannot roll back completion.

## User experience

### Dashboard

Use the prototype's Timer First layout.

- Wide: Tasks and Today sit side by side above a large, full-width countdown.
- Narrow: Tasks stack above the countdown; Today and Session History are
  separate views rather than compressed panels.
- Keep Current Task, attributed focus time, Focus Cycle position, and the next
  Pending Session close to the countdown.
- Session History uses an explicit past/current/next hierarchy.
- Today shows Focus time, Completed Rounds, a compact seven-day trend, and its
  numeric average.

The signature themes are `Vermilion Paper Light` and `Vermilion Paper Dark`.
Focus and primary actions use Vermilion; Paused and Pending use warm gold with
explicit text labels; Break Sessions use a derived accessible green.

### Interaction

- Arrow keys and `j/k` move within Task lists.
- Arrow keys and `h/l` switch primary views.
- Uppercase `K` skips; destructive or less discoverable actions are also in the
  command palette.
- Settings is an overlay on wide terminals and a full-screen view on narrow
  terminals.
- Basic mouse targets mirror keyboard actions without being required.
- Help and the command palette expose all available actions and keybindings.
- After completion, a brief building-collapse character animation plays, then
  the next Pending Session is shown. The animation never delays the Timer
  Service transition.

## Frontends

### CLI

The CLI must support:

- Reading status in stable human-readable and JSON forms.
- Starting Focus/Short Break/Long Break Sessions, optionally selecting a Task.
- Pause, resume, stop, and skip.
- Task list/create/rename/complete/reopen/delete and explicit selection by ID.
- Reading Session History and Today/seven-day summaries.
- Opening or validating configuration where appropriate.

Commands use the versioned Unix-socket protocol; they do not access SQLite.
Failures use non-zero exit codes and actionable stderr. JSON output remains
machine-readable even when the service is unavailable or rejects a request.

### Waybar

Provide a long-running or polling-friendly command that emits Waybar JSON with
text, tooltip, class, and percentage. It reconnects after service restart and
does not own progression. Click actions may call the CLI.

## Architecture and storage

Follow ADRs 0001–0005:

- A systemd user-managed Timer Service is the sole owner of progression and
  sole SQLite writer.
- Frontends use a versioned request/response protocol over a Unix socket below
  `XDG_RUNTIME_DIR`; socket activation is supported.
- SQLite under the XDG data directory stores Current Session, Focus Cycle,
  Tasks, Session History, durable recovery metadata, and reminder emission.
- TOML under the XDG config directory stores durations, theme, keybindings,
  notification/sound preferences, and optional completion-animation path.
- The timer core remains independent of Ratatui, SQLite, systemd, D-Bus, audio,
  and wall-clock APIs through explicit ports/adapters.

Database schema changes are versioned and transactional. Startup either
migrates and recovers successfully or fails with a diagnostic without silently
discarding domain data.

## Time and recovery

While alive, the Timer Service uses a suspend-aware monotonic clock. Persisted
recovery metadata combines a boot identity with sufficient monotonic and
wall-clock observations to distinguish same-boot recovery from reboot recovery:

- Same boot: recover from the suspend-aware monotonic timeline.
- New boot: use persisted wall-clock observations only to estimate elapsed time
  during downtime, clamp impossible negative elapsed time to zero, and retain
  the original planned duration.
- Apply an overdue completion transition exactly once.
- Persist the transition and reminder emission state transactionally before or
  while dispatching external effects so restart cannot duplicate domain state.

The exact recovery representation may evolve, but tests must cover suspension,
timezone changes, forward/backward wall-clock adjustment, service restart
before and after deadline, reboot estimation, and duplicate reminder prevention.

## Configuration and animation

Ship documented defaults and tolerate a missing user config. Invalid fields
produce precise diagnostics and fall back only where doing so is safe.

The completion animation module accepts elapsed animation time and returns a
character-art frame plus whether playback is finished. The built-in animation
and a user-selected file share this interface. The editable file format has:

- `frame_ms`
- `hold_frames`
- fixed-canvas text frames separated by `---`

Validate nonzero timing, at least one frame, terminal-safe text, and bounded
frame/canvas sizes. An invalid custom file visibly falls back to the built-in
animation. Animation loading/playback stays entirely inside the TUI.

## Reliability and observability

- Protocol requests that mutate state carry an idempotency key.
- State-changing transactions are atomic and invariant-checked.
- The service logs lifecycle, recovery decisions, rejected commands, migrations,
  and external-effect failures without logging private Task text unnecessarily.
- Frontends show disconnected/reconnecting states without fabricating progress.
- Corrupt databases and incompatible protocol versions fail explicitly.

## Packaging

Install binaries, example/default config, built-in animation, systemd user
service/socket units, and Waybar usage documentation. Respect XDG paths. Provide
backup/import guidance for the SQLite database and TOML settings; automated
backup/import tooling is not required for v1.

## Acceptance criteria

1. Two simultaneous frontends observe and control the same Current Session.
2. Closing every frontend does not stop progression or deadline reminders.
3. Focus Cycle recommendations match completed, stopped, and skipped scenarios,
   including a skipped Focus round remaining due.
4. Actual duration and Task attribution are correct for completed and stopped
   Focus Sessions; skip records no elapsed focus time.
5. Time/recovery tests cover all cases listed above and never duplicate a
   transition or Session Reminder.
6. Task ambiguity, deletion constraints, rename snapshots, and history
   preservation behave as defined.
7. Wide and narrow TUI snapshots cover Focus, Short Break, Long Break, Paused,
   Pending, completed transition, disconnected, light, and dark states.
8. Keyboard, command-palette, settings, and basic mouse paths are usable; narrow
   settings is full-screen.
9. CLI JSON is stable and Waybar output reconnects after service restart.
10. Invalid custom animation, sound, or notification behavior cannot affect
    Session completion or Pending Session creation.
11. A clean Linux/Wayland install can activate the socket, launch the service,
    run the TUI and CLI, and configure a Waybar module using supplied docs.

## Delivery constraints

Before creating the production Rust skeleton, confirm the prototype report and
this spec retain every needed finding, then remove the nested `tuxedo/`
reference project. Do not merge or reuse the throwaway prototype implementation.

## Delivery

Completed 2026-07-26 through tickets 01–10. The implementation is verified by
strict formatting and Clippy gates, 39 automated Rust tests, a release workspace
build, and an isolated process-level XDG/socket/restart/Waybar/install E2E.
Requirement-to-test evidence is indexed in `docs/acceptance-matrix.md`.
