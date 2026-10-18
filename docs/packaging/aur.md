# Publish the stable AUR package

The **Publish AUR** GitHub Actions workflow publishes the checked-in stable
`packaging/aur` recipe. It runs from GitHub's network and does not depend on the
maintainer's local SSH connection. The workflow is manual; creating a GitHub
release alone does not update AUR.

## One-time SSH setup

Use a dedicated, unencrypted SSH key whose public key is registered with the AUR
account maintaining `pomotui`. Add its private key to the `SaintFore/pomotui`
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

First publish the application's version tag and update `PKGBUILD`, `.SRCINFO`,
and `pomotui.install` in this repository. The source checksum must match the
published tag archive. Then run a validation without pushing:

```sh
gh workflow run aur.yml --repo SaintFore/pomotui --ref main -f publish=false
```

The workflow regenerates `.SRCINFO` using Arch's `makepkg`, verifies the released
source archive's SHA-256, and prepares a commit against the current AUR branch.
To publish that recipe:

```sh
gh workflow run aur.yml --repo SaintFore/pomotui --ref main -f publish=true
```

A successful push makes the new package version available to AUR helpers:

```sh
paru -Syu pomotui
```
