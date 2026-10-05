# Allow explicit whole Ended Chain deletion

Pomotui will let the user permanently delete an entire Ended Chain after
reviewing an explicit destructive confirmation, including its Chain Links,
terminal Chain Break, and reward history, while preserving the independent
Session History. Individual archived entries remain non-deletable and retain
their limited text-edit rules. This trades an irrevocable audit trail for user
control over retained reflective history without allowing partial deletion to
misrepresent an Ended Chain.

ADR 0013 narrows the reward-history boundary: deletion hides reflective chain
and reward history but retains immutable Synchronization Records needed for
Reward Debt and repayment accounting. Deletion, milestone edits, and milestone
deletion cannot forgive an obligation or discard credited successes. Only an
explicit Fresh Start under ADR 0014 clears that accounting.
