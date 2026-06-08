---
title: Quick start
---

:::note

Gage is in early and active development. There are only a few scanners &mdash;
but more are coming soon! To help with development please visit our
[GitHub project](https://github.com/gageml/gage).

:::

Install Gage:

```shell
curl https://raw.githubusercontent.com/gageml/gage/refs/heads/main/scripts/install.sh | sh
```

Initialize the Claude Code plugin:

```shell
gage init
```

Run a scan:

```shell
gage scan
```

View any open issues:

```shell
gage issue list
```

User Claude to review and resolve issues:

```shell
claude /gage:review
```

For steps to uninstall, see [_Uninstall Gage_](/docs/uninstall).
