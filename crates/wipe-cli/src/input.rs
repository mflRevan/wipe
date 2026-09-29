//! Long-text arguments (ticket bodies, comments, forum posts) from a flag, a file,
//! or stdin.
//!
//! Passing multi-line text as a command-line argument is fragile: on Windows,
//! anything routed through `cmd.exe` (npm's `wipe.cmd` shim, `shell=True`)
//! silently cuts the argument at the first line break. `--body-file <path>` and
//! `--body -` (stdin) side-step argument quoting entirely on every platform.

use std::io::Read as _;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{bail, Context, Result};

/// Set once stdin has been consumed, so two `-` arguments fail loudly instead of
/// the second one silently reading nothing.
static STDIN_TAKEN: AtomicBool = AtomicBool::new(false);

fn read_stdin(what: &str) -> Result<String> {
    if STDIN_TAKEN.swap(true, Ordering::SeqCst) {
        bail!("only one argument per command can read stdin (`-`); {what} asked for it too");
    }
    let mut s = String::new();
    std::io::stdin()
        .read_to_string(&mut s)
        .with_context(|| format!("reading {what} from stdin"))?;
    Ok(s)
}

fn read_file(path: &Path, what: &str) -> Result<String> {
    if path.as_os_str() == "-" {
        return read_stdin(what);
    }
    let bytes =
        std::fs::read(path).with_context(|| format!("reading {what} from {}", path.display()))?;
    let s = String::from_utf8(bytes)
        .with_context(|| format!("{} is not UTF-8 text", path.display()))?;
    // A BOM (common from Windows editors / PowerShell `Out-File`) is not content.
    Ok(s.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(s))
}

/// Resolve a text argument given inline (`value`, where `-` means stdin) or as a
/// file (`file`, where `-` also means stdin). File and stdin sources lose their
/// trailing line breaks (editors add them; they are not content). Returns `None`
/// when neither was given.
pub fn text(value: Option<String>, file: Option<&Path>, what: &str) -> Result<Option<String>> {
    match (value, file) {
        (Some(_), Some(_)) => bail!("pass {what} inline or as a file, not both"),
        (Some(v), None) if v == "-" => Ok(Some(trim_trailing_newlines(read_stdin(what)?))),
        (Some(v), None) => Ok(Some(v)),
        (None, Some(p)) => Ok(Some(trim_trailing_newlines(read_file(p, what)?))),
        (None, None) => Ok(None),
    }
}

/// Like [`text`], but the text is mandatory.
pub fn required(
    value: Option<String>,
    file: Option<&Path>,
    what: &str,
    flag: &str,
) -> Result<String> {
    text(value, file, what)?.ok_or_else(|| {
        anyhow::anyhow!(
            "{what} is required - pass {flag} <TEXT>, {flag}-file <PATH>, or {flag} - to read stdin"
        )
    })
}

fn trim_trailing_newlines(mut s: String) -> String {
    while s.ends_with('\n') || s.ends_with('\r') {
        s.pop();
    }
    s
}
