use deeplink_lab::{preflight, LoadedSpec, Severity};

#[test]
fn reference_contract_is_statically_valid() {
    let loaded = LoadedSpec::load("deeplinklab.yml").unwrap();
    let report = preflight::validate(&loaded);
    let errors = report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .map(|issue| format!("{}: {}", issue.code, issue.message))
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn broken_associations_fail_closed() {
    let loaded = LoadedSpec::load("examples/broken-static.yml").unwrap();
    let report = preflight::validate(&loaded);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"ios.aasa_app_id_mismatch"));
    assert!(codes.contains(&"android.assetlinks_package_mismatch"));
    assert!(codes.contains(&"assetlinks.invalid_fingerprint"));
    assert!(!report.valid);
}
