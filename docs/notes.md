---
title: Notes
---

_Notes_ are values that scanners assign to sessions, session lines, config
settings, and other topics as support evidence for issues.

Scanners may write a lot of notes as they run. This information persists across
scans. It's helpful to review notes over time before concluding there's an
issue worth reporting.

List notes:

```shell
gage note list
```

To view note details, run:

```shell
gage note show <NOTE_ID>
```

You can delete notes using `gage note delete`. In general this is not needed.
Scanners benefit from having this note record across scans.
