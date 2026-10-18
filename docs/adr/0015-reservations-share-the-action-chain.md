# Let failed reservations end the Action Chain

Pomotui uses the existing Action Chain to give a Focus Reservation consequences rather than introducing an independent reservation chain. A successful Reservation Review preserves the chain without adding a Chain Link or advancing rewards; a failed Reservation Review ends the chain, makes its unlocked but unclaimed rewards unavailable, and preserves claimed rewards under the existing reward rules. Expiry requests the user's judgment rather than automatically failing a reservation, allowing the user to report that they started on time but forgot to operate Pomotui.

## Status and trade-off

Accepted by the user on 2026-10-18; implemented. This extends ADR-0006, where only failed Session Reviews end the chain, and represents a Chain Break without fabricating a Focus Session. It reuses the chain and reward progress the user values, at the cost of making reservations riskier as the chain grows; an independent reservation chain would isolate that risk but introduce a separate record the user does not want to maintain. The original reasoning is retained in [the design notes](../research/control-method-fit.md).

## Start and cancellation

Starting a Focus Session during the reservation window fulfills and closes the reservation immediately. If the window expires first, confirming success starts a Focus Session at confirmation time without backdating or crediting earlier time; confirming failure ends the current Action Chain. Cancelling an issued reservation also requires an explicit success or failure judgment rather than silently withdrawing the commitment.

## Interaction and persistence

Any Pending Session permits a reservation, provided there is no Pending Review or existing reservation. Running and Paused Sessions cannot create one. It defaults to the Current Task, permits another Task or no Task, and always targets Focus. The initial default is ten minutes; TOML and service-backed settings persist a new positive minute value for future reservations without retiming the current one.

The Timer Service owns expiry, notification, and recovery even when frontends are closed. Expiry enters Reservation Review and sends a desktop reminder; delivery failure or delayed judgment cannot end the chain. The judgment may be dismissed and reopened while the user rests, but new Focus and reservations remain blocked. Success ends an ongoing break and starts Focus from now, preserving actual break history. Failure requires a Reflection and explicit confirmation showing chain and unclaimed-reward consequences.

Live reservations remain local to a Device. Failure creates an immutable synchronized fact in Review Order, sharing the existing chain, reward, debt, archive and Fresh Start rules. It has no fabricated Session or focus duration. TUI, CLI and Waybar observe the same service state; duplicate or stale judgments cannot apply to a replacement reservation.
