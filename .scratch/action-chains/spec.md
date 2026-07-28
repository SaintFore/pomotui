# Action Chains

Status: ready-for-agent

## Problem Statement

Finishing a timer does not by itself tell the user whether meaningful action occurred. A Focus Session may be interrupted, stopped early, or only partially successful, and only the user can judge its result. Pomotui currently records time and Tasks but does not turn deliberate, reviewed action into a durable chain that can reinforce starting, expose failure honestly, preserve context for reflection, and unlock user-defined rewards.

The user needs this mechanism to remain subordinate to the Focus Session rather than becoming a separate task tree or mind map. Each reviewed success should be one durable link containing what was attempted, the actual time spent, and an optional Reflection. A reviewed failure should end the current chain without erasing it. The workflow must survive restarts, remain consistent across every Timer Frontend, and never infer success or failure automatically.

## Solution

Pomotui will maintain exactly one current Action Chain, initially empty. Every Focus Session that reaches its deadline, or that the user explicitly stops for review, creates a durable Pending Review. Break Sessions remain available while a review is pending, but another Focus Session cannot start until the user submits a successful or failed Session Review.

A successful Session Review appends one Chain Link to the current Action Chain. A failed Session Review requires a Reflection and an explicit destructive confirmation; it appends a non-counting Chain Break, turns the current chain into an immutable Ended Chain, makes its unclaimed rewards unavailable, and immediately creates a new empty current chain. The Session Review records the actual duration and Task snapshot. If the Session had no Current Task, the user assigns a regular Task or the system-owned Void Task during review; Void entries also require a Chain Entry Title.

The Ratatui interface will show a compact Action Chain summary on the Dashboard, a dedicated Chain page for the current chain and rewards, and a dedicated Chain Archive page for Ended Chains. The CLI and versioned service protocol will expose equivalent review and query operations. Reward Milestones will be configurable, unlock once per active chain at their threshold, and be claimed manually.

## User Stories

1. As a user, I want Pomotui to maintain one current Action Chain, so that I always know which sequence I am building.
2. As a new user, I want the current Action Chain to exist at length zero, so that I can begin without setup.
3. As a user, I want one completed Focus Session to await my judgment, so that elapsed time is not mistaken for success.
4. As a user, I want to judge a Session successful, so that it adds one Chain Link.
5. As a user, I want to judge a Session failed, so that the current Action Chain ends honestly.
6. As a user, I want partial work to remain my judgment call, so that Pomotui does not impose an automatic definition of success.
7. As a user, I want a stopped Focus Session to offer “stop and review,” “stop without review,” and cancel, so that an accidental stop or irrelevant interruption does not force a chain result.
8. As a user, I want an early Session stopped for review to retain its exact elapsed time, so that the record remains truthful.
9. As a user, I want a Session stopped without review to remain in Session History but not affect the Action Chain, so that ordinary stopping has predictable semantics.
10. As a CLI user, I want explicit stop-with-review and stop-without-review options, so that scripts do not depend on an interactive choice.
11. As an existing CLI user, I want an unqualified stop command to mean stop without review, so that the new feature does not unexpectedly break a chain.
12. As a user, I want skipped Focus Sessions to retain their existing behavior and never enter review, so that skipping remains distinct from attempting work.
13. As a user, I want Pending Review to survive frontend closure and Timer Service restart, so that I cannot lose an unresolved judgment.
14. As a user, I want to take Break Sessions while a review is pending, so that reflection need not consume recovery time.
15. As a user, I want another Focus Session blocked while a review is pending, so that reviewed Sessions cannot be reordered or silently abandoned.
16. As a user of multiple Timer Frontends, I want every frontend to observe the same Pending Review, so that review state cannot diverge.
17. As a user, I want a successful review to store the Task identity and its title at review time, so that later Task changes do not rewrite history.
18. As a user, I want a Chain Link to store the exact actual duration, so that early and completed Sessions are represented accurately.
19. As a user, I want whole-minute durations displayed as values such as `50m`, so that the chain stays compact.
20. As a user, I want durations with remaining seconds displayed as values such as `17m 32s`, so that the record does not round away real effort.
21. As a user, I want to add an optional Reflection to a successful Chain Link, so that I can record what I actually accomplished.
22. As a user, I want failure to require a Reflection, so that an ended chain retains useful review context.
23. As a Simplified Chinese user, I want Reflection labelled `复盘`, so that the concept is clearer than a generic comment.
24. As an English user, I want the same field labelled `Reflection`, so that terminology is consistent.
25. As a user, I want to revise a Reflection later, so that retrospective understanding can improve without changing the judgment.
26. As a user, I want Session Review judgment, source Session, and chain membership to become immutable after submission, so that history remains trustworthy.
27. As a user reviewing a Session without a Task, I want to assign an existing or new regular Task, so that its focus time and statistics are attributed correctly.
28. As a user reviewing a Session without a Task, I want to choose the Void Task instead, so that I can preserve work that lacks a reusable Task.
29. As a user, I want `Void` to be one system-owned Task whose title is never translated, so that its identity is stable in every interface.
30. As a user, I want the Void Task to be visible in focus-time statistics, so that unattributed work is not hidden.
31. As a user, I want the Void Task protected from rename and deletion, so that reviewed records cannot lose their attribution target.
32. As a user, I want a Void review to require a Chain Entry Title, so that `Void` does not erase what I attempted.
33. As a user, I want to correct a Void Chain Entry Title later, so that wording can improve without changing chain history.
34. As a user, I want an ordinary Task that happens to be titled `Void` to remain an ordinary Task, so that system identity is not inferred from display text.
35. As a user, I want task assignment during review to update Session History, Task focus time, statistics, and the chain atomically, so that partial updates cannot corrupt totals.
36. As a user, I want an already attributed reviewed Session to reject reassignment, so that time cannot move between Tasks after judgment.
37. As a user, I want a failed review to show the current chain length before confirmation, so that I understand what will end.
38. As a user, I want a failed review to show unlocked unclaimed rewards that will become unavailable, so that the consequence is explicit.
39. As a user, I want canceling the failure confirmation to leave the Pending Review unchanged, so that a mistaken keypress is harmless.
40. As a user, I want a confirmed failure recorded as a terminal Chain Break, so that the failed attempt is retained without increasing chain length.
41. As a user, I want a Chain Break to retain Task snapshot, actual duration, Reflection, and any required Void title, so that failure can be reviewed in context.
42. As a user, I want failure on a zero-length current chain to produce a valid zero-link Ended Chain, so that even the first failed attempt is not erased.
43. As a user, I want a new empty current Action Chain created immediately after failure, so that recovery can begin without manual reset.
44. As a user, I want Ended Chains to be immutable and undeletable, so that I cannot conceal past failures impulsively.
45. As a user, I want Ended Chains to be impossible to extend, reactivate, or merge, so that chain meaning stays unambiguous.
46. As a user, I want Task deletion or rename not to erase or rewrite Chain Links and Chain Breaks, so that archived snapshots remain durable.
47. As a user, I want midnight, inactivity, shutdown, and elapsed days not to break my chain, so that only my explicit review judgment matters.
48. As a user, I want Break Sessions, claimed rewards, and configuration changes not to break my chain, so that the mechanism reinforces action rather than arbitrary streak rules.
49. As a user, I want to configure a Reward Milestone with a chain length and name, so that longer chains can represent concrete promises.
50. As a user, I want an optional budget on a Reward Milestone, so that I can plan rewards such as food, a day off, or hardware.
51. As a user, I want a milestone to unlock once when the current chain reaches its threshold, so that repeated views or restarts do not duplicate it.
52. As a user, I want adding a milestone below the current length to unlock it immediately, so that new reward plans apply to my active progress.
53. As a user, I want lowering an active milestone threshold below the current length to unlock it immediately, so that configuration behaves consistently.
54. As a user, I want configuration changes not to alter Ended Chains, so that historical reward records remain truthful.
55. As a user, I want an unlocked reward to snapshot its name, threshold, and budget, so that later edits do not rewrite what I earned.
56. As a user, I want to claim a reward manually, so that Pomotui records my decision without pretending to fulfill it.
57. As a user, I want claimed rewards preserved when a chain ends, so that earned and used rewards remain part of its history.
58. As a user, I want unlocked but unclaimed rewards marked unavailable when a chain ends, so that they cannot be claimed from a broken chain.
59. As a user, I want deleting reward configuration to affect only rewards not already unlocked, so that reward history cannot disappear.
60. As a user, I want Pomotui never to purchase, pay for, or automatically perform a reward, so that external consequences remain under my control.
61. As a TUI user, I want the Dashboard to retain its Task list, so that Action Chains do not replace task management.
62. As a TUI user, I want a compact Dashboard chain card with length, recent links, next reward, and Pending Review state, so that important status is visible at a glance.
63. As a TUI user, I want a dedicated Chain page, so that the full current chain and its rewards have enough space.
64. As a TUI user, I want a dedicated Chain Archive page, so that Ended Chains are separate from Session History.
65. As a TUI user, I want Session History to remain a separate view, so that timer events and chain judgments are not conflated.
66. As a TUI user, I want the review, Chain, and Chain Archive views to work in narrow terminals, so that the feature remains usable in real terminal layouts.
67. As a TUI user, I want normal views to avoid displaying internal IDs, so that the interface emphasizes meaning rather than storage details.
68. As a CLI or debugging user, I want detailed and JSON output to expose stable IDs, so that records can be inspected and automated safely.
69. As a user, I want service errors for invalid review actions to be stable and understandable, so that every Timer Frontend can explain what must be corrected.
70. As a user, I want repeated submission of the same mutation to be idempotent, so that reconnects and retries cannot duplicate Chain Links, Chain Breaks, rewards, or claims.

## Implementation Decisions

- The Timer Service remains the sole owner of Pending Review, Session Review, Action Chain, Chain Break, and reward transitions. Timer Frontends issue commands and render authoritative responses; they do not infer or advance chain state locally.
- Domain data will be stored in SQLite, including the current and ended chains, entries, Pending Review, reward configuration, unlock snapshots, and claims. Reward configuration belongs in the domain store rather than TOML because unlocking and review must share transactional consistency.
- The schema migration will create a durable singleton Void Task distinguished by system identity or an explicit system-owned marker, never by matching the title string. User-created Tasks titled `Void` remain ordinary Tasks.
- Storage constraints will enforce exactly one current Action Chain, at most one Pending Review, one review per source Focus Session, one chain entry per submitted review, and at most one unlock for each milestone in an Action Chain.
- Completed Focus transition and “stop for review” will durably create Pending Review. Stop without review and Skip will use their existing Session History semantics without creating a review or chain entry.
- Starting a Focus Session while Pending Review exists will return a stable domain error. Starting and completing Break Sessions remains permitted and must not clear or mutate Pending Review.
- The review mutation is one atomic transaction. It validates the judgment and required fields; assigns a regular or Void Task when necessary; updates Session History, Task focus time, and statistics; records the Chain Link or Chain Break; unlocks or invalidates rewards; clears Pending Review; and, on failure, ends the old chain and creates the new current chain.
- A successful review accepts an optional Reflection. A failed review requires a non-empty Reflection. A review using Void requires a non-empty Chain Entry Title regardless of judgment.
- The command that submits a failed review is the service-side confirmation boundary. The TUI must present chain length and rewards at risk immediately before issuing it; canceling that presentation sends no mutation.
- Session Review judgment, source Session identity, Action Chain identity, Task attribution after submission, entry kind, actual duration, and Task title snapshot are immutable. Only Reflection and a Void entry's Chain Entry Title can be edited later.
- Chain Links and Chain Breaks retain enough snapshots to survive Task rename or deletion. Deleting Session History after review must not cascade into or alter Action Chain history; a Pending Review cannot be deleted through Session History.
- Ended Chains, their entries, reward snapshots, and claims cannot be deleted, extended, reactivated, or merged through any interface.
- Reward Milestones are configurable domain entities with a positive chain-length threshold, a user-visible name, and optional budget. Configuration identity is separate from per-chain unlock snapshots.
- Reaching a milestone, adding one at or below current length, or lowering its threshold to or below current length unlocks it once for the current chain. These operations never recalculate Ended Chains.
- Unlock records snapshot milestone name, threshold, and budget. Configuration deletion affects only configuration not already represented by an unlock; unlocked and claimed historical records remain.
- Claiming is an explicit idempotent mutation. Pomotui records claim state and time only and performs no external purchase, payment, notification to a merchant, or reward automation.
- When failure ends a chain, claimed rewards remain claimed and unlocked unclaimed rewards become permanently unavailable. This change occurs in the same transaction as the Chain Break and chain rollover.
- Actual duration is stored as exact seconds and rendered as a single value. Whole minutes use `<minutes>m`; values with a remainder use `<minutes>m <seconds>s`. Planned duration is available in Session History but not repeated as a planned/actual pair in chain displays.
- The versioned Timer Service protocol will be extended and its version advanced for review mutations, stop-review intent, chain summary/detail/archive queries, entry edits, Reward Milestone management, and reward claims.
- Mutation requests will carry the repository's idempotency mechanism so transport retries cannot duplicate durable effects. Repeating an already completed mutation returns the authoritative existing result or a stable conflict rather than applying it again.
- The ordinary service snapshot will expose only compact Pending Review and current-chain summary data needed by the Dashboard. Full entries, archives, and reward history will use bounded detail/list queries so a growing archive does not inflate every status response.
- The CLI will expose explicit review operations, `stop --review`, and `stop --no-review`. Legacy no-argument stop maps to no-review. Human output favors titles and formatted duration; JSON/detail output includes stable internal identities and exact seconds.
- The Ratatui navigation model will add Chain and Chain Archive views while retaining Dashboard, Today, and History. The Dashboard Task list remains intact and gains a compact current-chain card.
- The review UI will collect a Task only when the Session lacks attribution, offer the singleton Void Task, require a Chain Entry Title for Void, collect optional or required Reflection according to judgment, and show a separate failure confirmation.
- Normal TUI list and detail presentation will not display database IDs. Stable IDs remain available in protocol responses and detailed or JSON CLI output for diagnosis and automation.
- User-visible strings will use the established localization system. `Void` is invariant across languages; `Reflection` is rendered as `复盘` in Simplified Chinese and `Reflection` in English.
- Action Chain state has no clock-based expiry and no automatic failure transition. Timer Service startup, reconciliation, wall-clock changes, midnight, inactivity, Break Sessions, and reward operations cannot end a chain.
- Existing Current Session progression, reminder outbox, degraded durability, and transport recovery guarantees remain in force. Review persistence failures must be reported through the existing durability/error boundary rather than silently accepting an in-memory review.

## Testing Decisions

- Tests will assert externally observable domain behavior rather than table layout, private helper calls, or widget internals. Schema and migration tests are the exception because their external responsibility is durable compatibility and constraint enforcement.
- Domain scenario tests will cover the review state machine at its highest inexpensive seam: completed and deliberately reviewable stopped Focus Sessions create Pending Review; Break is allowed; Focus is blocked; success appends; failure archives and rolls over; stop without review and Skip do neither.
- Domain scenarios will prove that only explicit Session Review changes an Action Chain. Time passage, midnight, restart, Task completion, Task deletion, Break Sessions, and reward claims must not create or break links.
- Service integration tests with real SQLite will be the primary consistency seam. They will verify that task assignment, focus-time totals, Session History, chain entry, reward changes, Pending Review clearing, and chain rollover commit together or not at all.
- Service integration tests will cover successful review with an existing Task, assignment of a new or existing Task to an unattributed Session, Void assignment, required Void title, optional success Reflection, and required failure Reflection.
- Service integration tests will verify immutable judgment and attribution, permitted Reflection and Void-title edits, preservation across Task rename/deletion, and rejection of edits to immutable entry fields.
- Service integration tests will cover failure at chain length zero and nonzero, terminal Chain Break data, immediate creation of one empty current chain, preservation of claimed rewards, and invalidation of unlocked unclaimed rewards.
- Reward integration tests will cover threshold crossing, exactly-once unlock, restart safety, retroactive unlock on add or threshold decrease for the active chain only, immutable unlock snapshots, configuration deletion, idempotent claim, and refusal to claim unavailable rewards.
- Restart tests will verify that Pending Review, current chain, Ended Chains, Void Task identity, unlocks, and claims survive Timer Service restart without duplication.
- Migration tests will upgrade representative existing schema data, create exactly one system Void Task and one empty current Action Chain, preserve user Tasks including any ordinary Task titled `Void`, and remain safe when migration is retried.
- Protocol request/response tests will cover the version bump, all new commands and bounded queries, compact snapshot data, exact-second values, stable IDs, stable validation errors, and idempotent retry behavior.
- CLI parsing tests will cover `stop --review`, `stop --no-review`, legacy stop, review inputs, entry edits, reward management, human duration output, and JSON output containing stable IDs and exact values.
- Ratatui `TestBackend` tests will cover the Dashboard chain card, retained Task list, current Chain page, Chain Archive page, Pending Review flow, three-way Stop choice, failed-review warning, cancellation, reward claim, and navigation.
- TUI tests will exercise both wide and narrow terminal sizes and English and Simplified Chinese labels, including invariant `Void` and localized `Reflection`/`复盘`.
- Formatting tests will prove a single duration value is shown as `50m` for exact minutes and `17m 32s` when seconds remain, including an early stopped Session.
- End-to-end tests across a real Timer Service process will cover a completed Focus Session through review, service restart with Pending Review, Focus rejection and Break allowance, successful chain growth, and failed review producing an Ended Chain plus a new current chain.
- Regression tests will retain existing Focus Cycle, Session History, Task totals, reminder, degraded durability, reconnect, and stop/skip behavior. New assertions will ensure reviewed attribution updates those existing views exactly once.
- The completed implementation must pass formatting, linting, the full workspace test suite, and the repository's end-to-end test suite.

## Out of Scope

- Markmap, WebView, browser rendering, or any non-Ratatui chain visualization.
- A task tree, graph editor, mind map, dependency graph, or national-policy-style rule tree.
- Modeling the physical “sacred seat” trigger, sensors, conditioning, or environmental automation.
- A separate interruption or disturbance domain model beyond Pause, Stop, Session Review, and Reflection.
- Automatically deciding success or failure from duration, percentage completed, Task status, input activity, or any other heuristic.
- Automatically ending a chain because of elapsed time, date boundaries, shutdown, inactivity, missed days, Break Sessions, or reward behavior.
- Purchasing, paying for, scheduling, enforcing, or externally fulfilling rewards.
- Displaying internal database IDs in normal TUI views.
- Deleting, restoring, merging, or reactivating Ended Chains.
- Reassigning an already attributed reviewed Session or changing a submitted review judgment.
- Changing the existing Focus Cycle definition or making stopped Sessions count as Completed Rounds.

## Further Notes

- This spec uses the domain language and decision recorded in ADR 0006: the user's explicit review is the only authority for Action Chain success and failure.
- Session History and Chain Archive are intentionally different views. Session History records timer outcomes; Chain Archive records immutable sequences of reviewed action ending in a Chain Break.
- The design records an early stop faithfully rather than treating it as a full planned duration. Whether that attempt succeeded remains entirely the user's decision.
- The default no-Task review path is Void, but the review UI should make assigning a meaningful regular Task easy before falling back to Void.
- The test seams follow existing repository practice: domain scenarios for rules, Timer Service plus real SQLite for transactional behavior, protocol tests for frontend contracts, Ratatui `TestBackend` for UI behavior, and a small end-to-end layer for restart and cross-process guarantees.
