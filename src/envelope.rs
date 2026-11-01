//! The per-request `_meta` envelope of protocol revision 2026-07-28, validated with the schema and
//! messages of the TypeScript SDK (`RequestMetaEnvelopeSchema`). Pinned by
//! `tests/golden/protocol.json`.

use serde_json::{Map, Value};

use crate::validate::invalid_type;

pub const PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";
pub const CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
pub const LOG_LEVEL: &str = "io.modelcontextprotocol/logLevel";
const RESERVED: [&str; 4] = [PROTOCOL_VERSION, CLIENT_INFO, CLIENT_CAPABILITIES, LOG_LEVEL];

pub const MODERN_VERSION: &str = "2026-07-28";

pub struct Issue {
    pub path: Vec<String>,
    pub message: String,
}

struct Issues(Vec<Issue>);

impl Issues {
    fn push(&mut self, path: &[&str], message: String) {
        self.0.push(Issue { path: path.iter().map(|s| s.to_string()).collect(), message });
    }
    fn wrong_type(&mut self, path: &[&str], expected: &str, got: &Value) {
        self.push(path, invalid_type(expected, Some(got)));
    }
}

/// `params._meta`, when it is an object.
pub fn meta(params: Option<&Value>) -> Option<&Map<String, Value>> {
    params.and_then(|p| p.get("_meta")).and_then(Value::as_object)
}

/// The envelope's protocol version claim, when `_meta` names one.
pub fn claim(params: Option<&Value>) -> Option<&Value> {
    meta(params).and_then(|m| m.get(PROTOCOL_VERSION))
}

/// The issues `RequestMetaEnvelopeSchema` reports for `meta`, in schema order.
pub fn schema_issues(meta: &Map<String, Value>) -> Vec<Issue> {
    let mut issues = Issues(Vec::new());
    match meta.get(PROTOCOL_VERSION) {
        Some(Value::String(_)) => {}
        Some(other) => issues.wrong_type(&[PROTOCOL_VERSION], "string", other),
        None => issues.push(&[PROTOCOL_VERSION], invalid_type("string", None)),
    }
    if let Some(info) = meta.get(CLIENT_INFO) {
        implementation(info, &[CLIENT_INFO], &mut issues);
    }
    match meta.get(CLIENT_CAPABILITIES) {
        Some(caps) => capabilities(caps, &[CLIENT_CAPABILITIES], &mut issues),
        None => issues.push(&[CLIENT_CAPABILITIES], invalid_type("object", None)),
    }
    if let Some(level) = meta.get(LOG_LEVEL) {
        const LEVELS: [&str; 8] = ["debug", "info", "notice", "warning", "error", "critical", "alert", "emergency"];
        if !level.as_str().is_some_and(|l| LEVELS.contains(&l)) {
            let list: Vec<String> = LEVELS.iter().map(|l| format!("\"{l}\"")).collect();
            issues.push(&[LOG_LEVEL], format!("Invalid option: expected one of {}", list.join("|")));
        }
    }
    issues.0
}

fn opt_string(obj: &Map<String, Value>, key: &str, path: &[&str], issues: &mut Issues) {
    if let Some(v) = obj.get(key).filter(|v| !v.is_string()) {
        issues.wrong_type(&[path, &[key]].concat(), "string", v);
    }
}

fn req_string(obj: &Map<String, Value>, key: &str, path: &[&str], issues: &mut Issues) {
    match obj.get(key) {
        Some(Value::String(_)) => {}
        Some(v) => issues.wrong_type(&[path, &[key]].concat(), "string", v),
        None => issues.push(&[path, &[key]].concat(), invalid_type("string", None)),
    }
}

/// `Implementation`: name, title, icons, version, websiteUrl, description.
fn implementation(v: &Value, path: &[&str], issues: &mut Issues) {
    let Some(info) = v.as_object() else {
        return issues.wrong_type(path, "object", v);
    };
    req_string(info, "name", path, issues);
    opt_string(info, "title", path, issues);
    if let Some(icons) = info.get("icons") {
        let icons_path = [path, &["icons"]].concat();
        match icons.as_array() {
            None => issues.wrong_type(&icons_path, "array", icons),
            Some(items) => {
                for (i, icon) in items.iter().enumerate() {
                    let index = i.to_string();
                    let icon_path = [icons_path.as_slice(), &[index.as_str()]].concat();
                    let Some(icon) = icon.as_object() else {
                        issues.wrong_type(&icon_path, "object", icon);
                        continue;
                    };
                    req_string(icon, "src", &icon_path, issues);
                    opt_string(icon, "mimeType", &icon_path, issues);
                    if let Some(sizes) = icon.get("sizes") {
                        let sizes_path = [icon_path.as_slice(), &["sizes"]].concat();
                        match sizes.as_array() {
                            None => issues.wrong_type(&sizes_path, "array", sizes),
                            Some(list) => {
                                for (j, size) in list.iter().enumerate().filter(|(_, s)| !s.is_string()) {
                                    let j = j.to_string();
                                    issues.wrong_type(&[sizes_path.as_slice(), &[j.as_str()]].concat(), "string", size);
                                }
                            }
                        }
                    }
                    if icon.get("theme").is_some_and(|t| !matches!(t.as_str(), Some("light" | "dark"))) {
                        issues.push(
                            &[icon_path.as_slice(), &["theme"]].concat(),
                            "Invalid option: expected one of \"light\"|\"dark\"".into(),
                        );
                    }
                }
            }
        }
    }
    req_string(info, "version", path, issues);
    opt_string(info, "websiteUrl", path, issues);
    opt_string(info, "description", path, issues);
}

/// `z.record(z.string(), JSONValue)`: any object.
fn record(v: &Value, path: &[&str], issues: &mut Issues) -> bool {
    let ok = v.is_object();
    if !ok {
        issues.wrong_type(path, "record", v);
    }
    ok
}

/// `z.record(z.string(), JSONObject)`.
fn record_of_records(v: &Value, path: &[&str], issues: &mut Issues) {
    if let Some(map) = v.as_object() {
        for (key, item) in map {
            record(item, &[path, &[key.as_str()]].concat(), issues);
        }
    } else {
        issues.wrong_type(path, "record", v);
    }
}

/// `ClientCapabilities2026`: experimental, sampling, elicitation, roots, extensions.
fn capabilities(v: &Value, path: &[&str], issues: &mut Issues) {
    let Some(caps) = v.as_object() else {
        return issues.wrong_type(path, "object", v);
    };
    if let Some(x) = caps.get("experimental") {
        record_of_records(x, &[path, &["experimental"]].concat(), issues);
    }
    if let Some(sampling) = caps.get("sampling") {
        let p = [path, &["sampling"]].concat();
        match sampling.as_object() {
            None => issues.wrong_type(&p, "object", sampling),
            Some(s) => {
                for key in ["context", "tools"] {
                    if let Some(item) = s.get(key) {
                        record(item, &[p.as_slice(), &[key]].concat(), issues);
                    }
                }
            }
        }
    }
    if let Some(elicitation) = caps.get("elicitation") {
        let p = [path, &["elicitation"]].concat();
        // An intersection of an object and an optional record: a non-object fails both sides.
        match elicitation.as_object() {
            None => {
                issues.wrong_type(&p, "object", elicitation);
                issues.wrong_type(&p, "record", elicitation);
            }
            Some(e) => {
                if let Some(form) = e.get("form") {
                    let fp = [p.as_slice(), &["form"]].concat();
                    match form.as_object() {
                        None => {
                            issues.wrong_type(&fp, "object", form);
                            issues.wrong_type(&fp, "record", form);
                        }
                        Some(f) => {
                            if let Some(d) = f.get("applyDefaults").filter(|d| !d.is_boolean()) {
                                issues.wrong_type(&[fp.as_slice(), &["applyDefaults"]].concat(), "boolean", d);
                            }
                        }
                    }
                }
                if let Some(url) = e.get("url") {
                    record(url, &[p.as_slice(), &["url"]].concat(), issues);
                }
            }
        }
    }
    if let Some(roots) = caps.get("roots") {
        let p = [path, &["roots"]].concat();
        match roots.as_object() {
            None => issues.wrong_type(&p, "object", roots),
            Some(r) => {
                if let Some(l) = r.get("listChanged").filter(|l| !l.is_boolean()) {
                    issues.wrong_type(&[p.as_slice(), &["listChanged"]].concat(), "boolean", l);
                }
            }
        }
    }
    if let Some(x) = caps.get("extensions") {
        record_of_records(x, &[path, &["extensions"]].concat(), issues);
    }
}

/// `validateEnvelopeMeta`, used before the era is pinned: missing required keys first, then
/// schema issues as `(dotted path, message)`.
pub fn first_problem(meta: &Map<String, Value>) -> Option<(String, String)> {
    let mut problems: Vec<(String, String)> = [PROTOCOL_VERSION, CLIENT_CAPABILITIES]
        .iter()
        .filter(|k| !meta.contains_key(**k))
        .map(|k| (k.to_string(), "missing".to_string()))
        .collect();
    for issue in schema_issues(meta) {
        let key = issue.path.join(".");
        if issue.path.len() == 1 && problems.iter().any(|(k, p)| *k == key && p == "missing") {
            continue;
        }
        problems.push((key, issue.message));
    }
    problems.into_iter().next()
}

/// A claim the opening phase treats as modern: the version is a string at or after 2026-07-28
/// and the envelope is valid.
pub fn valid_modern_claim(params: Option<&Value>) -> bool {
    let Some(meta) = meta(params) else { return false };
    matches!(meta.get(PROTOCOL_VERSION), Some(Value::String(v)) if v.as_str() >= MODERN_VERSION) && first_problem(meta).is_none()
}

/// `checkInboundEnvelope` once the session is pinned to 2026-07-28: `Err` is the error message.
pub fn check_pinned(params: Option<&Value>) -> Result<(), String> {
    let lifted: Map<String, Value> = meta(params)
        .map(|m| m.iter().filter(|(k, _)| RESERVED.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    if lifted.is_empty() {
        return Err(format!(
            "Request is missing the required _meta envelope for protocol revision {MODERN_VERSION} ({PROTOCOL_VERSION}, {CLIENT_CAPABILITIES})"
        ));
    }
    let issues = schema_issues(&lifted);
    if issues.is_empty() {
        return Ok(());
    }
    let messages: Vec<String> = issues.into_iter().map(|i| i.message).collect();
    Err(format!("Invalid _meta envelope for protocol revision {MODERN_VERSION}: {}", messages.join("; ")))
}
