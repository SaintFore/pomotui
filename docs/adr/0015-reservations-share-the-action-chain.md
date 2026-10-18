# Let failed reservations end the Action Chain

Pomotui will use the existing Action Chain to give a Focus Reservation consequences rather than introducing an independent reservation chain. A successful Reservation Review preserves the chain without adding a Chain Link or advancing rewards; a failed Reservation Review ends the chain, makes its unlocked but unclaimed rewards unavailable, and preserves claimed rewards under the existing reward rules. Expiry requests the user's judgment rather than automatically failing a reservation, allowing the user to report that they started on time but forgot to operate Pomotui.

## Status and trade-off

Accepted by the user on 2026-10-18; not implemented. This extends ADR-0006, where only failed Session Reviews end the chain, and requires representing a Chain Break without fabricating a Focus Session. It reuses the chain and reward progress the user values, at the cost of making reservations riskier as the chain grows; an independent reservation chain would isolate that risk but introduce a separate record the user does not want to maintain. Unresolved reviews and remaining interaction details are tracked in [the design notes](../research/control-method-fit.md).

## Start and cancellation

Starting a Focus Session during the reservation window fulfills and closes the reservation immediately. If the window expires first, confirming success starts a Focus Session at confirmation time without backdating or crediting earlier time; confirming failure ends the current Action Chain. Cancelling an issued reservation also requires an explicit success or failure judgment rather than silently withdrawing the commitment.
