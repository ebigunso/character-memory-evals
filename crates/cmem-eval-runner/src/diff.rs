use anyhow::{Result, bail};
use clap::Args;
use cmem_eval::{PerQuestionResult, RetrievedItem};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(crate) struct DiffArgs {
    pub(crate) run_a: PathBuf,
    pub(crate) run_b: PathBuf,
}

pub(crate) fn run(args: DiffArgs) -> Result<()> {
    let report = compare(
        normalize(read_rows(&args.run_a)?),
        normalize(read_rows(&args.run_b)?),
    )?;
    print!("{}", report.render());
    if report.differing_queries > 0 {
        bail!(
            "{} of {} queries differ semantically",
            report.differing_queries,
            report.queries
        );
    }
    Ok(())
}

fn read_rows(path: &Path) -> Result<Vec<PerQuestionResult>> {
    cmem_eval::read_jsonl(path)
}

fn normalize(mut rows: Vec<PerQuestionResult>) -> Vec<PerQuestionResult> {
    for row in &mut rows {
        row.run_id = "__RUN__".to_string();
        row.latency_ms = 0;
    }
    rows
}

#[derive(Debug, Default)]
struct DiffReport {
    details: Vec<String>,
    queries: usize,
    differing_queries: usize,
    missing_from_a: usize,
    missing_from_b: usize,
    identity_changes: usize,
    rank_changes: usize,
    metric_changes: usize,
    degradation_changes: usize,
}

impl DiffReport {
    fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.extend(self.details.iter().cloned());
        let mut output = lines.join("\n");
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!(
            "summary: queries={} differing={} missing_from_a={} missing_from_b={} identity_changes={} rank_changes={} metric_changes={} degradation_changes={}\n",
            self.queries,
            self.differing_queries,
            self.missing_from_a,
            self.missing_from_b,
            self.identity_changes,
            self.rank_changes,
            self.metric_changes,
            self.degradation_changes,
        ));
        output
    }
}

fn compare(run_a: Vec<PerQuestionResult>, run_b: Vec<PerQuestionResult>) -> Result<DiffReport> {
    if run_a.is_empty() || run_b.is_empty() {
        bail!(
            "refusing to compare: run A has {} rows and run B has {} rows; an empty run is a failed or missing evaluation, not a zero-difference proof",
            run_a.len(),
            run_b.len()
        );
    }
    let run_a = index(run_a, "run A")?;
    let run_b = index(run_b, "run B")?;
    let query_ids = run_a
        .keys()
        .chain(run_b.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut report = DiffReport {
        queries: query_ids.len(),
        ..Default::default()
    };

    for query_id in query_ids {
        let Some(a) = run_a.get(&query_id) else {
            report
                .details
                .push(format!("query {query_id}: missing from run A"));
            report.differing_queries += 1;
            report.missing_from_a += 1;
            continue;
        };
        let Some(b) = run_b.get(&query_id) else {
            report
                .details
                .push(format!("query {query_id}: missing from run B"));
            report.differing_queries += 1;
            report.missing_from_b += 1;
            continue;
        };

        let mut changes = Vec::new();
        let identities_a = identities(&a.retrieved);
        let identities_b = identities(&b.retrieved);
        if identities_a != identities_b {
            changes.push(format!("identities: a={identities_a:?} b={identities_b:?}"));
            report.identity_changes += 1;
        }
        let ranks_a = ranks(&a.retrieved);
        let ranks_b = ranks(&b.retrieved);
        if ranks_a != ranks_b {
            changes.push(format!("ranks: a={ranks_a:?} b={ranks_b:?}"));
            report.rank_changes += 1;
        }
        if a.metrics != b.metrics {
            changes.push(format!(
                "metrics: a={} b={}",
                serde_json::to_string(&a.metrics)?,
                serde_json::to_string(&b.metrics)?
            ));
            report.metric_changes += 1;
        }
        let degradation_a = degraded(a);
        let degradation_b = degraded(b);
        if degradation_a != degradation_b {
            changes.push(format!("degradation: a={degradation_a} b={degradation_b}"));
            report.degradation_changes += 1;
        }
        if !changes.is_empty() {
            report
                .details
                .push(format!("query {query_id}: {}", changes.join("; ")));
            report.differing_queries += 1;
        }
    }
    Ok(report)
}

fn index(rows: Vec<PerQuestionResult>, label: &str) -> Result<BTreeMap<String, PerQuestionResult>> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let question_id = row.question_id.clone();
        if indexed.insert(question_id.clone(), row).is_some() {
            bail!("{label} contains duplicate question_id {question_id:?}");
        }
    }
    Ok(indexed)
}

fn identities(items: &[RetrievedItem]) -> Vec<String> {
    let mut identities = items.iter().map(identity).collect::<Vec<_>>();
    identities.sort();
    identities
}

fn ranks(items: &[RetrievedItem]) -> Vec<(String, usize)> {
    let mut ranks = items
        .iter()
        .map(|item| (identity(item), item.rank))
        .collect::<Vec<_>>();
    ranks.sort();
    ranks
}

fn identity(item: &RetrievedItem) -> String {
    serde_json::to_string(&(
        &item.kind,
        &item.internal_id,
        &item.external_id,
        &item.episode_external_id,
    ))
    .expect("retrieved item identity always serializes")
}

fn degraded(row: &PerQuestionResult) -> bool {
    cmem_eval::summarize_degradation(std::slice::from_ref(row)).any_degradation
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn row(run_id: &str, latency_ms: u128, rank: usize) -> PerQuestionResult {
        row_with(run_id, latency_ms, rank, |_| {})
    }

    fn row_with(
        run_id: &str,
        latency_ms: u128,
        rank: usize,
        edit: impl FnOnce(&mut Value),
    ) -> PerQuestionResult {
        let result = PerQuestionResult {
            schema_version: cmem_eval::RESULT_SCHEMA_VERSION.into(),
            run_id: run_id.into(),
            question_id: "q1".into(),
            dataset: cmem_eval::DatasetId::new("continuity").unwrap(),
            dataset_kind: cmem_eval::DatasetKind::Continuity,
            embedding_binding: cmem_eval::EmbeddingBindingRecord::Bm25,
            adapter: Default::default(),
            question_type: None,
            question: "query".into(),
            gold_episode_ids: Vec::new(),
            gold_observation_ids: Vec::new(),
            retrieved: vec![RetrievedItem {
                kind: cmem_eval::ObjectType::Episode,
                internal_id: "one".into(),
                external_id: Some("one".into()),
                episode_external_id: Some("one".into()),
                rank,
                score: Some(1.0),
                rationale: Vec::new(),
                text: None,
            }],
            context_text: String::new(),
            context_char_count: 0,
            context_word_count: 0,
            context: Default::default(),
            composition: Default::default(),
            integrity: Default::default(),
            retrieval_outcomes: Vec::new(),
            link_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
            write_outcomes: vec![cmem_eval::RecordedOutcome {
                operation_id: "write-1".into(),
                outcome: cmem_eval::RememberOutcome {
                    persisted_object_ids: Vec::new(),
                    persisted_link_ids: Vec::new(),
                    vector_indexed_object_ids: Vec::new(),
                    vector_indexing_failure: None,
                    stats_update_status: Default::default(),
                    repair_needed: Vec::new(),
                    diagnostics: Default::default(),
                },
            }],
            metrics: cmem_eval::MetricsRecord::try_from(
                json!({"recall_any@1":1.0}).as_object().unwrap().clone(),
            )
            .unwrap(),
            latency_ms,
        };
        let mut value = serde_json::to_value(result).unwrap();
        edit(&mut value);
        serde_json::from_value(value).unwrap()
    }

    fn only_counter(
        report: &DiffReport,
        identity: usize,
        rank: usize,
        metric: usize,
        degradation: usize,
    ) {
        assert_eq!(report.differing_queries, 1);
        assert_eq!(report.identity_changes, identity);
        assert_eq!(report.rank_changes, rank);
        assert_eq!(report.metric_changes, metric);
        assert_eq!(report.degradation_changes, degradation);
    }

    #[test]
    fn identity_only_change_is_reported_as_identity_and_rank() {
        // A different object at the same rank changes the identity list and,
        // because ranks are keyed by identity, the rank list too.
        let report = compare(
            normalize(vec![row("a", 1, 1)]),
            normalize(vec![row_with("b", 1, 1, |value| {
                value["retrieved"][0]["internal_id"] = json!("two");
                value["retrieved"][0]["external_id"] = json!("two");
                value["retrieved"][0]["episode_external_id"] = json!("two");
            })]),
        )
        .unwrap();
        only_counter(&report, 1, 1, 0, 0);
        let rendered = report.render();
        assert!(rendered.contains("query q1: identities: a=["), "{rendered}");
        assert!(
            rendered.contains("one") && rendered.contains("two"),
            "{rendered}"
        );
        assert!(!rendered.contains("metrics:"), "{rendered}");
        assert!(!rendered.contains("degradation:"), "{rendered}");
    }

    #[test]
    fn metric_only_change_is_reported_once() {
        let report = compare(
            normalize(vec![row("a", 1, 1)]),
            normalize(vec![row_with("b", 1, 1, |value| {
                value["metrics"] = json!({"recall_any@1": 0.0});
            })]),
        )
        .unwrap();
        only_counter(&report, 0, 0, 1, 0);
        let rendered = report.render();
        assert!(
            rendered
                .contains("query q1: metrics: a={\"recall_any@1\":1.0} b={\"recall_any@1\":0.0}"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("identities:") && !rendered.contains("ranks:"),
            "{rendered}"
        );
    }

    #[test]
    fn degradation_only_change_is_reported_once() {
        let report = compare(
            normalize(vec![row("a", 1, 1)]),
            normalize(vec![row_with("b", 1, 1, |value| {
                value["write_outcomes"][0]["outcome"]["vector_indexing_failure"] =
                    serde_json::to_value(cmem_eval::character_memory::VectorIndexingFailure {
                        unindexed_objects: Vec::new(),
                        cause:
                            cmem_eval::character_memory::VectorIndexingCause::CardinalityMismatch {
                                expected: 1,
                                actual: 0,
                            },
                    })
                    .unwrap();
            })]),
        )
        .unwrap();
        only_counter(&report, 0, 0, 0, 1);
        let rendered = report.render();
        assert!(
            rendered.contains("query q1: degradation: a=false b=true"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("metrics:") && !rendered.contains("identities:"),
            "{rendered}"
        );
    }

    #[test]
    fn run_identity_and_latency_are_the_only_normalized_fields() {
        let report = compare(
            normalize(vec![row("a", 1, 1)]),
            normalize(vec![row("b", 99, 1)]),
        )
        .unwrap();
        assert_eq!(report.differing_queries, 0);
        assert_eq!(
            report.render(),
            "summary: queries=1 differing=0 missing_from_a=0 missing_from_b=0 identity_changes=0 rank_changes=0 metric_changes=0 degradation_changes=0\n"
        );
    }

    #[test]
    fn rows_missing_a_compared_field_or_native_outcome_are_rejected() {
        let valid = serde_json::to_value(row("a", 1, 1)).unwrap();
        for field in [
            "retrieved",
            "metrics",
            "write_outcomes",
            "link_outcomes",
            "lifecycle_outcomes",
        ] {
            let mut value = valid.clone();
            value.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<PerQuestionResult>(value).is_err(),
                "missing {field}"
            );
        }
        let mut missing = valid.clone();
        missing["write_outcomes"][0]
            .as_object_mut()
            .unwrap()
            .remove("outcome");
        assert!(serde_json::from_value::<PerQuestionResult>(missing).is_err());
        let mut malformed = valid;
        malformed["write_outcomes"][0]["outcome"]["vector_indexing_failure"] =
            json!({"unknown":"cause"});
        assert!(serde_json::from_value::<PerQuestionResult>(malformed).is_err());
    }

    fn write_rows(directory: &std::path::Path, name: &str, rows: &[Value]) -> PathBuf {
        let path = directory.join(name);
        let text = rows
            .iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        std::fs::write(&path, text).unwrap();
        path
    }

    fn row_value(run_id: &str, metric: f64) -> Value {
        serde_json::to_value(row_with(run_id, 1, 1, |value| {
            value["metrics"] = json!({"x":metric});
        }))
        .unwrap()
    }

    #[test]
    fn run_fails_on_a_semantic_difference_and_succeeds_on_none() {
        let directory = tempfile::tempdir().unwrap();
        let a = write_rows(directory.path(), "a.jsonl", &[row_value("a", 1.0)]);
        let same = write_rows(directory.path(), "same.jsonl", &[row_value("b", 1.0)]);
        let different = write_rows(directory.path(), "different.jsonl", &[row_value("b", 2.0)]);

        run(DiffArgs {
            run_a: a.clone(),
            run_b: same,
        })
        .unwrap();
        let error = run(DiffArgs {
            run_a: a,
            run_b: different,
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("1 of 1 queries differ semantically"),
            "{error}"
        );
    }

    #[test]
    fn empty_runs_are_rejected_rather_than_reported_equivalent() {
        let error = compare(Vec::new(), Vec::new()).unwrap_err().to_string();
        assert!(error.contains("refusing to compare"), "{error}");
        let error = compare(vec![row("a", 1, 0)], Vec::new())
            .unwrap_err()
            .to_string();
        assert!(error.contains("run B has 0 rows"), "{error}");
    }

    #[test]
    fn rank_only_change_is_reported_once() {
        let report = compare(
            normalize(vec![row("a", 1, 1)]),
            normalize(vec![row("b", 2, 2)]),
        )
        .unwrap();
        assert_eq!(report.differing_queries, 1);
        assert_eq!(report.identity_changes, 0);
        assert_eq!(report.rank_changes, 1);
        assert_eq!(report.metric_changes, 0);
        assert_eq!(report.degradation_changes, 0);
        assert!(report.render().contains("query q1: ranks:"));
    }
}
