#!/usr/bin/env bash
# Effort level is provided to command hooks as $CLAUDE_EFFORT.
#
# Ref: https://code.claude.com/docs/en/hooks

printf "effort=${CLAUDE_EFFORT:-unknown}"
