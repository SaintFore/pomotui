# Complete TUI interaction and visual fidelity

Status: resolved
Blocked by: 08

## Objective

Correct the production TUI so its interaction coverage and visual hierarchy
match the accepted Timer First prototype and the frontend requirements.

## Scope

- Make Escape close the command palette and every other overlay.
- Add executable Task create, rename, complete/reopen, and delete flows.
- Separate Help's structured reference role from the command palette's
  executable-action role.
- Restore the accepted header, Task selection, Today statistics, large
  countdown, progress, Focus Cycle, next Session, state colors, and responsive
  wide/narrow hierarchy.
- Surface protocol rejection and disconnection feedback inside the TUI.

## Acceptance

- A deterministic regression test covers Escape through the TUI input model.
- Task mutations produce the same versioned protocol commands as the CLI.
- Help is grouped by Navigation, Sessions, Tasks, and Tools.
- The command palette exposes all Session and Task mutations.
- Wide and narrow TestBackend rendering retains the accepted Timer First
  hierarchy across themes and Session states.

## Comments

Reported from hands-on use after the initial v1 installation.

Resolved 2026-07-26. Replaced the character-only input path with a typed input
model that handles Escape, Enter, Backspace, arrows, configured characters, and
text entry. The executable command palette now covers every Session mutation,
all Task lifecycle operations, Today, History, Settings, and Help. Task create
and rename use in-TUI text entry; deletion requires confirmation; command
rejections are shown in the footer.

The Dashboard was re-authored from the accepted Timer First design evidence:
branded header, responsive selected-Task list, full Today statistics, large
countdown, progress gauge, Current Task time, Focus Cycle/next Session context,
semantic state colors, structured Session History, and distinct Help,
Settings, and command overlays. Regression tests cover Escape, structured
Help, palette coverage, protocol-equivalent Task commands, themes, states, and
wide/narrow layouts. Workspace clippy/tests, release build, diff checks, and
the end-to-end smoke test all pass.
