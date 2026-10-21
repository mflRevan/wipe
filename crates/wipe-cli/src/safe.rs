//! Guardrails for printing user-written content (titles, bodies, comments,
//! forum posts) into a terminal or an agent's context.
//!
//! Board text is written by anyone - humans pasting from the web, agents
//! pasting tool output - and is printed into terminals and LLM contexts. So
//! before it is printed:
//!
//! - terminal escape sequences, other control characters and bidi overrides are
//!   removed (no cursor tricks, no hidden "trojan source" reordering);
//! - `data:…;base64,…` URIs and long encoded runs (base64, hex) become a short
//!   `[… KB … omitted]` marker - a pasted screenshot never floods a context;
//! - unbroken runs of more than [`MAX_RUN`] characters (minified JSON, a giant
//!   URL) are cut, keeping the start.
//!
//! [`clip`] then bounds a field to a character budget, saying how much was left
//! out so the reader knows to ask for more.

use std::borrow::Cow;

/// Longest run of base64/hex-like characters printed as is.
const MAX_ENCODED: usize = 200;
/// Longest unbroken (whitespace-free) run printed as is.
pub const MAX_RUN: usize = 400;

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

fn is_encoded(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_')
}

/// Remove control sequences and characters (keeping `\n` and `\t`).
fn strip_controls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\u{1b}' => match it.next() {
                // CSI: parameters until a final byte in @..~
                Some('[') => {
                    for d in it.by_ref() {
                        if ('@'..='~').contains(&d) {
                            break;
                        }
                    }
                }
                // OSC: until BEL or ST (ESC \)
                Some(']') => {
                    while let Some(d) = it.next() {
                        if d == '\u{7}' {
                            break;
                        }
                        if d == '\u{1b}' {
                            it.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' => {
                if it.peek() != Some(&'\n') {
                    out.push('\n');
                }
            }
            '\n' | '\t' => out.push(c),
            c if c.is_control() || is_bidi_control(c) => {}
            c => out.push(c),
        }
    }
    out
}

fn size(n: usize) -> String {
    if n >= 1024 {
        format!("{} KB", n.div_ceil(1024))
    } else {
        format!("{n} B")
    }
}

/// Collapse encoded blobs and over-long unbroken runs.
fn collapse_runs(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(64 * 1024));
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // One whitespace-free token.
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let token: String = chars[start..i].iter().collect();
        out.push_str(&collapse_token(&token));
    }
    out
}

fn collapse_token(token: &str) -> Cow<'_, str> {
    // `data:image/png;base64,AAAA...` (possibly inside markdown `![](...)`).
    if let Some(at) = token.find("data:") {
        if let Some(rel) = token[at..].find(";base64,") {
            let mime = &token[at + 5..at + rel];
            let payload_start = at + rel + ";base64,".len();
            let payload_len = token[payload_start..]
                .chars()
                .take_while(|c| is_encoded(*c))
                .count();
            if payload_len > 32 {
                let rest = &token[payload_start + payload_len..];
                let mime = if mime.is_empty() { "data" } else { mime };
                return Cow::Owned(format!(
                    "{}[{mime} base64, {} omitted]{}",
                    &token[..at],
                    size(payload_len * 3 / 4),
                    collapse_token(rest)
                ));
            }
        }
    }
    let n = token.chars().count();
    // A long run that is all base64/hex characters is encoded data.
    let mut out = String::new();
    let mut run = String::new();
    let mut changed = false;
    let flush = |run: &mut String, out: &mut String, changed: &mut bool| {
        let len = run.chars().count();
        if len > MAX_ENCODED && run.chars().any(|c| c.is_ascii_digit()) {
            out.push_str(&format!("[encoded data, {} omitted]", size(len * 3 / 4)));
            *changed = true;
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in token.chars() {
        if is_encoded(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out, &mut changed);
            out.push(c);
        }
    }
    flush(&mut run, &mut out, &mut changed);
    let out_n = out.chars().count();
    if out_n > MAX_RUN {
        let head: String = out.chars().take(MAX_RUN / 2).collect();
        return Cow::Owned(format!("{head}…[{} more chars]", out_n - MAX_RUN / 2));
    }
    if changed || out_n != n {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(token)
    }
}

/// Make `s` safe to print: no control sequences, no encoded blobs, no endless
/// lines. Returns `s` unchanged (borrowed) in the common case.
pub fn clean(s: &str) -> Cow<'_, str> {
    let needs_strip = s
        .chars()
        .any(|c| (c.is_control() && c != '\n' && c != '\t') || is_bidi_control(c));
    let long_token = s
        .split_whitespace()
        .any(|t| t.len() > MAX_ENCODED || t.contains(";base64,"));
    if !needs_strip && !long_token {
        return Cow::Borrowed(s);
    }
    let stripped = if needs_strip {
        strip_controls(s)
    } else {
        s.to_string()
    };
    Cow::Owned(
        if long_token || stripped.split_whitespace().any(|t| t.len() > MAX_ENCODED) {
            collapse_runs(&stripped)
        } else {
            stripped
        },
    )
}

/// [`clean`] on one line: newlines and runs of whitespace become one space.
pub fn line(s: &str) -> String {
    clean(s).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// [`clean`] `s` and bound it to `max` characters. Returns the text and how
/// many characters were left out (0 when it fit). Cuts at a line or word break
/// near the limit when there is one.
pub fn clip(s: &str, max: usize) -> (String, usize) {
    let c = clean(s);
    let total = c.chars().count();
    if total <= max {
        return (c.into_owned(), 0);
    }
    let head: String = c.chars().take(max).collect();
    let cut = head
        .rfind('\n')
        .filter(|&b| head[..b].chars().count() > max * 2 / 3)
        .or_else(|| {
            head.rfind(' ')
                .filter(|&b| head[..b].chars().count() > max * 2 / 3)
        })
        .map(|b| head[..b].to_string())
        .unwrap_or(head);
    let kept = cut.chars().count();
    (cut.trim_end().to_string(), total - kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_strips_controls_and_collapses_encoded_blobs() {
        let s = "Fix the foam - see T-02A.\n\n- [x] done\tok ✓ Ünïcødé";
        assert!(
            matches!(clean(s), Cow::Borrowed(_)),
            "no copy in the common case"
        );

        let s = "red \u{1b}[31mALERT\u{1b}[0m title\u{1b}]0;pwned\u{7} bell\u{7} \u{202E}txt.exe\r\nnext";
        assert_eq!(clean(s), "red ALERT title bell txt.exe\nnext");

        let png = format!(
            "before ![shot](data:image/png;base64,{}) after",
            "iVBORw0KGgo".repeat(400)
        );
        let got = clean(&png);
        assert!(
            got.starts_with("before ![shot]([image/png base64, "),
            "{got}"
        );
        assert!(got.ends_with("omitted]) after"), "{got}");
        assert!(got.len() < 80, "{got}");

        let blob = format!("key: {}", "aGVsbG8gd29ybGQ1".repeat(100));
        assert_eq!(clean(&blob), "key: [encoded data, 2 KB omitted]");
        // A long but normal URL survives; a giant unbroken token is cut.
        let url = "https://example.com/a/b/c?x=1&y=2";
        assert_eq!(clean(url), url);
        let giant = "{\"a\":1,".repeat(200);
        let got = clean(&giant);
        assert!(
            got.contains("more chars]") && got.chars().count() < 260,
            "{got}"
        );
    }

    #[test]
    fn clip_reports_what_was_left_out() {
        assert_eq!(clip("short", 10), ("short".into(), 0));
        let (t, rest) = clip("one two three four five six seven", 20);
        assert_eq!(t, "one two three four");
        assert_eq!(rest, 33 - 18);
        assert_eq!(line("a\n\n b\t c"), "a b c");
    }
}
