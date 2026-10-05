# Exchange mergeable activity through one provider-neutral file

Cross-device Pomotui will keep each Device's live Current Session and Focus Cycle local while converging Shared Activity through one user-selected sync file transported by any file replication mechanism. The file contains a mergeable set of globally and randomly identified immutable records rather than a database snapshot; each Timer Service unions the file with its locally retained records and atomically replaces it with the merged result. Local databases retain every record they have observed and are durable replicas, while the sync file is a disposable exchange artifact that can be rebuilt from any one local database's known records. Consequently, an older file temporarily overwriting a newer copy can be repaired when a local database retaining the missing records participates again, but rebuilding from one database cannot recover a record that database never observed. Tasks and deletions use versioned records, while Session History and submitted Session Reviews are never discarded. Deterministic ordering resolves mutable state and projects merged Reviews into Action Chains and reward progress; late offline records may revise derived history but never alter a Device's Focus Cycle or revoke a claimed reward. Pomotui maintains no Device registry, acknowledgement protocol, or handoff workflow. SQLite is never synchronized, live and unfinished work remains device-local, and Pomotui has no dependency on Syncthing or any other particular transport provider.

## Accepted conflict-cleanup boundary

On 2026-10-05, the user accepted automatic cleanup under the immutable-copy,
atomic-pathname-replacement contract used by Syncthing. Candidates are removed
only after durable absorption and publication, no-overwrite quarantine, and exact
revalidation. Detected changes, replacement races, unsafe objects, and uncertain
cleanup failures retain recoverable evidence and report diagnostics.

This narrows issue #1's original unconditional cleanup guarantee. A third-party
program that holds a writable descriptor and modifies the quarantined inode
after final verification is outside the supported cleanup contract. Renaming
does not revoke that descriptor; automatic cleanup cannot protect such writes.
The user chose this supported transport boundary over retaining every copy when
exclusive stability cannot be proved. The adapter remains provider-neutral, but
automatic cleanup requires the stated writing behavior.
