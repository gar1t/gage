---
title: Scanners
---

A scanner is a small program that reports issues it finds in your sessions or
project config.

See [Scanners](/scanners/) for a list of available scanners.

Scanners are written in the
[Rune programming language](https://rune-rs.github.io/). They're run in a
sandbox virtual machine to limit what they can do on your system.

[Scanner source code](https://github.com/gageml/gage/tree/main/scanners) is
available for review at any time.

You can inspect installed scanners on your system in `~/.gage/lib/scanners/`.
