use std::sync::{Arc, Mutex};

use mcp_tavily_research::envfile;
use mcp_tavily_research::http::Client;
use mcp_tavily_research::server::{SERVER_NAME, SERVER_VERSION, serve};

fn main() {
    let protected = envfile::protected();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // Both `--env-file=PATH` and `--env-file PATH`, as Node accepts.
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, path)) => (flag.to_string(), Some(path.to_string())),
            None => (arg.clone(), None),
        };
        let required = match flag.as_str() {
            "--env-file-if-exists" => false,
            "--env-file" => true,
            _ => fail(&format!("unknown argument: {arg}")),
        };
        let Some(path) = inline.or_else(|| args.next()) else { fail(&format!("{flag} needs a path")) };
        if let Err(e) = envfile::load(&path, required, &protected) {
            fail(&e);
        }
    }

    // stderr only: stdout carries the JSON-RPC stream.
    eprintln!("{SERVER_NAME} {SERVER_VERSION} is running over stdio.");
    serve(
        std::io::stdin().lock(),
        Arc::new(Mutex::new(std::io::stdout())),
        Arc::new(Client::new()),
        Arc::new(|| std::env::var("TAVILY_API_KEY").ok()),
    );
}

fn fail(message: &str) -> ! {
    eprintln!("{SERVER_NAME}: {message}");
    std::process::exit(9);
}
