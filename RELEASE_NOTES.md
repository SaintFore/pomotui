# Pomotui v0.3.1

## Upgrade reliability

- Recover configured Timer Services automatically after AUR upgrades, including
  failed or start-limited services and switching between `pomotui` and
  `pomotui-git`. Supported packaged installations no longer need a manual
  `systemctl --user restart pomotui.service` after upgrading.
- Preserve deliberately disabled services, user configuration, Current Session
  timing, Pending Reviews, Focus Reservations, and durable history during
  upgrades. Diagnose conflicting custom service units instead of replacing them
  or resetting user data.

## Verified Linux delivery

- Verify real Arch clean installations and upgrades, data migrations, and Ubuntu
  24.04 archive compatibility before publishing Linux x86_64 packages.
- Coordinate GitHub Release and both AUR channels from one release PR. Verify
  published contents and user discovery, retain diagnostics, and resume partial
  publications with the original verified artifacts.
- Fix GitHub Actions artifact restoration so interrupted publications can resume
  without rebuilding their accepted packages.

macOS artifacts continue to be delivered independently of Linux verification.
