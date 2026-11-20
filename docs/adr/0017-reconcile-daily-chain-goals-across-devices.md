# Reconcile daily chain goals across devices

The Daily Chain Goal uses a shared time zone and counts eligible Focus Sessions across Devices. At the end of a goal day, the Timer Service can project a Chain Break from the activity it has observed. Later Shared Activity can remove that break and restore the chain if the merged records show the goal was met. This extends the revisable Action Chain projection in ADR-0008 so the result reflects the user's full activity regardless of when an offline Device synchronizes.

This extends ADR-0006 by allowing a missed goal to end the chain automatically. The prior chain is archived, unlocked and unclaimed rewards become unavailable, and claimed rewards remain historical facts under the existing reward rules. Changes to an active quota and disabling the goal take effect on the next calendar day, so the current day's commitment survives a late settings change.

New Chain Links require a successfully reviewed Completed Round, using the Focus Session's own planned duration. Existing Chain Links earned under the earlier review rule remain valid, including those from Stopped Sessions. This preserves historical chain membership and reward eligibility when Devices exchange activity recorded under the earlier rule.
