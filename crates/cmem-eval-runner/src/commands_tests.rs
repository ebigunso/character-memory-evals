use super::*;
use cmem_eval::RetrievalMode;

#[test]
fn checked_in_vector_configs_use_raw_candidate_ingestion_only() {
    for path in [
        "../../configs/longmemeval_s_vector.toml",
        "../../configs/locomo_vector.toml",
    ] {
        let path = PathBuf::from(path);
        let config = read_config(&path)
            .unwrap_or_else(|err| panic!("read vector config {}: {err}", path.display()));
        assert_eq!(config.retrieval.mode, RetrievalMode::VectorOnly);
        assert_eq!(
            config.backend.embedding.provider,
            cmem_eval::EmbeddingProviderConfig::OpenAi
        );
        assert_eq!(
            config.retrieval.surface_policy.object_types,
            [
                cmem_eval::ObjectType::Episode,
                cmem_eval::ObjectType::Observation,
            ]
        );
        assert_eq!(config.retrieval.surface_policy.sections.derived_memories, 0);
        assert_eq!(config.retrieval.surface_policy.sections.active_threads, 0);
        assert!(!config.ingest.index_session_summaries);
        assert!(!config.ingest.index_generated_observations);
        assert!(config.ingest.enrichment_path.is_none());
        config.validate().unwrap();
    }
}

#[test]
fn config_reader_rejects_misspelled_backend_table() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        r#"
run_id = "typo"
dataset = "locomo"

[backend.character_memroy]
selectivity_gamma = 0.5
"#,
    )
    .unwrap();

    let error = read_config(&path).unwrap_err().to_string();
    assert!(
        error.contains("unknown field `character_memroy`"),
        "{error}"
    );
}

#[test]
fn current_checked_in_configs_parse_and_validate_under_the_strict_schema() {
    let mut paths = fs::read_dir("../../configs")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            // Sealed continuity configs are cited by hash and may predate the
            // current schema; maintained smoke and cross-mode configs must track it.
            path.extension()
                .is_some_and(|extension| extension == "toml")
                && (!name.starts_with("continuity_")
                    || name == "continuity_smoke.toml"
                    || name.starts_with("continuity_crossmode_"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());

    for path in paths {
        read_config(&path)
            .and_then(|config| config.validate())
            .unwrap_or_else(|error| {
                panic!("checked-in config {} failed: {error:#}", path.display())
            });
    }
}
