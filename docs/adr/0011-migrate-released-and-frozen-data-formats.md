# Migrate released and frozen data formats

Pomotui supports versioned migration from the unversioned persisted state with
SQLite schema 3 shipped by v0.1.0 and v0.2.0, and from the persisted-state format
2 and sync-document format 4 frozen when cross-device acceptance issue 09 was
completed. Pre-freeze formats that were never released remain subject to ADR 0009's
explicit reset policy, while unknown formats are rejected without mutation. This
boundary preserves every format users were promised without permanently carrying
forward each experimental representation created while synchronization was under
development.

Migration preserves only information durably and unambiguously represented by its
input; it does not fabricate deleted history or inferred lifecycle events. A new
sync-document version may losslessly upgrade format 4, including replacing implicit
Task attribution with an explicit distinction between the system Void Task and a
regular Task identity. The exact normalized title `Void` is reserved for the
system Void Task, allowing format 4's implicit attribution to be upgraded without
confusing it with an ordinary Task. Older Devices safely reject the future document version and
resume convergence after upgrade rather than requiring a Device registry, upgrade
acknowledgements, or a dual-format exchange document.
