# Build CLI and Waybar frontends

Status: resolved
Blocked by: 06

## Objective

Provide complete scriptable control and resilient Waybar output over the shared
protocol client.

## Scope

- Implement status, Session controls, Task commands, history, and reporting.
- Provide stable human and JSON output with actionable exit behavior.
- Require explicit Task ID when a title is ambiguous.
- Emit Waybar JSON text, tooltip, class, and percentage.
- Reconnect or support efficient polling after Timer Service restart.

## Acceptance

- CLI integration tests cover success, rejection, ambiguity, and disconnect.
- JSON schemas are snapshot-tested and contain no incidental log output.
- Waybar output reflects every Session state and reconnects after restart.

## Comments

Implemented 2026-07-26. CLI covers Session controls, ID/title Task workflows,
history, summaries, human/JSON status, and actionable disconnect behavior.
Waybar emits stable text/tooltip/class/percentage JSON. Unit tests cover output
schemas and disconnects; the process E2E verifies Task attribution, JSON,
Waybar, service restart, and reconnection.
