//! `--env-file` / `--env-file-if-exists`, read the way Node reads them, so the existing launch
//! arguments keep working. Variables set in the real environment win; among several files the
//! last one wins.

use std::collections::HashSet;
use std::ffi::OsString;

/// Parses dotenv text into `(key, value)` pairs in file order.
pub fn parse(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map(str::trim_start).unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value.trim();
        let quote = value.chars().next().filter(|c| matches!(c, '"' | '\'' | '`'));
        let value = match quote {
            Some(q) => {
                let rest = &value[1..];
                if let Some(end) = rest.find(q) {
                    unquote(q, &rest[..end])
                } else {
                    // A quoted value may continue over several lines until its closing quote.
                    let mut joined = rest.to_string();
                    let mut closed = false;
                    for next in lines.by_ref() {
                        joined.push('\n');
                        if let Some(end) = next.find(q) {
                            joined.push_str(&next[..end]);
                            closed = true;
                            break;
                        }
                        joined.push_str(next);
                    }
                    if closed { unquote(q, &joined) } else { value.to_string() }
                }
            }
            None => value.split('#').next().unwrap_or_default().trim().to_string(),
        };
        out.push((key.to_string(), value));
    }
    out
}

fn unquote(quote: char, inner: &str) -> String {
    if quote == '"' { inner.replace("\\n", "\n") } else { inner.to_string() }
}

/// The variables of the real environment, which no env file may override.
pub fn protected() -> HashSet<OsString> {
    std::env::vars_os().map(|(k, _)| k).collect()
}

/// Loads the file into the process environment, skipping `protected` variables. A missing file is
/// an error only when `required`, like Node's `--env-file` versus `--env-file-if-exists`.
pub fn load(path: &str, required: bool, protected: &HashSet<OsString>) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if !required && e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{path}: {e}")),
    };
    for (key, value) in parse(&text) {
        if !protected.contains(&OsString::from(&key)) {
            // SAFETY: called from `main` before any other thread exists.
            unsafe { std::env::set_var(key, value) };
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_the_forms_node_accepts() {
        let text = "# comment\nTAVILY_API_KEY=tvly-abc\nexport A = spaced value # note\nB=\"line1\\nline2\"\nC='raw \\n'\nD=\"multi\nline\"\nE=`tick\nmore`\n\nnot a pair\n=novalue\n";
        let got = parse(text);
        let want = [
            ("TAVILY_API_KEY", "tvly-abc"),
            ("A", "spaced value"),
            ("B", "line1\nline2"),
            ("C", "raw \\n"),
            ("D", "multi\nline"),
            ("E", "tick\nmore"),
        ];
        assert_eq!(got.len(), want.len(), "{got:?}");
        for ((k, v), (wk, wv)) in got.iter().zip(want) {
            assert_eq!((k.as_str(), v.as_str()), (wk, wv));
        }
    }
}
