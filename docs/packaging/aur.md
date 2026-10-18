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
