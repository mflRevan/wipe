//! Output helpers: a tiny abstraction over "human text vs `--json`" plus a
//! consistent color style used across every command.

use owo_colors::{OwoColorize, Stream, Style};
use serde_json::Value;

/// Controls how a command reports its result.
#[derive(Debug, Clone, Copy)]
pub struct Out {
    /// Whether `--json` was requested.
    pub json: bool,
    /// Whether `--echo` was requested: writes print the full updated object
    /// instead of a short receipt.
    pub echo: bool,
    /// Whether `--pretty` was requested (indented JSON instead of one line).
    pub pretty: bool,
}

impl Out {
    /// Create an output mode.
    pub fn new(json: bool, echo: bool, pretty: bool) -> Self {
        Out { json, echo, pretty }
    }

    /// Print a JSON value to stdout: one compact line, or indented with `--pretty`.
    pub fn json_value(&self, value: &Value) {
        let s = if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        };
        println!("{}", s.expect("serializable"));
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

    /// Report a successful write. JSON mode prints the short `receipt` - or, with
    /// `--echo`, the `full` object; human mode prints `human` (plus the full
    /// object as JSON under `--echo`, for piping).
    pub fn write(&self, human: impl AsRef<str>, receipt: Value, full: impl FnOnce() -> Value) {
        if self.json {
            self.json_value(&if self.echo { full() } else { receipt });
        } else {
            self.ok(human, Value::Null);
            if self.echo {
                self.json_value(&full());
            }
        }
    }

    /// Print a plain human line (ignored in JSON mode).
    pub fn line(&self, text: impl AsRef<str>) {
        if !self.json {
            println!("{}", text.as_ref());
        }
    }
}

/// A one-line hint on stderr (so `--json` stdout stays a single object).
pub fn hint(text: impl AsRef<str>) {
    eprintln!(
        "{}",
        format!("hint: {}", text.as_ref())
            .if_supports_color(Stream::Stderr, |t| t.dimmed().to_string())
    );
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
        println!("{}", serde_json::to_string(&v).expect("serializable"));
    } else {
        let tag =
            "error:".if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold()));
        eprintln!("{tag} {msg}");
    }
}
