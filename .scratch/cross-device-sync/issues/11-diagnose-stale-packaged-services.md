# 11: Diagnose stale packaged services

**What to build:** Make package upgrades and CLI errors clearly identify an older running Timer Service, so a successfully installed synchronization command works after restart or tells the user exactly what remains stale.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] The package upgrade flow makes a best-effort attempt to restart an existing user Timer Service after installing a new build.
- [x] A failed or unavailable restart does not abort installation, kill an unrelated process, or hide the problem; it emits a visible instruction to restart the Timer Service manually.
- [x] When a new CLI reaches a service whose protocol does not recognize a synchronization operation, the CLI reports a targeted stale-service/version-mismatch diagnostic instead of presenting the response as an ordinary malformed request.
- [x] Other protocol and transport failures retain their existing distinct error behavior.
- [x] Automated packaging and CLI tests cover successful restart, restart failure with warning, stale-service diagnosis, and false-positive avoidance.
- [x] Upgrade and troubleshooting documentation explains how to verify and restart the running service before testing newly installed commands.

## Comments

- The package hook tests exercise successful restart, non-fatal restart failure, and an unavailable installing-user context with a fake service manager. CLI tests distinguish unknown synchronization operations from unrelated malformed requests.
