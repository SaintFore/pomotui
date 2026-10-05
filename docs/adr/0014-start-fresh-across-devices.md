# Start fresh across devices

Pomotui will offer a distinct explicit command to reset all database data and establish a Fresh Start across Devices. It clears Tasks, Session History, Action Chains, Reward Milestones, reward claims, Reward Debt, and local current work while preserving session duration, sound, language, and synchronization settings. This trades continuity of old work for a complete new beginning without requiring users to configure their Devices again.

An offline Device that later observes the reset must discard all data belonging to its previous beginning, including work it created before learning of the reset, and clearly inform the user. Old retained records must not resurrect the cleared history or debt. This is deliberately stronger than repairing a local database or rebuilding an exchange file.

## Status

The user accepted this scope on 2026-10-05. Ticket #7 implements `pomotui fresh-start --confirm` through the Timer Service. The existing `pomotui reset --all-data --confirm` remains a local repair operation with a database backup; it does not acquire cross-device semantics. No production database was reset during implementation.

The portable format-7 document carries a validated beginning consisting of a generation and UUID. An observed reset's successor increments the generation; concurrent beginnings resolve by the same UUID order on every Device, without consulting clocks. Every business record has immutable beginning membership. A durable selected beginning cannot decrease, so stale documents and offline work from retired beginnings are excluded. Legacy formats 4, 5, and 6 share one genesis and retain normalized Record IDs.

The repository commits the new payload together with clearing business rows, reminder/outbox work, and old idempotency keys. Remote adoption uses the same transaction and persists a separate reset notice. Rebuild and an empty document retain the beginning. Unknown newer formats are rejected without mutation and require an upgrade. This extends ADR 0008 while preserving its one-main-file transport.
