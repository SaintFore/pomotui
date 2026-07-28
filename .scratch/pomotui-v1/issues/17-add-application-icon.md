# Add application icon

Status: resolved

## Classification

New requirement.

## Problem

Pomotui's desktop application entry has no application icon, so desktop
launchers show a generic placeholder. Icon assets are available in the
repository but are not installed or referenced by the launcher.

## Scope

- Use the PNG assets under `favicon_io/` as the Pomotui application icon.
- Install the available icon sizes under the appropriate XDG `hicolor` icon
  directories using the stable application icon name `pomotui`.
- Set the desktop entry's `Icon` field to the installed icon name.
- Include icon installation in reinstall and uninstall behavior.
- Extend isolated packaging verification to cover icon installation, desktop
  entry linkage, idempotent reinstall, and removal.

## Acceptance

- Installation places the 16x16, 32x32, 192x192, and 512x512 PNG assets in the
  matching `hicolor/<size>x<size>/apps/pomotui.png` directories beneath the
  active XDG data location.
- The installed `pomotui.desktop` contains `Icon=pomotui`.
- Desktop environments can resolve the icon without referring to the source
  checkout or an absolute repository path.
- Reinstall replaces the installed icon files idempotently.
- Uninstall removes every Pomotui icon installed by the package while leaving
  unrelated icons untouched.
- The packaging end-to-end test verifies installation, desktop entry linkage,
  reinstall, and removal of the icons.

## Comments

Created from user feedback on 2026-07-28.

Resolved 2026-07-28. Packaging now installs the four PNG sizes under the XDG
`hicolor` icon theme, and the desktop entry resolves them through
`Icon=pomotui`. Uninstall removes only Pomotui's icon files. The end-to-end
test verifies exact asset installation, idempotent reinstall, desktop entry
linkage, cache refresh, removal, and preservation of an unrelated icon. The
source assets use consistent `favicon_io/pomotui-*` names, and the retained Web
manifest references the renamed PNG files.
