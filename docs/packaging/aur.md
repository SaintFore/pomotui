# Publish AUR packages

The **Publish AUR** GitHub Actions workflow publishes either checked-in recipe:
`packaging/aur` for `pomotui`, or `packaging/aur/pomotui-git` for `pomotui-git`. It runs from GitHub's network and does not depend on the
maintainer's local SSH connection. The workflow is manual; creating a GitHub
release alone does not update AUR.

## One-time SSH setup

Use a dedicated, unencrypted SSH key whose public key is registered with the AUR
account maintaining the selected package. Add its private key to the `SaintFore/pomotui`
repository secret named `AUR_SSH_PRIVATE_KEY`. Do not commit the private key or
paste it into issues, workflow files, or logs.

```sh
gh secret set AUR_SSH_PRIVATE_KEY --repo SaintFore/pomotui < /path/to/dedicated-aur-key
```

The checked-in `packaging/aur/known_hosts` pins AUR's Ed25519 host key. Its
fingerprint is `SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4`, verified
against the [AUR homepage](https://aur.archlinux.org/). Check the official
fingerprint before changing this file if AUR rotates its host key.

## Validate and publish

Update the selected recipe's `PKGBUILD` and `.SRCINFO`, and the shared
`packaging/aur/pomotui.install` hook in this repository. For `pomotui`, first
publish the application's version tag; its source checksum must match that
archive. For `pomotui-git`, record a current `r<count>.<commit>` version snapshot;
`pkgver()` recalculates it from the latest source during each build. No new
release tag is needed for the Git package.

Run a validation without pushing (the default package is `pomotui`):

```sh
gh workflow run aur.yml --repo SaintFore/pomotui --ref main -f package=pomotui -f publish=false
```

The workflow regenerates `.SRCINFO` using Arch's `makepkg`, verifies either the
release archive's SHA-256 or the configured Git source, and prepares a commit
against the selected package's current AUR branch. Both packages use the shared
install hook, including automatic Timer Service recovery after upgrades.
To publish that recipe:

```sh
gh workflow run aur.yml --repo SaintFore/pomotui --ref main -f package=pomotui -f publish=true
```

A successful push makes the new package version available to AUR helpers:

```sh
paru -Syu pomotui
```

To publish the Git package instead:

```sh
gh workflow run aur.yml --repo SaintFore/pomotui --ref main -f package=pomotui-git -f publish=true
```

Its AUR page shows the last published version snapshot. Builds still fetch the
latest Git source. Rebuild with `paru -S pomotui-git`, or check development-package
updates using `paru -Syu --devel`.
