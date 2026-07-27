# Build a useful Today report

Status: resolved

## Classification

Interaction issue.

## Problem

The dedicated Today view repeats the compact Dashboard summary in a mostly
empty panel. It does not help the user understand daily progress, the shape of
the seven-day trend, or which Tasks received today's focus time.

## Scope

- Give the dedicated Today view a clear daily-report hierarchy.
- Show prominent Focus time, Completed Rounds, and Tasks touched metrics.
- Expand the seven-day trend into labeled daily values and bars.
- Add a today-only per-Task focus breakdown based on Session History.
- Preserve a compact Today panel on the wide Dashboard.
- Support English, Simplified Chinese, wide, and narrow layouts.

## Acceptance

- Today-only Task totals exclude older Session History.
- The dedicated view shows seven labeled days with numeric durations.
- Task contribution rows are ordered by descending focus time.
- Empty and disconnected states are informative.
- Deterministic service and TUI tests cover the projection and report hierarchy.

## Comments

Created from installed-product feedback on 2026-07-27.

Resolved 2026-07-27. The dedicated Today view now presents an at-a-glance
metric row, seven dated daily bars with numeric durations, and a descending
today-only Task contribution report including unattributed Focus time. The
wide Dashboard retains its compact summary. Service projection tests prove
older records are excluded and Task totals are ordered; wide/narrow
English/Chinese rendering tests and the process E2E pass.
