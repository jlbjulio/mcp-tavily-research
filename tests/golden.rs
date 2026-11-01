//! Replays every tool call recorded from the TypeScript server (`tests/golden/cases.json`) against
//! a fake Tavily and requires the same request and the same JSON-RPC response bytes.

use std::sync::Mutex;

use mcp_tavily_research::server::{Action, Session};
use mcp_tavily_research::tavily::{Http, HttpRequest, HttpResponse};
use serde_json::Value;

struct Fake {
    upstream: Value,
    seen: Mutex<Vec<HttpRequest>>,
}

impl Http for Fake {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        self.seen.lock().unwrap().push(request.clone());
        match self.upstream.get("error").and_then(Value::as_str) {
            Some("network") => return Err("fetch failed".into()),
            Some("timeout") => return Err("The operation was aborted due to timeout".into()),
            _ => {}
        }
        if self.upstream.is_null() {
            return Err("golden: unexpected fetch".into());
        }
        Ok(HttpResponse {
            status: self.upstream["status"].as_u64().unwrap() as u16,
            status_text: self.upstream["statusText"].as_str().unwrap_or_default().to_string(),
            body: Ok(self.upstream["body"].as_str().unwrap().as_bytes().to_vec()),
        })
    }
}

/// The 2026-07-28 `_meta` envelope the capture script sent with every modern-era message.
const META_2026: &str = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"golden","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}"#;

#[test]
fn every_recorded_tool_call_matches_in_the_2025_era() {
    replay("cases.json");
}

#[test]
fn every_recorded_tool_call_matches_in_the_2026_era() {
    replay("cases-2026.json");
}

fn replay(file: &str) {
    let path = format!("{}/tests/golden/{file}", env!("CARGO_MANIFEST_DIR"));
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(cases.len() > 100, "golden file looks truncated");
    let mut failures = Vec::new();

    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let tool = case["tool"].as_str().unwrap();
        let modern = case["era"] == "2026-07-28";
        let extra = if modern { format!(",{META_2026}") } else { String::new() };
        let params = match case["arguments_json"].as_str() {
            Some(args) => format!(r#"{{"name":"{tool}","arguments":{args}{extra}}}"#),
            None => format!(r#"{{"name":"{tool}"{extra}}}"#),
        };
        let line = format!(r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{params}}}"#);
        let fake = Fake { upstream: case["upstream"].clone(), seen: Mutex::new(Vec::new()) };

        let mut session = Session::new();
        if modern {
            let discover = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{{{META_2026}}}}}"#);
            assert!(matches!(session.handle(&discover), Action::Reply(_)), "{name}: discover");
        }
        let reply = match session.handle(&line) {
            Action::Call(call) => call.run(&fake, case["api_key"].as_str()),
            Action::Reply(reply) => reply,
            _ => panic!("{name}: no reply"),
        };
        // JavaScript keeps the half surrogate pair `slice(0, 500)` leaves; Rust strings cannot hold
        // it, so that one character becomes U+FFFD. The server version moved from 1.0.0 to 2.x.
        let expected = case["expected_result_json"]
            .as_str()
            .unwrap()
            .replace("\\ud83d", "\u{fffd}")
            .replace(r#""version":"1.0.0""#, &format!(r#""version":"{}""#, env!("CARGO_PKG_VERSION")));
        if reply != expected {
            failures.push(format!("{name}: response differs\n  want: {expected}\n  got:  {reply}"));
        }

        let seen = fake.seen.lock().unwrap();
        let want = &case["expected_request"];
        match (want.is_null(), seen.as_slice()) {
            (true, []) => {}
            (false, [got]) => {
                let want_url = format!("{}{}", want["origin"].as_str().unwrap(), want["path"].as_str().unwrap());
                let checks = [
                    ("method", Value::from(got.method), want["method"].clone()),
                    ("url", Value::from(got.url.clone()), Value::from(want_url)),
                    ("authorization", Value::from(got.authorization.clone()), want["authorization"].clone()),
                    ("content_type", Value::from(got.body.as_ref().map(|_| "application/json")), want["content_type"].clone()),
                    ("body", Value::from(got.body.clone()), want["body"].clone()),
                    ("timeout_ms", Value::from(got.timeout_ms), Value::from(want["timeout_ms"].as_f64().unwrap())),
                ];
                for (field, got, want) in checks {
                    if got != want {
                        failures.push(format!("{name}: request {field}\n  want: {want}\n  got:  {got}"));
                    }
                }
            }
            _ => failures.push(format!("{name}: expected {} request(s), saw {}", u8::from(!want.is_null()), seen.len())),
        }
    }
    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures.join("\n"));
}
