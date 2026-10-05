# Isolated real Syncthing acceptance

This opt-in test uses two real Syncthing processes, two Timer Services and the
actual CLI. A third Timer Service checks portability of the single main file.
Every database, configuration, socket, key and exchange directory is inside a new
temporary scratch directory. Syncthing discovery, relays, NAT, browser launch,
usage reporting and automatic upgrades are disabled; peers connect on localhost.
No production service, profile or folder is opened or changed.

## Reproduce

Install Python 3 and Syncthing, then build the current service and CLI:

```sh
cargo build -p pomotui-service -p pomotui-cli
python3 tests/syncthing_acceptance.py --binaries target/debug
```

To retain logs, specify a **new** scratch directory:

```sh
python3 tests/syncthing_acceptance.py --binaries target/debug \
  --keep /tmp/pomotui-syncthing-evidence
```

The runner exits nonzero on any failed assertion or timed-out barrier. It prints
`PASS:` only after verifying a scenario. It stops every child in `finally`; the
default scratch directory is removed on exit. `--keep` retains service logs,
Syncthing logs and synthetic activity for diagnosis. Listening on localhost and
Unix sockets must be permitted by the execution environment.

## Barriers and observations

The transport driver polls Syncthing's authenticated REST API for startup and
confirmed disconnection. It requests scans on both folders. It pauses both peer
connections before divergent writes, disables application sync briefly so a real
conflict can be observed, then reconnects and requires an actual
`sync-conflict` artifact. The applications subsequently import it and publish the
complete union. Completion requires byte-identical main documents, matching
service retained-record counts, expected public projections and disappearance of
absorbed siblings. Service workers are polled through `sync status`; bounded waits
fail rather than assuming a fixed sleep proves convergence.

Synthetic facts use controlled review/session timestamps, independent UUIDs and
the public document format/checksum. They enter through the same real file import
as transported activity. The driver does not calculate chains, rewards, debt or
reset winners; assertions compare literal worked examples against real service
CLI/protocol results. Ended Chain deletion uses the public protocol also used by
the TUI. All other business operations use the CLI.

## Recorded run

See the final execution evidence below. Earlier development runs failed on a
real defect: adopting a newer nonempty beginning containing a System Void review
attempted review projection before reestablishing the Void invariant. Those runs
are not counted as passes; the regression was fixed and the final runner also
transports a reviewed Session under the newer beginning.

## Boundaries

The real acceptance is the transport supplement to the deterministic two-directory
process and filesystem fault suites. It does not replace their crash, cleanup
race, multiple-milestone, inverse-correction or concurrent-reset permutation
coverage. It does not measure public-network latency or test arbitrary providers,
production profiles, Windows/macOS, or noncooperating in-place writers. Supported
cleanup assumes immutable conflict copies and atomic pathname replacement.

Legacy migrations are exercised with frozen released fixtures by
`legacy_migration`, and format 4/5/6 identity/genesis migration by
`document_contract`. This run does not execute an archived older application
binary; newer-format refusal is current validation-test evidence, not a claim
that an old installed environment was run successfully.

Migration checks run on 2026-10-05 with:

```sh
cargo test -p pomotui-service --test legacy_migration \
  -p pomotui-sync --test document_contract
```

Result: **3 service migration tests and 17 document-contract tests passed**.
The frozen released database scenarios covered independent/restorable migration,
one-time export and first-export rollback/restart. Document scenarios covered
formats 4/5/6, stable record identities, universal genesis and Void attribution.
