//! Output helpers: a tiny abstraction over "human text vs `--json`" plus a
//! consistent color style used across every command.

use owo_colors::{OwoColorize, Stream, Style};
use serde_json::Value;

/// Controls how a command reports its result.
#[derive(Debug, Clone, Copy)]
pub struct Out {
    /// Whether `--json` was requested.
    pub json: bool,
}

impl Out {
    /// Create an output mode.
    pub fn new(json: bool) -> Self {
        Out { json }
    }

    /// Print a JSON value (pretty) to stdout.
    pub fn json_value(&self, value: &Value) {
        println!(
            "{}",
            serde_json::to_string_pretty(value).expect("serializable")
        );
    }

    /// Report success. In JSON mode prints `value`; otherwise prints `human`.
    pub fn ok(&self, human: impl AsRef<str>, value: Value) {
        if self.json {
            self.json_value(&value);
        } else {
            println!(
                "{} {}",
                "✓".if_supports_color(Stream::Stdout, |t| t.green()),
                human.as_ref()
            );
        }
    }

    /// Print a plain human line (ignored in JSON mode).
    pub fn line(&self, text: impl AsRef<str>) {
        if !self.json {
            println!("{}", text.as_ref());
        }
    }
}

/// Style a ticket ID for human output.
pub fn id_style(id: &str) -> String {
    id.if_supports_color(Stream::Stdout, |t| t.bold().to_string())
        .to_string()
}

/// Style dim/secondary text.
pub fn dim(text: &str) -> String {
    text.if_supports_color(Stream::Stdout, |t| t.dimmed().to_string())
        .to_string()
}

/// Print an error to the appropriate stream. In JSON mode a machine-readable
/// error object goes to stdout (so agents always parse stdout); otherwise a
/// styled message goes to stderr.
pub fn emit_error(json: bool, msg: &str) {
    if json {
        let v = serde_json::json!({ "ok": false, "error": msg });
        println!(
            "{}",
            serde_json::to_string_pretty(&v).expect("serializable")
        );
    } else {
        let tag =
            "error:".if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold()));
        eprintln!("{tag} {msg}");
    }
}
