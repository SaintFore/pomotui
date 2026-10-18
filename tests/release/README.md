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

### Hosted domain-state evidence

[Run 39109947982](https://github.com/SaintFore/pomotui/actions/runs/39109947982)
passed on 2026-10-18 at source
`22f4ef6046309601d6e4d17b934ce383433550fe`, including all domain-state scenarios
and the integrated lifecycle suite. It upgraded recorded v0.3.0 source
`ad9ce4cf4c43997b807485d3a590563ad66c68f1` to the candidate through real pacman.
Running Session recovery changed PID 431 to 479 with candidate executable SHA-256
`b4bf6a1d377bf5ce93ae5e5fbeb4015bba2c48c5fe5f1fc2b9641413a7d533d4`.
Running and Paused timing, Pending Review, represented history/reviews/claimed
reward/settings, reservation countdown/expiry/pending judgment, both released
migration fixtures and backups, and corrupt/future-schema/future-format byte
preservation all passed. The combined booted scenario took 160 seconds, including
two real one-minute reservation deadlines, with 30.6 MB peak memory and no swap.

[Artifact 12130197002](https://github.com/SaintFore/pomotui/actions/runs/39109947982/artifacts/12130197002)
retains packages, source identities, journals, pacman logs, before/after public
snapshots, per-scenario process identities and invalid-data diagnostic/checksum
records. Ordinary [CI run 39109947981](https://github.com/SaintFore/pomotui/actions/runs/39109947981)
also passed. No domain/service change was required by these state scenarios.

## Source-identified candidate preparation (#23)

The non-publishing public interface is `packaging/release/candidate.py` (Python
3.11+, Git, Docker, GNU tar with zstd support for build output inspection):

```sh
python3 packaging/release/candidate.py plan --source HEAD --output /tmp/candidate
python3 packaging/release/candidate.py build --output /tmp/candidate
python3 packaging/release/candidate.py verify --output /tmp/candidate --require-built
```

Planning reads the committed source, not dirty working files, and requires full
Git history. Ordinary candidates require only `aur-git`. Stable intent must be
explicit: add `--intent stable --notes release-notes.md` to `plan`; this requires
`github-linux`, `aur-stable`, and `aur-git`. The workspace version is the only
stable-version input; every workspace package must inherit it and its lockfile
entry must agree. The stable tag is derived and checked against any existing
local tag. The later publisher must also reconcile the authoritative remote tag.
This command does not establish release-PR approval; the coordinator owns that
authorization check.

`candidate.json` records schema version, exact source commit, workspace version,
commit-derived Git snapshot, tag intent, required channels, expected outputs,
selected toolchain/target, and SHA-256 identities of the source, recipes,
`.SRCINFO`, current shared hook, notes, and built artifacts. `planned` never
passes `verify --require-built`. Building uses a disposable Arch image recorded
by digest; it installs the target into the toolchain selected by the source's
`rust-toolchain.toml`, executes the generated recipes' real build and checks,
compares `makepkg --printsrcinfo`, verifies stable-source checksums, and inspects
actual package metadata and packaged hook. The Git build freezes the recorded
commit using a local Git bundle and makepkg's `--holdver`; the distributed recipe
still fetches moving upstream HEAD and recalculates its snapshot version.

Built outputs include `built/aur-stable/*.pkg.tar.zst` when stable was selected,
`built/aur-git/*.pkg.tar.zst`, Linux binary archive/checksum, binary hashes, and
`build.log`. Pass the recorded package path to the existing
`acceptance.sh OLD_PACKAGE CANDIDATE_PACKAGE OUTPUT_DIRECTORY` boundary. A built
candidate still records `acceptance: required`: package construction never
substitutes for either real acceptance suite, credential preflight, or release
approval. The Git package provides/conflicts with `pomotui`; the acceptance
adapter must account for its distinct actual package name.

The stable source is the generated `pomotui-VERSION-source.tar.gz`, whose exact
bytes and embedded Git source identity are checked. The generated stable recipe
uses that source asset's eventual GitHub Release URL, **not** a separately
regenerated GitHub tag archive. After all gates, the publisher must reconcile or
create the immutable `vVERSION` tag at the recorded commit, upload this exact
source asset, verify it is externally available, then publish its matching AUR
recipe. No source tag is created during preparation. The legacy platform workflow
is now explicitly non-publishing and has no tag trigger, so creating a source tag
cannot bypass the acceptance gate.

The `Non-publishing Linux candidate` workflow exercises this path for relevant
PRs without deployment secrets. Its stable fixture uses `0.0.0` only in a
disposable local source commit, builds both real package recipes, and never
publishes that commit, tag, or version. Run fast isolated boundary regressions
with `python3 -m unittest discover -s tests/release -p '*_test.py'`.

[Candidate build run 39109386492](https://github.com/SaintFore/pomotui/actions/runs/39109386492)
passed both cases on 2026-10-18. The ordinary source was
`660d9da3259a071f472d33e20dcb114a2fea283c`, producing
`pomotui-git r191.g660d9da3259a-1`. The disposable stable source was
`c79d701dd8ec9b027646ba450440d3ebe17f24ef`, producing `pomotui 0.0.0-1`
and `pomotui-git r192.gc79d701dd8ec-1`. Both used Rust `1.96.1`, target
`x86_64-unknown-linux-gnu`, and Arch image
`archlinux@sha256:061679925e5d5028f2df9395d441ce6c4b9accf0157ff3486c06b6962e014ef1`.
The generated recipes completed actual workspace release builds, workspace
checks, package construction, canonical metadata comparison, and output/hook
inspection. Source commits, package/archive/binary hashes and complete build
logs are retained in that run's candidate artifacts. These are package-build
results, not installation/upgrade acceptance or delivery evidence.
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


### Candidate adapter compatibility

The feasibility-only baseline assumed both packages were named `pomotui` and
required different old/new executable hashes. Production acceptance must also
accept the actual `pomotui-git` package (which provides/conflicts with `pomotui`).
Packaging-only changes can retain identical executable bytes. Those upgrades
must still establish the recorded candidate package/version/hash, a replacement
service PID, the installed candidate executable hash, and the effective packaged
unit identity. Removing an unequal-code-hash assumption must never remove those
candidate-identity checks.

### Automatic Git-package publication

`git-package.yml` listens to pushes on this repository's `main`.
`git_refresh.py select --event EVENT.json` compares the push's complete committed
`before`/`after` range, so a documentation-only final commit cannot hide an earlier
source change in the same push. The event must identify a non-deleted main push.
`select --source COMMIT` supports explicit local selection (`--before` chooses the
baseline; otherwise the first parent is used). Source,
packaging, toolchain, and release workflow changes select a candidate. Unrelated
documentation and generated `.SRCINFO` snapshots do not. Publication never
commits generated metadata back to the source repository.

The selected source passes the shared `candidate-gate.yml`: actual package builds,
source checks, and both real upgrade suites. The publication job consumes that
exact retained candidate, checks its source and receipts again through
`git_refresh.py publish`, and shares `pomotui-aur-publication` concurrency with the
stable path. Both paths reconcile identical Git-package bytes instead of creating
duplicate AUR commits. A Git snapshot such as `r123.g0123456789ab` describes the
verified publication source; the distributed recipe deliberately fetches latest
upstream source when a user builds it.

The reusable AUR channel boundary supports both package identities:

```sh
python3 packaging/release/aur.py preflight --candidate DIR --channel aur-git
python3 packaging/release/aur.py publish --candidate DIR --channel aur-git --acceptance DIR/acceptance.json
python3 packaging/release/aur.py verify --candidate DIR --channel aur-git
```

The workflow supplies `AUR_SSH_KEY_FILE` only after its required gate succeeds.
Preflight uses strict checked-in host trust, authenticates with AUR `list-repos`,
checks the selected maintained package, and exercises receive access with a
nonmutating Git dry run. It does not adopt packages, configure an account, replace
keys, or trust a newly observed host. The AUR server's permission check is
implemented in [aurweb's SSH command handler](https://github.com/archlinux/aurweb/blob/master/aurweb/git/serve.py).

Publication compares actual remote `PKGBUILD`, `.SRCINFO`, and the shared install
hook before checking discovery through the AUR RPC. `pending-indexing` means the
remote bytes are verified but user discovery has not caught up. The `verify`
command resumes just those checks, without a push or new version. A changed
already published version or a newer Git snapshot is a conflict, never a reason
to force push. Local `--remote` bare repositories and local HTTP `--rpc-url`
endpoints exercise the same CLI boundary in `aur_test.py`; they need no AUR key
and do not claim to replace real package or upgrade acceptance.

## Production candidate gate and stable GitHub coordinator (#24)

`verify-candidate.sh CANDIDATE_DIRECTORY OUTPUT_DIRECTORY` verifies a built
manifest, selects the newest published non-draft/non-prerelease stable GitHub
release and verifies its immutable tag/source, builds only that predecessor, then boots a
fresh Arch system and installs **the exact manifest package files**. Both stable
and Git channels run the full clean-install/domain/migration/lifecycle matrix.
Stable→Git is a real pacman conflict replacement; same-name Git reinstall also
requires a replacement running PID with candidate bytes. Packaging-only changes
need not invent different executable hashes. Installed package version/name,
package checksum, installed/running executable bytes, packaged unit and public
health remain mandatory. Only the separate nonpublishing fixture workflow uses
`--allow-nonpublishing-fixture-predecessor`; production never overrides version
ordering.

The v0.3.0 source tag contains an old PKGBUILD version, so the predecessor builder
derives package metadata from that immutable source's Cargo version. It never
changes predecessor application source or rebuilds/modifies a candidate.

`release.py select` checks authoritative merged PR data, final-source review or
authorized maintainer merge, matching release title/workspace version and changed
notes. Prior identity comes from the original PR commit ancestry, not moving
main/API base. `candidate-gate.yml` performs all applicable checks and actual
acceptance before retaining portable manifest-declared artifacts, including
hidden metadata. Its output includes source-bound checks and manifest/package-
bound acceptance receipts. Publication jobs depend on its successful completion;
a built label alone is insufficient.

`release.py verify-gates`, `preflight`, `publish-github` and `pack` are normal
consumer CLI boundaries. The coordinator preflights both AUR packages before
GitHub publication. GitHub publication preserves conflicting tags/assets/notes,
reuses matching outputs, verifies authoritative bytes and user-visible downloads,
and reports `pending-required-aur-channels` rather than aggregate completion.
The stable workflow has no manual/tag entry point. Legacy manual AUR publication
is closed. macOS jobs depend only on stable selection, independently of Linux.

`publication_test.py` exercises the coordinator using isolated HTTP endpoints
and real local bare AUR remotes: ordinary-vs-explicit release intent, fresh/stale
review and solo-maintainer approval, moving API base, failed/mismatched gates or
credentials, two-target preflight, immutable rerun, tag/asset/notes conflicts and
upload-success-with-wrong-download rejection. Tiny transport fixtures test the
publication boundary only; hosted real package acceptance supplies upgrade proof.

Actual production-adapter proof passed:
https://github.com/SaintFore/pomotui/actions/runs/39111512146 at
`74e01588556c94d7be52bf3b1a19dd44d1fe3de4`. Both real generated package builds,
portable upload/download, and both complete real Arch matrices passed. The
nonpublishing source fixture changes only its temporary version to `0.0.0`; the
actual predecessor was published v0.3.0 at
`ad9ce4cf4c43997b807485d3a590563ad66c68f1`. The later predecessor selector hardening
was separately exercised against the actual authenticated release API, resolving
the same published release/source, and by an isolated regression proving an
unpublished newer tag/draft is ignored. Guest behavior is unchanged.
Full existing CI passed on the same source:
https://github.com/SaintFore/pomotui/actions/runs/39111512132. Proof artifacts:
`adapter-candidate-74e01588556c94d7be52bf3b1a19dd44d1fe3de4` and
`production-adapter-74e01588556c94d7be52bf3b1a19dd44d1fe3de4-1`.
The proof workflow is now nonpublishing manual-only after its narrow branch
experiment; it cannot publish a release.

## Stable AUR delivery and discovery

The approved stable workflow now invokes `release.py publish-stable`, consuming
only the retained candidate, all-package acceptance receipt, source checks and
release selection. It preflights both AUR repositories before any publication,
publishes/verifies the GitHub source and Linux downloads, then delivers the exact
generated `PKGBUILD`, `.SRCINFO` and shared hook to both required AUR channels.
The source archive must be usable before the stable recipe is announced.

The aggregate JSON contains candidate identity, per-channel results and ordered
`pushed`, `remote-verified`, and discovery events. A bounded old AUR index produces
`pending-indexing` and a failing workflow, never aggregate completion. Run
`release.py verify-stable` with the same candidate, selection, acceptance, checks
and output arguments to retry read-only verification; it creates no tag, release,
asset, AUR commit or push. Remote byte conflicts remain paused independently of
index availability. Publication tests use the normal CLI against disposable Git
repositories and HTTP endpoints; these transport fixtures do not replace the
separate real Arch package/upgrade acceptance evidence.
