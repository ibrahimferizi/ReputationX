use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone)]
pub struct SolanaRpc {
    client: Client,
    url: String,
    max_retries: u32,
    retry_base_ms: u64,
}

impl SolanaRpc {
    pub fn new(url: impl Into<String>, max_retries: u32, retry_base_ms: u64) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .expect("http client"),
            url: url.into(),
            max_retries,
            retry_base_ms,
        }
    }

    /// Call JSON-RPC with retries on rate limits (HTTP 429 / RPC error -32429 / message contains 429).
    pub async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let mut attempt = 0u32;
        loop {
            match self.call_once::<T>(method, params.clone()).await {
                Ok(value) => return Ok(value),
                Err(err) if is_rate_limited(&err) && attempt < self.max_retries => {
                    let wait_ms = self.retry_base_ms.saturating_mul(2u64.saturating_pow(attempt));
                    tracing::warn!(
                        method,
                        attempt = attempt + 1,
                        wait_ms,
                        "rpc rate limited, retrying"
                    );
                    sleep(Duration::from_millis(wait_ms)).await;
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn call_once<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let payload = jsonrpc_request(method, params);
        let response = self
            .client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("rpc request failed for method {method}"))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .context("failed to read rpc response body")?;

        if status.as_u16() == 429 {
            return Err(anyhow!(
                "rpc http 429 for {method}: {}",
                truncate(&body_text, 400)
            ));
        }

        if !status.is_success() {
            return Err(anyhow!(
                "rpc http {} for {method}: {}",
                status,
                truncate(&body_text, 400)
            ));
        }

        let envelope: RpcResponse = serde_json::from_str(&body_text).map_err(|err| {
            anyhow!(
                "invalid rpc json for {method}: {err}; body={}",
                truncate(&body_text, 400)
            )
        })?;

        if let Some(error) = envelope.error {
            if error.code == 429 || error.message.to_ascii_lowercase().contains("too many") {
                return Err(anyhow!(
                    "rpc error {} for {method}: {}",
                    error.code,
                    error.message
                ));
            }
            return Err(anyhow!(
                "rpc error {} for {method}: {}",
                error.code,
                error.message
            ));
        }

        let result = envelope
            .result
            .ok_or_else(|| anyhow!("rpc response missing result for {method}"))?;

        deserialize_result(result).with_context(|| format!("failed to decode result for {method}"))
    }
}

fn is_rate_limited(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("429") || msg.contains("too many requests") || msg.contains("rate limit")
}

/// `{ "context": { "slot": N }, "value": T }` — standard Solana RPC shape.
#[derive(Debug, Deserialize)]
struct RpcContextValue<T> {
    value: T,
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

fn jsonrpc_request(method: &str, params: Value) -> RpcRequest<'_> {
    RpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method,
        params,
    }
}

fn deserialize_result<T: DeserializeOwned>(result: Value) -> Result<T> {
    let debug_body = truncate(&result.to_string(), 400);

    if let Ok(value) = serde_json::from_value::<T>(result.clone()) {
        return Ok(value);
    }

    if let Ok(wrapped) = serde_json::from_value::<RpcContextValue<T>>(result) {
        return Ok(wrapped.value);
    }

    Err(anyhow!("unexpected result shape: {debug_body}"))
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        format!("{}…", &text[..max])
    }
}
