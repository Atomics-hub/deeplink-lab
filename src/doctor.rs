use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::runtime::{find_adb, find_on_path};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub static_preflight: bool,
    pub ios_simulator: ToolStatus,
    pub android_emulator: ToolStatus,
    pub maestro: ToolStatus,
    pub appium: ToolStatus,
    pub boundaries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub available: bool,
    pub command: String,
    pub detail: String,
}

pub fn inspect() -> DoctorReport {
    let ios = command_status("xcrun", &["simctl", "list", "devices", "available"]);
    let adb = find_adb();
    let android = if adb.exists() || find_on_path("adb").is_some() {
        command_status(adb.to_string_lossy().as_ref(), &["devices", "-l"])
    } else {
        ToolStatus {
            available: false,
            command: "adb".into(),
            detail: "adb not found; set ANDROID_HOME or ANDROID_SDK_ROOT".into(),
        }
    };
    DoctorReport {
        static_preflight: true,
        ios_simulator: ios,
        android_emulator: android,
        maestro: optional_status("maestro", &["--version"], "required for Safari/Chrome/controlled-page taps and generic visible-text probes"),
        appium: optional_status("appium", &["--version"], "not required by the built-in local lanes; supported as an external orchestration alternative"),
        boundaries: vec![
            "Simulator/emulator observations do not prove physical-device behavior".into(),
            "No authenticated consumer-app automation".into(),
            "No deferred App Store or Play Store attribution proof".into(),
            "No managed real-device cloud in v1".into(),
        ],
    }
}

fn optional_status(command: &str, args: &[&str], unavailable: &str) -> ToolStatus {
    if find_on_path(command).is_some() {
        command_status(command, args)
    } else {
        ToolStatus {
            available: false,
            command: command.into(),
            detail: unavailable.into(),
        }
    }
}

fn command_status(command: &str, args: &[&str]) -> ToolStatus {
    match Command::new(command).args(args).output() {
        Ok(output) => ToolStatus {
            available: output.status.success(),
            command: format!(
                "{} {}",
                Path::new(command)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or(command),
                args.join(" ")
            ),
            detail: String::from_utf8_lossy(if output.stdout.is_empty() {
                &output.stderr
            } else {
                &output.stdout
            })
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n"),
        },
        Err(error) => ToolStatus {
            available: false,
            command: command.into(),
            detail: error.to_string(),
        },
    }
}
