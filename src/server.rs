//! The MCP side: JSON-RPC over newline-delimited stdio, answered byte for byte like the official
//! TypeScript SDK (v2.0.0) answered for this server, in both protocol eras: the 2025 era with its
//! `initialize` handshake and the stateless 2026-07-28 era. `tests/golden/protocol.json` pins it.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};

use crate::envelope::{self, MODERN_VERSION};
use crate::json::{js_number, js_ordered, quote, stringify, stringify_pretty};
use crate::tavily::{self, Http, ToolResult};
use crate::validate::{Invalid, invalid_type, validate};

pub const SERVER_NAME: &str = "mcp-tavily-research";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTRUCTIONS: &str = "Use tavily_search for focused lookups and source discovery. Use tavily_extract when specific page URLs are already known and their full content is needed. Use tavily_crawl for controlled multi-page traversal of one website. Use tavily_research only for broad multi-source analysis because it creates a billable asynchronous task. After tavily_research returns a request ID, poll that same task with tavily_research_status; never create a duplicate task.";

/// The `tools/list` result exactly as the TypeScript server generated it from its Zod schemas.
const TOOLS_LIST: &str = include_str!("tools_list.json");

const LATEST_LEGACY_VERSION: &str = "2025-11-25";
const LEGACY_VERSIONS: [&str; 5] = [LATEST_LEGACY_VERSION, "2025-06-18", "2025-03-26", "2024-11-05", "2024-10-07"];
const MAX_SUBSCRIPTIONS: usize = 1024;

pub type Output = Arc<Mutex<dyn Write + Send>>;
pub type ApiKey = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Serves newline-delimited JSON-RPC from `input` until it ends, then waits for running tool calls
/// to answer. Each tool call runs on its own thread, so a slow crawl never blocks other requests.
pub fn serve(mut input: impl BufRead, output: Output, http: Arc<dyn Http>, api_key: ApiKey) {
    let in_flight: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> = Arc::default();
    let mut workers = Vec::new();
    let mut session = Session::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match input.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&buf);
        let line = line.strip_suffix('\n').unwrap_or(&line);
        match session.handle(line) {
            Action::Reply(reply) => write_line(&output, &reply),
            Action::Ignore => {}
            Action::Cancel(key) => {
                if let Some(flag) = lock(&in_flight).get(&key) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
            Action::Call(call) => {
                let key = call.key();
                let cancelled = Arc::new(AtomicBool::new(false));
                lock(&in_flight).insert(key.clone(), cancelled.clone());
                let (http, output, in_flight, api_key) = (http.clone(), output.clone(), in_flight.clone(), api_key.clone());
                workers.push(std::thread::spawn(move || {
                    // A bug that panics must still answer, or the client waits until it times out.
                    let run = std::panic::AssertUnwindSafe(|| call.run(http.as_ref(), api_key().as_deref()));
                    let reply = std::panic::catch_unwind(run).unwrap_or_else(|_| error(&call.id, -32603, "Internal error"));
                    // Decided under the lock that `Cancel` takes: a cancel either lands first and
                    // suppresses the reply, or comes after this point. The write happens after the
                    // lock is released so a slow reader never blocks the stdin loop.
                    let send = {
                        let mut map = lock(&in_flight);
                        if map.get(&key).is_some_and(|f| Arc::ptr_eq(f, &cancelled)) {
                            map.remove(&key);
                        }
                        !cancelled.load(Ordering::SeqCst)
                    };
                    if send {
                        write_line(&output, &reply);
                    }
                }));
                workers.retain(|w| !w.is_finished());
            }
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
}

fn lock<T: ?Sized>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn write_line(output: &Mutex<dyn Write + Send>, line: &str) {
    let mut out = lock(output);
    // A closed stdout means the client is gone; nothing useful is left to do with the error.
    let _ = out.write_all(format!("{line}\n").as_bytes()).and_then(|()| out.flush());
}

/// What to do with one line read from stdin.
pub enum Action {
    /// Write this line to stdout.
    Reply(String),
    /// Nothing to send: notifications, responses, and malformed or unparsable messages.
    Ignore,
    /// Run a tool; its reply is written when it finishes unless the request was cancelled.
    Call(ToolCall),
    /// `notifications/cancelled` for this request id key.
    Cancel(String),
}

pub struct ToolCall {
    pub id: Value,
    pub name: String,
    pub arguments: Map<String, Value>,
    /// Answer in the 2026-07-28 result shape.
    pub modern: bool,
}

impl ToolCall {
    /// Key used to match a later `notifications/cancelled`.
    pub fn key(&self) -> String {
        id_key(&self.id)
    }

    /// Validates the arguments, calls Tavily and returns the JSON-RPC response line.
    pub fn run(&self, http: &dyn Http, api_key: Option<&str>) -> String {
        let result = match validate(&tavily::schema(&self.name), &self.arguments) {
            Ok(args) => tavily::run(&self.name, &args, http, api_key),
            Err(Invalid::Thrown(message)) => ToolResult::error(message),
            Err(Invalid::Issues(issues)) => {
                ToolResult::error(format!("Input validation error: Invalid arguments for tool {}: {}", self.name, issues.join(", ")))
            }
        };
        let mut out = format!("{{\"content\":[{{\"type\":\"text\",\"text\":{}}}]", quote(&result.text));
        if let Some(structured) = result.structured {
            out.push_str(",\"structuredContent\":");
            out.push_str(&stringify(&js_ordered(structured)));
        }
        if result.is_error {
            out.push_str(",\"isError\":true");
        }
        if self.modern {
            out.push_str(",\"resultType\":\"complete\",");
            out.push_str(&server_info_meta());
        }
        out.push('}');
        success(&self.id, &out)
    }
}

/// Which protocol era the session speaks. The first message decides, as in the SDK's stdio entry:
/// a 2026-07-28 `_meta` envelope pins the modern era, anything else pins the legacy one.
#[derive(Clone, Copy, PartialEq)]
enum Era {
    Opening,
    /// A modern `server/discover` was answered; the next request decides.
    Probe,
    Legacy,
    Modern,
}

pub struct Session {
    era: Era,
    /// Open `subscriptions/listen` ids (modern era).
    listens: HashSet<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// A message that passed the JSON-RPC schema checks.
struct Message<'a> {
    method: &'a str,
    /// `None` for a notification.
    id: Option<&'a Value>,
    params: Option<&'a Value>,
}

impl Session {
    pub fn new() -> Self {
        Self { era: Era::Opening, listens: HashSet::new() }
    }

    pub fn handle(&mut self, line: &str) -> Action {
        let line = crate::json::replace_lone_surrogates(line.strip_suffix('\r').unwrap_or(line));
        let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(&line) else {
            // `JSON.parse` failures were dropped too, with only a transport-error log line.
            eprintln!("MCP transport error: unparsable JSON-RPC line");
            return Action::Ignore;
        };
        let Some(msg) = parse_message(&raw) else {
            return Action::Ignore;
        };
        match self.era {
            Era::Legacy => legacy(&msg),
            Era::Modern => self.pinned_modern(&msg),
            Era::Opening | Era::Probe => self.opening(&msg),
        }
    }

    /// Classifies a message that arrives before the era is pinned.
    fn opening(&mut self, msg: &Message) -> Action {
        let has_claim = envelope::claim(msg.params).is_some();
        let legacy_initialize = msg.method == "initialize" && !envelope::valid_modern_claim(msg.params);
        if legacy_initialize || !has_claim {
            self.era = Era::Legacy;
            return legacy(msg);
        }
        let meta = envelope::meta(msg.params).expect("a claim lives in _meta");
        if let Some((key, problem)) = envelope::first_problem(meta) {
            let Some(id) = msg.id else { return Action::Ignore };
            let mut data = Map::new();
            let mut detail = Map::new();
            detail.insert("key".into(), Value::from(key.clone()));
            detail.insert("problem".into(), Value::from(problem.clone()));
            data.insert("envelope".into(), Value::Object(detail));
            let message = format!("Invalid _meta envelope for protocol revision {MODERN_VERSION}: {key}: {problem}");
            return Action::Reply(error_with_data(id, -32602, &message, &Value::Object(data)));
        }
        let version = meta[envelope::PROTOCOL_VERSION].as_str().unwrap_or_default();
        if version != MODERN_VERSION {
            return match msg.id {
                Some(id) => Action::Reply(unsupported_version(id, Some(version))),
                None => Action::Ignore,
            };
        }
        // `server/discover` only probes; a notification during the probe does not decide either.
        let discover = msg.id.is_some() && msg.method == "server/discover";
        let probe_notification = msg.id.is_none() && self.era == Era::Probe;
        if discover || probe_notification {
            self.era = Era::Probe;
            return modern(msg, &mut self.listens);
        }
        self.era = Era::Modern;
        self.pinned_modern(msg)
    }

    fn pinned_modern(&mut self, msg: &Message) -> Action {
        if let Some(id) = msg.id {
            if msg.method == "initialize" && !envelope::valid_modern_claim(msg.params) {
                let requested = msg.params.and_then(|p| p.get("protocolVersion")).and_then(Value::as_str);
                return Action::Reply(unsupported_version(id, requested));
            }
            if msg.method == "subscriptions/listen" {
                return Action::Reply(self.listen(id, msg.params));
            }
        }
        modern(msg, &mut self.listens)
    }

    /// `subscriptions/listen`: this server never emits list changes, so a subscription only gets
    /// its acknowledgement, with the subset the server can honor (tool list changes).
    fn listen(&mut self, id: &Value, params: Option<&Value>) -> String {
        let Some(meta) = envelope::meta(params).filter(|m| m.contains_key(envelope::PROTOCOL_VERSION)) else {
            return error(
                id,
                -32602,
                &format!("Invalid _meta envelope: _meta: the per-request envelope is required on protocol revision {MODERN_VERSION}"),
            );
        };
        if let Some((key, problem)) = envelope::first_problem(meta) {
            return error(id, -32602, &format!("Invalid _meta envelope: {key}: {problem}"));
        }
        let version = meta[envelope::PROTOCOL_VERSION].as_str().unwrap_or_default();
        if version != MODERN_VERSION {
            return unsupported_version(id, Some(version));
        }
        let key = id_key(id);
        if self.listens.len() >= MAX_SUBSCRIPTIONS && !self.listens.contains(&key) {
            return error(id, -32603, "Subscription limit reached");
        }
        let Some(filter) = params.and_then(|p| p.get("notifications")).and_then(Value::as_object).filter(|f| valid_filter(f)) else {
            return error(id, -32602, "Invalid params: 'notifications' is required and must be a valid SubscriptionFilter");
        };
        self.listens.insert(key);
        let honored = if filter.get("toolsListChanged") == Some(&Value::Bool(true)) { "{\"toolsListChanged\":true}" } else { "{}" };
        format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/subscriptions/acknowledged\",\"params\":{{\"notifications\":{honored},\"_meta\":{{\"io.modelcontextprotocol/subscriptionId\":{}}}}}}}",
            stringify(id)
        )
    }
}

fn valid_filter(filter: &Map<String, Value>) -> bool {
    ["toolsListChanged", "promptsListChanged", "resourcesListChanged"].iter().all(|k| filter.get(*k).is_none_or(Value::is_boolean))
        && filter.get("resourceSubscriptions").is_none_or(|v| v.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
}

/// The JSON-RPC message schema of the SDK: anything that fails it is dropped without a reply.
fn parse_message(raw: &Map<String, Value>) -> Option<Message<'_>> {
    if raw.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return None;
    }
    let method = raw.get("method")?.as_str()?;
    let params = raw.get("params");
    if params.is_some_and(|p| !p.is_object()) {
        return None;
    }
    let meta = params.and_then(|p| p.get("_meta"));
    if meta.is_some_and(|m| !m.is_object()) {
        return None;
    }
    let id = raw.get("id");
    if let Some(id) = id {
        if !valid_id(id) {
            return None;
        }
        let meta = meta.and_then(Value::as_object);
        if meta.and_then(|m| m.get("progressToken")).is_some_and(|t| !valid_id(t)) {
            return None;
        }
        let task = meta.and_then(|m| m.get("io.modelcontextprotocol/related-task"));
        if task.is_some_and(|t| !t.get("taskId").is_some_and(Value::is_string)) {
            return None;
        }
    }
    Some(Message { method, id, params })
}

fn legacy(msg: &Message) -> Action {
    let Some(id) = msg.id else {
        return cancel(msg, None);
    };
    match msg.method {
        "initialize" => initialize(id, msg.params),
        "ping" => Action::Reply(success(id, "{}")),
        "tools/list" => tools_list(id, msg.params, false),
        "tools/call" => tools_call(id, msg.params, false),
        _ => Action::Reply(error(id, -32601, "Method not found")),
    }
}

fn modern(msg: &Message, listens: &mut HashSet<String>) -> Action {
    let Some(id) = msg.id else {
        return cancel(msg, Some(listens));
    };
    if !matches!(msg.method, "tools/list" | "tools/call" | "server/discover") {
        return Action::Reply(error(id, -32601, "Method not found"));
    }
    if let Err(message) = envelope::check_pinned(msg.params) {
        return Action::Reply(error(id, -32602, &message));
    }
    match msg.method {
        "tools/list" => tools_list(id, msg.params, true),
        "tools/call" => tools_call(id, msg.params, true),
        _ => {
            let result = format!(
                "{{\"supportedVersions\":[\"{MODERN_VERSION}\"],\"capabilities\":{{\"tools\":{{\"listChanged\":true}}}},\"instructions\":{},\"resultType\":\"complete\",\"ttlMs\":0,\"cacheScope\":\"private\",{}}}",
                quote(INSTRUCTIONS),
                server_info_meta()
            );
            Action::Reply(success(id, &result))
        }
    }
}

/// `notifications/cancelled`; every other notification is ignored. In the modern era a cancel
/// first closes a matching subscription.
fn cancel(msg: &Message, listens: Option<&mut HashSet<String>>) -> Action {
    if msg.method != "notifications/cancelled" {
        return Action::Ignore;
    }
    // The SDK ignores a falsy request id (0 or "").
    let key = match msg.params.and_then(|p| p.get("requestId")) {
        Some(id @ Value::String(s)) if !s.is_empty() => id_key(id),
        Some(id @ Value::Number(n)) if n.as_f64() != Some(0.0) => id_key(id),
        _ => return Action::Ignore,
    };
    if listens.is_some_and(|l| l.remove(&key)) {
        return Action::Ignore;
    }
    Action::Cancel(key)
}

/// JSON-RPC ids (and progress tokens) the SDK accepts: a string or a safe integer.
fn valid_id(id: &Value) -> bool {
    match id {
        Value::String(_) => true,
        Value::Number(n) => n.as_f64().is_some_and(|x| x.fract() == 0.0 && x.abs() <= 9_007_199_254_740_991.0),
        _ => false,
    }
}

fn id_key(id: &Value) -> String {
    match id {
        Value::String(s) => format!("s:{s}"),
        Value::Number(n) => format!("n:{}", js_number(n.as_f64().unwrap_or_default())),
        other => format!("o:{}", stringify(other)),
    }
}

fn server_info_meta() -> String {
    format!(
        "\"_meta\":{{\"io.modelcontextprotocol/serverInfo\":{{\"name\":{},\"version\":{}}}}}",
        quote(SERVER_NAME),
        quote(SERVER_VERSION)
    )
}

fn success(id: &Value, result: &str) -> String {
    format!("{{\"result\":{result},\"jsonrpc\":\"2.0\",\"id\":{}}}", stringify(id))
}

fn error(id: &Value, code: i32, message: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{code},\"message\":{}}}}}", stringify(id), quote(message))
}

fn error_with_data(id: &Value, code: i32, message: &str, data: &Value) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{code},\"message\":{},\"data\":{}}}}}",
        stringify(id),
        quote(message),
        stringify(data)
    )
}

fn unsupported_version(id: &Value, requested: Option<&str>) -> String {
    let supported = Value::Array(vec![Value::from(MODERN_VERSION)]);
    let mut data = Map::new();
    data.insert("supported".into(), supported);
    match requested {
        Some(v) => {
            data.insert("requested".into(), Value::from(v));
            error_with_data(id, -32022, &format!("Unsupported protocol version: {v}"), &Value::Object(data))
        }
        None => {
            error_with_data(id, -32022, "Unsupported protocol version: the request did not name a protocol version", &Value::Object(data))
        }
    }
}

/// A Zod `invalid_type` issue as the SDK printed it inside error messages.
fn type_issue(expected: &str, path: &[&str], received: Option<&Value>) -> Value {
    let mut issue = Map::new();
    issue.insert("expected".into(), Value::from(expected));
    issue.insert("code".into(), Value::from("invalid_type"));
    issue.insert("path".into(), Value::Array(path.iter().map(|p| Value::from(*p)).collect()));
    issue.insert("message".into(), Value::from(invalid_type(expected, received)));
    Value::Object(issue)
}

fn is_plain_object(v: Option<&Value>) -> bool {
    v.is_some_and(Value::is_object)
}

fn initialize(id: &Value, params: Option<&Value>) -> Action {
    let mut issues = Vec::new();
    match params {
        None => issues.push(type_issue("object", &["params"], None)),
        Some(p) => {
            let version = p.get("protocolVersion");
            if !matches!(version, Some(Value::String(_))) {
                issues.push(type_issue("string", &["params", "protocolVersion"], version));
            }
            let capabilities = p.get("capabilities");
            if !is_plain_object(capabilities) {
                issues.push(type_issue("object", &["params", "capabilities"], capabilities));
            }
            match p.get("clientInfo") {
                Some(Value::Object(info)) => {
                    for key in ["name", "version"] {
                        let v = info.get(key);
                        if !matches!(v, Some(Value::String(_))) {
                            issues.push(type_issue("string", &["params", "clientInfo", key], v));
                        }
                    }
                }
                other => issues.push(type_issue("object", &["params", "clientInfo"], other)),
            }
        }
    }
    if !issues.is_empty() {
        return Action::Reply(error(id, -32603, &stringify_pretty(&Value::Array(issues))));
    }
    let requested = params.and_then(|p| p["protocolVersion"].as_str()).unwrap_or_default();
    let version = if LEGACY_VERSIONS.contains(&requested) { requested } else { LATEST_LEGACY_VERSION };
    let result = format!(
        "{{\"protocolVersion\":{},\"capabilities\":{{\"tools\":{{\"listChanged\":true}}}},\"serverInfo\":{{\"name\":{},\"version\":{}}},\"instructions\":{}}}",
        quote(version),
        quote(SERVER_NAME),
        quote(SERVER_VERSION),
        quote(INSTRUCTIONS)
    );
    Action::Reply(success(id, &result))
}

fn tools_list(id: &Value, params: Option<&Value>, modern: bool) -> Action {
    let cursor = params.and_then(|p| p.get("cursor"));
    if cursor.is_some_and(|c| !c.is_string()) {
        let issues = Value::Array(vec![type_issue("string", &["params", "cursor"], cursor)]);
        return Action::Reply(error(id, -32603, &stringify_pretty(&issues)));
    }
    let list = TOOLS_LIST.trim_end();
    if !modern {
        return Action::Reply(success(id, list));
    }
    let open = list.strip_suffix('}').expect("tools_list.json is a JSON object");
    let result = format!("{open},\"resultType\":\"complete\",\"ttlMs\":0,\"cacheScope\":\"private\",{}}}", server_info_meta());
    Action::Reply(success(id, &result))
}

fn tools_call(id: &Value, params: Option<&Value>, modern: bool) -> Action {
    let invalid = |issues: Vec<Value>| {
        Action::Reply(error(id, -32602, &format!("Invalid tools/call request: {}", stringify_pretty(&Value::Array(issues)))))
    };
    let Some(params) = params else {
        return invalid(vec![type_issue("object", &["params"], None)]);
    };
    let mut issues = Vec::new();
    let name = params.get("name");
    if !matches!(name, Some(Value::String(_))) {
        issues.push(type_issue("string", &["params", "name"], name));
    }
    let arguments = params.get("arguments");
    if arguments.is_some() && !is_plain_object(arguments) {
        issues.push(type_issue("record", &["params", "arguments"], arguments));
    }
    if !issues.is_empty() {
        return invalid(issues);
    }
    if params.get("requestState").is_some_and(|s| !s.is_string()) {
        let data = r#"{"reason":"invalid_request_state"}"#;
        return Action::Reply(format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":-32602,\"message\":\"Invalid or expired requestState\",\"data\":{data}}}}}",
            stringify(id)
        ));
    }
    let name = name.and_then(Value::as_str).unwrap_or_default();
    if !tavily::TOOL_NAMES.contains(&name) {
        return Action::Reply(error(id, -32602, &format!("Tool {name} not found")));
    }
    Action::Call(ToolCall {
        id: id.clone(),
        name: name.to_string(),
        arguments: arguments.and_then(Value::as_object).cloned().unwrap_or_default(),
        modern,
    })
}
