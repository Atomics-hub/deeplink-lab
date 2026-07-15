use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{Classification, GateConfig, RunReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Passed,
    Failed,
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GateResult {
    pub status: GateStatus,
    pub observed: String,
    pub required: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GateReport {
    pub schema_version: u32,
    pub novelty: GateResult,
    pub repeatability: GateResult,
    pub speed: GateResult,
    pub integration: GateResult,
    pub evidence: GateResult,
    pub all_verified_gates_passed: bool,
}

pub fn evaluate(
    representative: &RunReport,
    repeatability: Option<&RunReport>,
    config: &GateConfig,
    integration_seconds: Option<u64>,
) -> GateReport {
    let failure_classes = representative
        .results
        .iter()
        .filter(|result| result.classification.is_failure())
        .filter(|result| result.static_preflight_passed)
        .filter_map(|result| result.runtime_failure_class.clone())
        .collect::<BTreeSet<_>>();
    let novelty = GateResult {
        status: if failure_classes.len() >= config.novel_failure_classes {
            GateStatus::Passed
        } else {
            GateStatus::Failed
        },
        observed: format!(
            "{} unique runtime-only failure classes: {}",
            failure_classes.len(),
            failure_classes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
        required: format!("at least {}", config.novel_failure_classes),
    };

    let repeatability_result =
        repeatability.map(|run| repeatability_percent(run, config.repeat_runs));
    let repeatability = match repeatability_result {
        Some((percent, complete)) => GateResult {
            status: if !complete {
                GateStatus::Unverified
            } else if percent >= config.repeatability_percent {
                GateStatus::Passed
            } else {
                GateStatus::Failed
            },
            observed: format!("{percent:.2}% minimum per-case modal classification agreement"),
            required: format!(
                "at least {:.2}% over at least {} repeats per case",
                config.repeatability_percent, config.repeat_runs
            ),
        },
        None => GateResult {
            status: GateStatus::Unverified,
            observed: "no repeated-run report supplied".to_owned(),
            required: format!(
                "at least {:.2}% over {} repeats",
                config.repeatability_percent, config.repeat_runs
            ),
        },
    };

    let speed_ready = representative.summary.total >= config.representative_case_count
        && representative.summary.platforms.len() >= config.representative_platform_count;
    let speed = GateResult {
        status: if !speed_ready {
            GateStatus::Unverified
        } else if representative.summary.duration_ms < config.max_duration_ms {
            GateStatus::Passed
        } else {
            GateStatus::Failed
        },
        observed: format!(
            "{} cases, {} platform(s), {:.3}s",
            representative.summary.total,
            representative.summary.platforms.len(),
            representative.summary.duration_ms as f64 / 1000.0
        ),
        required: format!(
            "at least {} cases across {} platforms in under {:.0}s",
            config.representative_case_count,
            config.representative_platform_count,
            config.max_duration_ms as f64 / 1000.0
        ),
    };

    let integration = match integration_seconds {
        Some(seconds) => GateResult {
            status: if seconds < config.max_integration_seconds {
                GateStatus::Passed
            } else {
                GateStatus::Failed
            },
            observed: format!("{seconds}s declarative integration path"),
            required: format!(
                "under {}s without runner code",
                config.max_integration_seconds
            ),
        },
        None => GateResult {
            status: GateStatus::Unverified,
            observed: "no timed integration evidence supplied".to_owned(),
            required: format!(
                "under {}s without runner code",
                config.max_integration_seconds
            ),
        },
    };

    let failures = representative
        .results
        .iter()
        .filter(|result| result.classification.is_failure())
        .collect::<Vec<_>>();
    let complete = failures
        .iter()
        .filter(|result| result.evidence.complete_for_failure)
        .count();
    let evidence = GateResult {
        status: if failures.is_empty() {
            GateStatus::Unverified
        } else if complete == failures.len() {
            GateStatus::Passed
        } else {
            GateStatus::Failed
        },
        observed: format!("{complete}/{} runtime failures have complete evidence", failures.len()),
        required: "every runtime failure has actual destination, screenshot, logs, starting state, and replay".to_owned(),
    };
    let all_verified_gates_passed = [&novelty, &repeatability, &speed, &integration, &evidence]
        .iter()
        .all(|gate| gate.status == GateStatus::Passed);
    GateReport {
        schema_version: 1,
        novelty,
        repeatability,
        speed,
        integration,
        evidence,
        all_verified_gates_passed,
    }
}

fn repeatability_percent(report: &RunReport, required_runs: usize) -> (f64, bool) {
    let mut cases = BTreeMap::<&str, Vec<Classification>>::new();
    for result in &report.results {
        cases
            .entry(&result.case_id)
            .or_default()
            .push(result.classification);
    }
    if cases.is_empty() {
        return (0.0, false);
    }
    let complete = cases.values().all(|values| values.len() >= required_runs);
    let minimum = cases
        .values()
        .map(|values| {
            let mut counts = BTreeMap::<String, usize>::new();
            for value in values {
                *counts.entry(format!("{value:?}")).or_default() += 1;
            }
            let modal = counts.values().copied().max().unwrap_or(0);
            modal as f64 / values.len() as f64 * 100.0
        })
        .fold(100.0_f64, f64::min);
    (minimum, complete)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::model::*;

    fn report(repeats: usize) -> RunReport {
        let mut results = Vec::new();
        for repeat in 1..=repeats {
            results.push(CaseResult {
                case_id: "wrong-route".into(),
                repeat,
                platform: Platform::Ios,
                classification: Classification::DestinationMismatch,
                expected: ObservedDestination {
                    target: "app".into(),
                    destination: Some("product/42".into()),
                },
                actual: ObservedDestination {
                    target: "app".into(),
                    destination: Some("home".into()),
                },
                duration_ms: 10,
                starting_state: StartingState::default(),
                source: SourceSpec::default(),
                evidence: Evidence {
                    screenshot: Some("s.png".into()),
                    logs: "l".into(),
                    commands: "c".into(),
                    sha256: BTreeMap::new(),
                    complete_for_failure: true,
                },
                replay: "deeplink-lab replay".into(),
                runtime_failure_class: Some("wrong_route".into()),
                static_preflight_passed: true,
                notes: vec![],
            });
        }
        RunReport {
            schema_version: 1,
            tool_version: "0".into(),
            run_id: "x".into(),
            generated_at: Utc::now(),
            spec: "x.yml".into(),
            reference_setup: None,
            preflight: PreflightReport {
                schema_version: 1,
                spec_version: 1,
                valid: true,
                errors: 0,
                warnings: 0,
                issues: vec![],
            },
            summary: RunSummary {
                total: results.len(),
                passed: 0,
                failed: results.len(),
                unavailable: 0,
                duration_ms: 100,
                platforms: vec![Platform::Ios],
                repeats,
            },
            results,
        }
    }

    #[test]
    fn repeatability_requires_twenty_samples() {
        let base = report(1);
        let repeated = report(20);
        let gates = evaluate(&base, Some(&repeated), &GateConfig::default(), None);
        assert_eq!(gates.repeatability.status, GateStatus::Passed);
    }
}
