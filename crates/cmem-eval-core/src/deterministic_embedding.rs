use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub struct DeterministicEmbeddingProvider {
    vector_size: usize,
}

impl DeterministicEmbeddingProvider {
    pub fn new(vector_size: usize) -> Result<Self> {
        if vector_size == 0 {
            bail!("deterministic embedding vector_size must be greater than zero");
        }
        Ok(Self { vector_size })
    }

    pub fn vector_size(&self) -> usize {
        self.vector_size
    }

    pub fn vector_for_text(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; self.vector_size];
        for token in text.split(|ch: char| !ch.is_alphanumeric()) {
            if token.is_empty() {
                continue;
            }
            let idx = stable_hash(token) % self.vector_size;
            embedding[idx] += 1.0;
        }
        if embedding.iter().all(|value| *value == 0.0) {
            embedding[0] = 1.0;
        }
        embedding
    }
}

fn stable_hash(text: &str) -> usize {
    text.bytes().fold(2166136261usize, |hash, byte| {
        hash.wrapping_mul(16777619) ^ usize::from(byte.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_embeddings_are_stable_and_sized() {
        let provider = DeterministicEmbeddingProvider::new(8).unwrap();
        let first = provider.vector_for_text("Alice likes tea");
        let second = provider.vector_for_text("Alice likes tea");

        assert_eq!(first, second);
        assert_eq!(first.len(), 8);
        assert!(first.iter().any(|value| *value > 0.0));
    }

    #[test]
    fn rejects_zero_vector_size_at_construction() {
        assert!(
            DeterministicEmbeddingProvider::new(0)
                .unwrap_err()
                .to_string()
                .contains("greater than zero")
        );
    }
}
