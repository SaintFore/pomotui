# Target Linux and Wayland first

The first release will support Linux/Wayland and may rely on systemd user services, desktop D-Bus notifications, and Waybar-specific integration. The timer core will remain platform-independent, but service management and notification adapters for macOS, Windows, or non-systemd Linux are out of scope so the initial product can provide a reliable native Linux experience without premature portability abstractions.
