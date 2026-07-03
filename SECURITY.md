# Security Policy

## Supported Versions

`wipe` is **pre-1.0** and under active development. There are no long-term
support branches yet - only the **latest released version** is supported
with security fixes. Please make sure you are on the most recent release
before reporting an issue, and be prepared to upgrade as part of any fix.

## Reporting a Vulnerability

If you discover a security vulnerability in `wipe`, please report it
responsibly and **do not open a public GitHub issue**.

Instead, email **aiman@shabib.net** with:

- A description of the vulnerability and its potential impact.
- Steps to reproduce it (proof-of-concept code or commands are very helpful).
- The version/commit of `wipe` you tested against.

We will acknowledge your report as soon as possible, investigate, and work
with you on a fix and coordinated disclosure timeline. Please give us a
reasonable amount of time to address the issue before any public disclosure.

## Scope

Given `wipe`'s git-native design, areas of particular interest for security
reports include (but are not limited to):

- Handling of untrusted `.wipe/` board data (e.g. maliciously crafted
  `board.json`, `definitions.json`, or ticket files from a cloned repo).
- The local daemon (`wipe-daemon`) and its network/IPC surface.
- Any code path that shells out, reads environment variables, or handles
  file paths derived from repository content.

Thank you for helping keep `wipe` and its users safe.
