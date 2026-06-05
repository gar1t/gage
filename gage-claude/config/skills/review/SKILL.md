---
description: |
  Review Gage issues and close them as completed or skipped.
---

Gage scans Claude Code sessions and records anything that warrants user
attention as an issue. An issue is open or closed.

Walk the user through their open issues and close each one according to
the user's decision.

1. Call ListIssues to see whats open

2. Work with user to identify highest value issues and start there

3. Call GetIssue to show issue detailed description, evidence, and
   comments

4. Work with user as needed to resolve the issue, either by completing
   it (e.g. applying recommended fix or another solution) or by skipping
   it. Call CloseIssue with reason completed or skipped along with a
   comment explaining either how the issue was completed or why it was
   skipped

The list of issues is advisory. Provide honest and accurate feedback to
help the user address issues based on user values and priorities.

The issue description and fix MAY contain errors. Reported evidence MAY
be outdated. Verify evidence and conduct further analysis to arrive at a
correct fix. If the issue does not warrant any action, close it with a
"skipped" reason with a comment.

Do not close an issue as "completed" until you have confirmed with the
user that the underlying issue is resolved. If the user cannot confirm
that the issue is resolved, consider waiting for more scanner evidence
and re-evaluate the issue later.
