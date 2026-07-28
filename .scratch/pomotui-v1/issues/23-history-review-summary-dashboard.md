# Add a Session History review dashboard

Status: resolved

## Classification

Reporting and visualization feature.

Blocked by: 21

## Problem

The Today page gives a short operational summary, but there is no review page
that answers what was worked on, how focus changed over time, or how sessions
were completed versus interrupted.

## Scope

- Add a Review primary view sourced only from Session History.
- Show a time-range focus trend with numeric totals and averages.
- Show focus allocation by Task.
- Show Session Outcome distribution and completed-round consistency.
- Show a compact focus/break rhythm timeline where terminal size permits.
- Provide readable narrow-terminal fallbacks and bilingual labels.
- Recompute all charts after History deletion.
- Add summary projection and rendering tests.

## Acceptance

- Review makes “what did I work on?” visible through ranked Task totals.
- A trend chart shows when focus happened and includes exact values.
- An outcome chart distinguishes Completed, Stopped, and Skipped Sessions.
- Charts remain meaningful with no history, one record, deleted Tasks, and
  narrow terminals.
- Every number and chart mark is derived from Session History.

## Comments

Created from user feedback on 2026-07-28. The initial chart set deliberately
combines trend, Task allocation, outcome quality, and work/break rhythm because
these support both planning review and data-quality inspection.

Resolved 2026-07-28. Review is now a primary TUI view sourced from Session
History. It shows exact focus/break totals, a seven-day trend and average,
ranked per-Task bars, Completed/Stopped/Skipped distribution, and a compact
Focus/Short Break/Long Break rhythm. Empty and disconnected states are handled,
and rendering is regression-tested.
