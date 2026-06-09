To answer the question, "What model effort level should I use", we need to
capture effort level in the session logs. We do this by installing a hook.

This consists of the following change to project settings
(`$PROJECT/.claude/settings.json`) or system settings
(`~/.claude/settings.json`) to run a `Stop` hook.

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/.gage/lib/scanners/effort/effort-hook.sh"
          }
        ]
      }
    ]
  }
}
```

`Stop` fires once per turn, which lets us capture effort levels by reading
`attachment` entries.

The script prints effort as `CLAUDE_EFFORT=<value>` to stdout where `<value>`
is read from the `CLAUDE_EFFORT` environment variable. If the variable isn't
defined, value is `unset`.
