# Use a Unix socket for the timer service protocol

Timer Frontends will communicate with Timer Service through a versioned request/response protocol over a Unix domain socket under `XDG_RUNTIME_DIR`, with systemd socket activation available to start the service on first access. This keeps application control independent of desktop D-Bus and makes CLI and TUI integration directly testable; D-Bus remains an adapter for desktop notifications only.
