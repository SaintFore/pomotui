# 11: Diagnose stale packaged services

**What to build:** Make package upgrades and CLI errors clearly identify an older running Timer Service, so a successfully installed synchronization command works after restart or tells the user exactly what remains stale.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] The package upgrade flow makes a best-effort attempt to restart an existing user Timer Service after installing a new build.
- [ ] A failed or unavailable restart does not abort installation, kill an unrelated process, or hide the problem; it emits a visible instruction to restart the Timer Service manually.
- [ ] When a new CLI reaches a service whose protocol does not recognize a synchronization operation, the CLI reports a targeted stale-service/version-mismatch diagnostic instead of presenting the response as an ordinary malformed request.
- [ ] Other protocol and transport failures retain their existing distinct error behavior.
- [ ] Automated packaging and CLI tests cover successful restart, restart failure with warning, stale-service diagnosis, and false-positive avoidance.
- [ ] Upgrade and troubleshooting documentation explains how to verify and restart the running service before testing newly installed commands.
