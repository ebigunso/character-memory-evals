use super::*;

fn run_args(adapter: Option<AdapterKind>, allow_mock_benchmark: bool) -> RunArgs {
    RunArgs {
        dataset: PathBuf::from("dataset.json"),
        config: PathBuf::from("config.toml"),
        out: PathBuf::from("out.jsonl"),
        summary_out: PathBuf::from("summary.json"),
        adapter,
        allow_mock_benchmark,
    }
}

#[test]
fn omitted_adapter_selects_real() {
    let args = run_args(None, false);
    assert_eq!(args.selected_adapter(), AdapterKind::Real);
    args.validate_adapter_selection(&benchmark_config())
        .unwrap();
}

#[test]
fn mock_adapter_requires_explicit_benchmark_guard() {
    let err = run_args(Some(AdapterKind::Mock), false)
        .validate_adapter_selection(&benchmark_config())
        .unwrap_err()
        .to_string();
    assert!(err.contains("mock adapter is test/smoke-only"));
}

#[test]
fn guarded_mock_adapter_is_allowed_for_smoke_runs() {
    let args = run_args(Some(AdapterKind::Mock), true);
    assert_eq!(args.selected_adapter(), AdapterKind::Mock);
    args.validate_adapter_selection(&benchmark_config())
        .unwrap();
}

#[test]
fn vector_only_mode_rejects_mock_adapter() {
    let err = run_args(Some(AdapterKind::Mock), true)
        .validate_adapter_selection(&benchmark_config_with_mode(RetrievalMode::VectorOnly))
        .unwrap_err()
        .to_string();
    assert!(err.contains("retrieval.mode=vector_only"));
    assert!(err.contains("cannot run with `--adapter mock`"));
}

#[test]
fn vector_only_mode_allows_default_real_adapter() {
    let args = run_args(None, false);
    assert_eq!(args.selected_adapter(), AdapterKind::Real);
    args.validate_adapter_selection(&benchmark_config_with_mode(RetrievalMode::VectorOnly))
        .unwrap();
}

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
            cmem_eval_core::EmbeddingProviderConfig::OpenAi
        );
        assert_eq!(
            config.retrieval.surface_policy.object_types,
            [
                cmem_eval_core::ObjectType::Episode,
                cmem_eval_core::ObjectType::Observation,
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
fn current_checked_in_configs_parse_under_the_strict_schema() {
    let mut paths = fs::read_dir("../../configs")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
                && !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("continuity_")
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());

    for path in paths {
        read_config(&path).unwrap_or_else(|error| {
            panic!("checked-in config {} failed: {error:#}", path.display())
        });
    }
}

fn benchmark_config() -> BenchmarkRunConfig {
    benchmark_config_with_mode(RetrievalMode::Hybrid)
}

fn benchmark_config_with_mode(mode: RetrievalMode) -> BenchmarkRunConfig {
    BenchmarkRunConfig {
        run_id: "r".into(),
        dataset: cmem_eval_core::DatasetId::new("locomo").unwrap(),
        backend: Default::default(),
        retrieval: cmem_eval_core::RetrievalConfig {
            mode,
            ..Default::default()
        },
        ingest: Default::default(),
        metrics: Default::default(),
    }
}
