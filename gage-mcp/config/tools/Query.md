+++
name = "Query"

[parameters.sql]
type = "string"
required = true
description = "SQL query to execute against Gage session data."

[annotations]
read_only_hint = true
idempotent_hint = true
+++

Execute PostgreSQL SQL againt Claude Code sessions and Gage notes.

Tables:

- session (id, project, path, size, mtime title, message_count,
  is_empty) - list of available sessions

- entry (session_id, line, type, raw) - get raw JSON per line per
  session

- message (session_id, line, type, subtype, text, timestamp) - get
  conversation text (user and assistant) per session

- note (id, name, value, metadata, finding, fix) - get notes written by
  scanners including `fixing` and `fix` text

Hints:

- Users often refer to sessions using their prefix ID
- `message.text` is convenient for message text content in one value
- `entry.raw` is same as reading session JSONL at `line`

---eof-123---
