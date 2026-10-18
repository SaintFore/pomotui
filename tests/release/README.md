# Real Arch package acceptance

This experiment is a required feasibility gate for issue #20, implementing ADR
0016. It publishes no release or AUR package and uses no deployment credentials.
The experimental workflow runs on the feature branch and retains diagnostics
on failure. A failed or timed-out scenario is a failed gate.

`acceptance.sh OLD_PACKAGE CANDIDATE_PACKAGE OUTPUT_DIRECTORY` is the shared
acceptance boundary. Run it as root **only inside a fresh disposable booted Arch
system**, with pacman, systemd, a real user manager, dbus and runtime dependencies.
It creates a disposable `pomotui-acceptance` user and state. The packages must
have the real `pomotui` package name and different package and executable
identities. Subsequent domain-state scenarios should extend this entry point,
using the public CLI/socket and installed/running artifact and unit identity.

The initial scenario installs the candidate cleanly and starts it using the
documented socket enable path. It then installs the recorded previous stable
package into fresh user state and upgrades it with pacman. After upgrade it
requires a different running process, candidate executable SHA-256, packaged
unit path and healthy public JSON status, without manually restarting it.

`hosted.sh OUTPUT_DIRECTORY` constructs packages outside the booted system,
exports a real Arch rootfs, probes the host and boots systemd-nspawn. The
packaging experiment builds source tag v0.3.0 and the recorded candidate commit
using the existing Arch recipe. Because the current candidate has the same
application version as the previous stable tag, it embeds an explicit
`0.3.0-acceptance.<count>.<commit>` Cargo version and a greater experimental
Arch package version. This is a local experimental artifact, never a new stable
version. Source commits, image digest, package hashes, unit properties, process
identities, journals, pacman logs, runtime and resource observations are retained.

The host shares its Linux kernel with nspawn. Actual hosted behavior, rather
than documentation alone, must establish that this arrangement can run the real
package hooks and user manager. This experiment does not claim full domain-state
or failed-service acceptance, which later issues add at the same boundary.

## Hosted feasibility evidence

[Run 39108327354](https://github.com/SaintFore/pomotui/actions/runs/39108327354)
passed on 2026-10-18 at candidate source
`307943f2d7d5f8b0a19ce50067578d9a33999180`. The install/upgrade step ran from
07:37:09 to 07:41:13 UTC (about 4m04s including both cold package builds and
rootfs preparation). The booted Arch acceptance scenario took 7 seconds. The
systemd transient scenario reported 19.7 MB peak memory and no swap; the host had
4 CPUs and 15,988 MB RAM. Host/guest memory and disk snapshots, complete boot,
build, pacman and journal logs, packages and JSON statuses are in the run's
`arch-acceptance-307943f2d7d5f8b0a19ce50067578d9a33999180` artifact.

The observed old source was v0.3.0, peeled to commit
`ad9ce4cf4c43997b807485d3a590563ad66c68f1` (the run's `sources.txt` recorded
annotated tag object `30908419a26a4b7fa8172d182e1a5d82e03de819`),
package `pomotui 0.3.0-1`, SHA-256
`38ba0967c4b4567194bd4db5a8fae7cded10a96d303d640c195bb8a9dd39958d`.
The candidate package was `pomotui 0.3.0.r185.g307943f2d7d5-1`, SHA-256
`56b6fbfa564d43a1ba36084b5f9c8f9e274339bb0adb17e26b24ea740910e62f`.
Pacman recorded an actual upgrade between those versions. Service PID changed
245 → 281; the executable hashes were respectively
`37fd41bf0abdc97c7a763ecc6995f0f145c5efd13e179e9a0542dacac0feabed` and
`9e98253bbce11f03e21ca298f2669b71efbe498f06fc86ddbfb48656b75cf235`.
The running candidate matched `/usr/bin/pomotui-service`, used
`/usr/lib/systemd/user/pomotui.service`, and returned healthy public JSON status.
No service restart was issued after the upgrade: the genuine package hook
performed recovery.

The Arch base image was recorded as
`archlinux@sha256:061679925e5d5028f2df9395d441ce6c4b9accf0157ff3486c06b6962e014ef1`.
`/dev/kvm` existed on this runner, but nspawn did not use it. The demonstrated
nspawn environment provides the required actual pacman and systemd user-manager
boundary without depending on nested virtualization. This evidence establishes
initial feasibility; it does not establish the remaining session/data/failure
scenarios or constitute a complete production publication gate.

## Upgrade lifecycle and disabled intent

The shared AUR hook treats an enabled `pomotui.socket` as the durable opt-in for
upgrade recovery. A stopped or failed service behind that enabled socket is
recovered, including systemd start-limit exhaustion. The hook reloads the user
manager, checks effective units, clears failed service/socket state and restarts
the service. Public JSON status must report healthy durable storage, and the
running process executable must match the installed `/usr/bin/pomotui-service`.
These checks diagnose failures; pacman does not roll back binaries or user data.

To deliberately opt out, run `systemctl --user disable --now pomotui.socket`
and `systemctl --user stop pomotui.service`. Upgrade never enables a disabled
socket or starts its service. Disabling without `--now` does not stop an already
running socket/service; the hook still leaves that disabled installation alone.
A disabled but temporarily active socket is not durable permission to restart.
Manual launches and custom service enablement without the documented socket
opt-in fall outside supported automatic recovery.

Only the packaged service/socket with no drop-ins qualifies for automatic
recovery. A user unit, runtime unit or drop-in may change executable paths or
service policy, so the hook reports its exact `FragmentPath` and `DropInPaths`,
leaves it intact and recommends inspecting `systemctl --user cat UNIT`. Review
and move aside conflicting source-install units/drop-ins yourself before
reloading and restarting; do not delete business data to resolve a unit conflict.
The hook distinguishes manager discovery/reload, effective-unit conflict,
failed-state reset, restart, public health and executable-identity failures.

`lifecycle.sh`, sourced by the same real acceptance entry point, installs the old
package anew for stopped, failed, start-limited, disabled, user-unit override and
source-executable drop-in scenarios. Crashes and start-limit exhaustion use the
real manager. The conflict scenarios keep an old source-install executable
responding, proving that a status response alone cannot establish acceptance;
its exact override is diagnosed and its content and process are preserved.
All scenario state belongs to the disposable acceptance user.

### Hosted lifecycle evidence

[Red run 39109196865](https://github.com/SaintFore/pomotui/actions/runs/39109196865)
at `bd3ba03f6333` reproduced the actual systemd start-limit defect on 2026-10-18:
the stopped-service case passed, but crash-loop recovery was refused with
“start of the service was attempted too often” and the service remained failed.
The manager explicitly required `reset-failed` before another start.

[Green run 39109471604](https://github.com/SaintFore/pomotui/actions/runs/39109471604)
at `e2d675cca5e015fc18a7c1b6f3297b5daae0c23a` passed clean install/upgrade and all
six lifecycle scenarios on 2026-10-18. Pacman genuinely upgraded `0.3.0-1` to
`0.3.0.r191.ge2d675cca5e0-1` for each scenario. Real stopped, failed and
`start-limit-hit` services recovered to healthy candidate processes without a
manual restart. Disabled socket/service remained disabled/inactive. Both live
old source-install conflicts were diagnosed by exact user unit/drop-in path;
their checksums and existing process IDs remained unchanged, and their executable
hashes differed from the installed candidate. The complete booted acceptance
scenario took 9.358 seconds and 19.4 MB peak memory. The run artifact
`arch-acceptance-e2d675cca5e015fc18a7c1b6f3297b5daae0c23a` retains package hashes,
unit properties before each scenario, public statuses, pacman output and journals.
This establishes the lifecycle boundary; it does not claim publication readiness
or domain-state acceptance, which are verified separately.
