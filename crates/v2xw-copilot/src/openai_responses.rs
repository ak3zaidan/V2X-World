//! OpenAI's Responses API (`POST /v1/responses`): the request and reply shapes, pure.
//!
//! Why a second wire format for one vendor: the reasoning tiers of the 6.x generation do
//! not take function tools on Chat Completions while they reason. `gpt-6.1-sol` answers a
//! tool request there with "Function tools with reasoning_effort are not supported for
//! gpt-6.1-sol in /v1/chat/completions. To use function tools, use /v1/responses or set
//! reasoning_effort to 'none'", and then refuses `reasoning_effort: "none"` ("Supported
//! values are: 'low', 'medium', 'high', and 'xhigh'"). So the agent, which is nothing but
//! function tools, can only drive that model through this endpoint. [`crate::openai`]
//! switches here on that refusal and stays here; `OPENAI_API=responses` starts here.
//!
//! # Stateless, and replayed verbatim
//!
//! Requests are sent with `store: false`, so nothing of a conversation is kept on the
//! vendor's side, and with `include: ["reasoning.encrypted_content"]`, so a reasoning item
//! comes back in a form that can be handed back on the next request. Every reply's `output`
//! items are kept verbatim in [`ChatMessage::provider_blocks`] and replayed unchanged in
//! order — the reasoning item before the function call it led to — exactly as the Claude
//! provider replays its content blocks. A history built elsewhere (a scripted model, the
//! other provider) is rebuilt from its text and tool calls.

use serde_json::{Map, Value, json};

use crate::error::{CopilotError, Result};
use crate::provider::{ChatMessage, ChatRequest, Completion, Role, ToolCall, Usage};

/// The request body.
///
/// # Errors
/// [`CopilotError::Json`] if the body will not serialise.
pub fn encode_request(
    model: &str,
    request: &ChatRequest,
    temperature: Option<f64>,
    reasoning_effort: Option<&str>,
) -> Result<String> {
    let mut input: Vec<Value> = Vec::new();
    for m in &request.messages {
        match m.role {
            Role::System => input.push(json!({"role": "system", "content": m.text()})),
            Role::User => input.push(json!({"role": "user", "content": m.text()})),
            Role::Tool => input.push(json!({
                "type": "function_call_output",
                "call_id": m.tool_call_id.clone().unwrap_or_default(),
                "output": m.text(),
            })),
            Role::Assistant => match m.provider_blocks.as_ref().and_then(Value::as_array) {
                Some(items) if items.iter().all(is_replayable) => {
                    input.extend(items.iter().cloned())
                }
                _ => {
                    if !m.text().is_empty() {
                        input.push(json!({"role": "assistant", "content": m.text()}));
                    }
                    for c in &m.tool_calls {
                        input.push(json!({
                            "type": "function_call",
                            "call_id": c.id,
                            "name": c.name,
                            "arguments": c.arguments,
                        }));
                    }
                }
            },
        }
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("input".into(), Value::Array(input));
    if !request.tools.is_empty() {
        body.insert(
            "tools".into(),
            Value::Array(request.tools.iter().map(responses_tool).collect()),
        );
        body.insert("tool_choice".into(), json!("auto"));
    }
    body.insert("store".into(), json!(false));
    body.insert("include".into(), json!(["reasoning.encrypted_content"]));
    if let Some(effort) = reasoning_effort {
        body.insert("reasoning".into(), json!({"effort": effort}));
    }
    if let Some(t) = temperature {
        body.insert("temperature".into(), json!(t));
    }
    Ok(serde_json::to_string(&Value::Object(body))?)
}

/// An output item this module knows how to hand back: a reply's own items, never a Claude
/// content block that happens to sit in the same field.
fn is_replayable(item: &Value) -> bool {
    matches!(
        item.get("type").and_then(Value::as_str),
        Some("reasoning" | "message" | "function_call")
    )
}

/// A tool in this API's flat shape, from the chat-completions shape the agent builds.
fn responses_tool(tool: &Value) -> Value {
    match tool.get("function") {
        Some(f) => {
            let mut t = Map::new();
            t.insert("type".into(), json!("function"));
            for k in ["name", "description", "parameters"] {
                if let Some(v) = f.get(k) {
                    t.insert(k.into(), v.clone());
                }
            }
            Value::Object(t)
        }
        None => tool.clone(),
    }
}

/// The assistant message out of a reply.
///
/// # Errors
/// [`CopilotError::Provider`] for an error body, a refusal or a reply cut off by its token
/// limit; [`CopilotError::Decode`] for a reply with no `output`.
pub fn decode_completion(value: &Value) -> Result<Completion> {
    if let Some(e) = value.get("error").filter(|e| !e.is_null()) {
        return Err(CopilotError::Provider(
            e.get("message")
                .and_then(Value::as_str)
                .unwrap_or("the API returned an error with no message")
                .to_string(),
        ));
    }
    let items = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| CopilotError::Decode("the reply has no output".to_string()))?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            if let Some(t) = part.get("text").and_then(Value::as_str) {
                                if !text.is_empty() && !t.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(t);
                            }
                        }
                        Some("refusal") => {
                            return Err(CopilotError::Provider(format!(
                                "the model declined: {}",
                                part.get("refusal")
                                    .and_then(Value::as_str)
                                    .unwrap_or("no reason given")
                            )));
                        }
                        _ => {}
                    }
                }
            }
            Some("function_call") => {
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CopilotError::Decode("a function call has no name".into()))?;
                let id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                tool_calls.push(ToolCall {
                    id,
                    name: name.to_string(),
                    arguments: item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}")
                        .to_string(),
                });
            }
            // `reasoning`: kept verbatim in `provider_blocks` for the replay.
            _ => {}
        }
    }
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    if status == "incomplete" && tool_calls.is_empty() {
        let why = value
            .pointer("/incomplete_details/reason")
            .and_then(Value::as_str)
            .unwrap_or("unspecified");
        if text.is_empty() {
            return Err(CopilotError::Provider(format!(
                "the reply stopped before it finished ({why}); ask for less at once"
            )));
        }
    }
    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls"
    } else if status == "incomplete" {
        "length"
    } else {
        "stop"
    }
    .to_string();
    Ok(Completion {
        message: ChatMessage {
            role: Role::Assistant,
            content: (!text.is_empty()).then_some(text),
            tool_calls,
            tool_call_id: None,
            provider_blocks: Some(Value::Array(items.clone())),
        },
        finish_reason,
    })
}

/// The `usage` block: `input_tokens` (cached included, and counted again in
/// `input_tokens_details.cached_tokens`) and `output_tokens` (reasoning included).
#[must_use]
pub fn decode_usage(value: &Value) -> Option<Usage> {
    let u = value.get("usage")?;
    let n = |p: &str| u.pointer(p).and_then(Value::as_u64).unwrap_or(0);
    Some(Usage {
        input_tokens: n("/input_tokens"),
        cached_input_tokens: n("/input_tokens_details/cached_tokens"),
        output_tokens: n("/output_tokens"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply() -> Value {
        serde_json::from_str(
            r#"{"id":"resp_1","status":"completed","output":[
              {"type":"reasoning","id":"rs_1","summary":[],"encrypted_content":"gAAA"},
              {"type":"message","id":"msg_1","role":"assistant","content":[{"type":"output_text","text":"I will run it."}]},
              {"type":"function_call","id":"fc_1","call_id":"call_1","name":"simulate","arguments":"{\"base\":\"x\"}"}],
              "usage":{"input_tokens":900,"input_tokens_details":{"cached_tokens":512},"output_tokens":40}}"#,
        )
        .expect("fixture")
    }

    #[test]
    fn a_reply_decodes_into_text_calls_usage_and_verbatim_items() {
        let c = decode_completion(&reply()).expect("decodes");
        assert_eq!(c.finish_reason, "tool_calls");
        assert_eq!(c.message.text(), "I will run it.");
        assert_eq!(c.message.tool_calls[0].id, "call_1");
        assert_eq!(c.message.tool_calls[0].name, "simulate");
        assert_eq!(
            c.message
                .provider_blocks
                .as_ref()
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(
            decode_usage(&reply()),
            Some(Usage {
                input_tokens: 900,
                cached_input_tokens: 512,
                output_tokens: 40
            })
        );
    }

    #[test]
    fn the_history_replays_the_items_in_order_and_answers_each_call() {
        let c = decode_completion(&reply()).expect("decodes");
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("rules"),
                ChatMessage::user("go"),
                c.message,
                ChatMessage::tool("call_1", "{\"status\":\"ok\"}"),
            ],
            tools: vec![json!({"type": "function", "function": {"name": "simulate",
                "description": "d", "parameters": {"type": "object"}}})],
        };
        let body: Value = serde_json::from_str(
            &encode_request("gpt-6.1-sol", &request, None, Some("low")).expect("encodes"),
        )
        .expect("json");
        let input = body["input"].as_array().expect("input");
        assert_eq!(input[0]["role"], "system");
        assert_eq!(input[2]["type"], "reasoning");
        assert_eq!(input[2]["encrypted_content"], "gAAA");
        assert_eq!(input[4]["type"], "function_call");
        assert_eq!(input[5]["type"], "function_call_output");
        assert_eq!(input[5]["call_id"], "call_1");
        assert_eq!(body["tools"][0]["name"], "simulate");
        assert!(
            body["tools"][0].get("function").is_none(),
            "tools are flat here"
        );
        assert_eq!(body["store"], false);
        assert_eq!(body["reasoning"]["effort"], "low");
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn a_foreign_history_is_rebuilt_from_text_and_calls() {
        let mut m = ChatMessage::calls(
            Some("plan".into()),
            vec![ToolCall {
                id: "c9".into(),
                name: "t".into(),
                arguments: "{}".into(),
            }],
        );
        // A Claude content block is not an output item of this API.
        m.provider_blocks = Some(json!([{"type": "thinking", "thinking": ""}]));
        let body: Value = serde_json::from_str(
            &encode_request(
                "m",
                &ChatRequest {
                    messages: vec![m],
                    tools: vec![],
                },
                Some(0.0),
                None,
            )
            .expect("encodes"),
        )
        .expect("json");
        assert_eq!(body["input"][0]["role"], "assistant");
        assert_eq!(body["input"][1]["type"], "function_call");
        assert_eq!(body["input"][1]["call_id"], "c9");
        assert_eq!(body["temperature"], 0.0);
    }

    #[test]
    fn an_incomplete_reply_with_nothing_in_it_is_an_error() {
        let v = json!({"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"},
                       "output": [{"type": "reasoning", "id": "rs", "summary": []}]});
        assert!(decode_completion(&v).is_err());
    }
}
