# effort

How does changing model `effort` affect performance?

Anecdotally, this would appear hard to know. If a model consistently spends
more time thinking at higher effort levels, how much more time? What's the
quality improvement or other benefit? How would one even measure this?

To make answering these questions harder, Claude Code does not record
configured effort levels in session logs. There's no convenient way to
correlate model performance with session evidence.

To give us the evidence we need to compare effort levels, we need to save
effort levels to the logs. We can do that via a [Claude Code hook].

[Claude Code hook]: https://code.claude.com/docs/en/hooks

See [effort-hook.md](effort-hook.md) for details.

The hook uses on `CLAUDE_EFFORT` env var and emits the value as
`additionalContext` in `hookSpecificOutput`.

NOTE: `additionalContext` is replayed on `--continue`/`--resume` rather than
re-run, so historical records reflect the effort level captured at the time the
turn first ran.
