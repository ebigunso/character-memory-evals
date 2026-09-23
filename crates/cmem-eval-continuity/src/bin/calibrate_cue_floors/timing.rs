//! Optional warm recall readings, kept outside deterministic measurement JSON.
use super::*;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Timings {
    pub enabled: bool,
    pub rows: Vec<Value>,
}

impl Timings {
    pub async fn record(
        &mut self,
        runtime: &ContinuityRuntime,
        family: &str,
        opposed_ids: bool,
        queries: Vec<(String, RetrieveInput)>,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        for (probe, input) in queries {
            let mut samples_ms = Vec::new();
            let mut first = None;
            for _ in 0..3 {
                let request = input.clone();
                let start = Instant::now();
                let pack = runtime.adapter().retrieve(request).await?;
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                // Validation/serialization is deliberately outside the timed boundary.
                let observed = snapshot(&pack, &input)?;
                ensure!(
                    observed["telemetry"]["graph_expansion"]["bounded_failure_count"] == 0,
                    "degraded timed recall"
                );
                ensure!(
                    matches!(
                        observed["telemetry"]["vector_recall_completeness"]["kind"].as_str(),
                        Some("exhaustive" | "not_requested")
                    ),
                    "incomplete timed vector recall"
                );
                if let Some(prior) = &first {
                    ensure!(
                        prior == &observed,
                        "timed recall changed deterministic output: {family}/{probe}"
                    );
                } else {
                    first = Some(observed);
                }
                samples_ms.push(elapsed_ms);
            }
            self.rows.push(json!({"family":family,"opposed_ids":opposed_ids,"probe":probe,
                "input":input,"samples_ms":samples_ms,"median_ms":median(&samples_ms),
                "sample_count":samples_ms.len(),"bounded_failures":0,"repeat_measurements_identical":true}));
        }
        Ok(())
    }
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

pub(super) fn probes(probes: &[Probe]) -> Vec<(String, RetrieveInput)> {
    probes
        .iter()
        .flat_map(|probe| {
            FLOORS.map(|floor| {
                let mut input = probe.input.clone();
                input.cue_floors = Some(floors(&probe.measured_kind, floor));
                (
                    format!("{}/{}-floor-{floor}", probe.name, probe.measured_kind),
                    input,
                )
            })
        })
        .collect()
}

pub(super) fn scene_queries(inputs: &[Probe]) -> Vec<(String, RetrieveInput)> {
    let mut control = inputs[0].input.clone();
    for kind in ["participant", "place", "activity"] {
        remove_cue(&mut control, kind);
    }
    let mut queries = vec![("topic-only-control".into(), control)];
    queries.extend(probes(inputs));
    for probe in inputs {
        if probe.target.is_some() {
            let mut isolated = probe.input.clone();
            for kind in KINDS {
                if kind != probe.measured_kind {
                    remove_cue(&mut isolated, kind);
                }
            }
            let mut removed = probe.input.clone();
            remove_cue(&mut removed, &probe.measured_kind);
            queries.push((format!("{}/isolated-control", probe.name), isolated));
            queries.push((format!("{}/removed-control", probe.name), removed));
        }
        if probe.name.starts_with("unlived-scene-") {
            for floor in FLOORS {
                let mut removed = probe.input.clone();
                removed.cue_floors = Some(floors(&probe.measured_kind, floor));
                remove_cue(&mut removed, &probe.measured_kind);
                queries.push((format!("{}/removed-floor-{floor}", probe.name), removed));
            }
        }
    }
    queries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_middle_recall_not_the_mean_or_last_sample() {
        assert_eq!(median(&[100.0, 2.0, 4.0]), 4.0);
    }
}
