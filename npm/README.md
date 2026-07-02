# wipe (npm wrapper)

This package is a **thin install wrapper**, not a JavaScript implementation
of `wipe`. `wipe` is a Rust CLI (see the [main repo][repo]); this package
exists purely so you can install it with `npm i -g wipe` / run it with
`npx wipe` if that's more convenient than a Rust toolchain in your workflow.

## What it does

On `npm install`, the package's `postinstall` script (`install.js`)
downloads the prebuilt `wipe` binary that matches your OS/architecture from
the project's [GitHub Releases][releases] (built by [cargo-dist][cargo-dist])
and stores it in this package's `bin/` directory. The `wipe` command
installed on your `$PATH` is a tiny launcher (`bin/wipe.js`) that execs that
binary, forwarding all arguments, stdio, and the exit code.

No JavaScript dependencies are installed -- `install.js` only uses Node
built-ins, and shells out to the OS's own `tar` (or, on Windows,
PowerShell's `Expand-Archive` as a fallback) to unpack the downloaded
archive.

Supported platforms: Linux x64 (glibc or musl), macOS x64/arm64, and
Windows x64. If your platform isn't covered, installation will fail with a
message pointing you at `cargo install` / building from source instead of
silently doing nothing.

## Alternative: install with Cargo

If you already have a Rust toolchain, installing directly with Cargo avoids
the download-a-binary-over-npm indirection entirely:

```sh
cargo install wipe-cli   # once published to crates.io
# or, from a checkout / for the latest main:
cargo install --git https://github.com/mflRevan/wipe wipe-cli
```

## Version pinning

This package's version tracks the Rust crate's version (see the workspace's
`Cargo.toml`). `install.js` downloads the GitHub Release tagged
`v<package-version>`, so `npm install wipe@0.1.0` always fetches the
`wipe-cli` v0.1.0 binaries -- there's no separate "npm package version" to
keep in sync by hand.

## Troubleshooting

- **Installed with `--ignore-scripts`?** The binary was never downloaded.
  Run `npm rebuild wipe` (or reinstall without `--ignore-scripts`).
- **Corporate proxy / no network at install time?** Download the archive
  manually from the [Releases page][releases], extract the `wipe` binary,
  and drop it into this package's `bin/` directory (or just put it directly
  on your `$PATH` and skip this package entirely).

[repo]: https://github.com/mflRevan/wipe
[releases]: https://github.com/mflRevan/wipe/releases
[cargo-dist]: https://opensource.axo.dev/cargo-dist/
