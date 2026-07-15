use schemars::schema_for;

use crate::gates::GateReport;
use crate::model::{LabSpec, RunReport};

pub fn spec_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(LabSpec)).expect("schema serialization")
}

pub fn run_report_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(RunReport)).expect("schema serialization")
}

pub fn gate_report_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(GateReport)).expect("schema serialization")
}
