# Timer service owns progression

The first release will use a persistent Timer Service as the single owner of timer progression. This adds service lifecycle and recovery complexity, but ensures deadlines advance and notifications fire even when the TUI is closed and Waybar is restarting; the TUI, CLI, and Waybar therefore remain control and display frontends rather than independent timers.
