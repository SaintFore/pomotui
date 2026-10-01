# Reserve the Void Task title

Pomotui reserves the exact normalized title `Void` for the singleton system Void
Task. Creating an ordinary Task with that title, or renaming one to it, is
rejected. Existing and imported records using another title remain regular Tasks.

Earlier synchronization documents represented reviewed Task attribution with a
Task identity but did not explicitly distinguish the system Void Task. Reserving
its invariant title makes that representation unambiguous and permits a lossless
upgrade: a format-4 reviewed Task whose referenced Task version is titled `Void`
is upgraded to system-Void attribution; every other referenced Task remains a
regular Task. This trades one ordinary title for a stable migration boundary and
avoids user prompts or heuristic identity matching during synchronization.
