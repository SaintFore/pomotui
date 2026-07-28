# Build action chains from explicit session reviews

Pomotui will maintain one durable Action Chain whose Chain Links come from
successful user reviews of completed or deliberately reviewable stopped Focus
Sessions, rather than inferring success from elapsed time, Task completion, or
streak dates. A Pending Review permits Break Sessions but blocks the next Focus
Session; Success extends the current chain, while confirmed Failure immutably
archives it with a terminal Chain Break and creates a new empty chain. This adds
a review state and transaction boundaries to the Timer Service, but preserves
the user's authority to judge partial work while keeping Session History,
rewards, and chain history internally consistent.
