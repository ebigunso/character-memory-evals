use anyhow::{Context, Result, bail};
use clap::Args;
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
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
    Ok(())
}

#[derive(Debug, Deserialize)]
struct DiffRow {
    run_id: String,
    question_id: String,
    #[serde(default)]
    retrieved: Vec<DiffRetrievedItem>,
    #[serde(default)]
    write_outcomes: Vec<DiffWriteOutcome>,
    #[serde(default)]
    lifecycle_outcomes: Vec<DiffLifecycleOutcome>,
    #[serde(default)]
    metrics: Value,
    latency_ms: u128,
    #[serde(default, rename = "reader", deserialize_with = "present")]
    reader_present: bool,
}

#[derive(Debug, Deserialize)]
struct DiffRetrievedItem {
    kind: Value,
    internal_id: String,
    #[serde(default)]
    external_id: Option<String>,
    #[serde(default)]
    episode_external_id: Option<String>,
    rank: usize,
}

#[derive(Debug, Default, Deserialize)]
struct DiffStatsUpdateStatus {
    #[serde(default)]
    failure: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct DiffWriteOutcome {
    #[serde(default)]
    vector_indexing_failure: Option<Value>,
    #[serde(default)]
    stats_update_status: DiffStatsUpdateStatus,
    #[serde(default)]
    repair_needed: Vec<Value>,
    #[serde(default, rename = "attempt_index", deserialize_with = "present")]
    attempt_index_present: bool,
}

#[derive(Debug, Deserialize)]
struct DiffLifecycleOutcome {
    #[serde(default)]
    vector_maintenance_failures: Vec<Value>,
    #[serde(default)]
    stats_update_status: DiffStatsUpdateStatus,
    #[serde(default, rename = "attempt_index", deserialize_with = "present")]
    attempt_index_present: bool,
}

fn present<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    IgnoredAny::deserialize(deserializer)?;
    Ok(true)
}

fn read_rows(path: &Path) -> Result<Vec<DiffRow>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut rows = Vec::new();
    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line =
            line.with_context(|| format!("read {} line {}", path.display(), line_index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line).with_context(|| {
            format!("parse {} line {} for diff", path.display(), line_index + 1)
        })?);
    }
    Ok(rows)
}

fn normalize(mut rows: Vec<DiffRow>) -> Vec<DiffRow> {
    for row in &mut rows {
        row.run_id = "__RUN__".to_string();
        row.latency_ms = 0;
    }
    rows
}

#[derive(Debug, Default)]
struct DiffReport {
    details: Vec<String>,
    informational_fields: Vec<String>,
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
        if !self.informational_fields.is_empty() {
            lines.push(format!(
                "informational: fields absent on one side: {}",
                self.informational_fields.join(", ")
            ));
        }
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

fn compare(run_a: Vec<DiffRow>, run_b: Vec<DiffRow>) -> Result<DiffReport> {
    if run_a.is_empty() || run_b.is_empty() {
        bail!(
            "refusing to compare: run A has {} rows and run B has {} rows; an empty run is a failed or missing evaluation, not a zero-difference proof",
            run_a.len(),
            run_b.len()
        );
    }
    let fields_a = field_presence(&run_a);
    let fields_b = field_presence(&run_b);
    let run_a = index(run_a, "run A")?;
    let run_b = index(run_b, "run B")?;
    let query_ids = run_a
        .keys()
        .chain(run_b.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut report = DiffReport {
        queries: query_ids.len(),
        informational_fields: fields_a
            .union(&fields_b)
            .filter_map(
                |field| match (fields_a.contains(field), fields_b.contains(field)) {
                    (true, false) => Some(format!("{field} (run B)")),
                    (false, true) => Some(format!("{field} (run A)")),
                    _ => None,
                },
            )
            .collect(),
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

fn field_presence(rows: &[DiffRow]) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    for row in rows {
        if row.reader_present {
            fields.insert("reader".to_string());
        }
        if row
            .write_outcomes
            .iter()
            .any(|outcome| outcome.attempt_index_present)
        {
            fields.insert("write_outcomes[].attempt_index".to_string());
        }
        if row
            .lifecycle_outcomes
            .iter()
            .any(|outcome| outcome.attempt_index_present)
        {
            fields.insert("lifecycle_outcomes[].attempt_index".to_string());
        }
    }
    fields
}

fn index(rows: Vec<DiffRow>, label: &str) -> Result<BTreeMap<String, DiffRow>> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let question_id = row.question_id.clone();
        if indexed.insert(question_id.clone(), row).is_some() {
            bail!("{label} contains duplicate question_id {question_id:?}");
        }
    }
    Ok(indexed)
}

fn identities(items: &[DiffRetrievedItem]) -> Vec<String> {
    let mut identities = items.iter().map(identity).collect::<Vec<_>>();
    identities.sort();
    identities
}

fn ranks(items: &[DiffRetrievedItem]) -> Vec<(String, usize)> {
    let mut ranks = items
        .iter()
        .map(|item| (identity(item), item.rank))
        .collect::<Vec<_>>();
    ranks.sort();
    ranks
}

fn identity(item: &DiffRetrievedItem) -> String {
    serde_json::to_string(&(
        &item.kind,
        &item.internal_id,
        &item.external_id,
        &item.episode_external_id,
    ))
    .expect("retrieved item identity always serializes")
}

fn degraded(row: &DiffRow) -> bool {
    row.write_outcomes.iter().any(|outcome| {
        outcome.vector_indexing_failure.is_some()
            || outcome.stats_update_status.failure.is_some()
            || !outcome.repair_needed.is_empty()
    }) || row.lifecycle_outcomes.iter().any(|outcome| {
        !outcome.vector_maintenance_failures.is_empty()
            || outcome.stats_update_status.failure.is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(run_id: &str, latency_ms: u128, rank: usize, legacy_fields: bool) -> DiffRow {
        row_with(run_id, latency_ms, rank, legacy_fields, |_| {})
    }

    fn row_with(
        run_id: &str,
        latency_ms: u128,
        rank: usize,
        legacy_fields: bool,
        edit: impl FnOnce(&mut Value),
    ) -> DiffRow {
        let mut value = json!({
            "schema_version": "2",
            "run_id": run_id,
            "question_id": "q1",
            "retrieved": [{
                "kind": "episode",
                "internal_id": "one",
                "external_id": "one",
                "episode_external_id": "one",
                "rank": rank,
                "score": 1.0
            }],
            "write_outcomes": [{
                "operation_id": "write-1",
                "vector_indexing_failure": null,
                "stats_update_status": {"failure": null},
                "repair_needed": []
            }],
            "lifecycle_outcomes": [{
                "operation_id": "lifecycle-1",
                "vector_maintenance_failures": [],
                "stats_update_status": {"failure": null}
            }],
            "metrics": {"recall_any@1": 1.0},
            "latency_ms": latency_ms
        });
        if legacy_fields {
            value["reader"] = json!({"model": null, "answer": null});
            value["write_outcomes"][0]["attempt_index"] = json!(0);
            value["lifecycle_outcomes"][0]["attempt_index"] = json!(0);
        }
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
            normalize(vec![row("a", 1, 1, false)]),
            normalize(vec![row_with("b", 1, 1, false, |value| {
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
            normalize(vec![row("a", 1, 1, false)]),
            normalize(vec![row_with("b", 1, 1, false, |value| {
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
            normalize(vec![row("a", 1, 1, false)]),
            normalize(vec![row_with("b", 1, 1, false, |value| {
                value["write_outcomes"][0]["vector_indexing_failure"] =
                    json!({"kind": "zero_norm"});
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
            normalize(vec![row("a", 1, 1, false)]),
            normalize(vec![row("b", 99, 1, false)]),
        )
        .unwrap();
        assert_eq!(report.differing_queries, 0);
        assert_eq!(
            report.render(),
            "summary: queries=1 differing=0 missing_from_a=0 missing_from_b=0 identity_changes=0 rank_changes=0 metric_changes=0 degradation_changes=0\n"
        );
    }

    #[test]
    fn prior_fields_are_informational_and_do_not_create_semantic_differences() {
        let report = compare(
            normalize(vec![row("parent", 1, 1, true)]),
            normalize(vec![row("candidate", 2, 1, false)]),
        )
        .unwrap();
        assert_eq!(report.differing_queries, 0);
        assert_eq!(report.identity_changes, 0);
        assert_eq!(report.rank_changes, 0);
        assert_eq!(report.metric_changes, 0);
        assert_eq!(report.degradation_changes, 0);
        assert_eq!(
            report.informational_fields,
            vec![
                "lifecycle_outcomes[].attempt_index (run B)",
                "reader (run B)",
                "write_outcomes[].attempt_index (run B)",
            ]
        );
        assert!(
            report
                .render()
                .starts_with("informational: fields absent on one side:")
        );
    }

    #[test]
    fn empty_runs_are_rejected_rather_than_reported_equivalent() {
        let error = compare(Vec::new(), Vec::new()).unwrap_err().to_string();
        assert!(error.contains("refusing to compare"), "{error}");
        let error = compare(vec![row("a", 1, 0, false)], Vec::new())
            .unwrap_err()
            .to_string();
        assert!(error.contains("run B has 0 rows"), "{error}");
    }

    #[test]
    fn rank_only_change_is_reported_once() {
        let report = compare(
            normalize(vec![row("a", 1, 1, false)]),
            normalize(vec![row("b", 2, 2, false)]),
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
