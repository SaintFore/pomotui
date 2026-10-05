# Carry reward debt across action chains

Late Shared Activity can revise Review Order after an offline Device has already recorded a reward claim. Pomotui will preserve that claim and compensate for its reduced eligibility through Reward Debt measured in missing successful Focus Sessions, rather than revoking the reward or charging its entire threshold. Future successful Session Reviews repay the debt before contributing toward the next reward; a Chain Break preserves both outstanding debt and repayment progress, actual Action Chain length remains unchanged by debt, and repeated observation of a claim must never charge it again.

## Status and scope

The user accepted this policy on 2026-10-05. It extends ADR 0008's deterministic cross-device projection and the existing rule that claimed rewards remain historical facts. A claim left supported by one success at a threshold of seven owes six, not seven; repeating rewards have not been adopted as a new product feature. Tickets #5 and #6 implement immutable claim evidence and deterministic per-milestone accounting. Existing claims without evidence remain claimed without fabricated debt history.

## Additional accepted rules

Debt is accounted for separately for each Reward Milestone. One successful Session Review can repay one outstanding success for each milestone simultaneously, matching how successes normally advance all milestone thresholds. Further Shared Activity recalculates the shortfall without discarding repayment progress; if a later revision reduces the shortfall below already repaid successes, the excess contributes toward future reward progress.

Explicit Ended Chain deletion retains outstanding debt and repayment progress. This narrows ADR 0007's permission to delete reward history: deleting displayed history must preserve enough information to maintain debt correctly. The user subsequently accepted a distinct command for a Fresh Start: all database data, including debt, starts over across Devices, and retained pre-reset records must not restore it. Rebuilding only the exchange artifact from locally retained records is not a Fresh Start.

Debt capacity follows the claim's snapshotted supporting reviews under revised Review Order. Late successes before the claim frontier can reduce the shortfall; eligible later successes remain uniquely allocated within each milestone, preserving surplus credit. Later claims snapshot any surplus they consume so it cannot support another reward twice. ADR 0014 records the distinct Fresh Start boundary. Implementation and tests use isolated data; no production database reset was performed.
