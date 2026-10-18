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

## Domain state across upgrades (#21)

`scenarios/domain-state.py` extends the same three-argument acceptance boundary.
Each scenario reinstalls v0.3.0 in isolated state, creates work through the public
v6 socket, and upgrades through pacman. It checks the candidate process and
executable before comparing public snapshots. It covers Running and Paused
Sessions, attributed History, Pending Review, a successful review and claimed
reward, reservation countdown and pending judgment, settings, and upgrades of
both immutable released migration fixtures. Backup bytes are preserved across
upgrade; independent backup restoration remains covered by `legacy_migration`.

The Running Session check observes continued task attribution, planned duration,
clock progression, and one resulting History record whose identity matches its
Pending Review. The live public snapshot does not expose a Current Session ID;
this is the strongest identity observation available at the approved seam.

Reservation expiry uses a real one-minute deadline and never submits a judgment.
Separate upgrades cover a live countdown and an already pending judgment.
Corrupt databases, future schema versions, and future durable-state formats are
injected only as unsupported input; startup must fail with its diagnostic while
preserving the original database bytes. No database query is used to assert
business state. Each scenario retains before/after snapshots, candidate process
identity, pacman output, and failure diagnostics under `result/domain-state`.
