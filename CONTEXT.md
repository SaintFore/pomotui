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
The single shared Running, Paused, or Pending Session on one Device. Every Timer Frontend on that Device observes and controls the same Current Session; different Devices may have independent Current Sessions.
_Avoid_: Window timer, task timer

**Device**:
One Pomotui installation with its own Timer Service, Current Session, and Focus Cycle. A Device may work offline and later exchange Shared Activity with the user's other Devices; Pomotui does not maintain a user-visible registry of Devices.
_Avoid_: Account, client, computer name

**Shared Activity**:
The converged collection of ended Session facts, submitted Session Reviews, failed Reservation Reviews, and explicit early Focus stops from all of the user's Devices. The shared Daily Chain Goal determines how that activity is evaluated for each goal day. It is the source for cross-device Session History, statistics, Action Chains, and Reward Milestones; live Current Sessions, Focus Reservations, unresolved Reservation Reviews, Focus Cycle progress, and unsubmitted Pending Reviews are not part of it.
_Avoid_: Synced database, cloud state, backup

**Synchronization Record**:
A globally identified immutable fact exchanged between Devices and retained by every Device that observes it. Its kind determines one valid payload shape, and a deterministic projection turns a set of records into Shared Activity and synchronized Task state.
_Avoid_: Database row, mutable event, service snapshot

**Review Order**:
The deterministic cross-device order of Chain Links and Chain Breaks, including breaks projected at a Daily Chain Goal deadline. It decides Action Chain membership and reward progress and may be revised when an older offline review arrives, without changing the immutable content of any review.
_Avoid_: File order, sync order, device order

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
A lightweight work item with a stable identity, a non-unique title, an open or completed status, and actual focus time accumulated from its Focus Sessions. Starting by a new title creates a Task, while ambiguous existing titles require an explicit identity; `Void` is reserved for the system Void Task and cannot be used as an ordinary Task title. Completing a Task does not stop its Current Session, deleting it never deletes Session History, and a Task referenced by the Current Session cannot be deleted.
_Avoid_: Project, todo.txt entry

**Void Task**:
The single system-owned Task used when a reviewed Focus Session had no Current Task and the user does not assign a regular Task during Session Review. Its reserved title is always `Void` in every interface language; an ordinary Task cannot use that title, and the Void Task cannot be renamed or deleted. It remains visible in focus-time statistics. Reviewing against the Void Task requires a Chain Entry Title.
_Avoid_: Missing Task, a newly created `Void` Task, empty Task

**Action Chain**:
The user's single current sequence of successful, explicitly reviewed Focus Sessions across Shared Activity, including when its length is zero. A successful Session Review appends one Chain Link in Review Order. A failed Session Review, failed Reservation Review, an explicit early Focus stop, or a missed Daily Chain Goal ends it and starts an empty Action Chain. Calendar boundaries affect the Action Chain only through a Daily Chain Goal.
_Avoid_: Task tree, mind map, Focus Cycle

**Daily Chain Goal**:
The optional shared daily quota of successfully reviewed Completed Rounds for keeping the Action Chain across the user's Devices. It applies from its first date through its optional last date, inclusive, in a fixed shared time zone. Changes to an active quota and disabling the goal take effect from the next calendar day. Successful reviews count on the date the Completed Round ended.

**Chain Link**:
One durable successful step in an Action Chain. A new Chain Link requires a Completed Round and a successful Session Review. Existing Chain Links from successfully reviewed Stopped Sessions remain valid under the rule that originally admitted them. A Chain Link retains a stable identity, the reviewed Focus Session's Current Task identity and title snapshot, its exact actual duration in seconds, and an optional Reflection. When the Current Task is the Void Task, it also requires a Chain Entry Title. Whole-minute durations use `50m`, while a remainder uses `17m 32s`.
_Avoid_: Task, Action Node, Completed Round

**Ended Chain**:
An Action Chain archived after a Chain Break. It retains its Chain Links, terminal Chain Break, and reward history until the user explicitly deletes the whole Ended Chain. Its individual entries cannot be deleted, extended, reactivated, or merged, while any Chain Entry Title or Reflection may be revised. An Ended Chain may have zero Chain Links when a Session Review or Reservation Review fails before its first success.
_Avoid_: Deleted chain, failed Task

**Chain Break**:
The non-counting terminal record for a failed Session Review, failed Reservation Review, explicit early Focus stop, or missed Daily Chain Goal. A Session Review break retains its Focus Session's Current Task identity, title snapshot and exact actual duration. A Reservation Review break identifies its reservation. An early-stop break identifies its Stopped Session. A Daily Chain Goal break identifies the date, quota, and achieved count. Session Review breaks require a Reflection, and Reservation Review breaks permit an optional Reflection. Both support a Chain Entry Title, which is required when reviewing the Void Task.
_Avoid_: Failed Chain Link, deleted Session

**Chain Entry Title**:
The user-controlled display title of a Chain Link or Chain Break. It initially falls back to the Task title snapshot for a regular Task, while an entry using the Void Task requires an explicit title during Session Review. Any entry's title may later be created or corrected independently without renaming its Task or changing the Session, chain membership, or review judgment.
_Avoid_: Task title, comment, Action Node title

**Reflection**:
User-authored text attached during or after Session Review or Reservation Review to describe progress, interruption, failure context, or a useful next step. It is optional for Reservation Review and successful Session Review, and required before a failed Session Review can be submitted. It may be revised later without changing the immutable Session Review result. It is labelled `复盘` in Simplified Chinese and `Reflection` in English.
_Avoid_: Comment, Task description, Session Outcome

**Reward Milestone**:
A user-configured Action Chain length and real-world reward promise. Reaching the length unlocks the reward once for the current Action Chain. Creating a milestone or changing its threshold requires a threshold strictly greater than the current Action Chain length; editing only its name or budget remains allowed after reaching the threshold. Deleting a milestone cancels unclaimed rewards while preserving claimed history. The user explicitly claims it and Pomotui records the claim without purchasing, paying, or performing the reward. Unlocking snapshots the reward name, threshold, and optional budget so later configuration changes cannot rewrite history. Claimed rewards remain historical facts even when late Shared Activity revises Review Order, while unlocked but unclaimed rewards follow the latest projected Action Chain; configuration changes never retroactively affect Ended Chains.
_Avoid_: Account balance, automatic payment, Completed Round reward

**Reward Debt**:
The outstanding successful Focus Sessions needed to compensate for a claimed reward whose eligibility is reduced by late Shared Activity. The reward remains claimed; the debt covers the missing successes rather than the reward's entire threshold. Debt is accounted for separately for each Reward Milestone, and one later successful Session Review may repay each milestone's outstanding debt once before contributing toward that milestone's next reward. Further Shared Activity recalculates the shortfall while preserving repayment progress; excess repayment contributes toward future reward progress. Reward Debt and repayment progress survive a Chain Break and explicit Ended Chain deletion, the Action Chain length continues to count actual Chain Links, and observing the same claim again never creates an additional debt. A Fresh Start clears Reward Debt together with the other database data.
_Avoid_: Negative chain length, money owed, revoked reward

**Fresh Start**:
An explicit user-requested reset of all Pomotui database data, including reward debt, that establishes a new beginning for the user's Devices. Synchronization must not restore pre-reset Shared Activity from a Device retaining old data. Session duration, reservation default, sound, language, and synchronization settings are preserved, while Tasks, Session History, Action Chains, the Daily Chain Goal, Reward Milestones, claims, debt, local reservations, and local current work start over. A Device that receives the reset discards all work belonging to its previous beginning, including work created offline before it learned about the reset, and informs the user.
_Avoid_: Sync rebuild, Ended Chain deletion, local database repair

**Session Review**:
The user's explicit judgment after a Focus Session reaches its deadline: successful or failed. A successful review appends a Chain Link; a failed review requires a Reflection and explicit confirmation that shows the ending chain length and any unlocked rewards that will become unavailable, then ends the current Action Chain. When the Session had no Current Task, Session Review must assign either a regular Task or the Void Task, and that assignment atomically updates Session History, Task focus time, statistics, and the chain entry; an already attributed Session cannot be reassigned. Once submitted, the judgment and its Session and chain identities are immutable. The Timer Service records the judgment rather than inferring success from duration, Task status, or Session Outcome.
_Avoid_: Automatic success, Session Outcome, Task completion

**Pending Review**:
A Completed Round awaiting Session Review. Older histories may also contain a Pending Review for a Stopped Session. It remains durable when Timer Frontends close or the Timer Service restarts. A Pending Review permits Break Sessions but prevents another Focus Session from starting, so reviewed Focus Sessions cannot be reordered or skipped.
_Avoid_: Pending Session, Paused Session, unfinished Focus Session

**Running Session**:
A Focus Session or Break Session whose remaining time advances with real elapsed time. System suspension counts, while timezone changes and manual wall-clock adjustments do not change its planned duration.
_Avoid_: Active timer

**Paused Session**:
A Session explicitly frozen by the user with its remaining duration preserved. Only an explicit resume makes its time advance again.
_Avoid_: Pending Session, suspended session

**Completed Round**:
A Focus Session that ran for the full duration set when it started. A stopped or skipped Focus Session contributes its actual focus time to history and its Current Task but does not count as a Completed Round.
_Avoid_: Completed task, elapsed session

**Focus Cycle**:
A per-Device sequence of a configurable number of Completed Rounds separated by Short Breaks and followed by a Long Break. The default cycle contains four Completed Rounds; interrupted Focus Sessions and Shared Activity imported from other Devices do not advance it, and completing the Long Break resets it.
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
A Session ended early without advancing the Focus Cycle or changing the next recommended session type. Its actual elapsed time remains in Session History. Actively stopping an unfinished Focus Session ends the Action Chain immediately.
_Avoid_: Skipped Session, completed session

**Skipped Session**:
A Session deliberately passed over to reach the following Pending Session without starting it. Skipping a Focus Session does not add a Completed Round, so that round remains due after the intervening break.
_Avoid_: Stopped Session, completed session

**Session Reminder**:
The once-only desktop notification and optional sound emitted when a Session reaches its deadline. The sound may be disabled, selected from built-ins, or loaded from a local audio file with configurable volume; playback failure does not undo the transition, and Timer Service restarts must not repeat an emitted reminder.
_Avoid_: Timer tick, repeated alarm

## Reservations

Reservation judgment is distinct from Session Review; a Pending Review always refers to an ended Focus Session.

**Focus Reservation**:
The user's per-Device commitment to begin a Focus Session within a chosen time window, defaulting to ten minutes. Any Pending Session permits one reservation when no other reservation or Pending Review exists. Its target is the optional Current Task or another chosen Task; its goal is always Focus. It is optional, permits an earlier start, and shares the existing Action Chain rather than maintaining a separate chain; the configurable default is retained for future reservations while an issued reservation keeps its original deadline.
_Avoid_: Pending Session, scheduled automatic start, reservation chain

**Reservation Review**:
The user's explicit judgment of whether a Focus Reservation was fulfilled by starting on time, including when the user forgot to record the start. Success preserves the Action Chain without adding a Chain Link or reward progress; failure ends it with the existing consequences for unclaimed and claimed rewards, while expiry alone makes no failure judgment. Starting early fulfills the reservation; confirming success begins Focus at confirmation time without backdating and ends an ongoing Break Session. A pending judgment is durable and allows rest but blocks new Focus and reservations. Cancellation requires the same judgment, and failure requires explicit confirmation and permits an optional Reflection. Reservation state remains local; failed judgments join Shared Activity.
_Avoid_: Session Review, automatic failure, successful focus
