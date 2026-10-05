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

On 2026-10-05, the final run against integration commit
`45504dc0fc67a27b4537dbb9df9b50d40ce00273` exited **0**, with all nine
scenario assertions passing. Earlier development runs failed on a
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
`document_contract`. The optional `--old-binaries` check executed the actual baseline application at
`93a4ba1` (sync format 5). It rejected the format-7 file, kept its bytes and local
Tasks unchanged, and still started its local timer. This does not test every
intermediate or historical release.

Migration checks run on 2026-10-05 with:

```sh
cargo test -p pomotui-service --test legacy_migration \
  -p pomotui-sync --test document_contract
```

Result: **3 service migration tests and 17 document-contract tests passed**.
The frozen released database scenarios covered independent/restorable migration,
one-time export and first-export rollback/restart. Document scenarios covered
formats 4/5/6, stable record identities, universal genesis and Void attribution.


Final command (scratch logs retained at `/tmp/pomotui-real-acceptance-final`):

```sh
python3 tests/syncthing_acceptance.py \
  --binaries /home/saintfore/code/pomotui/target/debug \
  --old-binaries /tmp/pomotui-old-93a4ba1/target/debug \
  --keep /tmp/pomotui-real-acceptance-final
```

Versions: Syncthing **2.1.5**, linux-amd64, built with Go 1.27.0;
Python **3.14.7**; Rust **1.96.1**. The application package is **0.1.0**,
with sync format **7**, public protocol **5** and persisted state **4**.
The old binary was built from an isolated `git archive 93a4ba1` checkout with a
separate Cargo target directory, preventing stale build artifacts from mixing
protocol versions. To repeat that optional check:

```sh
legacy_root=$(mktemp -d /tmp/pomotui-legacy-XXXXXX)
git archive 93a4ba1 | tar -x -C "$legacy_root"
cargo build --manifest-path "$legacy_root/Cargo.toml" \
  -p pomotui-service -p pomotui-cli --target-dir "$legacy_root/target"
python3 tests/syncthing_acceptance.py --binaries target/debug \
  --old-binaries "$legacy_root/target/debug"
```

| Actual scenario | Result |
|---|---|
| Confirmed disconnect, divergent CLI activity, reconnect creates real Syncthing conflict | PASS |
| Both replicas retain the complete Task/Session/review union; absorbed copy removed; live timers/settings remain independent | PASS |
| Repeated idle sync preserves inode/mtime; subset copy cleans safely; stale main overwrite repairs the union | PASS |
| JSON/checksum/version/immutable-ID errors and unsafe symlink retain evidence with diagnosis; timer commands remain usable | PASS |
| Actual claim, late timestamp-controlled review import, chain revision and debt six on both devices | PASS |
| Independent offline repayments merge to four credits once; late correction gives excess credit one; history/config deletion and restart retain accounting | PASS |
| Fresh Start with other device offline retires old work; reset notice; stale main/copy/restart cannot resurrect history/debt | PASS |
| A single copied main file gives fresh C current activity and beginning while keeping C's settings | PASS |
| Actual old binary rejects version 7 without mutating the exchange or local Tasks, and timer remains usable | PASS |

The old binary's diagnostic was:
`unsupported sync document version 7`. Its old advice to reset pre-release data
is not upgrade guidance for current users: upgrade all devices and preserve
backups instead.

## Final review and verification

The reviewed integration commit `152bdf5669ce2d498dbb16c3a54e71eefc207515`
passed formatting, strict workspace Clippy, the full all-targets/all-features
test suite, debug and release builds, and `tests/e2e.sh`. All nine real scenarios
also passed again with the same old binary and logs retained at
`/tmp/pomotui-real-acceptance-reviewed`.

The Standards review has zero unresolved findings after naming the file identity
fields, sharing conflict filename recognition, and removing unused computation.
The Spec review found and fixed a reward eligibility error: ordinary or late
Chain Breaks must not reopen an unclaimed Ended Chain reward, and a claim must
snapshot its own eligible current chain. Four regression combinations cover an
empty/nonempty current chain after ordinary/late breaks.

The final **P1 Spec finding concerned the cleanup contract**. The original spec
said “No unobserved candidate content may be destroyed.” Cleanup verifies a
quarantined inode before unlinking it, but an already-open writable descriptor
can modify that inode after verification. An executable platform test demonstrates
this limit. The passing Syncthing scenarios establish the documented immutable
copy/atomic-replacement contract, not the stronger unconditional guarantee.

On 2026-10-05, after the two cleanup options and this limitation were explained,
the user replied “按你推荐的来”, accepting the supported atomic-replacement
contract. ADR 0008 records that decision. The finding is resolved by this explicit
specification amendment; no stronger writer exclusion is claimed. Both review
axes have zero unresolved findings under the accepted contract. Implementation
remains on the local integration branch until publishing is separately authorized.
No production profiles or data were modified.
