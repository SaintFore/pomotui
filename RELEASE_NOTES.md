# Pomotui v0.4.0

## Daily Chain Goal

- Set a daily Focus target in the TUI with `s` then `d`. Choose a start
  date, an optional inclusive end date, and a time zone. The goal is off by
  default.
- Each Focus Session counts after it reaches the duration set when it started
  and you review it as successful. The current Action Chain ends at midnight
  when the goal is missed. The archive shows the date, count, and target.
- Devices share the goal and count sessions in its time zone. A late successful
  review can restore the Action Chain if the combined activity met the goal.

## Focus Review and upgrades

- New Chain Links require a successfully reviewed Completed Round. Pausing
  preserves progress toward the planned duration. Stopping an unfinished Focus
  Session ends the Action Chain and records the elapsed time.
- Existing Chain Links and claimed rewards remain in history. If an upgrade
  leaves a Pending Review for a previously stopped Focus Session, use the TUI
  dismissal action or `pomotui review dismiss`.
- Upgrade the Timer Service and frontends together to protocol v7. The new
  Timer Service reads older sync documents and writes exchange format v9.
  Upgrade all Devices that share an exchange file before they resume syncing.
