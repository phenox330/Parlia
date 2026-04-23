use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
/// Hard cap on output tokens. Emails + short rewrites don't need more, and the
/// cap protects the user from a runaway response if a custom prompt goes wrong.
const MAX_TOKENS: u32 = 2048;
/// Overall request timeout. Anthropic's p99 for Haiku is <5 s for small
/// outputs; a 60 s ceiling is defensive against hanging sockets.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicErrorBody {
    error: AnthropicErrorDetail,
}

#[derive(Deserialize)]
struct AnthropicErrorDetail {
    #[serde(default)]
    #[allow(dead_code)]
    kind: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Call Anthropic's Messages API. CPU-cheap but network-bound — callers on
/// async contexts can just `.await` it; no `spawn_blocking` needed.
pub async fn generate_anthropic(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("Anthropic API key is empty"));
    }

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("Failed to build Anthropic HTTP client")?;

    let body = AnthropicRequest {
        model,
        max_tokens: MAX_TOKENS,
        system: system_prompt,
        messages: vec![AnthropicMessage {
            role: "user",
            content: user_text,
        }],
    };

    let resp = client
        .post(ANTHROPIC_ENDPOINT)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_API_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to reach Anthropic")?;

    let status = resp.status();
    if !status.is_success() {
        let raw = resp
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        // Try to extract the API's own error message for a friendlier surface.
        let friendly = serde_json::from_str::<AnthropicErrorBody>(&raw)
            .ok()
            .and_then(|b| b.error.message)
            .unwrap_or_else(|| raw.clone());
        return Err(anyhow!("Anthropic API error ({}): {}", status, friendly));
    }

    let parsed: AnthropicResponse = resp
        .json()
        .await
        .context("Failed to parse Anthropic response")?;

    // Concatenate all text blocks. Haiku almost always returns a single
    // block, but the API allows multiple and we shouldn't drop any.
    let text: String = parsed
        .content
        .into_iter()
        .filter(|b| b.kind == "text")
        .filter_map(|b| b.text)
        .collect::<Vec<_>>()
        .join("");

    if text.trim().is_empty() {
        return Err(anyhow!("Anthropic returned an empty response"));
    }

    Ok(text)
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<OpenAiMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

/// Call any OpenAI-compatible `/chat/completions` endpoint — Ollama, LM Studio,
/// Groq, OpenRouter, DeepSeek, vLLM, etc. `api_key` is optional: blank skips
/// the `Authorization` header (Ollama/LM Studio don't require it).
///
/// `base_url` should include the version prefix (e.g. `http://localhost:11434/v1`);
/// we append `/chat/completions`. Trailing slashes are tolerated.
pub async fn generate_openai_compatible(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String> {
    let trimmed_base = base_url.trim().trim_end_matches('/');
    if trimmed_base.is_empty() {
        return Err(anyhow!("Base URL is empty"));
    }
    if model.trim().is_empty() {
        return Err(anyhow!("Model name is empty"));
    }
    let url = format!("{}/chat/completions", trimmed_base);

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("Failed to build HTTP client")?;

    let body = OpenAiRequest {
        model,
        max_tokens: MAX_TOKENS,
        messages: vec![
            OpenAiMessage {
                role: "system",
                content: system_prompt,
            },
            OpenAiMessage {
                role: "user",
                content: user_text,
            },
        ],
        stream: false,
    };

    let mut req = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body);
    if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    let resp = req
        .send()
        .await
        .with_context(|| format!("Failed to reach {}", url))?;

    let status = resp.status();
    if !status.is_success() {
        let raw = resp
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(anyhow!("Provider error ({}): {}", status, raw));
    }

    let parsed: OpenAiResponse = resp
        .json()
        .await
        .context("Failed to parse provider response (not OpenAI-compatible JSON?)")?;

    let text = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default();

    if text.trim().is_empty() {
        return Err(anyhow!("Provider returned an empty response"));
    }

    Ok(text)
}
