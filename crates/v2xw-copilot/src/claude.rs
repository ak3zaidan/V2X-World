//! The Claude provider: Anthropic's Messages API behind [`LlmProvider`], and the default.
//!
//! Rust has no official Anthropic SDK, so this speaks the documented HTTP surface
//! (`POST /v1/messages`, `x-api-key`, `anthropic-version: 2023-06-01`) through the same
//! [`HttpPost`] seam the OpenAI provider uses. As there, the two interesting functions,
//! [`encode_request`] and [`decode_completion`], are pure and tested against recorded
//! bodies; the only impure part is one [`HttpPost::post_json`] between them.
//!
//! # The key
//!
//! [`resolve_key`] reads `ANTHROPIC_API_KEY` from the server's environment, or failing
//! that a key file outside the repository (`V2XW_ANTHROPIC_KEY_FILE`, else
//! `$XDG_CONFIG_HOME/v2xw/anthropic.key`, else `~/.config/v2xw/anthropic.key`). It lands in
//! a [`Secret`] and goes into the `x-api-key` header, written to `curl`'s standard input
//! and nowhere else: not into a log, an argument vector, a transcript, a tool result or
//! anything served to a browser. Everything the API says back is passed through
//! [`Secret::redact`] before it is surfaced.
//!
//! # The request
//!
//! Following Anthropic's guidance for the current models (the `claude-api` reference):
//!
//! * model `claude-opus-5-5` ([`DEFAULT_MODEL`]); `V2XW_AGENT_MODEL` overrides it;
//! * adaptive thinking (`thinking: {type: "adaptive"}`), with the effort set explicitly
//!   (`output_config.effort`). The default here is `medium` — the model's own default — because
//!   the expensive reasoning in this harness (bottlenecks, numbers) is done by the
//!   deterministic analyst and the model mostly plans and writes; `V2XW_AGENT_EFFORT`
//!   raises it;
//! * `max_tokens` 16 000 on a non-streaming request, which keeps a reply within the HTTP
//!   timeout while leaving room for thinking;
//! * prompt caching with a top-level `cache_control`, which places the breakpoint on the
//!   last cacheable block: the tool list and the system prompt are built deterministically
//!   (`BTreeMap` order, no clock, no ids), so the prefix is byte-stable between rounds;
//! * server-side refusal fallbacks, `fallbacks: "default"` under the
//!   `server-side-fallback-2026-07-01` beta, so a classifier false positive is re-run on
//!   Anthropic's recommended substitute instead of ending the run's analysis.
//!   `V2XW_AGENT_FALLBACKS=off` drops it (a proxy that rejects beta fields);
//! * tool choice left at `auto` (forced tool use is rejected by this model).
//!
//! # The history is append-only
//!
//! Thinking blocks are bound to the conversation that produced them: replaying an edited
//! prefix is refused. Every assistant reply's content blocks are therefore kept verbatim
//! in [`ChatMessage::provider_blocks`] and sent back unchanged, and the agent loop never
//! rewrites an earlier message. All tool results of one reply go back in a single user
//! message, failed ones marked `is_error`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::error::{CopilotError, Result};
use crate::http::{CurlPost, HttpPost, HttpRequest};
use crate::provider::{ChatMessage, ChatRequest, Completion, LlmProvider, Role, ToolCall};
use crate::secret::Secret;

/// The environment variable the key is read from.
pub const KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";
/// The environment variable naming a key file, outside the repository.
pub const KEY_FILE_VARIABLE: &str = "V2XW_ANTHROPIC_KEY_FILE";
/// The environment variable that overrides [`DEFAULT_MODEL`].
pub const MODEL_VARIABLE: &str = "V2XW_AGENT_MODEL";
/// The environment variable that overrides [`DEFAULT_EFFORT`].
pub const EFFORT_VARIABLE: &str = "V2XW_AGENT_EFFORT";
/// The environment variable that turns the refusal fallbacks off (`off`).
pub const FALLBACKS_VARIABLE: &str = "V2XW_AGENT_FALLBACKS";
/// The SDKs' own variable for another endpoint (a gateway); `/v1/messages` is appended.
pub const URL_VARIABLE: &str = "ANTHROPIC_BASE_URL";
/// The default endpoint.
pub const DEFAULT_URL: &str = "https://api.anthropic.com/v1/messages";
/// The default model.
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// The default effort. See the module notes for why it is not higher.
pub const DEFAULT_EFFORT: &str = "medium";
/// The API version header's value.
pub const API_VERSION: &str = "2023-06-01";
/// The beta that enables `fallbacks: "default"`.
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// `max_tokens` for one non-streaming reply.
pub const MAX_TOKENS: u32 = 16_000;
/// The efforts the API accepts.
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// What a person reads when no key is configured: the one place the instructions live, so
/// the page, the CLI and the error all say the same thing.
pub const HOW_TO_ADD_A_KEY: &str = "No Anthropic API key is configured, so the agent cannot \
     plan or write. Everything that needs no model still works: running a scenario and the \
     deterministic run analysis. To add a key, set ANTHROPIC_API_KEY in the environment of \
     the process serving the simulator (`ANTHROPIC_API_KEY=... v2xw serve ...`), or put the \
     key alone in ~/.config/v2xw/anthropic.key (or the file V2XW_ANTHROPIC_KEY_FILE names), \
     outside the repository, and restart it. The key stays in that process: it is never \
     sent to the browser, written to a log or recorded.";

/// Where the key came from, for a status line that names the source and never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    /// The environment variable.
    Environment,
    /// A key file, at this path.
    File(PathBuf),
}

impl core::fmt::Display for KeySource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KeySource::Environment => write!(f, "the {KEY_VARIABLE} environment variable"),
            KeySource::File(p) => write!(f, "the key file {}", p.display()),
        }
    }
}

/// The key file this process would read, if any is configured or present by default.
#[must_use]
pub fn key_file_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var(KEY_FILE_VARIABLE) {
        let p = p.trim();
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        let x = x.trim();
        if !x.is_empty() {
            return Some(Path::new(x).join("v2xw").join("anthropic.key"));
        }
    }
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .map(|h| Path::new(&h).join(".config").join("v2xw").join("anthropic.key"))
}

/// Finds the key: the environment first, then the key file.
///
/// A key file inside the working directory is refused, because the working directory of
/// the server is the repository and a key there is one `git add` from a commit.
///
/// # Errors
/// [`CopilotError::MissingApiKey`] when neither holds a key, and
/// [`CopilotError::Provider`] for a key file inside the working directory.
pub fn resolve_key() -> Result<(Secret, KeySource)> {
    if let Ok(secret) = Secret::from_env(KEY_VARIABLE) {
        return Ok((secret, KeySource::Environment));
    }
    if let Some(path) = key_file_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let key = text.trim();
            if !key.is_empty() {
                if inside_working_directory(&path) {
                    return Err(CopilotError::Provider(format!(
                        "the key file {} is inside the working directory (the repository); \
                         move it outside, e.g. to ~/.config/v2xw/anthropic.key",
                        path.display()
                    )));
                }
                return Ok((Secret::new(key), KeySource::File(path)));
            }
        }
    }
    Err(CopilotError::MissingApiKey {
        variable: KEY_VARIABLE.to_string(),
    })
}

fn inside_working_directory(path: &Path) -> bool {
    let (Ok(file), Ok(cwd)) = (
        std::fs::canonicalize(path),
        std::env::current_dir().and_then(std::fs::canonicalize),
    ) else {
        return false;
    };
    file.starts_with(cwd)
}

/// Claude over the Messages API.
#[derive(Debug)]
pub struct Claude<H: HttpPost> {
    http: H,
    key: Secret,
    model: String,
    url: String,
    effort: String,
    fallbacks: bool,
    name: String,
    retries: u32,
    pause_s: u64,
}

impl Claude<CurlPost> {
    /// A provider configured from the environment, over `curl`, with a timeout long
    /// enough for a reply that thinks.
    ///
    /// # Errors
    /// As [`resolve_key`].
    pub fn from_env_curl() -> Result<Self> {
        Claude::from_env(CurlPost::new().with_timeout_s(300))
    }
}

impl<H: HttpPost> Claude<H> {
    /// A provider with an explicit key and the defaults.
    #[must_use]
    pub fn new(http: H, key: Secret) -> Self {
        let model = DEFAULT_MODEL.to_string();
        Claude {
            http,
            key,
            name: format!("anthropic:{model}"),
            fallbacks: supports_default_fallbacks(&model),
            model,
            url: DEFAULT_URL.to_string(),
            effort: DEFAULT_EFFORT.to_string(),
            retries: 2,
            pause_s: 3,
        }
    }

    /// A provider configured from the environment (see the module notes).
    ///
    /// # Errors
    /// As [`resolve_key`].
    pub fn from_env(http: H) -> Result<Self> {
        let (key, _) = resolve_key()?;
        let mut provider = Claude::new(http, key);
        if let Some(model) = env_value(MODEL_VARIABLE) {
            provider = provider.with_model(model);
        }
        if let Some(effort) = env_value(EFFORT_VARIABLE) {
            if EFFORTS.contains(&effort.as_str()) {
                provider.effort = effort;
            }
        }
        if let Some(base) = env_value(URL_VARIABLE) {
            provider.url = format!("{}/v1/messages", base.trim_end_matches('/'));
        }
        if env_value(FALLBACKS_VARIABLE).is_some_and(|v| v.eq_ignore_ascii_case("off")) {
            provider.fallbacks = false;
        }
        Ok(provider)
    }

    /// The same, with another model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self.name = format!("anthropic:{}", self.model);
        self.fallbacks = self.fallbacks && supports_default_fallbacks(&self.model);
        self
    }

    /// The same, with another effort level.
    #[must_use]
    pub fn with_effort(mut self, effort: impl Into<String>) -> Self {
        self.effort = effort.into();
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

    /// The HTTP double, for a test that inspects what was sent.
    #[must_use]
    pub fn http(&self) -> &H {
        &self.http
    }

    fn request(&self, body: String) -> HttpRequest {
        let mut request = HttpRequest::json_unauthenticated(self.url.clone(), body);
        request
            .secret_headers
            .push(("x-api-key".to_string(), self.key.clone()));
        request
            .headers
            .push(("anthropic-version".to_string(), API_VERSION.to_string()));
        if self.fallbacks {
            request
                .headers
                .push(("anthropic-beta".to_string(), FALLBACK_BETA.to_string()));
        }
        request
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Whether a model takes `fallbacks: "default"` on the Claude API.
#[must_use]
pub fn supports_default_fallbacks(model: &str) -> bool {
    ["claude-opus-5-5", "claude-opus-5", "claude-fable-5-1", "claude-sonnet-5-5"].contains(&model)
}

impl<H: HttpPost> LlmProvider for Claude<H> {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&mut self, request: &ChatRequest) -> Result<Completion> {
        let body = encode_request(&self.model, &self.effort, self.fallbacks, request)?;
        let mut attempt = 0;
        loop {
            let http_request = self.request(body.clone());
            let response = self.http.post_json(&http_request)?;
            let text = self.key.redact(&response.body);
            if response.is_success() {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|e| CopilotError::Decode(format!("the reply is not JSON: {e}")))?;
                return decode_completion(&value);
            }
            // 429 (rate limit), 529 (overloaded) and 5xx are worth a second try; anything
            // else is the request's fault and would fail the same way again.
            let retryable = response.status == 429 || response.status >= 500;
            if retryable && attempt < self.retries {
                attempt += 1;
                // A pause between attempts is not a reading of the clock: nothing is
                // derived from it.
                std::thread::sleep(std::time::Duration::from_secs(u64::from(attempt) * self.pause_s));
                continue;
            }
            return Err(CopilotError::Provider(format!(
                "HTTP {}: {}",
                response.status,
                api_error_message(&text)
            )));
        }
    }
}

/// The request body for one completion.
///
/// # Errors
/// [`CopilotError::Json`] if the body will not serialise.
pub fn encode_request(
    model: &str,
    effort: &str,
    fallbacks: bool,
    request: &ChatRequest,
) -> Result<String> {
    let mut system = String::new();
    let mut messages: Vec<Value> = Vec::new();
    let mut pending_results: Vec<Value> = Vec::new();
    let flush = |messages: &mut Vec<Value>, pending: &mut Vec<Value>| {
        if !pending.is_empty() {
            messages.push(json!({"role": "user", "content": std::mem::take(pending)}));
        }
    };
    for m in &request.messages {
        match m.role {
            Role::System => {
                if !system.is_empty() {
                    system.push_str("\n\n");
                }
                system.push_str(m.text());
            }
            Role::Tool => pending_results.push(tool_result(m)),
            Role::User => {
                flush(&mut messages, &mut pending_results);
                messages.push(json!({"role": "user", "content": m.text()}));
            }
            Role::Assistant => {
                flush(&mut messages, &mut pending_results);
                messages.push(json!({"role": "assistant", "content": assistant_content(m)}));
            }
        }
    }
    flush(&mut messages, &mut pending_results);

    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("max_tokens".into(), json!(MAX_TOKENS));
    if !system.is_empty() {
        body.insert("system".into(), json!([{"type": "text", "text": system}]));
    }
    body.insert("messages".into(), Value::Array(messages));
    if !request.tools.is_empty() {
        body.insert(
            "tools".into(),
            Value::Array(request.tools.iter().map(claude_tool).collect()),
        );
    }
    body.insert("thinking".into(), json!({"type": "adaptive"}));
    body.insert("output_config".into(), json!({"effort": effort}));
    body.insert("cache_control".into(), json!({"type": "ephemeral"}));
    if fallbacks {
        body.insert("fallbacks".into(), json!("default"));
    }
    Ok(serde_json::to_string(&Value::Object(body))?)
}

/// A tool definition in the Messages API's shape, from the chat-completions shape
/// [`crate::tools::ToolSurface::to_chat_tools`] builds (or one already in this shape).
fn claude_tool(tool: &Value) -> Value {
    match tool.get("function") {
        Some(f) => json!({
            "name": f.get("name").cloned().unwrap_or(Value::Null),
            "description": f.get("description").cloned().unwrap_or_else(|| json!("")),
            "input_schema": f.get("parameters").cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
        }),
        None => tool.clone(),
    }
}

/// An assistant message's content: the provider's own blocks when it has them, else text
/// and `tool_use` blocks rebuilt from the message (a scripted or foreign history).
fn assistant_content(m: &ChatMessage) -> Value {
    if let Some(blocks) = &m.provider_blocks {
        return blocks.clone();
    }
    let mut blocks = Vec::new();
    if !m.text().is_empty() {
        blocks.push(json!({"type": "text", "text": m.text()}));
    }
    for c in &m.tool_calls {
        let input = serde_json::from_str::<Value>(&c.arguments)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        blocks.push(json!({"type": "tool_use", "id": c.id, "name": c.name, "input": input}));
    }
    Value::Array(blocks)
}

/// One `tool_result` block. A result whose status is not `ok` is an error result.
fn tool_result(m: &ChatMessage) -> Value {
    let is_error = serde_json::from_str::<Value>(m.text())
        .ok()
        .and_then(|v| v.get("status").and_then(Value::as_str).map(str::to_string))
        .is_some_and(|s| s != "ok");
    let mut block = json!({
        "type": "tool_result",
        "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
        "content": m.text(),
    });
    if is_error {
        block["is_error"] = json!(true);
    }
    block
}

/// The assistant message out of a reply body.
///
/// # Errors
/// [`CopilotError::Provider`] for an error body, a refusal (after any fallbacks) or a
/// reply cut off at `max_tokens`; [`CopilotError::Decode`] for a body with no content.
pub fn decode_completion(value: &Value) -> Result<Completion> {
    if value.get("type").and_then(Value::as_str) == Some("error") || value.get("error").is_some()
    {
        return Err(CopilotError::Provider(api_error_message(&value.to_string())));
    }
    let stop = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or("end_turn");
    if stop == "refusal" {
        let category = value
            .pointer("/stop_details/category")
            .and_then(Value::as_str)
            .unwrap_or("unspecified");
        return Err(CopilotError::Provider(format!(
            "the model declined this request (category: {category}), and so did its \
             fallbacks. Rephrase the request; the run and its analysis are unaffected."
        )));
    }
    if stop == "max_tokens" {
        return Err(CopilotError::Provider(format!(
            "the reply reached the {MAX_TOKENS}-token limit before it finished; ask for less \
             at once"
        )));
    }
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| CopilotError::Decode("the reply has no content".to_string()))?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for b in blocks {
        match b.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = b.get("text").and_then(Value::as_str) {
                    if !text.is_empty() && !t.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
            }
            Some("tool_use") => {
                let id = b.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let name = b
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CopilotError::Decode("a tool_use block has no name".into()))?
                    .to_string();
                let input = b.get("input").cloned().unwrap_or_else(|| json!({}));
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments: input.to_string(),
                });
            }
            // thinking, redacted_thinking, fallback: kept verbatim in `provider_blocks`.
            _ => {}
        }
    }
    let finish_reason = match stop {
        "tool_use" => "tool_calls",
        "end_turn" | "stop_sequence" => "stop",
        other => other,
    }
    .to_string();
    Ok(Completion {
        message: ChatMessage {
            role: Role::Assistant,
            content: (!text.is_empty()).then_some(text),
            tool_calls,
            tool_call_id: None,
            provider_blocks: Some(Value::Array(blocks.clone())),
        },
        finish_reason,
    })
}

/// The `error.message` of an API error body, or the body itself when it is not one.
fn api_error_message(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| text.trim().to_string())
}

/// The provider the harness uses, chosen from the server's environment: Claude unless
/// `V2XW_AGENT_PROVIDER=openai` asks for the OpenAI provider. `Err` carries the sentence a
/// person reads when there is no key ([`HOW_TO_ADD_A_KEY`], or why a key was refused).
///
/// # Errors
/// As above; never a key.
pub fn provider_from_env() -> core::result::Result<Box<dyn LlmProvider + Send>, String> {
    let wanted = env_value("V2XW_AGENT_PROVIDER").unwrap_or_else(|| "anthropic".to_string());
    if wanted.eq_ignore_ascii_case("openai") {
        return crate::openai::OpenAi::from_env(CurlPost::new())
            .map(|p| Box::new(p) as Box<dyn LlmProvider + Send>)
            .map_err(|e| e.to_string());
    }
    match Claude::from_env_curl() {
        Ok(p) => Ok(Box::new(p)),
        Err(CopilotError::MissingApiKey { .. }) => Err(HOW_TO_ADD_A_KEY.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Where the key would come from, named and never shown.
#[must_use]
pub fn key_source() -> Option<String> {
    resolve_key().ok().map(|(_, source)| source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpResponse, ScriptedPost};

    fn tool_reply() -> &'static str {
        r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-opus-5-5",
        "content":[{"type":"thinking","thinking":"","signature":"sig"},
                   {"type":"text","text":"Let me look."},
                   {"type":"tool_use","id":"toolu_1","name":"run_scenario","input":{"duration_s":30}}],
        "stop_reason":"tool_use","usage":{"input_tokens":10,"output_tokens":5}}"#
    }

    #[test]
    fn a_tool_use_reply_decodes_and_keeps_its_blocks_verbatim() {
        let v: Value = serde_json::from_str(tool_reply()).expect("fixture");
        let c = decode_completion(&v).expect("decodes");
        assert_eq!(c.finish_reason, "tool_calls");
        assert_eq!(c.message.tool_calls.len(), 1);
        assert_eq!(c.message.tool_calls[0].name, "run_scenario");
        assert_eq!(c.message.tool_calls[0].id, "toolu_1");
        assert_eq!(c.message.content.as_deref(), Some("Let me look."));
        let blocks = c.message.provider_blocks.expect("blocks kept");
        assert_eq!(blocks[0]["type"], "thinking");
        assert_eq!(blocks[0]["signature"], "sig");
    }

    #[test]
    fn a_refusal_is_an_error_not_an_empty_answer() {
        let v = json!({"type":"message","content":[],"stop_reason":"refusal",
                       "stop_details":{"type":"refusal","category":"cyber"}});
        let err = decode_completion(&v).expect_err("a refusal is not a completion");
        assert!(err.to_string().contains("cyber"));
    }

    #[test]
    fn the_request_follows_the_messages_api() {
        let assistant = decode_completion(&serde_json::from_str(tool_reply()).expect("fixture"))
            .expect("decodes")
            .message;
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("rules"),
                ChatMessage::user("run it"),
                assistant,
                ChatMessage::tool("toolu_1", r#"{"status":"ok","result":{}}"#),
                ChatMessage::tool("toolu_2", r#"{"status":"failed","message":"no"}"#),
            ],
            tools: vec![json!({"type":"function","function":{"name":"run_scenario",
                "description":"d","parameters":{"type":"object","properties":{}}}})],
        };
        let body = encode_request("claude-opus-5-5", "medium", true, &request).expect("encodes");
        let v: Value = serde_json::from_str(&body).expect("json");
        assert_eq!(v["model"], "claude-opus-5-5");
        assert_eq!(v["max_tokens"], 16_000);
        assert_eq!(v["system"][0]["text"], "rules");
        assert_eq!(v["thinking"]["type"], "adaptive");
        assert_eq!(v["output_config"]["effort"], "medium");
        assert_eq!(v["cache_control"]["type"], "ephemeral");
        assert_eq!(v["fallbacks"], "default");
        assert!(v.get("tool_choice").is_none(), "forced tool choice is rejected by the model");
        assert!(v.get("temperature").is_none(), "sampling parameters are rejected");
        assert_eq!(v["tools"][0]["name"], "run_scenario");
        assert!(v["tools"][0]["input_schema"].is_object());
        let msgs = v["messages"].as_array().expect("messages");
        assert_eq!(msgs.len(), 3, "user, assistant, one user message of tool results");
        // The thinking block is replayed byte for byte.
        assert_eq!(msgs[1]["content"][0]["signature"], "sig");
        let results = msgs[2]["content"].as_array().expect("results");
        assert_eq!(results.len(), 2, "both results in a single message");
        assert_eq!(results[0]["tool_use_id"], "toolu_1");
        assert!(results[0].get("is_error").is_none());
        assert_eq!(results[1]["is_error"], true);
    }

    #[test]
    fn the_key_goes_only_in_its_header_and_is_redacted_from_errors() {
        let post = ScriptedPost::new(vec![HttpResponse::new(
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key sk-ant-secret"}}"#,
        )]);
        let mut p = Claude::new(post, Secret::new("sk-ant-secret")).without_retries();
        let err = p
            .complete(&ChatRequest {
                messages: vec![ChatMessage::user("hi")],
                tools: Vec::new(),
            })
            .expect_err("401");
        let text = err.to_string();
        assert!(text.contains("HTTP 401"));
        assert!(!text.contains("sk-ant-secret"), "the key was echoed: {text}");
        let sent = &p.http().seen[0];
        assert!(!sent.body.contains("sk-ant-secret"));
        assert!(sent.bearer.is_none(), "Anthropic takes x-api-key, not a bearer token");
        assert_eq!(sent.secret_headers[0].0, "x-api-key");
        assert_eq!(sent.secret_headers[0].1.expose(), "sk-ant-secret");
        assert!(sent.headers.iter().any(|(k, v)| k == "anthropic-version" && v == API_VERSION));
        assert!(sent.headers.iter().any(|(k, v)| k == "anthropic-beta" && v == FALLBACK_BETA));
        assert_eq!(sent.url, DEFAULT_URL);
    }

    #[test]
    fn an_overloaded_endpoint_is_retried() {
        let post = ScriptedPost::new(vec![
            HttpResponse::new(529, r#"{"type":"error","error":{"message":"overloaded"}}"#),
            HttpResponse::new(200, tool_reply()),
        ]);
        let mut p = Claude::new(post, Secret::new("k"));
        p.retries = 1;
        p.pause_s = 0;
        let c = p.complete(&ChatRequest::default()).expect("the retry succeeds");
        assert_eq!(c.finish_reason, "tool_calls");
        assert_eq!(p.http().calls(), 2);
    }
}
