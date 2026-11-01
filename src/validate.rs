//! Tool-argument validation with the rules and messages the Zod v4 schemas of the TypeScript server
//! produced, so a model sees the same errors. `tests/golden/cases.json` pins every message.

use serde_json::{Map, Value};

use crate::json::{js_number, js_trim};

/// One schema field, in the order the TypeScript schema declared it (issues follow this order).
pub struct Field {
    pub name: &'static str,
    pub kind: Kind,
    /// Applied when the key is absent, without validation (Zod's `.default()`).
    pub default: Option<Default>,
    /// An absent key is an issue instead of being skipped.
    pub required: bool,
}

#[derive(Clone, Copy)]
pub enum Default {
    Str(&'static str),
    Num(f64),
    Bool(bool),
}

pub enum Kind {
    /// `z.string().trim().min(min).max(max)`
    Str {
        min: usize,
        max: Option<usize>,
    },
    /// `z.number().int().min(min).max(max)`
    Int {
        min: f64,
        max: f64,
    },
    /// `z.number().min(min).max(max)`
    Num {
        min: f64,
        max: f64,
    },
    Bool,
    Enum(&'static [&'static str]),
    /// `z.array(z.string().trim().min(1)).max(max)`
    StrList {
        max: usize,
    },
    /// `z.string().trim().url()` refined to HTTP(S)
    WebUrl,
    /// `z.union([webUrl, z.array(webUrl).min(1).max(20)])`
    WebUrls,
}

pub enum Invalid {
    /// The schema issues, already formatted as `path: message`.
    Issues(Vec<String>),
    /// The refine's `new URL()` threw: the tool returns only this message, as the TypeScript
    /// server did.
    Thrown(String),
}

struct Issues(Vec<String>);

impl Issues {
    fn push(&mut self, path: &str, message: impl Into<String>) {
        self.0.push(format!("{path}: {}", message.into()));
    }
}

const REFINE_MESSAGE: &str = "URL must use HTTP or HTTPS.";

/// Validates `args` against `fields`; on success returns the parsed values with defaults applied
/// and strings trimmed. Unknown keys are dropped, as Zod objects strip them.
pub fn validate(fields: &[Field], args: &Map<String, Value>) -> Result<Map<String, Value>, Invalid> {
    let mut out = Map::new();
    let mut issues = Issues(Vec::new());
    for field in fields {
        let value = args.get(field.name);
        let parsed = match (value, field.default) {
            (None, Some(d)) => Some(match d {
                Default::Str(s) => Value::from(s),
                Default::Num(n) => Value::from(n),
                Default::Bool(b) => Value::from(b),
            }),
            (None, None) if !field.required => None,
            _ => check(field, value, &mut issues)?,
        };
        if let Some(v) = parsed {
            out.insert(field.name.to_string(), v);
        }
    }
    if issues.0.is_empty() { Ok(out) } else { Err(Invalid::Issues(issues.0)) }
}

fn check(field: &Field, value: Option<&Value>, issues: &mut Issues) -> Result<Option<Value>, Invalid> {
    let path = field.name;
    Ok(match &field.kind {
        Kind::Str { min, max } => check_str(path, value, *min, *max, issues),
        Kind::Int { min, max } => check_number(path, value, true, *min, *max, issues),
        Kind::Num { min, max } => check_number(path, value, false, *min, *max, issues),
        Kind::Bool => match value {
            Some(Value::Bool(b)) => Some(Value::Bool(*b)),
            other => {
                issues.push(path, invalid_type("boolean", other));
                None
            }
        },
        Kind::Enum(options) => match value {
            Some(Value::String(s)) if options.contains(&s.as_str()) => Some(Value::String(s.clone())),
            _ => {
                let list: Vec<String> = options.iter().map(|o| format!("\"{o}\"")).collect();
                issues.push(path, format!("Invalid option: expected one of {}", list.join("|")));
                None
            }
        },
        Kind::StrList { max } => check_str_list(path, value, *max, issues),
        Kind::WebUrl => match value {
            Some(Value::String(s)) => {
                let (url, refine_failed) = web_url(s)?;
                if refine_failed {
                    issues.push(path, REFINE_MESSAGE);
                }
                Some(Value::String(url))
            }
            other => {
                issues.push(path, invalid_type("string", other));
                None
            }
        },
        Kind::WebUrls => check_web_urls(path, value, issues)?,
    })
}

fn check_str(path: &str, value: Option<&Value>, min: usize, max: Option<usize>, issues: &mut Issues) -> Option<Value> {
    match value {
        Some(Value::String(s)) => {
            let trimmed = js_trim(s);
            let len = trimmed.chars().count();
            if len < min {
                issues.push(path, format!("Too small: expected string to have >={min} characters"));
            }
            if let Some(max) = max.filter(|m| len > *m) {
                issues.push(path, format!("Too big: expected string to have <={max} characters"));
            }
            Some(Value::String(trimmed.to_string()))
        }
        other => {
            issues.push(path, invalid_type("string", other));
            // Zod still runs length checks on anything with a `.length`, such as an array.
            if let Some(Value::Array(items)) = other {
                if items.len() < min {
                    issues.push(path, format!("Too small: expected array to have >={min} items"));
                }
                if let Some(max) = max.filter(|m| items.len() > *m) {
                    issues.push(path, format!("Too big: expected array to have <={max} items"));
                }
            }
            None
        }
    }
}

/// `int` adds Zod's `.int()` (safe integer), whose failure stops the remaining checks.
fn check_number(path: &str, value: Option<&Value>, int: bool, min: f64, max: f64, issues: &mut Issues) -> Option<Value> {
    let x = match value {
        Some(Value::Number(n)) if crate::json::as_js_f64(n).is_finite() => crate::json::as_js_f64(n),
        other => {
            issues.push(path, invalid_type("number", other));
            return None;
        }
    };
    if int {
        const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
        if x.fract() != 0.0 {
            issues.push(path, "Invalid input: expected int, received number");
            return None;
        }
        // Unlike a fraction, an unsafe integer does not stop the min/max checks that follow.
        if x > MAX_SAFE {
            issues.push(path, format!("Too big: expected int to be <={}", js_number(MAX_SAFE)));
        }
        if x < -MAX_SAFE {
            issues.push(path, format!("Too small: expected int to be >={}", js_number(-MAX_SAFE)));
        }
    }
    if x < min {
        issues.push(path, format!("Too small: expected number to be >={}", js_number(min)));
    }
    if x > max {
        issues.push(path, format!("Too big: expected number to be <={}", js_number(max)));
    }
    Some(Value::from(x))
}

fn check_str_list(path: &str, value: Option<&Value>, max: usize, issues: &mut Issues) -> Option<Value> {
    match value {
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let item_path = format!("{path}.{i}");
                if let Some(v) = check_str(&item_path, Some(item), 1, None, issues) {
                    out.push(v);
                }
            }
            if items.len() > max {
                issues.push(path, format!("Too big: expected array to have <={max} items"));
            }
            Some(Value::Array(out))
        }
        other => {
            issues.push(path, invalid_type("array", other));
            if let Some(Value::String(s)) = other {
                if s.chars().count() > max {
                    issues.push(path, format!("Too big: expected string to have <={max} characters"));
                }
            }
            None
        }
    }
}

/// Returns the trimmed URL and whether the HTTP(S) refine failed. An unparsable URL makes the
/// refine's `new URL()` throw, which ended the whole TypeScript validation with "Invalid URL".
fn web_url(raw: &str) -> Result<(String, bool), Invalid> {
    let trimmed = js_trim(raw);
    match url::Url::parse(trimmed) {
        Ok(u) => Ok((trimmed.to_string(), !matches!(u.scheme(), "http" | "https"))),
        Err(_) => Err(Invalid::Thrown("Invalid URL".into())),
    }
}

fn check_web_urls(path: &str, value: Option<&Value>, issues: &mut Issues) -> Result<Option<Value>, Invalid> {
    match value {
        // The string branch of the union.
        Some(Value::String(s)) => {
            let (url, refine_failed) = web_url(s)?;
            if refine_failed {
                issues.push(path, REFINE_MESSAGE);
                return Ok(None);
            }
            Ok(Some(Value::Array(vec![Value::String(url)])))
        }
        // The array branch. A wrong item type aborts it, and with both branches aborted Zod
        // reports only "Invalid input".
        Some(Value::Array(items)) => {
            let mut branch = Issues(Vec::new());
            let mut aborted = false;
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                match item {
                    Value::String(s) => {
                        let (url, refine_failed) = web_url(s)?;
                        if refine_failed {
                            branch.push(&format!("{path}.{i}"), REFINE_MESSAGE);
                        }
                        out.push(Value::String(url));
                    }
                    _ => aborted = true,
                }
            }
            if aborted {
                issues.push(path, "Invalid input");
                return Ok(None);
            }
            if items.is_empty() {
                branch.push(path, "Too small: expected array to have >=1 items");
            }
            if items.len() > 20 {
                branch.push(path, "Too big: expected array to have <=20 items");
            }
            if branch.0.is_empty() {
                Ok(Some(Value::Array(out)))
            } else {
                issues.0.extend(branch.0);
                Ok(None)
            }
        }
        _ => {
            issues.push(path, "Invalid input");
            Ok(None)
        }
    }
}

/// Zod's `invalid_type` message, shared by tool arguments, the `_meta` envelope and request params.
pub fn invalid_type(expected: &str, received: Option<&Value>) -> String {
    format!("Invalid input: expected {expected}, received {}", type_name(received))
}

/// Zod's name for the type of a JSON value.
pub fn type_name(value: Option<&Value>) -> &'static str {
    match value {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(n)) => match crate::json::as_js_f64(n) {
            x if x == f64::INFINITY => "Infinity",
            x if x == f64::NEG_INFINITY => "-Infinity",
            _ => "number",
        },
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}
