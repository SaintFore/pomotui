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

**Void Task**:
The single system-owned Task used when a reviewed Focus Session had no Current Task and the user does not assign a regular Task during Session Review. Its title is always `Void` in every interface language, it cannot be renamed or deleted, and it remains visible in focus-time statistics. Reviewing against the Void Task requires a Chain Entry Title.
_Avoid_: Missing Task, a newly created `Void` Task, empty Task

**Action Chain**:
The user's single current sequence of successful, explicitly reviewed Focus Sessions. Pomotui always maintains exactly one current Action Chain, including when its length is zero; a failed review atomically ends the old chain and creates a new empty current chain. A Focus Session that reaches its deadline or is stopped after starting requires Session Review; each successful review appends one Chain Link, while a failed review ends the current Action Chain without deleting the information needed for later reflection. An Action Chain never expires or breaks because of midnight, elapsed days, shutdown, inactivity, Break Sessions, or claimed rewards.
_Avoid_: Task tree, mind map, Focus Cycle

**Chain Link**:
One durable successful step in an Action Chain, created only after the user reviews a Focus Session as successful. It retains a stable identity, the reviewed Focus Session's Current Task identity and title snapshot, its exact actual duration in seconds, and an optional reflection describing the work in more detail. When the Current Task is the Void Task, it also requires a Chain Entry Title. Duration is presented as one value regardless of whether the Session reached its deadline or was stopped early: whole minutes use `50m`, while a remainder uses `17m 32s`.
_Avoid_: Task, Action Node, Completed Round

**Ended Chain**:
An Action Chain that is no longer current because the user reviewed a Focus Session as failed. It retains its Chain Links, terminal Chain Break, and reward history until the user explicitly deletes the whole Ended Chain; its individual entries cannot be deleted, extended, reactivated, or merged, while a Void entry's Chain Entry Title and any Reflection may be revised. An Ended Chain may have zero Chain Links when the first Session Review in a new chain is failed.
_Avoid_: Deleted chain, failed Task

**Chain Break**:
The non-counting terminal record appended when the user reviews a Focus Session as failed. It retains the reviewed Focus Session's Current Task identity and title snapshot, its exact actual duration in seconds, and an optional reflection. When the Current Task is the Void Task, it also requires a Chain Entry Title. It is not a Chain Link and never increases chain length; its duration uses the same single-value display as a Chain Link.
_Avoid_: Failed Chain Link, deleted Session, automatic failure

**Chain Entry Title**:
A required description of the attempted work when a Chain Link or Chain Break uses the Void Task. Non-Void entries use their Task title snapshot and do not need a separate title. It may be corrected after Session Review without changing the Session, chain membership, or review judgment.
_Avoid_: Task title, comment, Action Node title

**Reflection**:
User-authored text attached during or after Session Review to describe progress, interruption, failure context, or a useful next step. It is optional for a successful review and required before a failed review can be submitted. It may be revised later without changing the immutable Session Review result. It is labelled `复盘` in Simplified Chinese and `Reflection` in English.
_Avoid_: Comment, Task description, Session Outcome

**Reward Milestone**:
A user-configured Action Chain length and real-world reward promise. Reaching the length unlocks the reward once for the current Action Chain; adding a milestone or lowering its threshold may immediately unlock it when the current chain is already long enough. The user explicitly claims it and Pomotui records the claim without purchasing, paying, or performing the reward. Unlocking snapshots the reward name, threshold, and optional budget so later configuration changes cannot rewrite history. Claimed rewards remain part of an Ended Chain's history, while unlocked but unclaimed rewards become unavailable when the chain ends; configuration changes never retroactively affect Ended Chains.
_Avoid_: Account balance, automatic payment, Completed Round reward

**Session Review**:
The user's explicit judgment after a Focus Session reaches its deadline or is deliberately stopped for later review: successful or failed. A successful review appends a Chain Link; a failed review requires a Reflection and explicit confirmation that shows the ending chain length and any unlocked rewards that will become unavailable, then ends the current Action Chain. When the Session had no Current Task, Session Review must assign either a regular Task or the Void Task, and that assignment atomically updates Session History, Task focus time, statistics, and the chain entry; an already attributed Session cannot be reassigned. Once submitted, the judgment and its Session and chain identities are immutable. The Timer Service records the judgment rather than inferring success from duration, Task status, or Session Outcome. A skipped Focus Session and a Focus Session stopped without review are never reviewed.
_Avoid_: Automatic success, Session Outcome, Task completion

**Pending Review**:
One Focus Session that reached its deadline or was deliberately stopped for later review and whose Session Review has not yet been recorded. It remains durable when Timer Frontends close or the Timer Service restarts. A Pending Review permits Break Sessions but prevents another Focus Session from starting, so reviewed Focus Sessions cannot be reordered or skipped.
_Avoid_: Pending Session, Paused Session, unfinished Focus Session

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
A Session ended early without advancing the Focus Cycle or changing the next recommended session type. Its actual elapsed time remains in Session History. When stopping a Focus Session, the user explicitly chooses whether it creates a Pending Review; stopping without review has no effect on the Action Chain.
_Avoid_: Skipped Session, completed session

**Skipped Session**:
A Session deliberately passed over to reach the following Pending Session without starting it. Skipping a Focus Session does not add a Completed Round, so that round remains due after the intervening break.
_Avoid_: Stopped Session, completed session

**Session Reminder**:
The once-only desktop notification and optional sound emitted when a Session reaches its deadline. The sound may be disabled, selected from built-ins, or loaded from a local audio file with configurable volume; playback failure does not undo the transition, and Timer Service restarts must not repeat an emitted reminder.
_Avoid_: Timer tick, repeated alarm
