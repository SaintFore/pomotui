# Measure sessions by real elapsed time

Running Sessions will measure real elapsed time with a suspend-aware monotonic clock while Timer Service is alive, so system suspension counts but timezone and wall-clock adjustments do not distort a Session. Durable recovery metadata will also be stored for service restarts; this dual-clock design is more complex than persisting only a wall-clock deadline, but preserves the user-visible meaning of a configured duration.
