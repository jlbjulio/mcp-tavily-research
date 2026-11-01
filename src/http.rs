//! The real HTTP client. Error messages are the ones Node's `fetch` threw, because the tools put
//! them in front of the model.

use std::time::Duration;

use crate::tavily::{Http, HttpRequest, HttpResponse};

const TIMEOUT: &str = "The operation was aborted due to timeout";

pub struct Client {
    agent: ureq::Agent,
}

impl Client {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            // Non-2xx answers are read like any other: the tools report their status and body.
            .http_status_as_error(false)
            // Node's fetch ignores HTTP(S)_PROXY, so this does too.
            .proxy(None)
            .build()
            .into();
        Self { agent }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

fn message(error: &ureq::Error, otherwise: &str) -> String {
    match error {
        ureq::Error::Timeout(_) => TIMEOUT.into(),
        _ => otherwise.into(),
    }
}

impl Http for Client {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        let timeout = Some(Duration::from_secs_f64(request.timeout_ms / 1_000.0));
        let sent = match request.method {
            "GET" => {
                self.agent.get(&request.url).config().timeout_global(timeout).build().header("Authorization", &request.authorization).call()
            }
            "POST" => self
                .agent
                .post(&request.url)
                .config()
                .timeout_global(timeout)
                .build()
                .header("Authorization", &request.authorization)
                .header("Content-Type", "application/json")
                .send(request.body.as_deref().unwrap_or_default().as_bytes()),
            other => unreachable!("the tools only send GET and POST, not {other}"),
        };
        let mut response = sent.map_err(|e| message(&e, "fetch failed"))?;
        let status = response.status();
        // Node's body reader has no size cap; a 50-page crawl can pass ureq's 10 MB default.
        let body = response.body_mut().with_config().limit(u64::MAX).read_to_vec().map_err(|e| message(&e, "terminated"));
        Ok(HttpResponse { status: status.as_u16(), status_text: status.canonical_reason().unwrap_or_default().to_string(), body })
    }
}
