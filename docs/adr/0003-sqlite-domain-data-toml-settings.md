# Store domain data in SQLite and settings in TOML

Current Session state, Focus Cycle progress, Tasks, and Session History will live in a versioned SQLite database under the XDG data directory, with Timer Service as its sole writer. User-editable durations, themes, keybindings, notification, and sound settings will remain in TOML under the XDG config directory; this adds a database dependency but provides transactional recovery and durable relations while keeping preferences easy to edit.
