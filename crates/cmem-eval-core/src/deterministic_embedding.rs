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
            let idx = (stable_hash(token) % self.vector_size as u64) as usize;
            embedding[idx] += 1.0;
        }
        if embedding.iter().all(|value| *value == 0.0) {
            embedding[0] = 1.0;
        }
        embedding
    }
}

fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(2166136261u64, |hash, byte| {
        hash.wrapping_mul(16777619) ^ u64::from(byte.to_ascii_lowercase())
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
    fn deterministic_embedding_buckets_are_fixed_width_and_pinned() {
        assert_eq!(stable_hash("Alice"), 8_410_850_597_083_717_683);
        assert_eq!(stable_hash("likes"), 3_000_596_551_902_403_567);
        assert_eq!(stable_hash("tea"), 18_094_078_683_540_929_565);

        let embedding = DeterministicEmbeddingProvider::new(16)
            .unwrap()
            .vector_for_text("Alice likes tea");
        let occupied = embedding
            .iter()
            .enumerate()
            .filter_map(|(bucket, value)| (*value > 0.0).then_some((bucket, *value)))
            .collect::<Vec<_>>();
        assert_eq!(occupied, vec![(3, 1.0), (13, 1.0), (15, 1.0)]);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn fixed_width_hash_is_byte_identical_to_legacy_x86_64_embeddings() {
        let vector_size = 3072;
        let text = "Alice likes tea across continuity fixtures";
        let fixed_width = DeterministicEmbeddingProvider::new(vector_size)
            .unwrap()
            .vector_for_text(text);
        let legacy = legacy_x86_64_vector(text, vector_size);

        assert_eq!(embedding_bytes(&fixed_width), embedding_bytes(&legacy));
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

    #[cfg(target_pointer_width = "64")]
    fn legacy_x86_64_vector(text: &str, vector_size: usize) -> Vec<f32> {
        let mut embedding = vec![0.0; vector_size];
        for token in text.split(|ch: char| !ch.is_alphanumeric()) {
            if token.is_empty() {
                continue;
            }
            let hash = token.bytes().fold(2166136261usize, |hash, byte| {
                hash.wrapping_mul(16777619) ^ usize::from(byte.to_ascii_lowercase())
            });
            embedding[hash % vector_size] += 1.0;
        }
        embedding
    }

    #[cfg(target_pointer_width = "64")]
    fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect()
    }
}
