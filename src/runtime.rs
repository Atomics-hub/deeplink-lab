use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use regex::Regex;
use sha2::{Digest, Sha256};

use crate::model::{
    AppSpec, CaseResult, CaseSpec, Classification, Evidence, ExpectedTarget, InstallState, LabSpec,
    LaunchState, ObservedDestination, Platform, ProbeSpec, RunReport, RunSummary, SourceContext,
    UserDefaultState, VerificationState,
};
use crate::preflight;
use crate::report;
use crate::spec::LoadedSpec;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub output: PathBuf,
    pub case_ids: Vec<String>,
    pub platform: Option<Platform>,
    pub repeat: usize,
    pub reset_between_runs: bool,
    pub ios_device: String,
    pub android_device: Option<String>,
    pub allow_preflight_errors: bool,
    pub write_html: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            output: PathBuf::from("outputs/run"),
            case_ids: Vec::new(),
            platform: None,
            repeat: 1,
            reset_between_runs: true,
            ios_device: "booted".to_owned(),
            android_device: None,
            allow_preflight_errors: false,
            write_html: true,
        }
    }
}

#[derive(Debug)]
struct CommandRecorder {
    lines: Vec<String>,
}

impl CommandRecorder {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn run<I, S>(&mut self, program: &str, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_stdout(program, args, true)
    }

    fn run_without_stdout<I, S>(&mut self, program: &str, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_stdout(program, args, false)
    }

    fn run_with_stdout<I, S>(
        &mut self,
        program: &str,
        args: I,
        record_stdout: bool,
    ) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<_> = args
            .into_iter()
            .map(|value| value.as_ref().to_os_string())
            .collect();
        let rendered = std::iter::once(program.to_owned())
            .chain(args.iter().map(|arg| shell_quote(&arg.to_string_lossy())))
            .collect::<Vec<_>>()
            .join(" ");
        self.lines
            .push(format!("$ {}", redact_host_paths(&rendered)));
        let output = Command::new(program)
            .args(&args)
            .output()
            .with_context(|| format!("failed to execute {program}"))?;
        if record_stdout && !output.stdout.is_empty() {
            self.lines.push(redact_host_paths(
                String::from_utf8_lossy(&output.stdout).trim(),
            ));
        }
        if !output.stderr.is_empty() {
            self.lines.push(redact_host_paths(
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
        self.lines
            .push(format!("[exit {}]", output.status.code().unwrap_or(-1)));
        Ok(output)
    }

    fn text(&self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

pub fn run(loaded: &LoadedSpec, options: &RunOptions) -> Result<RunReport> {
    if options.repeat == 0 {
        bail!("--repeat must be at least 1");
    }
    let preflight = preflight::validate(loaded);
    if !preflight.valid && !options.allow_preflight_errors {
        bail!(
            "static preflight failed with {} error(s); use validate for details",
            preflight.errors
        );
    }
    ensure_empty_or_absent(&options.output)?;
    fs::create_dir_all(&options.output)
        .with_context(|| format!("failed to create {}", options.output.display()))?;

    let selected = select_cases(&loaded.spec, options)?;
    if selected.is_empty() {
        bail!("no cases matched the requested filters");
    }

    let started = Instant::now();
    let generated_at = Utc::now();
    let run_id = generated_at.format("%Y%m%dT%H%M%SZ").to_string();
    let mut installed = HashSet::<String>::new();
    let mut results = Vec::with_capacity(selected.len() * options.repeat);

    for repeat in 1..=options.repeat {
        if options.reset_between_runs {
            reset_apps(loaded, &selected, options, &mut installed)?;
        }
        for case in &selected {
            let app = loaded
                .spec
                .apps
                .get(&case.app)
                .ok_or_else(|| anyhow!("unknown app '{}'", case.app))?;
            let result = run_case(loaded, case, app, repeat, options, &mut installed)
                .unwrap_or_else(|error| infrastructure_result(case, app, repeat, loaded, error));
            results.push(result);
        }
    }

    let platforms = results
        .iter()
        .map(|result| result.platform)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let summary = RunSummary {
        total: results.len(),
        passed: results
            .iter()
            .filter(|result| result.classification == Classification::Passed)
            .count(),
        failed: results
            .iter()
            .filter(|result| result.classification.is_failure())
            .count(),
        unavailable: results
            .iter()
            .filter(|result| result.classification.is_unavailable())
            .count(),
        duration_ms: started.elapsed().as_millis() as u64,
        platforms,
        repeats: options.repeat,
    };
    let report = RunReport {
        schema_version: 1,
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        run_id,
        generated_at,
        spec: loaded.portable_spec_name(),
        reference_setup: loaded.spec.metadata.reference_setup.clone(),
        preflight,
        summary,
        results,
    };
    write_report(&report, &options.output, options.write_html)?;
    Ok(report)
}

fn select_cases<'a>(spec: &'a LabSpec, options: &RunOptions) -> Result<Vec<&'a CaseSpec>> {
    let requested = options.case_ids.iter().collect::<BTreeSet<_>>();
    if !requested.is_empty() {
        for id in &requested {
            if !spec
                .cases
                .iter()
                .any(|case| case.id.as_str() == id.as_str())
            {
                bail!("unknown case id '{id}'");
            }
        }
    }
    Ok(spec
        .cases
        .iter()
        .filter(|case| requested.is_empty() || requested.contains(&case.id))
        .filter(|case| {
            let platform = spec.apps.get(&case.app).map(|app| app.platform);
            options.platform.is_none() || platform == options.platform
        })
        .collect())
}

fn reset_apps(
    loaded: &LoadedSpec,
    cases: &[&CaseSpec],
    options: &RunOptions,
    installed: &mut HashSet<String>,
) -> Result<()> {
    let app_names = cases.iter().map(|case| &case.app).collect::<BTreeSet<_>>();
    for name in app_names {
        let app = loaded
            .spec
            .apps
            .get(name)
            .ok_or_else(|| anyhow!("unknown app '{name}'"))?;
        let mut recorder = CommandRecorder::new();
        uninstall(app, options, &mut recorder, true)?;
        installed.remove(name);
    }
    Ok(())
}

fn run_case(
    loaded: &LoadedSpec,
    case: &CaseSpec,
    app: &AppSpec,
    repeat: usize,
    options: &RunOptions,
    installed: &mut HashSet<String>,
) -> Result<CaseResult> {
    let started = Instant::now();
    let case_dir_name = format!(
        "{:02}-{}-r{:02}",
        loaded
            .spec
            .cases
            .iter()
            .position(|item| item.id == case.id)
            .unwrap_or(0)
            + 1,
        case.id,
        repeat
    );
    let evidence_dir = options.output.join("cases").join(case_dir_name);
    fs::create_dir_all(&evidence_dir)?;
    let mut recorder = CommandRecorder::new();
    let mut notes = Vec::new();

    match case.state.install {
        InstallState::Installed => {
            if !installed.contains(&case.app) {
                install(loaded, app, options, &mut recorder)?;
                installed.insert(case.app.clone());
            }
        }
        InstallState::Uninstalled => {
            uninstall(app, options, &mut recorder, true)?;
            installed.remove(&case.app);
        }
    }

    apply_verification_state(app, case, options, &mut recorder, &mut notes)?;
    if case.state.install == InstallState::Installed {
        clear_probe(loaded, app, options, &mut recorder)?;
        clear_logs(app, options, &mut recorder)?;
        prepare_lifecycle(app, case, options, &mut recorder)?;
    }

    let open_output = open_link(case, app, options, &evidence_dir, &mut recorder)?;
    thread::sleep(Duration::from_millis(case.settle_ms));

    let screenshot_rel = capture_screenshot(app, options, &evidence_dir, &mut recorder).ok();
    let logs = capture_logs(app, options, &mut recorder)
        .unwrap_or_else(|error| format!("DeepLink Lab could not capture platform logs: {error}\n"));
    let logs_path = evidence_dir.join("device.log");
    fs::write(&logs_path, &logs)?;

    let actual = observe_destination(
        loaded,
        case,
        app,
        options,
        &logs,
        &open_output,
        &mut recorder,
    )?;
    let expected = ObservedDestination {
        target: expected_target_name(case.expected.target).to_owned(),
        destination: case.expected.destination.clone(),
    };
    let classification = classify(&expected, &actual, open_output.status.success());
    let commands_path = evidence_dir.join("commands.log");
    fs::write(&commands_path, recorder.text())?;

    let screenshot = screenshot_rel.map(|path| path_to_report(&path, &options.output));
    let logs_report_path = path_to_report(&logs_path, &options.output);
    let commands_report_path = path_to_report(&commands_path, &options.output);
    let mut sha256 = BTreeMap::new();
    if let Some(path) = &screenshot {
        sha256.insert(path.clone(), digest_file(&options.output.join(path))?);
    }
    sha256.insert(logs_report_path.clone(), digest_file(&logs_path)?);
    sha256.insert(commands_report_path.clone(), digest_file(&commands_path)?);

    let replay = format!(
        "deeplink-lab replay --spec {} --case {}",
        shell_quote(&loaded.portable_spec_name()),
        shell_quote(&case.id)
    );
    let complete_for_failure = classification == Classification::Passed
        || (screenshot.is_some()
            && !logs.trim().is_empty()
            && actual.destination.is_some()
            && !replay.is_empty());
    Ok(CaseResult {
        case_id: case.id.clone(),
        repeat,
        platform: app.platform,
        classification,
        expected,
        actual,
        duration_ms: started.elapsed().as_millis() as u64,
        starting_state: case.state.clone(),
        source: case.source.clone(),
        evidence: Evidence {
            screenshot,
            logs: logs_report_path,
            commands: commands_report_path,
            sha256,
            complete_for_failure,
        },
        replay,
        runtime_failure_class: if classification.is_failure() {
            case.runtime_failure_class.clone()
        } else {
            None
        },
        static_preflight_passed: true,
        notes,
    })
}

fn infrastructure_result(
    case: &CaseSpec,
    app: &AppSpec,
    repeat: usize,
    loaded: &LoadedSpec,
    error: anyhow::Error,
) -> CaseResult {
    CaseResult {
        case_id: case.id.clone(),
        repeat,
        platform: app.platform,
        classification: Classification::InfrastructureError,
        expected: ObservedDestination {
            target: expected_target_name(case.expected.target).to_owned(),
            destination: case.expected.destination.clone(),
        },
        actual: ObservedDestination {
            target: "unobserved".to_owned(),
            destination: None,
        },
        duration_ms: 0,
        starting_state: case.state.clone(),
        source: case.source.clone(),
        evidence: Evidence {
            screenshot: None,
            logs: String::new(),
            commands: String::new(),
            sha256: BTreeMap::new(),
            complete_for_failure: false,
        },
        replay: format!(
            "deeplink-lab replay --spec {} --case {}",
            shell_quote(&loaded.portable_spec_name()),
            shell_quote(&case.id)
        ),
        runtime_failure_class: None,
        static_preflight_passed: true,
        notes: vec![format!(
            "infrastructure error: {}",
            redact_host_paths(&format!("{error:#}"))
        )],
    }
}

fn install(
    loaded: &LoadedSpec,
    app: &AppSpec,
    options: &RunOptions,
    recorder: &mut CommandRecorder,
) -> Result<()> {
    let artifact = loaded.resolve(&app.artifact);
    if !artifact.exists() {
        bail!("artifact does not exist: {}", app.artifact.display());
    }
    let output = match app.platform {
        Platform::Ios => recorder.run(
            "xcrun",
            [
                "simctl".to_owned(),
                "install".to_owned(),
                options.ios_device.clone(),
                artifact.to_string_lossy().into_owned(),
            ],
        )?,
        Platform::Android => {
            let (adb, prefix) = adb_command(options);
            let mut args = prefix;
            args.extend([
                "install".to_owned(),
                "--no-incremental".to_owned(),
                "-r".to_owned(),
                "-t".to_owned(),
                artifact.to_string_lossy().into_owned(),
            ]);
            recorder.run(&adb, args)?
        }
    };
    ensure_success(output, "app install")
}

fn uninstall(
    app: &AppSpec,
    options: &RunOptions,
    recorder: &mut CommandRecorder,
    ignore_missing: bool,
) -> Result<()> {
    let output = match app.platform {
        Platform::Ios => recorder.run(
            "xcrun",
            [
                "simctl".to_owned(),
                "uninstall".to_owned(),
                options.ios_device.clone(),
                app.app_id.clone(),
            ],
        )?,
        Platform::Android => {
            let (adb, mut args) = adb_command(options);
            args.extend(["uninstall".to_owned(), app.app_id.clone()]);
            recorder.run(&adb, args)?
        }
    };
    if output.status.success() || ignore_missing {
        Ok(())
    } else {
        ensure_success(output, "app uninstall")
    }
}

fn clear_probe(
    loaded: &LoadedSpec,
    app: &AppSpec,
    options: &RunOptions,
    recorder: &mut CommandRecorder,
) -> Result<()> {
    match &app.probe {
        ProbeSpec::IosAppDataFile { path } => {
            validate_relative_probe(path)?;
            let output = recorder.run(
                "xcrun",
                [
                    "simctl".to_owned(),
                    "get_app_container".to_owned(),
                    options.ios_device.clone(),
                    app.app_id.clone(),
                    "data".to_owned(),
                ],
            )?;
            ensure_success_ref(&output, "get iOS app data container")?;
            let container = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let destination = Path::new(&container).join(path);
            if destination.starts_with(&container) && destination.exists() {
                fs::remove_file(destination)?;
            }
        }
        ProbeSpec::AndroidRunAsFile { path } => {
            validate_relative_probe(path)?;
            let (adb, mut args) = adb_command(options);
            args.extend([
                "shell".to_owned(),
                "run-as".to_owned(),
                app.app_id.clone(),
                "rm".to_owned(),
                "-f".to_owned(),
                path.to_string_lossy().into_owned(),
            ]);
            let _ = recorder.run(&adb, args)?;
        }
        ProbeSpec::LogRegex { .. } | ProbeSpec::MaestroVisibleText { .. } => {}
    }
    let _ = loaded;
    Ok(())
}

fn prepare_lifecycle(
    app: &AppSpec,
    case: &CaseSpec,
    options: &RunOptions,
    recorder: &mut CommandRecorder,
) -> Result<()> {
    terminate(app, options, recorder)?;
    match case.state.launch {
        LaunchState::Cold => Ok(()),
        LaunchState::Warm => launch(app, options, recorder),
        LaunchState::Background => {
            launch(app, options, recorder)?;
            thread::sleep(Duration::from_millis(200));
            match app.platform {
                Platform::Ios => {
                    let output = recorder.run(
                        "xcrun",
                        [
                            "simctl".to_owned(),
                            "launch".to_owned(),
                            options.ios_device.clone(),
                            "com.apple.mobilesafari".to_owned(),
                        ],
                    )?;
                    ensure_success(output, "send iOS app to background")?;
                }
                Platform::Android => {
                    let (adb, mut args) = adb_command(options);
                    args.extend([
                        "shell".to_owned(),
                        "input".to_owned(),
                        "keyevent".to_owned(),
                        "KEYCODE_HOME".to_owned(),
                    ]);
                    let output = recorder.run(&adb, args)?;
                    ensure_success(output, "send Android app to background")?;
                }
            }
            thread::sleep(Duration::from_millis(200));
            Ok(())
        }
    }
}

fn terminate(app: &AppSpec, options: &RunOptions, recorder: &mut CommandRecorder) -> Result<()> {
    match app.platform {
        Platform::Ios => {
            let _ = recorder.run(
                "xcrun",
                [
                    "simctl".to_owned(),
                    "terminate".to_owned(),
                    options.ios_device.clone(),
                    app.app_id.clone(),
                ],
            )?;
        }
        Platform::Android => {
            let (adb, mut args) = adb_command(options);
            args.extend([
                "shell".to_owned(),
                "am".to_owned(),
                "force-stop".to_owned(),
                app.app_id.clone(),
            ]);
            let output = recorder.run(&adb, args)?;
            ensure_success(output, "force-stop Android app")?;
        }
    }
    Ok(())
}

fn launch(app: &AppSpec, options: &RunOptions, recorder: &mut CommandRecorder) -> Result<()> {
    let output = match app.platform {
        Platform::Ios => recorder.run(
            "xcrun",
            [
                "simctl".to_owned(),
                "launch".to_owned(),
                options.ios_device.clone(),
                app.app_id.clone(),
            ],
        )?,
        Platform::Android => {
            let activity = app
                .launch_activity
                .as_ref()
                .ok_or_else(|| anyhow!("Android warm/background state requires launchActivity"))?;
            let (adb, mut args) = adb_command(options);
            args.extend([
                "shell".to_owned(),
                "am".to_owned(),
                "start".to_owned(),
                "-W".to_owned(),
                "-n".to_owned(),
                activity.clone(),
            ]);
            recorder.run(&adb, args)?
        }
    };
    ensure_success(output, "launch app")
}

fn apply_verification_state(
    app: &AppSpec,
    case: &CaseSpec,
    options: &RunOptions,
    recorder: &mut CommandRecorder,
    notes: &mut Vec<String>,
) -> Result<()> {
    if app.platform == Platform::Ios {
        if case.state.verification != VerificationState::Preserve
            || case.state.user_default != UserDefaultState::Preserve
        {
            notes.push("iOS Simulator has no public command to invalidate the Apple AASA CDN or prove a physical-device user override; reinstall is only a local reset approximation".to_owned());
        }
        return Ok(());
    }
    let (adb, prefix) = adb_command(options);
    match case.state.verification {
        VerificationState::Preserve => {}
        VerificationState::Reset => {
            let mut args = prefix.clone();
            args.extend([
                "shell".to_owned(),
                "pm".to_owned(),
                "set-app-links".to_owned(),
                "--package".to_owned(),
                app.app_id.clone(),
                "0".to_owned(),
                "all".to_owned(),
            ]);
            let output = recorder.run(&adb, args)?;
            ensure_success(output, "reset Android App Links verification")?;
        }
        VerificationState::Reverify => {
            let mut args = prefix.clone();
            args.extend([
                "shell".to_owned(),
                "pm".to_owned(),
                "verify-app-links".to_owned(),
                "--re-verify".to_owned(),
                app.app_id.clone(),
            ]);
            let output = recorder.run(&adb, args)?;
            ensure_success(output, "request Android App Links re-verification")?;
        }
    }
    if case.state.user_default == UserDefaultState::Reset {
        let mut args = prefix;
        args.extend([
            "shell".to_owned(),
            "pm".to_owned(),
            "set-app-links-user-selection".to_owned(),
            "--user".to_owned(),
            "0".to_owned(),
            "--package".to_owned(),
            app.app_id.clone(),
            "false".to_owned(),
            "all".to_owned(),
        ]);
        let output = recorder.run(&adb, args)?;
        ensure_success(output, "reset Android user link selection")?;
    }
    Ok(())
}

fn open_link(
    case: &CaseSpec,
    app: &AppSpec,
    options: &RunOptions,
    evidence_dir: &Path,
    recorder: &mut CommandRecorder,
) -> Result<Output> {
    if case.source.context != SourceContext::Direct {
        return run_maestro_source(case, app, options, evidence_dir, recorder);
    }
    match app.platform {
        Platform::Ios => recorder.run(
            "xcrun",
            [
                "simctl".to_owned(),
                "openurl".to_owned(),
                options.ios_device.clone(),
                case.link.clone(),
            ],
        ),
        Platform::Android => {
            let (adb, mut args) = adb_command(options);
            args.extend([
                "shell".to_owned(),
                "am".to_owned(),
                "start".to_owned(),
                "-W".to_owned(),
                "-a".to_owned(),
                "android.intent.action.VIEW".to_owned(),
                "-c".to_owned(),
                "android.intent.category.BROWSABLE".to_owned(),
                "-d".to_owned(),
                case.link.clone(),
            ]);
            recorder.run(&adb, args)
        }
    }
}

fn run_maestro_source(
    case: &CaseSpec,
    app: &AppSpec,
    options: &RunOptions,
    evidence_dir: &Path,
    recorder: &mut CommandRecorder,
) -> Result<Output> {
    let page = case
        .source
        .page_url
        .as_deref()
        .ok_or_else(|| anyhow!("pageUrl is required"))?;
    let tap = case
        .source
        .tap_text
        .as_deref()
        .ok_or_else(|| anyhow!("tapText is required"))?;
    let browser_id = match (app.platform, case.source.context) {
        (Platform::Ios, SourceContext::Chrome) => "com.google.chrome.ios",
        (Platform::Ios, _) => "com.apple.mobilesafari",
        (Platform::Android, _) => "com.android.chrome",
    };
    let flow = format!(
        "appId: {}\n---\n- openLink:\n    link: {}\n- tapOn: {}\n",
        serde_json::to_string(browser_id)?,
        serde_json::to_string(page)?,
        serde_json::to_string(tap)?
    );
    let flow_path = evidence_dir.join("maestro-source.yml");
    fs::write(&flow_path, flow)?;
    let device = match app.platform {
        Platform::Ios => options.ios_device.clone(),
        Platform::Android => options.android_device.clone().unwrap_or_default(),
    };
    let mut args = Vec::new();
    if !device.is_empty() && device != "booted" {
        args.extend(["--device".to_owned(), device]);
    }
    args.extend(["test".to_owned(), flow_path.to_string_lossy().into_owned()]);
    recorder.run("maestro", args)
}

fn capture_screenshot(
    app: &AppSpec,
    options: &RunOptions,
    evidence_dir: &Path,
    recorder: &mut CommandRecorder,
) -> Result<PathBuf> {
    let path = evidence_dir.join(match app.platform {
        Platform::Ios => "screenshot.jpg",
        Platform::Android => "screenshot.png",
    });
    match app.platform {
        Platform::Ios => {
            let output = recorder.run(
                "xcrun",
                [
                    "simctl".to_owned(),
                    "io".to_owned(),
                    options.ios_device.clone(),
                    "screenshot".to_owned(),
                    "--type=jpeg".to_owned(),
                    path.to_string_lossy().into_owned(),
                ],
            )?;
            ensure_success(output, "capture iOS screenshot")?;
        }
        Platform::Android => {
            let (adb, mut args) = adb_command(options);
            args.extend([
                "exec-out".to_owned(),
                "screencap".to_owned(),
                "-p".to_owned(),
            ]);
            let output = recorder.run_without_stdout(&adb, args)?;
            ensure_success_ref(&output, "capture Android screenshot")?;
            fs::write(&path, output.stdout)?;
        }
    }
    if !path.exists() || path.metadata()?.len() == 0 {
        bail!("screenshot command produced no file");
    }
    Ok(path)
}

fn clear_logs(app: &AppSpec, options: &RunOptions, recorder: &mut CommandRecorder) -> Result<()> {
    if app.platform == Platform::Android {
        let (adb, mut args) = adb_command(options);
        args.extend(["logcat".to_owned(), "-c".to_owned()]);
        let output = recorder.run(&adb, args)?;
        ensure_success(output, "clear Android logs")?;
    }
    Ok(())
}

fn capture_logs(
    app: &AppSpec,
    options: &RunOptions,
    recorder: &mut CommandRecorder,
) -> Result<String> {
    let output = match app.platform {
        Platform::Ios => {
            let process = app.log_process.as_deref().unwrap_or(&app.app_id);
            recorder.run_without_stdout(
                "xcrun",
                [
                    "simctl".to_owned(),
                    "spawn".to_owned(),
                    options.ios_device.clone(),
                    "log".to_owned(),
                    "show".to_owned(),
                    "--style".to_owned(),
                    "compact".to_owned(),
                    "--last".to_owned(),
                    "4s".to_owned(),
                    "--predicate".to_owned(),
                    format!("process == \"{process}\""),
                ],
            )?
        }
        Platform::Android => {
            let (adb, mut args) = adb_command(options);
            args.extend([
                "logcat".to_owned(),
                "-d".to_owned(),
                "-v".to_owned(),
                "threadtime".to_owned(),
            ]);
            recorder.run_without_stdout(&adb, args)?
        }
    };
    ensure_success_ref(&output, "capture device logs")?;
    let raw = String::from_utf8_lossy(&output.stdout);
    let filtered = match app.platform {
        Platform::Ios => raw.into_owned(),
        Platform::Android => raw
            .lines()
            .filter(|line| line.contains("DeepLinkFixture") || line.contains(&app.app_id))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    Ok(if filtered.trim().is_empty() {
        "No matching app log lines were emitted during the capture window.\n".to_owned()
    } else {
        format!("{}\n", filtered.trim())
    })
}

fn observe_destination(
    loaded: &LoadedSpec,
    case: &CaseSpec,
    app: &AppSpec,
    options: &RunOptions,
    logs: &str,
    open_output: &Output,
    recorder: &mut CommandRecorder,
) -> Result<ObservedDestination> {
    if case.state.install == InstallState::Uninstalled
        && (!open_output.status.success() || open_indicates_unhandled(open_output))
    {
        return Ok(ObservedDestination {
            target: "unhandled".to_owned(),
            destination: Some("no_handler".to_owned()),
        });
    }
    let destination = match &app.probe {
        ProbeSpec::IosAppDataFile { path } => {
            validate_relative_probe(path)?;
            let output = recorder.run(
                "xcrun",
                [
                    "simctl".to_owned(),
                    "get_app_container".to_owned(),
                    options.ios_device.clone(),
                    app.app_id.clone(),
                    "data".to_owned(),
                ],
            )?;
            if output.status.success() {
                let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                fs::read_to_string(Path::new(&root).join(path))
                    .ok()
                    .map(|value| value.trim().to_owned())
            } else {
                None
            }
        }
        ProbeSpec::AndroidRunAsFile { path } => {
            validate_relative_probe(path)?;
            let (adb, mut args) = adb_command(options);
            args.extend([
                "shell".to_owned(),
                "run-as".to_owned(),
                app.app_id.clone(),
                "cat".to_owned(),
                path.to_string_lossy().into_owned(),
            ]);
            let output = recorder.run(&adb, args)?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                .filter(|value| !value.is_empty())
        }
        ProbeSpec::LogRegex { pattern } => Regex::new(pattern)?
            .captures(logs)
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned()),
        ProbeSpec::MaestroVisibleText { template } => {
            let Some(expected) = &case.expected.destination else {
                return Ok(unobserved());
            };
            let text = template.replace("{destination}", expected);
            let flow_path = loaded.root.join(".deeplink-lab-observe.yml");
            let flow = format!(
                "appId: {}\n---\n- assertVisible: {}\n",
                app.app_id,
                serde_yaml::to_string(&text)?.trim()
            );
            fs::write(&flow_path, flow)?;
            let output = recorder.run(
                "maestro",
                ["test".to_owned(), flow_path.to_string_lossy().into_owned()],
            )?;
            let _ = fs::remove_file(flow_path);
            output.status.success().then(|| expected.clone())
        }
    };
    if let Some(destination) = destination {
        return Ok(ObservedDestination {
            target: "app".to_owned(),
            destination: Some(destination),
        });
    }
    if app.platform == Platform::Android && open_output.status.success() {
        let (adb, mut args) = adb_command(options);
        args.extend([
            "shell".to_owned(),
            "dumpsys".to_owned(),
            "activity".to_owned(),
            "activities".to_owned(),
        ]);
        if let Ok(output) = recorder.run(&adb, args) {
            let state = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if state.contains("com.android.chrome") {
                return Ok(ObservedDestination {
                    target: "browser".to_owned(),
                    destination: None,
                });
            }
        }
    }
    Ok(unobserved())
}

pub fn classify(
    expected: &ObservedDestination,
    actual: &ObservedDestination,
    open_succeeded: bool,
) -> Classification {
    if expected.target == "unhandled" {
        return if actual.target == "unhandled" || !open_succeeded {
            Classification::Passed
        } else {
            Classification::FallbackMismatch
        };
    }
    if !open_succeeded {
        return Classification::OpenFailed;
    }
    if actual.target == "unobserved" {
        return Classification::ObservationUnavailable;
    }
    if expected.target != actual.target {
        return if expected.target == "app" {
            Classification::AppNotOpened
        } else {
            Classification::FallbackMismatch
        };
    }
    if expected.target == "app" && expected.destination != actual.destination {
        return Classification::DestinationMismatch;
    }
    Classification::Passed
}

fn unobserved() -> ObservedDestination {
    ObservedDestination {
        target: "unobserved".to_owned(),
        destination: None,
    }
}

fn expected_target_name(target: ExpectedTarget) -> &'static str {
    match target {
        ExpectedTarget::App => "app",
        ExpectedTarget::Browser => "browser",
        ExpectedTarget::Unhandled => "unhandled",
    }
}

fn adb_command(options: &RunOptions) -> (String, Vec<String>) {
    let adb = find_adb().to_string_lossy().into_owned();
    let mut args = Vec::new();
    if let Some(device) = &options.android_device {
        args.extend(["-s".to_owned(), device.clone()]);
    }
    (adb, args)
}

pub fn find_adb() -> PathBuf {
    if let Some(path) = find_on_path("adb") {
        return path;
    }
    for variable in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(root) = env::var_os(variable) {
            let candidate = PathBuf::from(root).join("platform-tools/adb");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let candidate = PathBuf::from(home).join("Library/Android/sdk/platform-tools/adb");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("adb")
}

pub fn find_on_path(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|root| root.join(command))
        .find(|candidate| candidate.is_file())
}

fn validate_relative_probe(path: &Path) -> Result<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("probe paths must be relative and cannot contain '..'");
    }
    Ok(())
}

fn ensure_success(output: Output, operation: &str) -> Result<()> {
    ensure_success_ref(&output, operation)
}

fn ensure_success_ref(output: &Output, operation: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "{operation} failed (exit {}): {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn ensure_empty_or_absent(path: &Path) -> Result<()> {
    if path.exists() {
        let mut entries = fs::read_dir(path)?;
        if entries.next().is_some() {
            bail!(
                "output directory already exists and is not empty: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn write_report(run_report: &RunReport, output: &Path, write_html: bool) -> Result<()> {
    let json_path = output.join("report.json");
    fs::write(&json_path, serde_json::to_vec_pretty(run_report)?)?;
    if write_html {
        let html = report::render_html(run_report, output)?;
        fs::write(output.join("report.html"), html)?;
    }
    Ok(())
}

fn open_indicates_unhandled(output: &Output) -> bool {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    [
        "unable to resolve intent",
        "no activity found to handle intent",
        "activity not started, unable to resolve",
        "lsapplicationworkspaceerrordomain",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn redact_host_paths(value: &str) -> String {
    let mut redacted = value.to_owned();
    if let Ok(workspace) = env::current_dir() {
        if let Some(workspace) = workspace.to_str() {
            redacted = redacted.replace(workspace, "$WORKSPACE");
        }
    }
    if let Some(home) = env::var_os("HOME") {
        if let Some(home) = home.to_str() {
            redacted = redacted.replace(home, "$HOME");
        }
    }
    redacted
}

fn path_to_report(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn digest_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-._/:".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_exact_destination() {
        let expected = ObservedDestination {
            target: "app".into(),
            destination: Some("product/42".into()),
        };
        let actual = expected.clone();
        assert_eq!(classify(&expected, &actual, true), Classification::Passed);
    }

    #[test]
    fn detects_wrong_screen_even_when_app_opened() {
        let expected = ObservedDestination {
            target: "app".into(),
            destination: Some("product/42".into()),
        };
        let actual = ObservedDestination {
            target: "app".into(),
            destination: Some("home".into()),
        };
        assert_eq!(
            classify(&expected, &actual, true),
            Classification::DestinationMismatch
        );
    }

    #[test]
    fn does_not_turn_unobserved_state_green() {
        let expected = ObservedDestination {
            target: "app".into(),
            destination: Some("product/42".into()),
        };
        assert_eq!(
            classify(&expected, &unobserved(), true),
            Classification::ObservationUnavailable
        );
    }

    #[test]
    fn recognizes_successful_adb_no_handler_output() {
        let output = Command::new("sh")
            .args([
                "-c",
                "printf 'Error: Activity not started, unable to resolve Intent'",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(open_indicates_unhandled(&output));
    }

    #[test]
    fn redacts_workspace_and_home_paths() {
        let cwd = env::current_dir().unwrap();
        let value = format!("{} {}", cwd.display(), env::var("HOME").unwrap());
        let redacted = redact_host_paths(&value);
        assert!(redacted.contains("$WORKSPACE"));
        assert!(redacted.contains("$HOME"));
        assert!(!redacted.contains(&cwd.to_string_lossy().to_string()));
    }
}
