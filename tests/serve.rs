//! The stdio loop: a cancelled tool call gets no reply, the others still answer, and closing
//! stdin waits for calls in flight.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mcp_tavily_research::server::serve;
use mcp_tavily_research::tavily::{Http, HttpRequest, HttpResponse};

/// Answers every search after a delay, long enough for a cancel to arrive first.
struct SlowTavily;

impl Http for SlowTavily {
    fn send(&self, _: &HttpRequest) -> Result<HttpResponse, String> {
        std::thread::sleep(Duration::from_millis(300));
        let body = r#"{"query":"q","results":[],"response_time":1,"request_id":"r"}"#;
        Ok(HttpResponse { status: 200, status_text: "OK".into(), body: Ok(body.as_bytes().to_vec()) })
    }
}

fn run(input: &str) -> Vec<String> {
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    serve(input.as_bytes(), out.clone(), Arc::new(SlowTavily), Arc::new(|| Some("tvly-test".into())));
    let bytes = out.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap().lines().map(String::from).collect()
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#;

fn call(id: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"tavily_search","arguments":{{"query":"q"}}}}}}"#)
}

#[test]
fn cancelled_call_gets_no_reply_and_others_still_answer() {
    let cancel = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#;
    let lines = run(&[INIT, &call("1"), &call("2"), cancel, ""].join("\n"));
    assert_eq!(lines.len(), 2, "{lines:#?}");
    assert!(lines[0].contains(r#""id":0"#));
    assert!(lines[1].ends_with(r#""jsonrpc":"2.0","id":2}"#), "{}", lines[1]);
}

#[test]
fn a_cancel_for_a_string_id_does_not_match_the_same_number() {
    let cancel = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"1"}}"#;
    let lines = run(&[INIT, &call("1"), cancel, ""].join("\n"));
    assert_eq!(lines.len(), 2, "{lines:#?}");
}

#[test]
fn closing_stdin_waits_for_calls_in_flight() {
    let lines = run(&[INIT, &call("\"a\""), &call("\"b\"")].join("\n"));
    assert_eq!(lines.len(), 3, "{lines:#?}");
}
