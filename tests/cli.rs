use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn validate_prints_machine_readable_result() {
    let mut command = Command::cargo_bin("deeplink-lab").unwrap();
    command
        .args(["validate", "--spec", "deeplinklab.yml", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"valid\": true"));
}

#[test]
fn broken_static_fixture_is_nonzero() {
    let mut command = Command::cargo_bin("deeplink-lab").unwrap();
    command
        .args(["validate", "--spec", "examples/broken-static.yml"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("ios.aasa_app_id_mismatch"));
}

#[test]
fn init_refuses_to_overwrite() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("contract.yml");
    Command::cargo_bin("deeplink-lab")
        .unwrap()
        .args(["init", "--output", output.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("deeplink-lab")
        .unwrap()
        .args(["init", "--output", output.to_str().unwrap()])
        .assert()
        .failure();
}
