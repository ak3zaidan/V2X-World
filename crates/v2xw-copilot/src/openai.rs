//! The OpenAI transport.
//!
//! Chat Completions with function calling, over whatever [`HttpPost`] the caller supplies.
//! The two interesting functions, [`encode_request`] and [`decode_completion`], are pure
//! and are tested against recorded bodies; the only impure part is the one call to
//! [`HttpPost::post_json`] between them.
//!
//! # The key
//!
//! Read once from `OPENAI_API_KEY` ([`KEY_VARIABLE`]) into a [`Secret`], which has no
//! `Display` and no `Serialize`. It goes into the `Authorization` header and nowhere else:
//! not into a log line, not into a file, not into an argument vector (see [`crate::http`]),
//! and not into a transcript — [`crate::session::Turn`] holds tool calls and results, never
//! a request. Anything the API says back is passed through [`Secret::redact`] before it is
//! surfaced, so an endpoint that echoed the key could not make this crate print it.
//!
//! # Model choice
//!
//! `OPENAI_MODEL` overrides [`DEFAULT_MODEL`], and `OPENAI_BASE_URL` overrides the
//! endpoint, which is what an Azure deployment or a local gateway needs. The reasoning
//! tiers reject a non-default `temperature`; a `400` that names the field is retried once
//! without it, so one build works from `gpt-4o-mini` upwards without per-model wiring.

use serde_json::{Map, Value, json};

use crate::error::{CopilotError, Result};
use crate::http::{HttpPost, HttpRequest};
use crate::provider::{ChatMessage, ChatRequest, Completion, LlmProvider, Role, ToolCall, Usage};
use crate::secret::Secret;

/// The environment variable the key is read from.
pub const KEY_VARIABLE: &str = "OPENAI_API_KEY";
/// The environment variable that overrides [`DEFAULT_MODEL`].
pub const MODEL_VARIABLE: &str = "OPENAI_MODEL";
/// The environment variable that overrides [`DEFAULT_URL`].
pub const URL_VARIABLE: &str = "OPENAI_BASE_URL";
/// The environment variable that sets `reasoning_effort` (`minimal`, `low`, `medium`,
/// `high`) for a reasoning tier. Unset, the field is not sent and the model's own default
/// applies.
pub const EFFORT_VARIABLE: &str = "OPENAI_REASONING_EFFORT";
/// The reasoning effort sent on the Responses API when [`EFFORT_VARIABLE`] is unset:
/// `low`. Measured on the agent's suite with `gpt-6.1-sol` (2026-10-07): a model call took
/// about 5 s at `low` against about 8 s at the model's default, with the same scores — the
/// expensive judgement (bottlenecks, numbers) is the deterministic analyst's, and the model
/// mostly plans one tool call and writes.
pub const RESPONSES_EFFORT: &str = "low";
/// The default endpoint.
pub const DEFAULT_URL: &str = "https://api.openai.com/v1/chat/completions";
/// The default model: the cheapest tier that calls tools reliably.
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";

/// The environment variable that picks the wire format: `chat` (Chat Completions, the
/// default) or `responses` (the Responses API, [`crate::openai_responses`]). Unset, the
/// provider starts on Chat Completions and moves to the Responses API, for good, the first
/// time the model says tools need it.
pub const API_VARIABLE: &str = "OPENAI_API";

/// Which of OpenAI's two request formats the provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// `POST /v1/chat/completions`.
    Chat,
    /// `POST /v1/responses`.
    Responses,
}

/// An OpenAI-compatible chat provider.
#[derive(Debug)]
pub struct OpenAi<H: HttpPost> {
    http: H,
    key: Secret,
    model: String,
    url: String,
    api: Api,
    temperature: Option<f64>,
    reasoning_effort: Option<String>,
    name: String,
    retries: u32,
    pause_s: u64,
    last_usage: Option<Usage>,
}

impl<H: HttpPost> OpenAi<H> {
    /// A provider with an explicit key.
    #[must_use]
    pub fn new(http: H, key: Secret) -> Self {
        let model = DEFAULT_MODEL.to_string();
        OpenAi {
            http,
            key,
            name: format!("openai:{model}"),
            model,
            url: DEFAULT_URL.to_string(),
            api: Api::Chat,
            // Zero rather than the legacy 0.2: a research assistant that answers the same
            // question two ways is one the reader cannot check. It is not determinism —
            // nothing about a hosted model is deterministic, and nothing this returns
            // reaches a result — but it is the least variance available.
            temperature: Some(0.0),
            reasoning_effort: None,
            retries: 2,
            pause_s: 2,
            last_usage: None,
        }
    }

    /// A provider configured entirely from the environment.
    ///
    /// # Errors
    /// [`CopilotError::MissingApiKey`] if [`KEY_VARIABLE`] is unset or empty.
    pub fn from_env(http: H) -> Result<Self> {
        let key = Secret::from_env(KEY_VARIABLE)?;
        let mut provider = OpenAi::new(http, key);
        if let Ok(model) = std::env::var(MODEL_VARIABLE) {
            let model = model.trim();
            if !model.is_empty() {
                provider = provider.with_model(model);
            }
        }
        if let Ok(url) = std::env::var(URL_VARIABLE) {
            let url = url.trim();
            if !url.is_empty() {
                provider = provider.with_url(url);
            }
        }
        if let Ok(effort) = std::env::var(EFFORT_VARIABLE) {
            let effort = effort.trim();
            if !effort.is_empty() {
                provider.reasoning_effort = Some(effort.to_string());
            }
        }
        if std::env::var(API_VARIABLE).is_ok_and(|v| v.trim().eq_ignore_ascii_case("responses")) {
            provider = provider.with_api(Api::Responses);
        }
        Ok(provider)
    }

    /// The same, with another model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self.name = format!("openai:{}", self.model);
        self
    }

    /// The same, with another endpoint.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// The same, with another sampling temperature, or none at all.
    #[must_use]
    pub fn with_temperature(mut self, temperature: Option<f64>) -> Self {
        self.temperature = temperature;
        self
    }

    /// The same, speaking another of OpenAI's request formats. Moving to the Responses API
    /// moves the endpoint with it (`…/chat/completions` becomes `…/responses`).
    #[must_use]
    pub fn with_api(mut self, api: Api) -> Self {
        if api == Api::Responses && self.api == Api::Chat {
            self.url = responses_url(&self.url);
        }
        self.api = api;
        self
    }

    /// The request format the provider speaks now.
    #[must_use]
    pub fn api(&self) -> Api {
        self.api
    }

    /// The same, with a `reasoning_effort`, or none.
    #[must_use]
    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    /// The same, with no retries of a busy or failed endpoint (tests).
    #[must_use]
    pub fn without_retries(mut self) -> Self {
        self.retries = 0;
        self
    }

    /// The model this provider talks to.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// One round trip, with the body already encoded. A rate limit (429) or a server error
    /// (5xx) is tried again after a pause, up to `retries` times; anything else is the
    /// request's fault and would fail the same way again.
    fn round_trip(&mut self, body: String) -> Result<Value> {
        let mut attempt = 0;
        loop {
            let request = HttpRequest::json(self.url.clone(), body.clone(), self.key.clone());
            // A request that got no answer (timeout, reset) is sent again like a 5xx.
            let response = match self.http.post_json(&request) {
                Ok(r) => r,
                Err(e) if crate::http::transient_failure(&e) && attempt < self.retries => {
                    attempt += 1;
                    std::thread::sleep(std::time::Duration::from_secs(
                        u64::from(attempt) * self.pause_s,
                    ));
                    continue;
                }
                Err(e) => return Err(e),
            };
            let text = self.key.redact(&response.body);
            if response.is_success() {
                return serde_json::from_str(&text)
                    .map_err(|e| CopilotError::Decode(format!("the reply is not JSON: {e}")));
            }
            let retryable = response.is_transient();
            if retryable && attempt < self.retries {
                attempt += 1;
                // A pause between attempts is not a reading of the clock: nothing is
                // derived from it.
                std::thread::sleep(std::time::Duration::from_secs(
                    u64::from(attempt) * self.pause_s,
                ));
                continue;
            }
            return Err(CopilotError::Provider(format!(
                "HTTP {}: {}",
                response.status,
                api_error_message(&text)
            )));
        }
    }

    /// The body for one request, with `reasoning_effort` when one is set.
    fn encode(&self, request: &ChatRequest) -> Result<String> {
        if self.api == Api::Responses {
            return crate::openai_responses::encode_request(
                &self.model,
                request,
                self.temperature,
                Some(self.reasoning_effort.as_deref().unwrap_or(RESPONSES_EFFORT)),
            );
        }
        let body = encode_request(&self.model, request, self.temperature)?;
        match &self.reasoning_effort {
            None => Ok(body),
            Some(effort) => {
                let mut value: Value = serde_json::from_str(&body)?;
                value["reasoning_effort"] = json!(effort);
                Ok(serde_json::to_string(&value)?)
            }
        }
    }
}

impl<H: HttpPost> LlmProvider for OpenAi<H> {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&mut self, request: &ChatRequest) -> Result<Completion> {
        self.last_usage = None;
        // Two refusals are adapted to, each at most once and then remembered for every
        // later request (a refusal costs a whole round trip, and the next request would be
        // refused the same way):
        //
        // * a reasoning tier that refuses a non-default `temperature` — the field is
        //   dropped;
        // * a reasoning tier that refuses function tools while it reasons on this endpoint
        //   ("Function tools with reasoning_effort are not supported … set
        //   reasoning_effort to 'none'", which `gpt-6.1-sol` answers on
        //   /v1/chat/completions) — `reasoning_effort: "none"` is sent, which is the
        //   documented way to call tools there.
        //
        // So one build works across model generations without per-model wiring.
        let mut adapted = 0;
        let value = loop {
            let body = self.encode(request)?;
            // Bound before the match: a `&mut self` borrow held in a match scrutinee lasts
            // the whole match, and the arms below change `self`.
            let attempt = self.round_trip(body);
            match attempt {
                Ok(value) => break value,
                Err(CopilotError::Provider(message))
                    if adapted < 3 && message.starts_with("HTTP 400") =>
                {
                    if self.temperature.is_some() && message.contains("temperature") {
                        self.temperature = None;
                    } else if self.api == Api::Chat && message.contains("/v1/responses") {
                        // "To use function tools, use /v1/responses": a reasoning tier that
                        // calls tools only there. Moving is preferred to switching its
                        // reasoning off, which the same tier refuses anyway.
                        self.api = Api::Responses;
                        self.url = responses_url(&self.url);
                    } else if message.contains("reasoning_effort")
                        && message.contains("'none'")
                        && self.reasoning_effort.as_deref() != Some("none")
                    {
                        self.reasoning_effort = Some("none".to_string());
                    } else {
                        return Err(CopilotError::Provider(message));
                    }
                    adapted += 1;
                }
                Err(e) => return Err(e),
            }
        };
        if self.api == Api::Responses {
            let completion = crate::openai_responses::decode_completion(&value)?;
            self.last_usage = crate::openai_responses::decode_usage(&value);
            return Ok(completion);
        }
        let completion = decode_completion(&value)?;
        self.last_usage = decode_usage(&value);
        Ok(completion)
    }

    fn last_usage(&self) -> Option<Usage> {
        self.last_usage
    }
}

/// The Responses endpoint beside a Chat Completions one: `…/chat/completions` becomes
/// `…/responses`, and any other URL (a gateway with its own path) is kept as given.
#[must_use]
pub fn responses_url(chat_url: &str) -> String {
    match chat_url.strip_suffix("/chat/completions") {
        Some(base) => format!("{base}/responses"),
        None => chat_url.to_string(),
    }
}

/// The `usage` block of a reply: `prompt_tokens` (cached ones included, and counted again
/// in `prompt_tokens_details.cached_tokens`) and `completion_tokens` (reasoning included).
#[must_use]
pub fn decode_usage(value: &Value) -> Option<Usage> {
    let u = value.get("usage")?;
    let n = |p: &str| u.pointer(p).and_then(Value::as_u64).unwrap_or(0);
    Some(Usage {
        input_tokens: n("/prompt_tokens"),
        cached_input_tokens: n("/prompt_tokens_details/cached_tokens"),
        output_tokens: n("/completion_tokens"),
    })
}

/// The request body for one completion.
///
/// # Errors
/// [`CopilotError::Json`] if the body will not serialise, which plain data does not fail
/// at.
pub fn encode_request(
    model: &str,
    request: &ChatRequest,
    temperature: Option<f64>,
) -> Result<String> {
    let mut body = Map::new();
    body.insert("model".to_string(), json!(model));
    body.insert(
        "messages".to_string(),
        Value::Array(request.messages.iter().map(encode_message).collect()),
    );
    if !request.tools.is_empty() {
        body.insert("tools".to_string(), Value::Array(request.tools.clone()));
        body.insert("tool_choice".to_string(), json!("auto"));
    }
    if let Some(t) = temperature {
        body.insert("temperature".to_string(), json!(t));
    }
    Ok(serde_json::to_string(&Value::Object(body))?)
}

/// One message in the shape the API takes.
fn encode_message(message: &ChatMessage) -> Value {
    let mut out = Map::new();
    out.insert("role".to_string(), json!(message.role.wire()));
    match &message.content {
        Some(text) => out.insert("content".to_string(), json!(text)),
        // An assistant message that only calls tools must still carry the field, as null.
        None => out.insert("content".to_string(), Value::Null),
    };
    if !message.tool_calls.is_empty() {
        let calls: Vec<Value> = message
            .tool_calls
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "type": "function",
                    "function": {"name": c.name, "arguments": c.arguments}
                })
            })
            .collect();
        out.insert("tool_calls".to_string(), Value::Array(calls));
    }
    if let Some(id) = &message.tool_call_id {
        out.insert("tool_call_id".to_string(), json!(id));
    }
    Value::Object(out)
}

/// The assistant message out of a reply body.
///
/// # Errors
/// [`CopilotError::Provider`] when the body is an API error object, and
/// [`CopilotError::Decode`] when it has no first choice with a message.
pub fn decode_completion(value: &Value) -> Result<Completion> {
    if let Some(error) = value.get("error") {
        return Err(CopilotError::Provider(
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the API returned an error with no message")
                .to_string(),
        ));
    }
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .ok_or_else(|| CopilotError::Decode("the reply has no choices".to_string()))?;
    let message = choice
        .get("message")
        .ok_or_else(|| CopilotError::Decode("the first choice has no message".to_string()))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut tool_calls = Vec::new();
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (i, call) in calls.iter().enumerate() {
            let function = call
                .get("function")
                .ok_or_else(|| CopilotError::Decode(format!("tool call {i} has no function")))?;
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| CopilotError::Decode(format!("tool call {i} has no name")))?;
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}")
                .to_string();
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .map_or_else(|| format!("call_{i}"), str::to_string);
            tool_calls.push(ToolCall {
                id,
                name: name.to_string(),
                arguments,
            });
        }
    }
    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .unwrap_or("stop")
        .to_string();
    Ok(Completion {
        message: ChatMessage {
            role: Role::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
            provider_blocks: None,
        },
        finish_reason,
    })
}

/// The `error.message` of an API error body, or the body itself when it is not one.
fn api_error_message(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpResponse, ScriptedPost};

    fn one_tool_call_reply() -> &'static str {
        r#"{"choices":[{"index":0,"finish_reason":"tool_calls","message":{"role":"assistant",
        "content":null,"tool_calls":[{"id":"call_a","type":"function","function":
        {"name":"registry__parameter","arguments":"{\"model\":\"m\",\"name\":\"p\"}"}}]}}]}"#
    }

    #[test]
    fn a_tool_call_reply_decodes() {
        let value: Value = serde_json::from_str(one_tool_call_reply()).expect("fixture parses");
        let c = decode_completion(&value).expect("decodes");
        assert_eq!(c.finish_reason, "tool_calls");
        assert_eq!(c.message.tool_calls.len(), 1);
        assert_eq!(c.message.tool_calls[0].name, "registry__parameter");
        assert_eq!(c.message.tool_calls[0].id, "call_a");
        assert_eq!(c.message.content, None);
    }

    #[test]
    fn an_error_body_is_a_provider_error_not_a_completion() {
        let value: Value =
            serde_json::from_str(r#"{"error":{"message":"model not found","type":"x"}}"#)
                .expect("fixture parses");
        let err = decode_completion(&value).expect_err("an error body is not a completion");
        assert!(err.to_string().contains("model not found"));
    }

    #[test]
    fn the_request_carries_the_tools_and_the_conversation() {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("rules"),
                ChatMessage::user("what is the path loss exponent?"),
                ChatMessage::calls(
                    None,
                    vec![ToolCall {
                        id: "call_a".to_string(),
                        name: "registry__parameter".to_string(),
                        arguments: "{}".to_string(),
                    }],
                ),
                ChatMessage::tool("call_a", "{\"known\":false}"),
            ],
            tools: vec![json!({"type": "function", "function": {"name": "registry__parameter"}})],
        };
        let body = encode_request("gpt-4o-mini", &request, Some(0.0)).expect("encodes");
        let value: Value = serde_json::from_str(&body).expect("valid JSON");
        assert_eq!(value["model"], json!("gpt-4o-mini"));
        assert_eq!(value["messages"].as_array().map(Vec::len), Some(4));
        assert_eq!(value["messages"][0]["role"], json!("system"));
        assert_eq!(value["messages"][2]["content"], Value::Null);
        assert_eq!(
            value["messages"][2]["tool_calls"][0]["function"]["name"],
            json!("registry__parameter")
        );
        assert_eq!(value["messages"][3]["tool_call_id"], json!("call_a"));
        assert_eq!(value["tool_choice"], json!("auto"));
        assert_eq!(value["temperature"], json!(0.0));
    }

    #[test]
    fn no_temperature_field_when_it_is_dropped() {
        let body = encode_request("o9", &ChatRequest::default(), None).expect("encodes");
        assert!(!body.contains("temperature"));
    }

    #[test]
    fn the_key_never_reaches_the_body_and_only_the_header_holds_it() {
        let post = ScriptedPost::new(vec![HttpResponse::new(200, one_tool_call_reply())]);
        let mut provider = OpenAi::new(post, Secret::new("sk-secret"));
        let completion = provider
            .complete(&ChatRequest {
                messages: vec![ChatMessage::user("hello")],
                tools: Vec::new(),
            })
            .expect("the scripted reply decodes");
        assert_eq!(completion.message.tool_calls.len(), 1);
        let sent = &provider.http.seen[0];
        assert!(!sent.body.contains("sk-secret"), "the key reached the body");
        assert_eq!(
            sent.bearer.as_ref().map(crate::secret::Secret::expose),
            Some("sk-secret")
        );
        assert_eq!(sent.url, DEFAULT_URL);
    }

    #[test]
    fn an_error_status_is_reported_with_the_key_stripped_out() {
        let post = ScriptedPost::new(vec![HttpResponse::new(
            401,
            r#"{"error":{"message":"Incorrect API key provided: sk-secret"}}"#,
        )]);
        let mut provider = OpenAi::new(post, Secret::new("sk-secret"));
        let err = provider
            .complete(&ChatRequest::default())
            .expect_err("401 is not a completion");
        let text = err.to_string();
        assert!(text.contains("HTTP 401"));
        assert!(
            !text.contains("sk-secret"),
            "the key was echoed into an error"
        );
        assert!(text.contains("<redacted>"));
    }

    #[test]
    fn a_temperature_refusal_is_retried_once_without_it() {
        let post = ScriptedPost::new(vec![
            HttpResponse::new(
                400,
                r#"{"error":{"message":"Unsupported value: 'temperature' does not support 0.0"}}"#,
            ),
            HttpResponse::new(200, one_tool_call_reply()),
        ]);
        let mut provider = OpenAi::new(post, Secret::new("sk-secret"));
        let completion = provider
            .complete(&ChatRequest::default())
            .expect("the retry succeeds");
        assert_eq!(completion.finish_reason, "tool_calls");
        assert_eq!(provider.http.calls(), 2);
        assert!(provider.http.seen[0].body.contains("temperature"));
        assert!(!provider.http.seen[1].body.contains("temperature"));
    }

    #[test]
    fn a_temperature_refusal_is_remembered_so_the_next_request_is_not_refused_again() {
        let post = ScriptedPost::new(vec![
            HttpResponse::new(
                400,
                r#"{"error":{"message":"Unsupported value: 'temperature' does not support 0.0"}}"#,
            ),
            HttpResponse::new(200, one_tool_call_reply()),
            HttpResponse::new(200, one_tool_call_reply()),
        ]);
        let mut provider = OpenAi::new(post, Secret::new("sk-secret"));
        provider
            .complete(&ChatRequest::default())
            .expect("the retry succeeds");
        provider
            .complete(&ChatRequest::default())
            .expect("the second request succeeds");
        assert_eq!(
            provider.http.calls(),
            3,
            "the second request was refused and retried"
        );
        assert!(!provider.http.seen[2].body.contains("temperature"));
    }

    /// What `gpt-6.1-sol` answers on Chat Completions to a request with tools (recorded
    /// 2026-10-06, the key stripped).
    const TOOLS_NEED_RESPONSES: &str = r#"{"error":{"message":"Function tools with reasoning_effort are not supported for gpt-6.1-sol in /v1/chat/completions. To use function tools, use /v1/responses or set reasoning_effort to 'none'."}}"#;

    fn responses_reply() -> &'static str {
        r#"{"id":"resp_1","status":"completed","output":[
          {"type":"function_call","id":"fc_1","call_id":"call_1","name":"simulate","arguments":"{}"}],
          "usage":{"input_tokens":10,"output_tokens":2}}"#
    }

    #[test]
    fn a_tools_need_responses_refusal_moves_to_the_responses_api_for_good() {
        let post = ScriptedPost::new(vec![
            HttpResponse::new(400, TOOLS_NEED_RESPONSES),
            HttpResponse::new(200, responses_reply()),
            HttpResponse::new(200, responses_reply()),
        ]);
        let mut provider = OpenAi::new(post, Secret::new("sk-test-not-a-key"))
            .with_temperature(None)
            .with_model("gpt-6.1-sol");
        let c = provider
            .complete(&ChatRequest::default())
            .expect("the retry succeeds");
        assert_eq!(c.message.tool_calls[0].name, "simulate");
        provider
            .complete(&ChatRequest::default())
            .expect("the next request succeeds");
        assert_eq!(
            provider.http.calls(),
            3,
            "the second request went straight to /v1/responses"
        );
        assert_eq!(provider.api(), Api::Responses);
        assert_eq!(provider.http.seen[0].url, DEFAULT_URL);
        assert_eq!(
            provider.http.seen[1].url,
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            provider.http.seen[2].url,
            "https://api.openai.com/v1/responses"
        );
        assert!(provider.http.seen[2].body.contains("\"input\""));
        assert_eq!(provider.last_usage().map(|u| u.input_tokens), Some(10));
    }

    #[test]
    fn a_reasoning_none_hint_without_the_responses_route_switches_reasoning_off() {
        let refusal = r#"{"error":{"message":"Function tools with reasoning_effort are not supported here; set reasoning_effort to 'none'."}}"#;
        let post = ScriptedPost::new(vec![
            HttpResponse::new(400, refusal),
            HttpResponse::new(200, one_tool_call_reply()),
        ]);
        let mut provider =
            OpenAi::new(post, Secret::new("sk-test-not-a-key")).with_temperature(None);
        provider
            .complete(&ChatRequest::default())
            .expect("the retry succeeds");
        assert!(
            provider.http.seen[1]
                .body
                .contains("\"reasoning_effort\":\"none\"")
        );
        assert_eq!(provider.api(), Api::Chat);
    }

    #[test]
    fn an_unrelated_400_is_not_retried() {
        let post = ScriptedPost::new(vec![HttpResponse::new(
            400,
            r#"{"error":{"message":"messages: too long"}}"#,
        )]);
        let mut provider = OpenAi::new(post, Secret::new("sk-test-not-a-key"));
        assert!(provider.complete(&ChatRequest::default()).is_err());
        assert_eq!(provider.http.calls(), 1);
    }

    #[test]
    fn usage_is_read_from_the_reply_and_a_busy_endpoint_is_retried() {
        let reply = r#"{"choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant",
        "content":"ok"}}],"usage":{"prompt_tokens":1200,"completion_tokens":80,
        "prompt_tokens_details":{"cached_tokens":1024}}}"#;
        let post = ScriptedPost::new(vec![
            HttpResponse::new(429, r#"{"error":{"message":"slow down"}}"#),
            HttpResponse::new(200, reply),
        ]);
        let mut provider = OpenAi::new(post, Secret::new("sk-secret"));
        provider.pause_s = 0;
        provider
            .complete(&ChatRequest::default())
            .expect("the retry succeeds");
        assert_eq!(provider.http.calls(), 2);
        assert_eq!(
            provider.last_usage(),
            Some(Usage {
                input_tokens: 1200,
                cached_input_tokens: 1024,
                output_tokens: 80
            })
        );
    }

    #[test]
    fn reasoning_effort_is_sent_only_when_asked_for() {
        let post = ScriptedPost::new(vec![
            HttpResponse::new(200, one_tool_call_reply()),
            HttpResponse::new(200, one_tool_call_reply()),
        ]);
        let mut provider = OpenAi::new(post, Secret::new("sk-test-not-a-key"))
            .with_reasoning_effort(Some("low".into()));
        provider.complete(&ChatRequest::default()).expect("decodes");
        assert!(
            provider.http.seen[0]
                .body
                .contains("\"reasoning_effort\":\"low\"")
        );
        provider = provider.with_reasoning_effort(None);
        provider.complete(&ChatRequest::default()).expect("decodes");
        assert!(!provider.http.seen[1].body.contains("reasoning_effort"));
    }
}
