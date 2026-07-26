# Pomotui

This context defines the language for Pomotui, a reliable Pomodoro timer controlled from a terminal interface, command line, and Waybar.

## Language

**Timer Service**:
The single owner of the active timer and its transitions. It remains active independently of any visible interface so deadlines and notifications are reliable.
_Avoid_: Daemon, backend timer, background timer

**Timer Frontend**:
An interface that displays or controls the Timer Service without owning timer progression. The TUI, CLI, and Waybar module are Timer Frontends.
_Avoid_: Timer, timer process

**Current Session**:
The user's single shared Running, Paused, or Pending Session. Every Timer Frontend observes and controls the same Current Session; Tasks and terminal windows cannot own concurrent timers.
_Avoid_: Window timer, task timer

**Focus Session**:
A bounded period in which the user intends to focus on the current task.
_Avoid_: Pomodoro, work timer

**Break Session**:
A bounded period reserved for recovery between Focus Sessions. It is either a Short Break or a Long Break.
_Avoid_: Rest timer, pause

**Pending Session**:
The next recommended Focus Session or Break Session, waiting for the user to start it explicitly. Completing one session does not start the Pending Session automatically.
_Avoid_: Paused session, idle timer

**Current Task**:
The optional Task to which a Focus Session and its actual focus time are attributed. A Focus Session may run without one; completing a Focus Session neither completes nor detaches the Current Task.
_Avoid_: Required task, timer name

**Task**:
A lightweight work item with a stable identity, a non-unique title, an open or completed status, and actual focus time accumulated from its Focus Sessions. Starting by a new title creates a Task, while ambiguous existing titles require an explicit identity; completing it does not stop its Current Session, deleting it never deletes Session History, and a Task referenced by the Current Session cannot be deleted.
_Avoid_: Project, todo.txt entry

**Running Session**:
A Focus Session or Break Session whose remaining time advances with real elapsed time. System suspension counts, while timezone changes and manual wall-clock adjustments do not change its planned duration.
_Avoid_: Active timer

**Paused Session**:
A Session explicitly frozen by the user with its remaining duration preserved. Only an explicit resume makes its time advance again.
_Avoid_: Pending Session, suspended session

**Completed Round**:
A Focus Session that reached its planned deadline. A stopped or skipped Focus Session contributes its actual focus time to history and its Current Task but does not count as a Completed Round.
_Avoid_: Completed task, elapsed session

**Focus Cycle**:
A configurable number of Completed Rounds separated by Short Breaks and followed by a Long Break. The default cycle contains four Completed Rounds; interrupted Focus Sessions do not advance it, and completing the Long Break resets it.
_Avoid_: Session, round

**Session Durations**:
The user-configured default lengths of new Focus Sessions, Short Breaks, and Long Breaks. A Session keeps the planned duration it had when it started, even if these settings later change.
_Avoid_: Fixed Pomodoro duration

**Session History**:
The durable record of actual Focus Sessions and Break Sessions, including their planned and actual durations, outcome, and optional Task identity and title snapshot. It survives Task renaming and deletion and is the source for daily focus time, Completed Round counts, seven-day trends, and per-task totals.
_Avoid_: Activity log, current timer state

**Session Outcome**:
The reason a Session entered Session History: it reached its planned deadline, was stopped after starting, or was skipped before starting. Only a Focus Session that reached its deadline is a Completed Round.
_Avoid_: Task status, timer state

**Stopped Session**:
A Session ended early without advancing the Focus Cycle or changing the next recommended session type. Its actual elapsed time remains in Session History.
_Avoid_: Skipped Session, completed session

**Skipped Session**:
A Session deliberately passed over to reach the following Pending Session without starting it. Skipping a Focus Session does not add a Completed Round, so that round remains due after the intervening break.
_Avoid_: Stopped Session, completed session

**Session Reminder**:
The once-only desktop notification and optional sound emitted when a Session reaches its deadline. The sound may be disabled, selected from built-ins, or loaded from a local audio file with configurable volume; playback failure does not undo the transition, and Timer Service restarts must not repeat an emitted reminder.
_Avoid_: Timer tick, repeated alarm
