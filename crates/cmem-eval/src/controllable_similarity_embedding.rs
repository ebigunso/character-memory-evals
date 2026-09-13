use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllableSimilarityFixture {
    pub seed: u64,
    pub vector_size: usize,
    pub noise_magnitude: f32,
    pub clusters: BTreeMap<String, Vec<f32>>,
    pub concepts: BTreeMap<String, SimilarityConceptFixture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimilarityConceptFixture {
    pub cluster: String,
    pub inputs: Vec<String>,
}

impl ControllableSimilarityFixture {
    pub fn canonical_sha256(&self) -> Result<String> {
        validate_fixture(self)?;
        let bytes = serde_json::to_vec(self)?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }
}

#[derive(Debug, Clone)]
pub struct ControllableSimilarityEmbeddingProvider {
    vector_size: usize,
    concept_vectors: BTreeMap<String, Vec<f32>>,
    input_concepts: BTreeMap<String, String>,
}

impl ControllableSimilarityEmbeddingProvider {
    pub fn new(fixture: ControllableSimilarityFixture) -> Result<Self> {
        validate_fixture(&fixture)?;

        let mut concept_vectors = BTreeMap::new();
        let mut input_concepts = BTreeMap::new();
        for (concept_id, concept) in &fixture.concepts {
            let cluster = &fixture.clusters[&concept.cluster];
            let vector = cluster
                .iter()
                .enumerate()
                .map(|(dimension, base)| {
                    Ok(base
                        + seeded_noise(
                            fixture.seed,
                            concept_id,
                            dimension,
                            fixture.noise_magnitude,
                        )?)
                })
                .collect::<Result<Vec<_>>>()?;
            if vector.iter().any(|component| !component.is_finite()) {
                bail!(
                    "similarity concept {concept_id:?} produces a non-finite vector; reduce the cluster components or noise_magnitude"
                );
            }
            concept_vectors.insert(concept_id.clone(), vector);

            let mut inputs = concept.inputs.iter().collect::<Vec<_>>();
            inputs.sort_unstable();
            for input in inputs {
                if let Some(previous) = input_concepts.insert(input.clone(), concept_id.clone()) {
                    bail!(
                        "similarity input {input:?} is assigned to both concepts {previous:?} and {concept_id:?}"
                    );
                }
            }
        }

        Ok(Self {
            vector_size: fixture.vector_size,
            concept_vectors,
            input_concepts,
        })
    }

    pub fn vector_size(&self) -> usize {
        self.vector_size
    }

    pub fn vector_for_concept(&self, concept_id: &str) -> Result<Vec<f32>> {
        self.concept_vectors
            .get(concept_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("similarity fixture has no concept {concept_id:?}"))
    }

    pub fn vector_for_text(&self, text: &str) -> Result<Vec<f32>> {
        let concept_id = self.input_concepts.get(text).ok_or_else(|| {
            anyhow::anyhow!("similarity fixture has no assignment for input {text:?}")
        })?;
        self.vector_for_concept(concept_id)
    }

    pub fn concept_for_text(&self, text: &str) -> Option<&str> {
        self.input_concepts.get(text).map(String::as_str)
    }
}

fn validate_fixture(fixture: &ControllableSimilarityFixture) -> Result<()> {
    if fixture.vector_size == 0 {
        bail!("controllable similarity vector_size must be greater than zero");
    }
    if !fixture.noise_magnitude.is_finite() || fixture.noise_magnitude < 0.0 {
        bail!("controllable similarity noise_magnitude must be finite and non-negative");
    }
    if fixture.clusters.is_empty() {
        bail!("controllable similarity fixture must declare at least one cluster");
    }
    if fixture.concepts.is_empty() {
        bail!("controllable similarity fixture must declare at least one concept");
    }

    for (cluster_id, vector) in &fixture.clusters {
        if cluster_id.trim().is_empty() {
            bail!("controllable similarity cluster IDs must be non-empty");
        }
        if vector.len() != fixture.vector_size {
            bail!(
                "similarity cluster {cluster_id:?} has vector size {}, expected {}",
                vector.len(),
                fixture.vector_size
            );
        }
        if vector.iter().any(|component| !component.is_finite()) {
            bail!("similarity cluster {cluster_id:?} contains a non-finite vector component");
        }
    }

    for (concept_id, concept) in &fixture.concepts {
        if concept_id.trim().is_empty() {
            bail!("controllable similarity concept IDs must be non-empty");
        }
        if !fixture.clusters.contains_key(&concept.cluster) {
            bail!(
                "similarity concept {concept_id:?} references unknown cluster {:?}",
                concept.cluster
            );
        }
        if concept.inputs.is_empty() {
            bail!("similarity concept {concept_id:?} must declare at least one input");
        }
        if concept.inputs.iter().any(|input| input.is_empty()) {
            bail!("similarity concept {concept_id:?} contains an empty input assignment");
        }
    }
    Ok(())
}

fn seeded_noise(seed: u64, concept_id: &str, dimension: usize, magnitude: f32) -> Result<f32> {
    let dimension = u64::try_from(dimension)
        .map_err(|_| anyhow::anyhow!("similarity vector dimension does not fit in u64"))?;
    let state = seed
        ^ stable_hash(concept_id).rotate_left(17)
        ^ dimension.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    Ok(match splitmix64(state) % 3 {
        0 => -magnitude,
        1 => 0.0,
        _ => magnitude,
    })
}

fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn splitmix64(state: u64) -> u64 {
    let mut value = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process::Command};

    use super::*;

    const PROCESS_PROBE_PATH: &str = "CMEM_SIMILARITY_PROBE_PATH";

    #[test]
    fn cosine_ordering_property_holds_across_seed_range() {
        for seed in 0..128 {
            let provider = ControllableSimilarityEmbeddingProvider::new(fixture(seed)).unwrap();
            let alpha_one = provider.vector_for_text("alpha one").unwrap();
            let alpha_two = provider.vector_for_text("alpha two").unwrap();
            let beta_one = provider.vector_for_text("beta one").unwrap();

            assert!(
                cosine(&alpha_one, &alpha_two) > cosine(&alpha_one, &beta_one),
                "fixture ordering failed for seed {seed}"
            );
        }

        let original = ControllableSimilarityEmbeddingProvider::new(fixture(41)).unwrap();
        let mut reassigned_fixture = fixture(41);
        reassigned_fixture
            .concepts
            .get_mut("alpha-two")
            .unwrap()
            .cluster = "beta".into();
        let reassigned = ControllableSimilarityEmbeddingProvider::new(reassigned_fixture).unwrap();

        assert!(
            cosine(
                &original.vector_for_text("alpha one").unwrap(),
                &original.vector_for_text("alpha two").unwrap()
            ) > cosine(
                &original.vector_for_text("beta one").unwrap(),
                &original.vector_for_text("alpha two").unwrap()
            )
        );
        assert!(
            cosine(
                &reassigned.vector_for_text("beta one").unwrap(),
                &reassigned.vector_for_text("alpha two").unwrap()
            ) > cosine(
                &reassigned.vector_for_text("alpha one").unwrap(),
                &reassigned.vector_for_text("alpha two").unwrap()
            )
        );
    }

    #[test]
    fn seeded_noise_bytes_are_fixed_width_and_pinned() {
        assert_eq!(stable_hash("alpha-one"), 0xe34e_d89b_7823_a9ca);
        let provider = ControllableSimilarityEmbeddingProvider::new(fixture(0x5eed)).unwrap();
        let bits = provider
            .vector_for_text("alpha one")
            .unwrap()
            .iter()
            .map(|component| component.to_bits())
            .collect::<Vec<_>>();

        assert_eq!(
            bits,
            vec![0x3f7f_c000, 0x3a80_0000, 0xba80_0000, 0x3a80_0000]
        );
    }

    #[test]
    fn exact_fixture_assignments_are_entity_neutral() {
        let mut fixture = fixture(7);
        fixture.concepts.insert(
            "neutral".into(),
            SimilarityConceptFixture {
                cluster: "alpha".into(),
                inputs: vec!["Alice".into(), "ROLE:user".into(), "ordinary input".into()],
            },
        );
        let provider = ControllableSimilarityEmbeddingProvider::new(fixture).unwrap();

        assert_eq!(provider.concept_for_text("Alice"), Some("neutral"));
        assert_eq!(provider.concept_for_text("ROLE:user"), Some("neutral"));
        assert_eq!(provider.concept_for_text("ordinary input"), Some("neutral"));
        assert_eq!(
            provider.vector_for_text("Alice").unwrap(),
            provider.vector_for_text("ROLE:user").unwrap()
        );
        assert!(provider.vector_for_text("alice").is_err());
        assert!(provider.vector_for_text("user").is_err());
    }

    #[test]
    fn invalid_fixture_contracts_are_rejected() {
        let mut invalid = fixture(1);
        invalid.vector_size = 0;
        assert!(
            ControllableSimilarityEmbeddingProvider::new(invalid)
                .unwrap_err()
                .to_string()
                .contains("greater than zero")
        );

        let mut invalid = fixture(1);
        invalid.clusters.get_mut("alpha").unwrap().pop();
        assert!(
            ControllableSimilarityEmbeddingProvider::new(invalid)
                .unwrap_err()
                .to_string()
                .contains("expected 4")
        );

        let mut invalid = fixture(7);
        invalid.noise_magnitude = f32::MAX;
        invalid.clusters.get_mut("alpha").unwrap()[0] = f32::MAX;
        assert!(
            ControllableSimilarityEmbeddingProvider::new(invalid)
                .unwrap_err()
                .to_string()
                .contains("produces a non-finite vector")
        );

        let mut invalid = fixture(1);
        invalid.concepts.get_mut("alpha-one").unwrap().cluster = "missing".into();
        assert!(
            ControllableSimilarityEmbeddingProvider::new(invalid)
                .unwrap_err()
                .to_string()
                .contains("unknown cluster")
        );

        let mut invalid = fixture(1);
        invalid
            .concepts
            .get_mut("beta-one")
            .unwrap()
            .inputs
            .push("alpha one".into());
        assert!(
            ControllableSimilarityEmbeddingProvider::new(invalid)
                .unwrap_err()
                .to_string()
                .contains("assigned to both concepts")
        );
    }

    #[test]
    fn identical_seed_is_byte_identical_across_process_runs() {
        let current_exe = env::current_exe().unwrap();
        let mut outputs = Vec::new();
        for run in 0..2 {
            let output_path =
                env::temp_dir().join(format!("cmem-similarity-{}-{run}.bin", std::process::id()));
            let status = Command::new(&current_exe)
                .args([
                    "--exact",
                    "controllable_similarity_embedding::tests::cross_process_probe",
                    "--nocapture",
                ])
                .env(PROCESS_PROBE_PATH, &output_path)
                .status()
                .unwrap();
            assert!(status.success());
            outputs.push(fs::read(&output_path).unwrap());
            fs::remove_file(output_path).unwrap();
        }

        assert_eq!(outputs[0], outputs[1]);
        assert!(!outputs[0].is_empty());
    }

    #[test]
    fn cross_process_probe() {
        let Ok(output_path) = env::var(PROCESS_PROBE_PATH) else {
            return;
        };
        let provider = ControllableSimilarityEmbeddingProvider::new(fixture(0x5eed)).unwrap();
        let bytes = provider
            .vector_for_text("alpha one")
            .unwrap()
            .iter()
            .flat_map(|component| component.to_le_bytes())
            .collect::<Vec<_>>();
        fs::write(output_path, bytes).unwrap();
    }

    fn fixture(seed: u64) -> ControllableSimilarityFixture {
        ControllableSimilarityFixture {
            seed,
            vector_size: 4,
            noise_magnitude: 1.0 / 1024.0,
            clusters: BTreeMap::from([
                ("alpha".into(), vec![1.0, 0.0, 0.0, 0.0]),
                ("beta".into(), vec![0.0, 1.0, 0.0, 0.0]),
            ]),
            concepts: BTreeMap::from([
                (
                    "alpha-one".into(),
                    SimilarityConceptFixture {
                        cluster: "alpha".into(),
                        inputs: vec!["alpha one".into()],
                    },
                ),
                (
                    "alpha-two".into(),
                    SimilarityConceptFixture {
                        cluster: "alpha".into(),
                        inputs: vec!["alpha two".into()],
                    },
                ),
                (
                    "beta-one".into(),
                    SimilarityConceptFixture {
                        cluster: "beta".into(),
                        inputs: vec!["beta one".into()],
                    },
                ),
            ]),
        }
    }

    fn cosine(left: &[f32], right: &[f32]) -> f64 {
        let dot = left
            .iter()
            .zip(right)
            .map(|(left, right)| f64::from(*left) * f64::from(*right))
            .sum::<f64>();
        let left_norm = left
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt();
        let right_norm = right
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt();
        dot / (left_norm * right_norm)
    }
}
