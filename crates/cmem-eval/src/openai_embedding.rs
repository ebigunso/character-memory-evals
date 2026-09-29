use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const OPENAI_EMBEDDINGS_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";
const OPENAI_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct OpenAiEmbeddingClient {
    http: reqwest::Client,
}

impl Default for OpenAiEmbeddingClient {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(OPENAI_REQUEST_TIMEOUT)
                .build()
                .expect("the static OpenAI embedding HTTP client configuration is valid"),
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
    ) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        if inputs.iter().any(|input| input.trim().is_empty()) {
            bail!("embedding inputs must not be blank");
        }

        let request = OpenAiEmbeddingRequest {
            model,
            input: inputs,
            dimensions,
        };
        let response = self
            .http
            .post(OPENAI_EMBEDDINGS_ENDPOINT)
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .await
            .context("request OpenAI embeddings")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("OpenAI embedding request failed with {status}: {body}");
        }
        let body = response
            .json::<OpenAiEmbeddingResponse>()
            .await
            .context("parse OpenAI embedding response")?;
        ordered_embeddings(model, inputs.len(), dimensions, body)
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
        bail!(
            "OpenAI embedding response returned {} vectors for {expected_count} inputs; missing indices: {missing_indices:?}",
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
        if let Some(expected) = expected_dimensions
            && item.embedding.len() != expected
        {
            bail!(
                "OpenAI embedding response index {} has vector size {}, expected {expected}",
                item.index,
                item.embedding.len()
            );
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

        assert_eq!(
            error.to_string(),
            "OpenAI embedding response contains duplicate index 0"
        );
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
            error.to_string(),
            "OpenAI embedding response returned 1 vectors for 2 inputs; missing indices: [1]"
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
            error.to_string(),
            "OpenAI embedding response index 2 is outside expected range 0..2"
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
            error.to_string(),
            "OpenAI embedding response model \"different-model\" does not match requested model \"requested-model\""
        );
    }

    #[test]
    fn rejects_response_dimension_mismatches() {
        let error = ordered_embeddings("model", 1, Some(2), response("model", [(0, vec![1.0])]))
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "OpenAI embedding response index 0 has vector size 1, expected 2"
        );
    }
}
