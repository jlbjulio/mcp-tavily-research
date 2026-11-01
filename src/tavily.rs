//! The five tools: argument schemas, the Tavily request each one sends, validation of Tavily's
//! answer and the text returned to the model.

use serde_json::{Map, Value, json};

use crate::json::{as_js_f64, encode_uri_component, js_number, js_slice_utf16, js_to_fixed, js_trim, stringify, stringify_pretty};
use crate::validate::{Default as D, Field, Kind};

pub const API_BASE_URL: &str = "https://api.tavily.com";
const DEFAULT_TIMEOUT_MS: f64 = 30_000.0;
const API_KEY_MISSING: &str = "TAVILY_API_KEY is required. Configure it before calling a Tavily tool.";

/// What a tool sends to Tavily.
// No Debug: `authorization` holds the API key and must never reach a log.
#[derive(Clone, PartialEq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub authorization: String,
    /// JSON body; sent with `Content-Type: application/json`.
    pub body: Option<String>,
    pub timeout_ms: f64,
}

pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    /// The body, or the message of the error that interrupted reading it.
    pub body: Result<Vec<u8>, String>,
}

/// Sends one request. `Err` carries the message `fetch` would have thrown.
pub trait Http: Send + Sync {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String>;
}

/// A finished tool call, in the shape of an MCP `CallToolResult`.
pub struct ToolResult {
    pub text: String,
    pub structured: Option<Value>,
    pub is_error: bool,
}

impl ToolResult {
    fn ok(text: String, structured: Value) -> Self {
        Self { text, structured: Some(structured), is_error: false }
    }
    pub fn error(text: String) -> Self {
        Self { text, structured: None, is_error: true }
    }
}

pub const TOOL_NAMES: [&str; 5] = ["tavily_search", "tavily_extract", "tavily_crawl", "tavily_research", "tavily_research_status"];

fn field(name: &'static str, kind: Kind, default: Option<D>) -> Field {
    Field { name, kind, default, required: false }
}

fn required(name: &'static str, kind: Kind) -> Field {
    Field { name, kind, default: None, required: true }
}

const DEPTHS: &[&str] = &["basic", "advanced"];
const FORMATS: &[&str] = &["markdown", "text"];

pub fn schema(tool: &str) -> Vec<Field> {
    match tool {
        "tavily_search" => vec![
            required("query", Kind::Str { min: 1, max: Some(2_000) }),
            field("search_depth", Kind::Enum(DEPTHS), Some(D::Str("basic"))),
            field("topic", Kind::Enum(&["general", "news", "finance"]), Some(D::Str("general"))),
            field("time_range", Kind::Enum(&["day", "week", "month", "year"]), None),
            field("max_results", Kind::Int { min: 1.0, max: 20.0 }, Some(D::Num(8.0))),
            field("include_domains", Kind::StrList { max: 300 }, None),
            field("exclude_domains", Kind::StrList { max: 150 }, None),
            field("include_answer", Kind::Enum(DEPTHS), None),
        ],
        "tavily_extract" => vec![
            required("urls", Kind::WebUrls),
            field("query", Kind::Str { min: 1, max: Some(2_000) }, None),
            field("chunks_per_source", Kind::Int { min: 1.0, max: 5.0 }, Some(D::Num(3.0))),
            field("extract_depth", Kind::Enum(DEPTHS), Some(D::Str("basic"))),
            field("include_images", Kind::Bool, Some(D::Bool(false))),
            field("include_favicon", Kind::Bool, Some(D::Bool(false))),
            field("format", Kind::Enum(FORMATS), Some(D::Str("markdown"))),
            field("timeout", Kind::Num { min: 1.0, max: 60.0 }, None),
        ],
        "tavily_crawl" => vec![
            required("url", Kind::WebUrl),
            field("instructions", Kind::Str { min: 1, max: Some(2_000) }, None),
            field("chunks_per_source", Kind::Int { min: 1.0, max: 5.0 }, Some(D::Num(3.0))),
            field("max_depth", Kind::Int { min: 1.0, max: 5.0 }, Some(D::Num(1.0))),
            field("max_breadth", Kind::Int { min: 1.0, max: 500.0 }, Some(D::Num(10.0))),
            field("limit", Kind::Int { min: 1.0, max: 50.0 }, Some(D::Num(10.0))),
            field("select_paths", Kind::StrList { max: 50 }, None),
            field("select_domains", Kind::StrList { max: 50 }, None),
            field("exclude_paths", Kind::StrList { max: 50 }, None),
            field("exclude_domains", Kind::StrList { max: 50 }, None),
            field("allow_external", Kind::Bool, Some(D::Bool(false))),
            field("include_images", Kind::Bool, Some(D::Bool(false))),
            field("extract_depth", Kind::Enum(DEPTHS), Some(D::Str("basic"))),
            field("format", Kind::Enum(FORMATS), Some(D::Str("markdown"))),
            field("include_favicon", Kind::Bool, Some(D::Bool(false))),
            field("timeout", Kind::Num { min: 10.0, max: 150.0 }, Some(D::Num(30.0))),
        ],
        "tavily_research" => vec![
            required("input", Kind::Str { min: 1, max: Some(20_000) }),
            field("model", Kind::Enum(&["mini", "pro", "auto"]), Some(D::Str("mini"))),
            field("citation_format", Kind::Enum(&["numbered", "mla", "apa", "chicago"]), Some(D::Str("numbered"))),
            field("output_length", Kind::Enum(&["short", "standard", "long"]), Some(D::Str("standard"))),
            field("include_domains", Kind::StrList { max: 20 }, None),
            field("exclude_domains", Kind::StrList { max: 20 }, None),
        ],
        "tavily_research_status" => vec![required("request_id", Kind::Str { min: 1, max: None })],
        _ => Vec::new(),
    }
}

/// Runs a tool whose arguments already passed `schema`. `api_key` is `TAVILY_API_KEY`, read for
/// each call like the TypeScript server did.
pub fn run(tool: &str, args: &Map<String, Value>, http: &dyn Http, api_key: Option<&str>) -> ToolResult {
    match tool {
        "tavily_search" => search(args, http, api_key),
        "tavily_extract" => extract(args, http, api_key),
        "tavily_crawl" => crawl(args, http, api_key),
        "tavily_research" => research_start(args, http, api_key),
        _ => research_status(args, http, api_key),
    }
}

// ---------- shared request path ----------

/// The name used in "unexpected ... response" and the schema of a successful answer.
struct Endpoint {
    name: &'static str,
    validate: fn(&Value) -> Option<Value>,
}

/// Mirrors `requestTavilyJson`: every failure message matches the TypeScript one.
fn request_json(
    http: &dyn Http,
    api_key: Option<&str>,
    method: &'static str,
    path: &str,
    body: Option<Value>,
    timeout_ms: f64,
    endpoint: Endpoint,
) -> Result<Value, String> {
    let key = api_key.map(js_trim).filter(|k| !k.is_empty()).ok_or(API_KEY_MISSING)?;
    let request = HttpRequest {
        method,
        url: format!("{API_BASE_URL}{path}"),
        authorization: format!("Bearer {key}"),
        body: body.as_ref().map(stringify),
        timeout_ms,
    };
    let response = http.send(&request).map_err(|e| format!("Tavily request failed: {e}"))?;
    if !(200..=299).contains(&response.status) {
        let text = decode(response.body?);
        let detail = js_slice_utf16(&text, 500);
        let detail = if detail.is_empty() { response.status_text } else { detail };
        return Err(format!("Tavily API returned HTTP {}: {detail}", response.status));
    }
    let not_json = || "Tavily API returned a response that was not valid JSON.".to_string();
    let text = decode(response.body.map_err(|_| not_json())?);
    let data: Value = serde_json::from_str(&text).map_err(|_| not_json())?;
    (endpoint.validate)(&data).ok_or_else(|| format!("Tavily API returned an unexpected {} response.", endpoint.name))
}

/// `Response.text()`: UTF-8 with replacement characters and the leading BOM removed.
fn decode(bytes: Vec<u8>) -> String {
    let text = String::from_utf8_lossy(&bytes);
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
}

fn str_arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn num_arg(args: &Map<String, Value>, key: &str) -> Option<f64> {
    args.get(key).and_then(Value::as_f64)
}

/// Copies a list argument into the body only when it has items, as the TypeScript `?.length` did.
fn put_list(body: &mut Map<String, Value>, args: &Map<String, Value>, key: &str) {
    if let Some(Value::Array(items)) = args.get(key) {
        if !items.is_empty() {
            body.insert(key.into(), Value::Array(items.clone()));
        }
    }
}

fn copy(body: &mut Map<String, Value>, args: &Map<String, Value>, key: &str) {
    if let Some(v) = args.get(key) {
        body.insert(key.into(), v.clone());
    }
}

/// Template-literal interpolation of a string-or-number field.
fn interp(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => js_number(as_js_f64(n)),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        None => "undefined".into(),
        Some(other) => stringify(other),
    }
}

fn credits(result: &Value) -> String {
    match result.get("usage").and_then(|u| u.get("credits")) {
        Some(Value::Number(n)) => js_number(as_js_f64(n)),
        _ => "unknown".into(),
    }
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn image_url(image: &Value) -> String {
    match image {
        Value::String(s) => s.clone(),
        other => text(other, "url"),
    }
}

/// Pages of an Extract or Crawl answer, separated as the TypeScript tools did.
fn format_pages(result: &Value, empty: &str) -> String {
    let pages = result["results"].as_array().map(Vec::as_slice).unwrap_or_default();
    if pages.is_empty() {
        return empty.into();
    }
    pages
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let mut sections = vec![format!("[{}] {}", i + 1, text(page, "url")), text(page, "raw_content")];
            if let Some(images) = page.get("images").and_then(Value::as_array).filter(|a| !a.is_empty()) {
                let list: Vec<String> = images.iter().map(|img| format!("- {}", image_url(img))).collect();
                sections.push(format!("Images:\n{}", list.join("\n")));
            }
            sections.join("\n\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

// ---------- tavily_search ----------

fn search(args: &Map<String, Value>, http: &dyn Http, api_key: Option<&str>) -> ToolResult {
    let mut body = Map::new();
    body.insert("query".into(), json!(js_trim(str_arg(args, "query").unwrap_or_default())));
    copy(&mut body, args, "search_depth");
    copy(&mut body, args, "topic");
    copy(&mut body, args, "max_results");
    body.insert("include_usage".into(), json!(true));
    body.insert("include_raw_content".into(), json!(false));
    body.insert("include_answer".into(), args.get("include_answer").cloned().unwrap_or(json!(false)));
    copy(&mut body, args, "time_range");
    put_list(&mut body, args, "include_domains");
    put_list(&mut body, args, "exclude_domains");

    let result = match request_json(
        http,
        api_key,
        "POST",
        "/search",
        Some(Value::Object(body)),
        DEFAULT_TIMEOUT_MS,
        Endpoint { name: "Search", validate: schemas::search },
    ) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(format!("Tavily search failed: {e}")),
    };

    let sources = result["results"].as_array().map(Vec::as_slice).unwrap_or_default();
    let sources = if sources.is_empty() {
        "No sources were returned.".to_string()
    } else {
        sources
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let score = s.get("score").and_then(Value::as_f64).unwrap_or_default();
                [
                    format!("[{}] {}", i + 1, text(s, "title")),
                    format!("URL: {}", text(s, "url")),
                    format!("Relevance score: {}", js_to_fixed(score, 3)),
                    text(s, "content"),
                ]
                .join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let mut sections = Vec::new();
    if let Some(answer) = result.get("answer").and_then(Value::as_str).filter(|a| !a.is_empty()) {
        sections.push(format!("Answer\n\n{answer}"));
    }
    sections.push(format!("Sources\n\n{sources}"));
    sections.push(
        [
            "Metadata".to_string(),
            String::new(),
            format!("- Query: {}", text(&result, "query")),
            format!("- Response time: {} seconds", interp(result.get("response_time"))),
            format!("- Credits used: {}", credits(&result)),
            format!("- Request ID: {}", text(&result, "request_id")),
        ]
        .join("\n"),
    );
    ToolResult::ok(sections.join("\n\n"), result)
}

// ---------- tavily_extract ----------

fn extract(args: &Map<String, Value>, http: &dyn Http, api_key: Option<&str>) -> ToolResult {
    let urls: Vec<Value> =
        args["urls"].as_array().map(|a| a.iter().map(|u| json!(js_trim(u.as_str().unwrap_or_default()))).collect()).unwrap_or_default();
    let depth = str_arg(args, "extract_depth").unwrap_or("basic");
    let mut body = Map::new();
    body.insert("urls".into(), Value::Array(urls));
    copy(&mut body, args, "extract_depth");
    copy(&mut body, args, "include_images");
    copy(&mut body, args, "include_favicon");
    copy(&mut body, args, "format");
    body.insert("include_usage".into(), json!(true));
    if let Some(query) = str_arg(args, "query").map(js_trim).filter(|q| !q.is_empty()) {
        body.insert("query".into(), json!(query));
        copy(&mut body, args, "chunks_per_source");
    }
    let timeout = num_arg(args, "timeout");
    copy(&mut body, args, "timeout");
    let api_timeout_s = timeout.unwrap_or(if depth == "advanced" { 30.0 } else { 10.0 });

    let result = match request_json(
        http,
        api_key,
        "POST",
        "/extract",
        Some(Value::Object(body)),
        (api_timeout_s + 5.0) * 1_000.0,
        Endpoint { name: "Extract", validate: schemas::extract },
    ) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(format!("Tavily extraction failed: {e}")),
    };

    let pages = format_pages(&result, "No pages were extracted successfully.");
    let failed = result.get("failed_results").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let failures = if failed.is_empty() {
        "None".to_string()
    } else {
        failed
            .iter()
            .map(|f| {
                let error = f.get("error").and_then(Value::as_str).unwrap_or("No error detail was provided.");
                format!("- {}: {error}", text(f, "url"))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let successful = result["results"].as_array().map_or(0, Vec::len);
    let text = [
        "Extracted pages".to_string(),
        String::new(),
        pages,
        String::new(),
        "Failed pages".into(),
        String::new(),
        failures,
        String::new(),
        "Metadata".into(),
        String::new(),
        format!("- Successful pages: {successful}"),
        format!("- Failed pages: {}", failed.len()),
        format!("- Response time: {} seconds", interp(result.get("response_time"))),
        format!("- Credits used: {}", credits(&result)),
        format!("- Request ID: {}", text(&result, "request_id")),
    ]
    .join("\n");
    ToolResult::ok(text, result)
}

// ---------- tavily_crawl ----------

fn crawl(args: &Map<String, Value>, http: &dyn Http, api_key: Option<&str>) -> ToolResult {
    let timeout = num_arg(args, "timeout").unwrap_or(30.0);
    let mut body = Map::new();
    body.insert("url".into(), json!(js_trim(str_arg(args, "url").unwrap_or_default())));
    for key in
        ["max_depth", "max_breadth", "limit", "allow_external", "include_images", "extract_depth", "format", "include_favicon", "timeout"]
    {
        copy(&mut body, args, key);
    }
    body.insert("include_usage".into(), json!(true));
    if let Some(instructions) = str_arg(args, "instructions").map(js_trim).filter(|s| !s.is_empty()) {
        body.insert("instructions".into(), json!(instructions));
        copy(&mut body, args, "chunks_per_source");
    }
    for key in ["select_paths", "select_domains", "exclude_paths", "exclude_domains"] {
        put_list(&mut body, args, key);
    }

    let result = match request_json(
        http,
        api_key,
        "POST",
        "/crawl",
        Some(Value::Object(body)),
        (timeout + 5.0) * 1_000.0,
        Endpoint { name: "Crawl", validate: schemas::crawl },
    ) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(format!("Tavily crawl failed: {e}")),
    };

    let pages = format_pages(&result, "No pages matched the crawl configuration.");
    let returned = result["results"].as_array().map_or(0, Vec::len);
    let text = [
        "Crawled pages".to_string(),
        String::new(),
        pages,
        String::new(),
        "Metadata".into(),
        String::new(),
        format!("- Base URL: {}", text(&result, "base_url")),
        format!("- Pages returned: {returned}"),
        format!("- Response time: {} seconds", interp(result.get("response_time"))),
        format!("- Credits used: {}", credits(&result)),
        format!("- Request ID: {}", text(&result, "request_id")),
    ]
    .join("\n");
    ToolResult::ok(text, result)
}

// ---------- tavily_research ----------

fn research_start(args: &Map<String, Value>, http: &dyn Http, api_key: Option<&str>) -> ToolResult {
    let mut body = Map::new();
    body.insert("input".into(), json!(js_trim(str_arg(args, "input").unwrap_or_default())));
    copy(&mut body, args, "model");
    body.insert("stream".into(), json!(false));
    copy(&mut body, args, "citation_format");
    copy(&mut body, args, "output_length");
    put_list(&mut body, args, "include_domains");
    put_list(&mut body, args, "exclude_domains");

    let task = match request_json(
        http,
        api_key,
        "POST",
        "/research",
        Some(Value::Object(body)),
        DEFAULT_TIMEOUT_MS,
        Endpoint { name: "Research task", validate: schemas::research_task },
    ) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(format!("Failed to start Tavily Research: {e}")),
    };
    let text = [
        "Tavily Research task created.".to_string(),
        String::new(),
        format!("- Status: {}", text(&task, "status")),
        format!("- Model: {}", text(&task, "model")),
        format!("- Request ID: {}", text(&task, "request_id")),
        format!("- Created at: {}", text(&task, "created_at")),
        String::new(),
        "Wait a few seconds, then call tavily_research_status with this request ID. Do not create the task again.".into(),
    ]
    .join("\n");
    ToolResult::ok(text, task)
}

fn research_status(args: &Map<String, Value>, http: &dyn Http, api_key: Option<&str>) -> ToolResult {
    let id = js_trim(str_arg(args, "request_id").unwrap_or_default());
    let path = format!("/research/{}", encode_uri_component(id));
    let result = match request_json(
        http,
        api_key,
        "GET",
        &path,
        None,
        DEFAULT_TIMEOUT_MS,
        Endpoint { name: "Research status", validate: schemas::research_status },
    ) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(format!("Failed to retrieve Tavily Research: {e}")),
    };
    let status = text(&result, "status");
    let request_id = text(&result, "request_id");

    if status == "failed" {
        let error = result.get("error").and_then(Value::as_str).unwrap_or("No error detail was provided.");
        let text =
            ["Tavily Research task failed.".to_string(), String::new(), format!("- Request ID: {request_id}"), format!("- Error: {error}")]
                .join("\n");
        return ToolResult { text, structured: Some(result), is_error: true };
    }
    if status != "completed" {
        let text = [
            "Tavily Research is still in progress.".to_string(),
            String::new(),
            format!("- Status: {status}"),
            format!("- Request ID: {request_id}"),
            String::new(),
            "Wait approximately five seconds before checking again.".into(),
        ]
        .join("\n");
        return ToolResult::ok(text, result);
    }

    let content = result.get("content");
    let report = match content {
        Some(Value::String(s)) => s.clone(),
        Some(v @ Value::Object(_)) => stringify_pretty(&crate::json::js_ordered(v.clone())),
        _ => "No report content was returned.".into(),
    };
    let sources = result.get("sources").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let formatted_sources = if sources.is_empty() {
        "No sources were returned.".to_string()
    } else {
        sources
            .iter()
            .enumerate()
            .map(|(i, s)| format!("[{}] {}\n{}", i + 1, text(s, "title"), text(s, "url")))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let already_has_sources = matches!(content, Some(Value::String(s)) if has_sources_heading(s));
    let mut sections = vec![report];
    if !already_has_sources {
        sections.push(format!("Sources\n\n{formatted_sources}"));
    }
    sections.push(
        [
            "Metadata".to_string(),
            String::new(),
            format!("- Status: {status}"),
            format!("- Request ID: {request_id}"),
            format!("- Response time: {} seconds", interp(result.get("response_time"))),
        ]
        .join("\n"),
    );
    ToolResult::ok(sections.join("\n\n"), result)
}

/// The JavaScript regex `/^\s{0,3}(?:#{1,6}\s*)?sources:?\s*$/im`, including how `^`, `$` and
/// `\s` treat `\r`, U+2028 and U+2029, which the `regex` crate would not.
pub fn has_sources_heading(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let terminator = |c: char| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    let ws = crate::json::is_js_whitespace;
    let starts = std::iter::once(0).chain((1..=chars.len()).filter(|&i| terminator(chars[i - 1])));
    starts.into_iter().any(|start| {
        let lead = chars[start..].iter().take_while(|c| ws(**c)).count();
        if lead > 3 {
            return false;
        }
        let mut p = start + lead;
        let hashes = chars[p..].iter().take_while(|c| **c == '#').count();
        if hashes > 6 {
            return false;
        }
        if hashes > 0 {
            p += hashes;
            p += chars[p..].iter().take_while(|c| ws(**c)).count();
        }
        let word = "sources";
        if chars.len() < p + word.len() || !chars[p..p + word.len()].iter().zip(word.chars()).all(|(c, w)| c.to_ascii_lowercase() == w) {
            return false;
        }
        p += word.len();
        if chars.get(p) == Some(&':') {
            p += 1;
        }
        // `\s*$`: the whitespace run must reach the end of the text or a line terminator.
        let run = &chars[p..];
        let ws_len = run.iter().take_while(|c| ws(**c)).count();
        ws_len == run.len() || run[..ws_len].iter().any(|c| terminator(*c))
    })
}

/// Zod's response schemas: `None` means "unexpected response". Objects come back with the schema's
/// keys first and any extra keys after, which is the order Zod's passthrough produced.
mod schemas {
    use serde_json::{Map, Value};

    type Check = fn(Option<&Value>) -> Option<Option<Value>>;

    fn reorder(map: &Map<String, Value>, shape: &[(&str, Check)]) -> Option<Value> {
        let mut out = Map::new();
        for (key, check) in shape {
            if let Some(v) = check(map.get(*key))? {
                out.insert((*key).to_string(), v);
            }
        }
        for (key, v) in map {
            if !shape.iter().any(|(k, _)| k == key) {
                out.insert(key.clone(), v.clone());
            }
        }
        Some(Value::Object(out))
    }

    fn object(v: &Value) -> Option<&Map<String, Value>> {
        v.as_object()
    }

    fn string(v: Option<&Value>) -> Option<Option<Value>> {
        matches!(v, Some(Value::String(_))).then(|| v.cloned())
    }
    /// `z.number()`, which rejects the Infinity that `JSON.parse` makes of `1e400`.
    fn finite(v: &Value) -> bool {
        v.as_number().is_some_and(|n| crate::json::as_js_f64(n).is_finite())
    }
    fn number(v: Option<&Value>) -> Option<Option<Value>> {
        v.is_some_and(finite).then(|| v.cloned())
    }
    fn str_or_num(v: Option<&Value>) -> Option<Option<Value>> {
        v.is_some_and(|v| v.is_string() || finite(v)).then(|| v.cloned())
    }
    /// `.nullable().optional()` and `.nullish()` of a string.
    fn opt_null_string(v: Option<&Value>) -> Option<Option<Value>> {
        matches!(v, None | Some(Value::Null | Value::String(_))).then(|| v.cloned())
    }
    /// `z.object({ credits: z.number() }).nullish()`: extra keys are stripped.
    fn usage(v: Option<&Value>) -> Option<Option<Value>> {
        match v {
            None => Some(None),
            Some(Value::Null) => Some(Some(Value::Null)),
            Some(Value::Object(m)) => {
                let credits = m.get("credits").filter(|c| finite(c))?;
                let mut out = Map::new();
                out.insert("credits".into(), credits.clone());
                Some(Some(Value::Object(out)))
            }
            _ => None,
        }
    }
    fn array_of(v: Option<&Value>, item: fn(&Value) -> Option<Value>) -> Option<Option<Value>> {
        let items = v?.as_array()?;
        let checked: Option<Vec<Value>> = items.iter().map(item).collect();
        Some(Some(Value::Array(checked?)))
    }
    fn optional_array_of(v: Option<&Value>, item: fn(&Value) -> Option<Value>) -> Option<Option<Value>> {
        if v.is_none() { Some(None) } else { array_of(v, item) }
    }

    fn search_result(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[
                ("title", string),
                ("url", string),
                ("content", string),
                ("score", number),
                ("raw_content", opt_null_string),
                ("favicon", opt_null_string),
            ],
        )
    }

    pub fn search(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[
                ("query", string),
                ("answer", opt_null_string),
                ("results", |v| array_of(v, search_result)),
                ("response_time", str_or_num),
                ("request_id", string),
                ("usage", usage),
            ],
        )
    }

    fn image(v: &Value) -> Option<Value> {
        match v {
            Value::String(_) => Some(v.clone()),
            Value::Object(m) => reorder(m, &[("url", string), ("description", opt_null_string)]),
            _ => None,
        }
    }

    fn page(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[("url", string), ("raw_content", string), ("images", |v| optional_array_of(v, image)), ("favicon", opt_null_string)],
        )
    }

    fn failure(v: &Value) -> Option<Value> {
        reorder(object(v)?, &[("url", string), ("error", opt_null_string)])
    }

    pub fn extract(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[
                ("results", |v| array_of(v, page)),
                ("failed_results", |v| optional_array_of(v, failure)),
                ("response_time", str_or_num),
                ("usage", usage),
                ("request_id", string),
            ],
        )
    }

    pub fn crawl(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[
                ("base_url", string),
                ("results", |v| array_of(v, page)),
                ("response_time", str_or_num),
                ("usage", usage),
                ("request_id", string),
            ],
        )
    }

    pub fn research_task(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[
                ("request_id", string),
                ("created_at", string),
                ("status", string),
                ("input", string),
                ("model", string),
                ("response_time", str_or_num),
            ],
        )
    }

    fn source(v: &Value) -> Option<Value> {
        reorder(object(v)?, &[("title", string), ("url", string), ("favicon", opt_null_string)])
    }

    /// `z.union([z.string(), z.record(z.string(), z.unknown())]).nullish()`
    fn content(v: Option<&Value>) -> Option<Option<Value>> {
        matches!(v, None | Some(Value::Null | Value::String(_) | Value::Object(_))).then(|| v.cloned())
    }

    pub fn research_status(v: &Value) -> Option<Value> {
        reorder(
            object(v)?,
            &[
                ("request_id", string),
                ("created_at", opt_null_string),
                ("status", string),
                ("content", content),
                ("sources", |v| optional_array_of(v, source)),
                ("response_time", str_or_num),
                ("error", opt_null_string),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::has_sources_heading;

    #[test]
    fn sources_heading_matches_the_javascript_regex() {
        let yes = [
            "Sources",
            "x\n## Sources:",
            "  ### sources  \nmore",
            "SOURCES\r\n- a",
            "#\n     sources",
            "body\u{2028}Sources",
            "\n\nSources",
        ];
        let no = ["These sources are good", "    Sources", "####### Sources", "Sources list", "sourcesx", ""];
        for t in yes {
            assert!(has_sources_heading(t), "{t:?}");
        }
        for t in no {
            assert!(!has_sources_heading(t), "{t:?}");
        }
    }
}
