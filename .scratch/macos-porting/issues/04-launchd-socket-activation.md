# 04 — launchd socket activation plist

**What to build:** Create `packaging/launchd/com.pomotui.service.plist` and `com.pomotui.socket.plist` that mirror the existing systemd units. The socket plist declares `Sockets` with `ListenStream` at the XDG-compatible socket path (`~/Library/Application Support/pomotui/pomotui.sock` or `$XDG_RUNTIME_DIR/pomotui/pomotui.sock`). The service plist uses `inetdCompatibility` so launchd passes the socket fd. Update the README with macOS installation instructions using `launchctl`.

**Blocked by:** 02 — DarwinClock, 03 — macOS notification and sound

**Status:** done

- [x] `packaging/launchd/com.pomotui.socket.plist` exists and declares a Unix domain socket
- [x] `packaging/launchd/com.pomotui.service.plist` exists and references the socket
- [ ] `launchctl load` succeeds on macOS with the plist (needs macOS runner)
- [ ] `pomotui status` connects to the service via the launchd socket (needs macOS runner)
- [x] README updated with macOS `launchctl` install instructions
