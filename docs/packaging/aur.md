# Verified Linux publication

The stable release decision belongs in a PR titled `release: vX.Y.Z`. Update the
Cargo workspace version and matching lockfile entries, and include nonempty
`RELEASE_NOTES.md`. A final-head approved review or an explicit merge by a
maintainer with write/maintain/admin permission authorizes that release PR.
The latter supports a solo maintainer without requiring another reviewer.
After merge to `main`, the stable workflow records the source and version,
checks and builds that candidate, and runs real Arch clean-install and upgrade
acceptance before publishing. Ordinary merges never select a stable release.

Relevant source/packaging changes pushed to `main` refresh the Git package through
the same required gate. Its `r<count>.g<commit12>` metadata is a tested snapshot;
users still fetch latest upstream source when building `pomotui-git`. Rebuild with
`paru -S pomotui-git`, or check VCS updates with `paru -Syu --devel`.

The previous manual AUR `publish=true` entry point and tag-triggered release
publisher are removed. Do not create a tag first or hand-edit generated version,
archive checksum or `.SRCINFO` values. The candidate generates them from the
recorded source, and the stable recipe uses the exact retained source asset.
A matching existing tag/release/asset can be verified and reused; conflicting
identities are preserved and diagnosed, never overwritten.

## One-time credentials and trust

Use the existing dedicated unencrypted SSH key registered with the AUR owner
account. Its private key belongs in the `SaintFore/pomotui` repository secret
`AUR_SSH_PRIVATE_KEY`, never in commits, issues or logs:

```sh
gh secret set AUR_SSH_PRIVATE_KEY --repo SaintFore/pomotui < /path/to/dedicated-aur-key
```

Before public writes, preflight authenticates with strict SSH host verification,
checks selected-package ownership and tests receive permission without pushing.
Stable publication checks both required AUR targets before its GitHub writes.
This preflight expects the existing owner credential; a co-maintainer-only key
is rejected before publication and requires an explicit maintainer
credential/permission repair.

`packaging/aur/known_hosts` pins AUR's Ed25519 host key, fingerprint
`SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4`. Compare the
[official AUR fingerprint](https://aur.archlinux.org/) before explicitly approving
a changed key. Automation never replaces credentials, grants permissions or
changes host trust automatically.

## Gates, delivery and recovery

Each candidate retains source/version identity, exact generated recipes,
`source.bundle`, source archive, real packages, Linux archive and checksums.
The Linux gate runs existing formatting/lint/tests/smoke checks, then installs
those actual package bytes in disposable hosted Arch with a real systemd user
manager. Its receipt binds the source, candidate manifest and each package hash.
Failed checks, upgrade failures and timeouts block publication. macOS builds run
independently and retain their own results.

Package recovery applies only to enabled packaged sockets. Disabled sockets
remain disabled, including stable/Git replacement; custom units and drop-ins are
preserved with exact conflict paths. Fresh installs use the documented explicit
`systemctl --user enable --now pomotui.socket` command.

GitHub delivery verifies tag/source identity, retained asset bytes, public
release visibility and actual user download bytes. A GitHub channel success is
not aggregate completion while either required AUR channel is outstanding.
AUR delivery distinguishes pushed/remote-verified from user discovery. A stale
index remains pending indexing; retry verification with the same candidate
rather than pushing another version. See the original Actions run summary and
retained diagnostic artifacts for the failed stage and retry context.

## Normal release and reports

1. Prepare one PR titled `release: vX.Y.Z`, change the Cargo workspace version
   and lockfile package versions, and write the user-facing `RELEASE_NOTES.md`.
2. Review that final source and merge the PR. The merge is the release decision;
   do not tag, compute checksums, dispatch AUR separately, or restart a user's
   Timer Service as part of the release process.
3. Read the single **Linux delivery** comment on that PR. Automation updates the
   same comment with source/version, required channel stages, authoritative
   verification evidence, current failures and the original-run retry command.
   Relevant ordinary `main` pushes instead show their Git snapshot in the
   **Verified Git package** Actions summary.

Required delivery is Linux GitHub assets plus `pomotui` and `pomotui-git` AUR
publication for stable releases; an independent Git upkeep run requires its
Git AUR channel. Completion requires every required result to be verified and
user discoverable. macOS build failures stay visible without blocking Linux.
An older stable run can accept a newer Git snapshot only after independently
restoring that newer run's trusted acceptance artifacts, proving descendant
source history and verifying all current remote bytes and discovery. Its report
identifies the actual newer source/version instead of claiming the old snapshot.

Before acceptance is transported to publication, the exact Linux archive also
starts a private Timer Service on Ubuntu 24.04 and obtains healthy public CLI
status. This catches future Arch toolchain/glibc/SQLite runtime incompatibilities
before publication. It uses the same immutable archive as the release, with
private HOME, XDG directories and socket, and cleans up even after failure. Its
receipt binds the source, candidate manifest, archive and executable hashes.
Package receipts separately cover real systemd user managers and actual pacman
clean install/upgrade for both packages, including released migrations, Current
Sessions, Pending Reviews, Focus Reservations and conservative service recovery.

## Same-publication recovery

Read the original run's report and diagnostics first. GitHub retains candidate
artifacts, receipts, journals and diagnostic artifacts for **90 days** (subject
to repository retention settings). Every publication attempt records completed
stages before proceeding. Failed checks and timeouts produce an incomplete
report even when publication never starts. GitHub's existing failed-workflow
notifications are the notification mechanism.

Use **Re-run failed jobs** for a failed prerequisite. Once accepted artifacts
exist, rerunning the original workflow restores those original bytes instead
of rebuilding. To resume from a later Actions invocation:

```sh
# Replace ORIGINAL_RUN_ID with the original stable or Git publication run ID.
gh workflow run resume-publication.yml --repo SaintFore/pomotui \
  --field run_id=ORIGINAL_RUN_ID --field intent=stable --field verify_only=false
```

Use `intent=git` for Git upkeep. For stable publication that is remotely verified
but waiting for AUR indexing, use `--field verify_only=true`: it only reads
GitHub assets, AUR remote content and discovery; it does not push again. A stale
index remains **pending-indexing**, never complete. Wait and run verification
again with the same original run ID; do not bump a version to refresh the index.

Transient network failures retry a bounded number of times and then retain a
recovery state. A retry rereads authoritative content before writing, so an
uncertain successful response cannot cause an overwrite or duplicate public
release. Immutable tag, notes, asset or same-version recipe conflicts pause with
the conflicting identity. Inspect and repair the cause explicitly; do not move
tags, overwrite published bytes, force-push, downgrade the remote or reset data.

Recovery requires the original trusted accepted artifacts. If those artifacts
are expired, deleted or fail identity checks, automation pauses rather than
recreating potentially published bytes. Retain an exact verified copy before
expiry if long-term recovery is needed; increasing retention cannot recover
already deleted artifacts. A maintainer must investigate unavailable originals
and the actual public state before making a new release decision.

Credential, package ownership and SSH host-key failures require explicit
maintainer repair using the credential/trust procedure above, followed by the
same-publication retry. Automation does not create accounts, change ownership,
replace keys or accept new host keys. User unit overrides, drop-ins, unsupported
or damaged data also require an explicit local repair after backup and inspection;
automation preserves them and reports exact conflicts instead of deleting data.

The manual **Nonpublishing complete Linux delivery proof** workflow validates
the finished contract against disposable Git/HTTP publication targets. It builds
real stable/Git fixture packages once, retains their identity across jobs, runs
the real Arch gate and Ubuntu archive smoke, then invokes the normal release,
Git-refresh and artifact-restore entry points for an approved merge, credential
preflight failure before writes, pending discovery and read-only recovery. It
also demonstrates required Linux success with an independent macOS failure, and
automatic Git upkeep delivers to its own fresh remote without a stable output.
It never creates a production stable version, tag, release or AUR push.
