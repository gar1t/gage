+++
name = "ListIssues"

[parameters.status]
type = "string"
required = false
description = "Optional status (default is 'open'): closed, open, any"

[annotations]
read_only_hint = true
idempotent_hint = true
+++

List issues. By default open issues are listed.

An issue is something noteworthy that should be investigated and closed,
either by applying a fix or skipping the issue.

Currently you don't have a tool for closing issues.

---eof-456---
