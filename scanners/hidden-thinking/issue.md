## Hidden thinking text

When extended thinking is turned on, Claude creates thinking content
blocks where it outputs its internal reasoning. Claude incorporates
insights from this reasoning before crafting a final response.

Access to thinking text is useful when analyzing model behavior.

If sessions logs contain thinking messages with empty text, Gage treats
this as evidence that thinking text is hidden --- i.e. it's not being
sent by the model.

## Fix

### Step 1. Show thinking summaries (Claude settings)

To show thinking summaries, Claude Code must set `showThinkingSummaries`
to `true`. This is best applied system wide in
`~/.claude/settings.json`.

```json
{
  "showThinkingSummaries": true
}
```

This enables thinking blocks for Claude Code sessions for all models
provided the model sends thinking text. Some models hide thinking text
by sending emptry strings.

### Step 2. Enable thinking summaries for Opus 4.7 and later

In Step 1 you configured Claude to show thinking text _when it receives
it from the model_. However, some models do not send thinking text.

Models that don't send thinking text unless enabled:

- Opus 4.8
- Opus 4.7
- Mythos

If thinking blocks aren't visible when `showThinkingSummaries` is `true`
(Step 1 above) you need to run Claude with the `--thinking-display`
option set to `summarized`.

**IMPORTANT:** The `--thinking-display` option is _not documented_. Do
not assume these instructions are wrong until you have confirmed by
running `claude` with this option. If `claude` exits with an error that
the option is not supported, it's not supported. Otherwise it is.

```bash
claude --thinking-display summarized
```

To make this setting permanent, take one of two steps:

- Create a shell alias to run `claude` with this option
- Create an alternative shell script that runs `claude` with this option

#### Step 2a. Create an alias

Create an alias for your system, e.g.:

```shell
alias claude='claude --thinking-display summarized'
```

To make this permanent, add the alias definition to your shell start
script (e.g. `~/.bashrc`, `~/.zshrc`, `~/.config/fish/config.fish`,
`~/.config/nushell/config.nu`)

#### Step 2b. Create an alternative script

Step 2a is a solution when running `claude` from your terminal. If you
run Claude from another program, e.g. VS Code, you can create an
alternative script that runs `claude` with the requires option and
reference that script from your program.

For example, if `~/.local/bin` is in your path, you can create
`~/.local/bin/claude-thinking` as follows:

```shell
#!/bin/bash
exec claude --thinking-display summarized "$@"
```

Change the permissions:

```shell
$ chmod 755 ~/.local/bin/claude-thinking
```

Run `claude-thinking` when you want thinking enabled or reference the
script from any program that uses Claude Code.

### Claude Code VS Code extension

**IMPORTANT:** From our latest experience, the only way to show thinking
blocks for Opus 4.7 and later models in VS Code is to configure VS Code
to use the alternative script (Step 2b) so that
`--thinking-display summarized` is used.

Set `claudeProcessWrapper` to run the alternative script created in step
2b above.

```json
{
  "claudeCode.claudeProcessWrapper": "~/.local/bin/claude-thinking"
}
```

## Verify

To verify that thinking blocks are enabled for new sessions, create a
new session and use the model to perform a task that requires thinking.
This may take a few messages. You should see thinking blocks in your
client.

To further verify, run:

```shell
gage scan -s hidden-thinking -y
```

If your recent session contains hidden thinking text, Gage will report
the issue again.

## LLM notes

Look at the evidence below. If `showThinkingSummaries=true` then user
has configured client correctly. Check the model. If it's opus-4.7 or
later, the issue likely `claude` is not being run with
`--thinking-display summarized`.

If the evidence shows `showThinkingSummaries=false|null` then that
setting should be set to `true` in `~/.claude/settings.json` regardless
of the model used.
