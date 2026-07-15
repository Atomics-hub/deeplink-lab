use std::fs;

use deeplink_lab::schema;

#[test]
fn committed_schemas_match_the_code() {
    let fixtures = [
        ("schemas/deeplinklab.schema.json", schema::spec_schema()),
        (
            "schemas/run-report.schema.json",
            schema::run_report_schema(),
        ),
        (
            "schemas/gate-report.schema.json",
            schema::gate_report_schema(),
        ),
    ];
    for (path, expected) in fixtures {
        let actual: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(actual, expected, "schema drift in {path}");
    }
}
