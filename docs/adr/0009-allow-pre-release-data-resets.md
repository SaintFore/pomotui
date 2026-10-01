# Allow pre-release data resets

Before Pomotui's first stable release, database and synchronization-format upgrades may require deleting the entire local database and starting again. The project currently has no external users, so choosing a clean compatibility baseline lets synchronization establish stable global identities and lifecycle semantics without carrying speculative legacy migration code. Passing the complete cross-device acceptance work in issue 09 freezes the data contract; upgrades after that point must preserve compatible data or provide an explicit migration.

## Consequences

Pre-release builds must label synchronization as experimental and clearly announce destructive upgrade steps. An incompatible database must fail explicitly instead of being ignored or deleted automatically, and reset requires a deliberate user command. Partial preservation of Tasks, Session History, Action Chains, or Rewards is not offered: an incompatible upgrade resets all local domain data together.
