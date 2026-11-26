# Security policy

## Supported versions

rust-use is pre-1.0. Security fixes land on the latest released minor version only.

## Reporting a vulnerability

Please do not open a public issue. Use the repository's **Security** tab → *Report a
vulnerability*, or go straight to
[the private advisory form](https://github.com/better-vault/rust-use/security/advisories/new).
Reports there are visible only to the maintainers.

rust-use lets software act on a user interface, so reports about the agent bridge are especially
welcome: a way around an `allow` permission hook, an IPC socket that accepts connections it
should not, or a snapshot that leaks state a component did not expose.

You can expect an acknowledgement within a few days, and a fix or a plan before anything is
disclosed.
