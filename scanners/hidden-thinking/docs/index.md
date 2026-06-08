---
title: hidden-thinking
---

Thinking blocks show what a model is "thinking" --- it's inner monologue, which
supports its next steps. These blocks are useful to get insight into how the
model is performing.

Unfortunately, thinking blocks are hidden by default.

The scanner checks for this condition and provides steps to fix it.

To scan for hidden thinking blocks, run:

```shell
gage scan -s hidden-thinking -y
```

If the scanner detects hidden thinking blocks, it opens an issue that you and
Claude can use to fix the issue. Use the `/gage:review` Claude command to kick
off an issue review session.

##### Manual checks

If you want to check a session directly, run `gage session view` and select a
recent session. Navigate to a `thinking` entry and check the contents of the
thinking block. If it's empty, thinking blocks are turned off (hidden).
