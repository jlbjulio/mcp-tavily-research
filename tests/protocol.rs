//! Runs the built server over real stdio with the message sessions recorded from the TypeScript
//! server (`tests/golden/protocol.json`) and requires the same stdout, line for line.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use serde_json::Value;

#[test]
fn every_recorded_session_matches() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/protocol.json");
    let sessions: serde_json::Map<String, Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = Vec::new();

    for (name, steps) in &sessions {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-tavily-research"))
            .env("TAVILY_API_KEY", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut expected = Vec::new();
        for step in steps.as_array().unwrap() {
            writeln!(stdin, "{}", step["sent"].as_str().unwrap()).unwrap();
            for line in step["received"].as_array().unwrap() {
                // Intended differences: the server version moved from 1.0.0 to this crate's, and a
                // lone surrogate (which a Rust string cannot hold) comes back as U+FFFD.
                let line = line
                    .as_str()
                    .unwrap()
                    .replace(r#""version":"1.0.0""#, &format!(r#""version":"{}""#, env!("CARGO_PKG_VERSION")))
                    .replace("\\ud83d", "\u{fffd}");
                expected.push(line);
            }
        }
        drop(stdin);
        let mut out = String::new();
        child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
        assert!(child.wait().unwrap().success(), "{name}: server exited with an error");
        // Tool calls answer asynchronously (in Node too), so a reply may overtake an earlier tool
        // result; every line must still match exactly, in any order.
        let mut got: Vec<&str> = out.lines().collect();
        got.sort_unstable();
        expected.sort_unstable();
        if got != expected {
            let i = got.iter().zip(&expected).position(|(g, e)| g != e).unwrap_or(got.len().min(expected.len()));
            failures.push(format!(
                "{name}: line {i} differs ({} lines vs {} expected)\n  want: {}\n  got:  {}",
                got.len(),
                expected.len(),
                expected.get(i).map_or("<none>", |s| s.as_str()),
                got.get(i).copied().unwrap_or("<none>")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn startup_message_goes_to_stderr_and_bad_arguments_fail() {
    let out = Command::new(env!("CARGO_BIN_EXE_mcp-tavily-research")).stdin(Stdio::null()).output().unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("is running over stdio."), "{stderr}");

    let out = Command::new(env!("CARGO_BIN_EXE_mcp-tavily-research")).arg("--nope").stdin(Stdio::null()).output().unwrap();
    assert!(!out.status.success());

    let missing = Command::new(env!("CARGO_BIN_EXE_mcp-tavily-research"))
        .arg("--env-file-if-exists=does-not-exist.env")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(missing.status.success());
    let required =
        Command::new(env!("CARGO_BIN_EXE_mcp-tavily-research")).arg("--env-file=does-not-exist.env").stdin(Stdio::null()).output().unwrap();
    assert!(!required.status.success());
}

/// Runs one tools/call that needs the API key; with no usable key the reply says so without any
/// network call.
fn key_reply(args: &[&str], env_key: Option<&str>) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-tavily-research"));
    cmd.args(args).env_remove("TAVILY_API_KEY").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    if let Some(k) = env_key {
        cmd.env("TAVILY_API_KEY", k);
    }
    let mut child = cmd.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"tavily_research_status","arguments":{{"request_id":"x"}}}}}}"#
    )
    .unwrap();
    drop(stdin);
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    child.wait().unwrap();
    out
}

#[test]
fn env_files_load_in_order_and_never_override_the_environment() {
    let dir = env!("CARGO_TARGET_TMPDIR");
    let first = format!("{dir}/first.env");
    let blank = format!("{dir}/blank.env");
    std::fs::write(&first, "TAVILY_API_KEY=from-first-file\n").unwrap();
    std::fs::write(&blank, "TAVILY_API_KEY=\n").unwrap();
    let missing = "TAVILY_API_KEY is required";

    // The last file wins: its blank key means no key, so no request is sent.
    let flags = [format!("--env-file={first}"), format!("--env-file-if-exists={blank}")];
    assert!(key_reply(&[&flags[0], &flags[1]], None).contains(missing));
    // The space-separated form works the same.
    assert!(key_reply(&["--env-file", &first, "--env-file-if-exists", &blank], None).contains(missing));
    // A variable from the real environment is never replaced by a file.
    assert!(key_reply(&[&flags[1]], Some("   ")).contains(missing));
    assert!(key_reply(&[], None).contains(missing));
}
