use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabSpec {
    pub version: u32,
    #[serde(default)]
    pub metadata: SpecMetadata,
    pub apps: BTreeMap<String, AppSpec>,
    #[serde(default)]
    pub associations: Vec<AssociationSpec>,
    pub cases: Vec<CaseSpec>,
    #[serde(default)]
    pub gates: GateConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub reference_setup: Option<String>,
    #[serde(default)]
    pub integration_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSpec {
    pub platform: Platform,
    pub app_id: String,
    pub artifact: PathBuf,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub entitlements: Option<PathBuf>,
    #[serde(default)]
    pub info_plist: Option<PathBuf>,
    #[serde(default)]
    pub manifest: Option<PathBuf>,
    #[serde(default)]
    pub cert_fingerprints: Vec<String>,
    #[serde(default)]
    pub launch_activity: Option<String>,
    #[serde(default)]
    pub log_process: Option<String>,
    pub probe: ProbeSpec,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Ios,
    Android,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ios => f.write_str("ios"),
            Self::Android => f.write_str("android"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeSpec {
    IosAppDataFile { path: PathBuf },
    AndroidRunAsFile { path: PathBuf },
    LogRegex { pattern: String },
    MaestroVisibleText { template: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssociationSpec {
    pub domain: String,
    #[serde(default)]
    pub aasa: Option<PathBuf>,
    #[serde(default)]
    pub assetlinks: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaseSpec {
    pub id: String,
    pub app: String,
    pub link: String,
    pub expected: ExpectedOutcome,
    #[serde(default)]
    pub state: StartingState,
    #[serde(default)]
    pub source: SourceSpec,
    #[serde(default)]
    pub runtime_failure_class: Option<String>,
    #[serde(default = "default_settle_ms")]
    pub settle_ms: u64,
}

fn default_settle_ms() -> u64 {
    400
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedOutcome {
    pub target: ExpectedTarget,
    #[serde(default)]
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedTarget {
    App,
    Browser,
    Unhandled,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartingState {
    #[serde(default)]
    pub install: InstallState,
    #[serde(default)]
    pub launch: LaunchState,
    #[serde(default)]
    pub verification: VerificationState,
    #[serde(default)]
    pub user_default: UserDefaultState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstallState {
    #[default]
    Installed,
    Uninstalled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LaunchState {
    #[default]
    Cold,
    Warm,
    Background,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    #[default]
    Preserve,
    Reset,
    Reverify,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserDefaultState {
    #[default]
    Preserve,
    Reset,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSpec {
    #[serde(default)]
    pub context: SourceContext,
    #[serde(default)]
    pub page_url: Option<String>,
    #[serde(default)]
    pub tap_text: Option<String>,
}

impl Default for SourceSpec {
    fn default() -> Self {
        Self {
            context: SourceContext::Direct,
            page_url: None,
            tap_text: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceContext {
    #[default]
    Direct,
    Safari,
    Chrome,
    ControlledPage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GateConfig {
    #[serde(default = "default_novel_failures")]
    pub novel_failure_classes: usize,
    #[serde(default = "default_repeat_runs")]
    pub repeat_runs: usize,
    #[serde(default = "default_repeatability")]
    pub repeatability_percent: f64,
    #[serde(default = "default_case_count")]
    pub representative_case_count: usize,
    #[serde(default = "default_platform_count")]
    pub representative_platform_count: usize,
    #[serde(default = "default_duration_ms")]
    pub max_duration_ms: u64,
    #[serde(default = "default_integration_seconds")]
    pub max_integration_seconds: u64,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            novel_failure_classes: default_novel_failures(),
            repeat_runs: default_repeat_runs(),
            repeatability_percent: default_repeatability(),
            representative_case_count: default_case_count(),
            representative_platform_count: default_platform_count(),
            max_duration_ms: default_duration_ms(),
            max_integration_seconds: default_integration_seconds(),
        }
    }
}

fn default_novel_failures() -> usize {
    5
}
fn default_repeat_runs() -> usize {
    20
}
fn default_repeatability() -> f64 {
    95.0
}
fn default_case_count() -> usize {
    20
}
fn default_platform_count() -> usize {
    2
}
fn default_duration_ms() -> u64 {
    600_000
}
fn default_integration_seconds() -> u64 {
    1_800
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreflightIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub schema_version: u32,
    pub spec_version: u32,
    pub valid: bool,
    pub errors: usize,
    pub warnings: usize,
    pub issues: Vec<PreflightIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub schema_version: u32,
    pub tool_version: String,
    pub run_id: String,
    pub generated_at: DateTime<Utc>,
    pub spec: String,
    pub reference_setup: Option<String>,
    pub preflight: PreflightReport,
    pub summary: RunSummary,
    pub results: Vec<CaseResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub unavailable: usize,
    pub duration_ms: u64,
    pub platforms: Vec<Platform>,
    pub repeats: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CaseResult {
    pub case_id: String,
    pub repeat: usize,
    pub platform: Platform,
    pub classification: Classification,
    pub expected: ObservedDestination,
    pub actual: ObservedDestination,
    pub duration_ms: u64,
    pub starting_state: StartingState,
    pub source: SourceSpec,
    pub evidence: Evidence,
    pub replay: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_failure_class: Option<String>,
    pub static_preflight_passed: bool,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Passed,
    DestinationMismatch,
    AppNotOpened,
    FallbackMismatch,
    OpenFailed,
    ObservationUnavailable,
    InfrastructureError,
}

impl Classification {
    pub fn is_failure(self) -> bool {
        matches!(
            self,
            Self::DestinationMismatch
                | Self::AppNotOpened
                | Self::FallbackMismatch
                | Self::OpenFailed
        )
    }

    pub fn is_unavailable(self) -> bool {
        matches!(
            self,
            Self::ObservationUnavailable | Self::InfrastructureError
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObservedDestination {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    pub logs: String,
    pub commands: String,
    pub sha256: BTreeMap<String, String>,
    pub complete_for_failure: bool,
}
