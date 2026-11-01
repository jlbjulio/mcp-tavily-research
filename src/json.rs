//! JSON output that matches what Node's `JSON.stringify` writes byte for byte: JavaScript number
//! formatting, JavaScript property order and `JSON.stringify` string escaping. MCP clients get the
//! same bytes the TypeScript server sent.

use serde_json::{Map, Value};

/// `Number.prototype.toString()` for a finite or non-finite double.
pub fn js_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    // `{:e}` prints the shortest digits that round-trip, which is what ECMAScript requires.
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').expect("LowerExp always has an exponent");
    let exp: i32 = exp.parse().expect("LowerExp exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let mut s = String::new();
    if x < 0.0 {
        s.push('-');
    }
    if k <= n && n <= 21 {
        s.push_str(&digits);
        s.push_str(&"0".repeat((n - k) as usize));
    } else if 0 < n && n <= 21 {
        s.push_str(&digits[..n as usize]);
        s.push('.');
        s.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        s.push_str("0.");
        s.push_str(&"0".repeat((-n) as usize));
        s.push_str(&digits);
    } else {
        s.push_str(&digits[..1]);
        if k > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        let e = n - 1;
        s.push('e');
        s.push(if e >= 0 { '+' } else { '-' });
        s.push_str(&e.abs().to_string());
    }
    s
}

/// `Number.prototype.toFixed(digits)`: rounds the exact binary value, ties go up.
pub fn js_to_fixed(x: f64, digits: usize) -> String {
    if !x.is_finite() || x.abs() >= 1e21 {
        return js_number(x);
    }
    // 1100 fractional digits print every double exactly (the longest needs 1074).
    let exact = format!("{:.1100}", x.abs());
    let (int_part, frac) = exact.split_once('.').expect("fixed precision has a point");
    let mut kept: Vec<u8> = int_part.bytes().chain(frac.bytes().take(digits)).collect();
    if frac.as_bytes()[digits] >= b'5' {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let mut s = String::new();
    if x < 0.0 {
        s.push('-');
    }
    let split = kept.len() - digits;
    s.push_str(std::str::from_utf8(&kept[..split]).expect("ascii digits"));
    if digits > 0 {
        s.push('.');
        s.push_str(std::str::from_utf8(&kept[split..]).expect("ascii digits"));
    }
    s
}

/// A number from parsed JSON, read the way `JSON.parse` reads it: always a double, and
/// `Infinity` when it overflows (`1e400`).
pub fn as_js_f64(n: &serde_json::Number) -> f64 {
    n.to_string().parse().expect("serde_json keeps valid JSON number text")
}

/// Replaces `\uXXXX` escapes of lone UTF-16 surrogates with `�`. `JSON.parse` accepts them,
/// Rust strings cannot hold them, and rejecting the whole line would leave a request unanswered.
pub fn replace_lone_surrogates(line: &str) -> std::borrow::Cow<'_, str> {
    if !line.contains("\\u") {
        return line.into();
    }
    let surrogate = |s: &str| u16::from_str_radix(s, 16).ok().filter(|u| (0xD800..=0xDFFF).contains(u));
    let escape_at = |s: &str, i: usize| s.get(i..i + 6).filter(|e| e.starts_with("\\u")).and_then(|e| surrogate(&e[2..]));
    let mut out = String::with_capacity(line.len());
    let (mut in_string, mut i) = (false, 0);
    while let Some(c) = line[i..].chars().next() {
        if !in_string {
            in_string = c == '"';
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        match c {
            '"' => in_string = false,
            '\\' => match escape_at(line, i) {
                Some(0xD800..=0xDBFF) if escape_at(line, i + 6).is_some_and(|low| low >= 0xDC00) => {
                    out.push_str(&line[i..i + 12]);
                    i += 12;
                    continue;
                }
                Some(_) => {
                    out.push_str("\\ufffd");
                    i += 6;
                    continue;
                }
                None => {
                    // Copy the escape and the character it escapes, so `\\` or `\"` do not end
                    // or confuse the scan.
                    out.push('\\');
                    i += 1;
                    if let Some(next) = line[i..].chars().next() {
                        out.push(next);
                        i += next.len_utf8();
                    }
                    continue;
                }
            },
            _ => {}
        }
        out.push(c);
        i += c.len_utf8();
    }
    out.into()
}

/// `JSON.stringify(value)`.
pub fn stringify(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out, None, 0);
    out
}

/// `JSON.stringify(value, null, 2)`.
pub fn stringify_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out, Some("  "), 0);
    out
}

/// `JSON.stringify(string)`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    write_str(s, &mut out);
    out
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(value: &Value, out: &mut String, indent: Option<&str>, level: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let x = as_js_f64(n);
            // JSON.stringify writes null for Infinity.
            out.push_str(&if x.is_finite() { js_number(x) } else { "null".into() });
        }
        Value::String(s) => write_str(s, out),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, indent, level + 1);
                write_value(item, out, indent, level + 1);
            }
            newline(out, indent, level);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, indent, level + 1);
                write_str(key, out);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_value(item, out, indent, level + 1);
            }
            newline(out, indent, level);
            out.push('}');
        }
    }
}

fn newline(out: &mut String, indent: Option<&str>, level: usize) {
    if let Some(unit) = indent {
        out.push('\n');
        for _ in 0..level {
            out.push_str(unit);
        }
    }
}

/// Reorders every object the way a JavaScript engine orders own properties: keys that are array
/// indices first, ascending, then the rest in insertion order.
pub fn js_ordered(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(js_ordered).collect()),
        Value::Object(map) => {
            let mut indices: Vec<(u32, String, Value)> = Vec::new();
            let mut rest = Map::new();
            for (key, item) in map {
                match array_index(&key) {
                    Some(i) => indices.push((i, key, js_ordered(item))),
                    None => {
                        rest.insert(key, js_ordered(item));
                    }
                }
            }
            indices.sort_by_key(|(i, _, _)| *i);
            let mut out = Map::new();
            for (_, key, item) in indices {
                out.insert(key, item);
            }
            out.extend(rest);
            Value::Object(out)
        }
        other => other,
    }
}

fn array_index(key: &str) -> Option<u32> {
    let n: u32 = key.parse().ok()?;
    (n != u32::MAX && n.to_string() == key).then_some(n)
}

/// `String.prototype.trim()`: ECMAScript WhiteSpace and LineTerminator, which differ from Rust's
/// `str::trim` (JavaScript trims U+FEFF and keeps U+0085).
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// `str.slice(0, n)` in UTF-16 code units. A surrogate pair cut in half becomes U+FFFD because a
/// Rust string cannot hold the lone surrogate JavaScript would keep.
pub fn js_slice_utf16(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().take(n + 1).collect();
    if units.len() <= n {
        return s.to_string();
    }
    String::from_utf16_lossy(&units[..n])
}

/// `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_like_javascript() {
        let cases = [
            (1.0, "1"),
            (1.5, "1.5"),
            (-0.0, "0"),
            (0.000001, "0.000001"),
            (1e-7, "1e-7"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123456789012345680000.0, "123456789012345680000"),
            (1.5e300, "1.5e+300"),
            (-2.5e-10, "-2.5e-10"),
            (0.1 + 0.2, "0.30000000000000004"),
        ];
        for (x, want) in cases {
            assert_eq!(js_number(x), want, "{x}");
        }
    }

    #[test]
    fn to_fixed_rounds_exact_value_half_up() {
        let cases = [
            (0.0625, "0.063"),
            (0.1235, "0.123"),
            (0.9995, "1.000"),
            (2.675, "2.675"),
            (9.9996, "10.000"),
            (-0.0005, "-0.001"),
            (-0.0, "0.000"),
            (0.0, "0.000"),
            (1e21, "1e+21"),
        ];
        for (x, want) in cases {
            assert_eq!(js_to_fixed(x, 3), want, "{x}");
        }
    }

    #[test]
    fn lone_surrogate_escapes_become_replacement_characters() {
        // Built from parts so no editor or tool turns the escapes into characters.
        let e = |hex: &str| format!("{}{}{hex}", '\\', 'u');
        let (hi, lo, rep, bs) = (e("d83d"), e("de00"), e("fffd"), '\\');
        let cases = [
            (format!(r#"{{"q":"a {hi} b"}}"#), format!(r#"{{"q":"a {rep} b"}}"#)),
            (format!(r#"{{"q":"{lo}x"}}"#), format!(r#"{{"q":"{rep}x"}}"#)),
            (format!(r#"{{"q":"{hi}{lo}"}}"#), format!(r#"{{"q":"{hi}{lo}"}}"#)),
            (format!(r#"{{"q":"{bs}{hi}"}}"#), format!(r#"{{"q":"{bs}{hi}"}}"#)),
            (format!(r#"{{"q":"{bs}"{hi}"}}"#), format!(r#"{{"q":"{bs}"{rep}"}}"#)),
            (format!(r#"{{"q":"{}{}"}}"#, e("0041"), e("d800")), format!(r#"{{"q":"{}{rep}"}}"#, e("0041"))),
        ];
        for (input, want) in cases {
            assert_eq!(replace_lone_surrogates(&input), want, "{input}");
        }
    }

    #[test]
    fn overflowing_numbers_read_as_infinity_and_print_as_null() {
        let v: Value = serde_json::from_str(r#"[1e400,-1e400,1.0]"#).unwrap();
        let n: Vec<f64> = v.as_array().unwrap().iter().map(|x| as_js_f64(x.as_number().unwrap())).collect();
        assert_eq!(n, [f64::INFINITY, f64::NEG_INFINITY, 1.0]);
        assert_eq!(stringify(&v), "[null,null,1]");
    }

    #[test]
    fn trim_uses_javascript_whitespace() {
        assert_eq!(js_trim("\u{feff}\u{a0} a \u{3000}"), "a");
        assert_eq!(js_trim("\u{85}a\u{85}"), "\u{85}a\u{85}");
    }

    #[test]
    fn utf16_slice_never_splits_into_invalid_text() {
        assert_eq!(js_slice_utf16("ab😀", 3), "ab\u{fffd}");
        assert_eq!(js_slice_utf16("ab😀", 4), "ab😀");
        assert_eq!(js_slice_utf16("abc", 500), "abc");
    }
}
