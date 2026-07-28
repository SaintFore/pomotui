# Timer Service trust-boundary hardening

Status: ready-for-agent

## Problem Statement

Pomotui reliably owns one Current Session in a persistent Timer Service, but
several trust boundaries can still undermine that reliability without producing
an immediate, actionable failure.

A local client can open the Unix socket, send an incomplete or oversized
newline-delimited request, and prevent other Timer Frontends from reaching the
Timer Service. Task titles can contain unbounded or terminal-hostile text that
is then stored in Tasks, copied into Session History, and rendered by multiple
Timer Frontends. When timer progression changes in memory but SQLite persistence
fails, the visible Current Session can temporarily disagree with the durable
state recovered after a restart. Finally, a completed deadline is durably
claimed before its desktop notification and sound are attempted, so an external
effect failure is logged but cannot be retried or inspected through a Timer
Frontend.

From the user's perspective, these failures are particularly damaging because
Pomotui may look healthy until a frontend hangs, a terminal is visually
corrupted, a restart rolls back an apparent transition, or a Session Reminder
silently never arrives.

## Solution

Harden the existing Timer Service and versioned Unix socket protocol without
changing their architectural roles.

The Timer Service will isolate stalled socket clients, bound every request
frame, and expose stable errors for rejected input. Task titles will become a
validated domain value with deterministic Unicode, terminal-safety, byte-length,
and display-width rules. A durable-state write failure will place the Timer
Service into an explicit degraded mode that prevents further state-changing
commands until persistence has recovered or the process has been restarted and
recovered from durable state.

Completed deadlines will atomically create durable Session Reminder outbox
entries. A Timer Service-owned dispatcher will attempt notification and sound
delivery independently, acknowledge successful effects, retry failed effects
within strict limits, and retain a visible terminal failure when delivery is
exhausted. Pomotui will continue to guarantee once-only Session progression;
external desktop effects will have an explicitly documented bounded
at-least-once delivery guarantee because they cannot participate in the SQLite
transaction.

Timer Frontends will be able to distinguish normal operation, degraded durable
state, pending effect delivery, retrying effects, and exhausted effects through
machine-readable protocol data and clear human-readable diagnostics.

## User Stories

1. As a TUI user, I want one stalled local socket client not to freeze my TUI,
   so that I can continue observing the Current Session.
2. As a CLI user, I want a malformed client connection not to block valid
   commands, so that automation remains dependable.
3. As a Waybar user, I want polling to continue when another Timer Frontend
   sends an incomplete request, so that the displayed remaining time stays
   current.
4. As a user, I want oversized socket requests to be rejected before they
   consume unbounded memory, so that the Timer Service remains responsive.
5. As a script author, I want rejected oversized and malformed requests to have
   stable machine-readable error codes, so that my script can handle them
   predictably.
6. As a maintainer, I want the number of simultaneously handled socket
   connections to be bounded, so that isolation cannot become a different
   resource-exhaustion path.
7. As a user creating a Task, I want unsafe terminal control sequences to be
   rejected, so that viewing the Task cannot alter or corrupt my terminal.
8. As a user renaming a Task, I want the same title rules as Task creation, so
   that every stored Task obeys one predictable policy.
9. As a CLI user starting Focus by title, I want title validation to match Task
   creation and rename, so that entry points do not disagree.
10. As a user, I want excessively large Task titles rejected with an actionable
    explanation, so that a title cannot make every snapshot or view needlessly
    large.
11. As a terminal user, I want Task titles bounded by display width as well as
    UTF-8 byte length, so that wide Unicode characters cannot defeat the layout
    limit.
12. As a user, I want bidirectional overrides and dangerous invisible
    separators rejected, so that a displayed title cannot misrepresent its
    stored order or identity.
13. As a multilingual user, I want ordinary English, Simplified Chinese, emoji,
    and combining text handled by documented deterministic rules, so that
    terminal safety does not unnecessarily restrict legitimate titles.
14. As a user with existing data, I want previously stored titles to remain
    recoverable and auditable, so that an upgrade does not silently rewrite or
    discard my Tasks or Session History.
15. As a JSON CLI consumer, I want title validation failures to identify the
    invalid field and rule without echoing dangerous raw text, so that errors
    are safe and machine-readable.
16. As a user, I want Pomotui to report when the Current Session can no longer
    be persisted, so that I do not mistake volatile state for durable state.
17. As a user, I want state-changing commands rejected while durable state is
    degraded, so that the visible state cannot drift progressively farther from
    what a restart will recover.
18. As a user, I want read-only status and diagnostic commands to remain
    available during degraded operation, so that I can understand the failure.
19. As a user, I want the degraded response to include the last successful
    durable commit and a concise recovery action, so that I can decide whether
    to fix storage or restart the Timer Service.
20. As a user, I want a deadline transition and its Session History record to
    remain once-only even when reminder delivery fails, so that an external
    effect never corrupts timer progression.
21. As a user, I want a completed deadline to create durable notification and
    sound work before either effect is attempted, so that a process restart
    cannot silently erase an unattempted Session Reminder.
22. As a user, I want notification and sound delivery tracked independently, so
    that one broken effect does not suppress the other.
23. As a user, I want transient effect failures retried after restart, so that a
    temporarily unavailable desktop component does not permanently lose the
    Session Reminder.
24. As a user, I want retries to have strict attempt and age limits, so that a
    broken effect does not retry forever or surprise me much later.
25. As a user, I want exhausted Session Reminder effects visible in diagnostics,
    so that delivery failure is not hidden in transient stderr output.
26. As a user, I want retry state to distinguish pending, retrying, delivered,
    and exhausted effects, so that the Timer Service's health is understandable.
27. As a user, I want duplicate external effects to be minimized across crashes,
    so that recovery is useful without becoming noisy.
28. As a maintainer, I want the unavoidable possibility of a duplicate desktop
    effect documented, so that Pomotui does not claim impossible exactly-once
    semantics.
29. As a maintainer, I want crash-boundary tests around durable commits and
    effect acknowledgements, so that future changes preserve the reliability
    contract.
30. As a maintainer, I want all new behavior expressed through existing domain,
    protocol, service, and repository boundaries, so that Timer Frontends remain
    display and control clients rather than owners of progression.

## Implementation Decisions

- ADR-0001 remains in force: the persistent Timer Service is the sole owner of
  Current Session progression.
- ADR-0004 remains in force: Timer Frontends use a versioned request/response
  protocol over a Unix domain socket. This work hardens that transport rather
  than replacing it with D-Bus, event sourcing, or a subscription protocol.
- Preserve one request and one response per connection. Long-lived
  subscriptions and multiplexing are not required.
- The socket accept loop must not synchronously wait for an individual client
  to finish sending its request. Connections may be handled by bounded worker
  threads or another bounded concurrency mechanism that does not require a new
  asynchronous runtime solely for this feature.
- A connection has a finite read deadline, a finite maximum request-frame size,
  and exactly one newline-delimited request. Timeout and frame limits must be
  constants with documented rationale and tests at their boundaries.
- Concurrency must be capped. When capacity is exhausted, behavior must be
  deterministic: either reject/close new connections promptly or wait only for
  a bounded period.
- Parsing and request validation happen before acquiring exclusive access to
  mutable Timer Service state. The state lock must never be held while waiting
  for socket input or output.
- Extend protocol errors with stable variants for an oversized request, an
  invalid Task title, unavailable durable writes, and any other condition that
  Timer Frontends must distinguish programmatically. Human-readable messages
  remain diagnostic but are not the API contract.
- Introduce a domain-owned Task title value. Tasks and title snapshots created
  after this change originate from that validated value rather than arbitrary
  strings.
- The Task title policy must specify a Unicode normalization form, maximum UTF-8
  byte length, maximum terminal display width, and the exact rejected character
  classes. At minimum it rejects C0/C1 controls, escape characters,
  bidirectional override/isolate controls, and selected invisible separators
  that can disguise identity.
- Empty or whitespace-only titles remain invalid. Non-unique titles remain
  valid, and ambiguous title lookup continues to require an explicit Task
  identity.
- Validation is shared by Task creation, rename, start-by-title, durable-state
  restoration, and future Task entry points.
- Protocol errors must not reproduce unsafe raw title content. They may include
  safe measurements, the failed rule, and a sanitized preview.
- Existing durable titles are not silently normalized or rewritten during
  ordinary startup. Restoration must either accept legacy-safe titles or expose
  a diagnostic that lets the user export and repair incompatible data without
  losing Session History.
- The Timer Service maintains explicit durable-health state. A failed durable
  write records the error and last successful commit observation.
- After a durable write fails, subsequent state-changing commands are rejected
  before changing domain state. Read-only requests remain available.
- The transition that first encounters a persistence failure must not continue
  to present an unqualified healthy state. If the domain transition cannot be
  rolled back safely, the Timer Service enters degraded mode immediately and
  exposes that the in-memory state is not guaranteed durable.
- Recovery from degraded mode does not silently replay arbitrary rejected
  commands. A bounded explicit health probe may clear the condition after a
  successful durable write, or process restart may recover from the last
  durable state.
- Add durable outbox records for Session Reminder effects. Each record has a
  deterministic completion identity, an effect kind, delivery state, attempt
  count, next-attempt time, last safe diagnostic, creation time, and optional
  acknowledgement time.
- Creation of outbox records, Session History, Focus Cycle advancement, Current
  Session transition, and the once-only completion claim occur in one SQLite
  transaction.
- Notification and sound are separate outbox effects. Disabling an effect
  prevents creation of work for that effect.
- The Timer Service owns the dispatcher. Timer Frontends do not claim effects,
  and desktop effects remain platform adapters.
- Acknowledgement is written only after an adapter reports success. Failed
  attempts use bounded exponential backoff with jitter and both an attempt limit
  and an age limit.
- Exhausted work is retained as a terminal diagnostic state rather than deleted
  or retried indefinitely.
- Timer progression remains exactly once. External notification and sound
  delivery are documented as bounded at-least-once: a crash after external
  success but before SQLite acknowledgement may repeat an effect.
- Status/diagnostic protocol data includes durable-health state and aggregate
  outbox counts without placing the full outbox journal in every ordinary
  Snapshot.
- Database schema evolution uses the existing versioned migration mechanism and
  must reject a newer unknown schema without recreating user data.

## Testing Decisions

- Tests assert externally observable behavior and durable invariants rather than
  worker implementation, thread count, private structs, SQL statement shape, or
  retry-loop internals.
- The primary socket seam is the existing process/protocol boundary: run the
  real listener and prove that a partial first connection cannot prevent a
  second valid client from receiving a response within a bounded deadline.
- Protocol tests cover a request exactly at the frame limit, one byte above it,
  missing newline until timeout, malformed JSON, multiple simultaneous valid
  clients, and overload at the configured concurrency limit.
- The existing simultaneous-client and polling-reconnect protocol tests are
  prior art. Extend that suite rather than creating a transport-specific mock
  layer.
- Task title rules are tested at the domain API, the highest seam shared by all
  entry points. Use table-driven cases for ASCII, Simplified Chinese, emoji,
  combining marks, normalization equivalents, byte boundaries, display-width
  boundaries, control characters, escape sequences, Bidi controls, and
  whitespace-only input.
- Service/protocol tests verify that create, rename, and start-by-title return
  the same stable validation error and never persist rejected content.
- Restoration tests use a real in-memory SQLite repository and representative
  legacy payloads to prove that upgrades do not silently mutate or discard
  existing Tasks and Session History.
- Durable degradation is tested through the Service/repository seam using a
  controllably failing repository boundary. One injected failure must prove:
  the failure is visible, later mutations are rejected before state changes,
  reads remain available, and restart recovers the last committed state.
- If introducing a controllably failing repository requires a new seam, add one
  service-level persistence port rather than separate seams for each write
  operation. Production SQLite and test failure injection both implement that
  single boundary.
- Session Reminder delivery is tested at the Service plus real in-memory
  repository seam, with fake notification and sound adapters that record
  externally requested effects.
- Fault injection covers crashes or failures before the completion transaction,
  after the completion transaction but before dispatch, after external success
  but before acknowledgement, after acknowledgement, during restart, and after
  retry exhaustion.
- Every fault boundary must preserve these observable invariants: a completed
  Session enters Session History once, a completed Focus Session advances the
  Focus Cycle once, a Pending Session is revealed but not started, and no
  pending effect is silently lost.
- Notification and sound isolation tests prove that failure of one effect does
  not prevent attempting or acknowledging the other.
- Retry tests use a controllable clock; they do not sleep in wall-clock time.
- Existing repository atomicity, reminder-claim, external-effect-isolation,
  deadline-ticker, restart-recovery, CLI JSON, and process E2E tests are prior
  art and remain part of the acceptance evidence.
- The final acceptance run includes formatting, linting with warnings denied,
  all workspace tests with all features, and the existing process E2E suite.

## Out of Scope

- Replacing the persistent Timer Service with leases, frontend ownership, or
  another authority model.
- Replacing the Unix socket request/response protocol with D-Bus, event
  sourcing, long-lived subscriptions, or multiplexed streams.
- Splitting Snapshot into revisioned resources or adding cursor-based
  incremental synchronization.
- Optimizing Session History through cold storage, monthly partitions, or
  pre-aggregated daily buckets without measured performance evidence.
- Cross-device synchronization, accounts, immutable time credentials, or
  conflict resolution.
- User-configurable Task title safety policies. The policy is a stable product
  invariant, not per-installation presentation configuration.
- Exactly-once delivery claims for desktop notification or sound effects.
- Frontend-owned effect inboxes or competition among Timer Frontends to deliver
  Session Reminders.
- New Task metadata such as next actions, per-Task duration presets, projects,
  due dates, or interruption maps.
- A composable TUI workspace or adaptive command-palette ranking.
- Automatic repair or destructive replacement of a damaged SQLite database.
- Snapshot performance work unless measurement performed during implementation
  reveals a regression caused by this feature.

## Further Notes

- The domain glossary remains authoritative. Use Timer Service, Timer Frontend,
  Current Session, Task, Session History, Focus Cycle, Pending Session, and
  Session Reminder exactly as defined there.
- This spec intentionally combines four small trust boundaries because they
  share one user promise: the Timer Service must remain responsive, represent
  durable truth honestly, render stored user text safely, and make external
  delivery failure inspectable.
- The most valuable first implementation slice is the socket
  partial-request/oversized-frame test and bounded connection handling. It is
  independently shippable and does not require a database migration.
- The Task title value is the second independently shippable slice.
- Durable degradation and the Session Reminder outbox should be implemented
  together only where their transaction and health-state semantics overlap;
  they may otherwise be delivered as successive commits under this spec.
- The outbox cannot make desktop effects transactional. Its purpose is to
  replace hidden loss with durable, bounded, observable delivery attempts.

