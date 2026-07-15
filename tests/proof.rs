use std::fs;
use std::path::{Component, Path};

use deeplink_lab::gates;
use deeplink_lab::{LoadedSpec, RunReport};
use sha2::{Digest, Sha256};

fn load_report(path: &str) -> RunReport {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[test]
fn committed_hostile_gates_pass() {
    let loaded = LoadedSpec::load("deeplinklab.yml").unwrap();
    let representative = load_report("proof/reference/report.json");
    let repeatability = load_report("proof/repeatability/report.json");
    let report = gates::evaluate(
        &representative,
        Some(&repeatability),
        &loaded.spec.gates,
        loaded.spec.metadata.integration_seconds,
    );

    assert!(report.all_verified_gates_passed, "{report:#?}");
    assert_eq!(representative.summary.total, 20);
    assert_eq!(representative.summary.unavailable, 0);
    assert_eq!(repeatability.summary.repeats, 20);
}

#[test]
fn committed_evidence_hashes_match() {
    for report_path in [
        "proof/reference/report.json",
        "proof/repeatability/report.json",
        "proof/integration/report.json",
    ] {
        let root = Path::new(report_path).parent().unwrap();
        let report = load_report(report_path);
        for result in report.results {
            for (relative, expected) in result.evidence.sha256 {
                let relative = Path::new(&relative);
                assert!(!relative.is_absolute());
                assert!(!relative
                    .components()
                    .any(|part| matches!(part, Component::ParentDir)));
                let actual = sha256_hex(fs::read(root.join(relative)).unwrap());
                assert_eq!(actual, expected, "hash mismatch for {}", relative.display());
            }
        }
    }
}

#[test]
fn integration_proof_matches_current_runner() {
    let evidence: serde_json::Value =
        serde_json::from_slice(&fs::read("proof/integration/integration-evidence.json").unwrap())
            .unwrap();
    let runtime = sha256_hex(fs::read("src/runtime.rs").unwrap());

    assert_eq!(evidence["runnerCodeChanged"], false);
    assert_eq!(evidence["classification"], "passed");
    assert_eq!(evidence["actualDestination"], "product/7");
    assert_eq!(evidence["runtimeSha256"], runtime);
    assert!(evidence["durationSeconds"].as_u64().unwrap() < 1_800);
}
