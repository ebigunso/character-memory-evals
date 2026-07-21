use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const OPENAI_EMBEDDINGS_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingRetryPolicy {
    pub max_attempts: usize,
    pub initial_backoff: Duration,
}

impl EmbeddingRetryPolicy {
    pub const fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            initial_backoff: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiEmbeddingClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for OpenAiEmbeddingClient {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            endpoint: OPENAI_EMBEDDINGS_ENDPOINT.to_string(),
        }
    }
}

impl OpenAiEmbeddingClient {
    pub async fn embed_batch(
        &self,
        api_key: &str,
        model: &str,
        inputs: &[String],
        dimensions: Option<usize>,
        retry: EmbeddingRetryPolicy,
    ) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        if retry.max_attempts == 0 {
            bail!("embedding retry policy max_attempts must be greater than zero");
        }
        if inputs.iter().any(|input| input.trim().is_empty()) {
            bail!("embedding inputs must not be blank");
        }

        let request = OpenAiEmbeddingRequest {
            model,
            input: inputs,
            dimensions,
        };
        for attempt in 1..=retry.max_attempts {
            let response = self
                .http
                .post(&self.endpoint)
                .bearer_auth(api_key)
                .json(&request)
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let body = response
                            .json::<OpenAiEmbeddingResponse>()
                            .await
                            .context("parse OpenAI embedding response")?;
                        return ordered_embeddings(model, inputs.len(), dimensions, body);
                    }
                    let retryable = status.as_u16() == 429 || status.is_server_error();
                    let body = response.text().await.unwrap_or_default();
                    if !retryable || attempt == retry.max_attempts {
                        bail!("OpenAI embedding request failed with {status}: {body}");
                    }
                }
                Err(error) => {
                    if attempt == retry.max_attempts || !(error.is_timeout() || error.is_connect())
                    {
                        return Err(error).context("request OpenAI embeddings");
                    }
                }
            }
            tokio::time::sleep(retry.initial_backoff.saturating_mul(attempt as u32)).await;
        }
        unreachable!("embedding retry loop always returns")
    }
}

#[derive(Debug, Serialize)]
struct OpenAiEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    model: String,
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

fn ordered_embeddings(
    requested_model: &str,
    expected_count: usize,
    expected_dimensions: Option<usize>,
    response: OpenAiEmbeddingResponse,
) -> Result<Vec<Vec<f32>>> {
    if response.model != requested_model {
        bail!(
            "OpenAI embedding response model {:?} does not match requested model {requested_model:?}",
            response.model
        );
    }
    if response.data.len() != expected_count {
        bail!(
            "OpenAI embedding response returned {} vectors for {expected_count} inputs",
            response.data.len()
        );
    }
    let mut ordered = vec![None; expected_count];
    for item in response.data {
        if item.index >= expected_count {
            bail!(
                "OpenAI embedding response index {} is outside expected range 0..{expected_count}",
                item.index
            );
        }
        if ordered[item.index].is_some() {
            bail!(
                "OpenAI embedding response contains duplicate index {}",
                item.index
            );
        }
        if let Some(expected_dimensions) = expected_dimensions
            && item.embedding.len() != expected_dimensions
        {
            bail!(
                "OpenAI embedding response index {} has vector size {}, expected {expected_dimensions}",
                item.index,
                item.embedding.len()
            );
        }
        ordered[item.index] = Some(item.embedding);
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| {
            embedding.with_context(|| format!("OpenAI embedding response omitted index {index}"))
        })
        .collect()
}
