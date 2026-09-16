use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

const OPENAI_EMBEDDINGS_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";
const OPENAI_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

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
            http: reqwest::Client::builder()
                .timeout(OPENAI_REQUEST_TIMEOUT)
                .build()
                .expect("the static OpenAI embedding HTTP client configuration is valid"),
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
                        return Ok(ordered_embeddings(model, inputs.len(), dimensions, body)?);
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

/// Why an OpenAI embedding response was refused; one variant per validation branch.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ResponseError {
    ModelMismatch {
        requested: String,
        returned: String,
    },
    CardinalityMismatch {
        expected: usize,
        returned: usize,
        missing_indices: Vec<usize>,
    },
    IndexOutOfRange {
        index: usize,
        expected_count: usize,
    },
    DuplicateIndex {
        index: usize,
    },
    DimensionMismatch {
        index: usize,
        expected: usize,
        returned: usize,
    },
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelMismatch {
                requested,
                returned,
            } => write!(
                f,
                "OpenAI embedding response model {returned:?} does not match requested model {requested:?}"
            ),
            Self::CardinalityMismatch {
                expected,
                returned,
                missing_indices,
            } => write!(
                f,
                "OpenAI embedding response returned {returned} vectors for {expected} inputs; missing indices: {missing_indices:?}"
            ),
            Self::IndexOutOfRange {
                index,
                expected_count,
            } => write!(
                f,
                "OpenAI embedding response index {index} is outside expected range 0..{expected_count}"
            ),
            Self::DuplicateIndex { index } => {
                write!(
                    f,
                    "OpenAI embedding response contains duplicate index {index}"
                )
            }
            Self::DimensionMismatch {
                index,
                expected,
                returned,
            } => write!(
                f,
                "OpenAI embedding response index {index} has vector size {returned}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ResponseError {}

fn ordered_embeddings(
    requested_model: &str,
    expected_count: usize,
    expected_dimensions: Option<usize>,
    response: OpenAiEmbeddingResponse,
) -> std::result::Result<Vec<Vec<f32>>, ResponseError> {
    if response.model != requested_model {
        return Err(ResponseError::ModelMismatch {
            requested: requested_model.to_string(),
            returned: response.model,
        });
    }
    if response.data.len() != expected_count {
        let mut present = vec![false; expected_count];
        for item in &response.data {
            if item.index < expected_count {
                present[item.index] = true;
            }
        }
        let missing_indices = present
            .iter()
            .enumerate()
            .filter_map(|(index, present)| (!present).then_some(index))
            .collect::<Vec<_>>();
        return Err(ResponseError::CardinalityMismatch {
            expected: expected_count,
            returned: response.data.len(),
            missing_indices,
        });
    }
    let mut ordered = vec![None; expected_count];
    for item in response.data {
        if item.index >= expected_count {
            return Err(ResponseError::IndexOutOfRange {
                index: item.index,
                expected_count,
            });
        }
        if ordered[item.index].is_some() {
            return Err(ResponseError::DuplicateIndex { index: item.index });
        }
        if let Some(expected) = expected_dimensions
            && item.embedding.len() != expected
        {
            return Err(ResponseError::DimensionMismatch {
                index: item.index,
                expected,
                returned: item.embedding.len(),
            });
        }
        ordered[item.index] = Some(item.embedding);
    }
    Ok(ordered
        .into_iter()
        .map(|embedding| {
            embedding.expect(
                "every slot is filled: the count matches and every index is in range and unique",
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(
        model: &str,
        data: impl IntoIterator<Item = (usize, Vec<f32>)>,
    ) -> OpenAiEmbeddingResponse {
        OpenAiEmbeddingResponse {
            model: model.to_string(),
            data: data
                .into_iter()
                .map(|(index, embedding)| OpenAiEmbeddingData { index, embedding })
                .collect(),
        }
    }

    #[test]
    fn orders_successful_embeddings_by_response_index() {
        let ordered = ordered_embeddings(
            "model",
            2,
            Some(2),
            response("model", [(1, vec![0.0, 1.0]), (0, vec![1.0, 0.0])]),
        )
        .unwrap();

        assert_eq!(ordered, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    }

    #[test]
    fn rejects_duplicate_response_indices() {
        let error = ordered_embeddings(
            "model",
            2,
            Some(2),
            response("model", [(0, vec![1.0, 0.0]), (0, vec![0.0, 1.0])]),
        )
        .unwrap_err();

        assert_eq!(error, ResponseError::DuplicateIndex { index: 0 });
    }

    #[test]
    fn rejects_missing_indices_and_response_cardinality_mismatches() {
        let error = ordered_embeddings(
            "model",
            2,
            Some(2),
            response("model", [(0, vec![1.0, 0.0])]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ResponseError::CardinalityMismatch {
                expected: 2,
                returned: 1,
                missing_indices: vec![1],
            }
        );
    }

    #[test]
    fn rejects_out_of_range_response_indices() {
        let error = ordered_embeddings(
            "model",
            2,
            Some(2),
            response("model", [(0, vec![1.0, 0.0]), (2, vec![0.0, 1.0])]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ResponseError::IndexOutOfRange {
                index: 2,
                expected_count: 2,
            }
        );
    }

    #[test]
    fn rejects_response_model_mismatches() {
        let error = ordered_embeddings(
            "requested-model",
            1,
            Some(2),
            response("different-model", [(0, vec![1.0, 0.0])]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ResponseError::ModelMismatch {
                requested: "requested-model".to_string(),
                returned: "different-model".to_string(),
            }
        );
    }

    #[test]
    fn rejects_response_dimension_mismatches() {
        let error = ordered_embeddings("model", 1, Some(2), response("model", [(0, vec![1.0])]))
            .unwrap_err();

        assert_eq!(
            error,
            ResponseError::DimensionMismatch {
                index: 0,
                expected: 2,
                returned: 1,
            }
        );
    }
}
