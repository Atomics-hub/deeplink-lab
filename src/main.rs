use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use deeplink_lab::gates;
use deeplink_lab::model::Platform;
use deeplink_lab::{doctor, preflight, report, runtime, schema, LoadedSpec};

#[derive(Debug, Parser)]
#[command(name = "deeplink-lab", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Validate YAML, AASA, assetlinks, entitlements, and manifests without a device.
    Validate {
        #[arg(long, default_value = "deeplinklab.yml")]
        spec: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
        /// Treat warnings as a failing exit status.
        #[arg(long)]
        strict: bool,
    },
    /// Execute selected cases on local Simulator/emulator lanes and write evidence.
    Run {
        #[arg(long, default_value = "deeplinklab.yml")]
        spec: PathBuf,
        #[arg(long = "case")]
        cases: Vec<String>,
        #[arg(long, value_enum)]
        platform: Option<PlatformArg>,
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        reset_between_runs: bool,
        #[arg(long, default_value = "booted")]
        ios_device: String,
        #[arg(long)]
        android_device: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        allow_preflight_errors: bool,
        /// Skip the self-contained HTML when running large repeatability suites.
        #[arg(long)]
        skip_html: bool,
    },
    /// Re-run one case with the same declarative runner and a fresh evidence directory.
    Replay {
        #[arg(long, default_value = "deeplinklab.yml")]
        spec: PathBuf,
        #[arg(long)]
        case: String,
        #[arg(long, default_value = "booted")]
        ios_device: String,
        #[arg(long)]
        android_device: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Rebuild a portable HTML report from machine-readable run JSON.
    Report {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Evaluate the predeclared hostile proof gates.
    Gates {
        #[arg(long)]
        representative: PathBuf,
        #[arg(long)]
        repeatability: Option<PathBuf>,
        #[arg(long, default_value = "deeplinklab.yml")]
        spec: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Print the machine-readable contract or report JSON Schema.
    Schema {
        #[arg(value_enum, default_value_t = SchemaKind::Spec)]
        kind: SchemaKind,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Inspect local platform tools and state the evidence boundaries.
    Doctor {
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Write a documented starter deeplinklab.yml.
    Init {
        #[arg(long, default_value = "deeplinklab.yml")]
        output: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SchemaKind {
    Spec,
    RunReport,
    GateReport,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PlatformArg {
    Ios,
    Android,
}

fn main() {
    if let Err(error) = execute() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Validate {
            spec,
            format,
            strict,
        } => {
            let loaded = LoadedSpec::load(spec)?;
            let validation = preflight::validate(&loaded);
            match format {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&validation)?),
                OutputFormat::Human => print_preflight(&validation),
            }
            if !validation.valid || (strict && validation.warnings > 0) {
                bail!("static preflight did not meet the requested policy");
            }
        }
        Commands::Run {
            spec,
            cases,
            platform,
            repeat,
            reset_between_runs,
            ios_device,
            android_device,
            out,
            allow_preflight_errors,
            skip_html,
        } => {
            let loaded = LoadedSpec::load(&spec)?;
            let output = out.unwrap_or_else(|| timestamped_output("run"));
            let options = runtime::RunOptions {
                output: output.clone(),
                case_ids: cases,
                platform: platform.map(Into::into),
                repeat,
                reset_between_runs,
                ios_device,
                android_device,
                allow_preflight_errors,
                write_html: !skip_html,
            };
            let run = runtime::run(&loaded, &options)?;
            if !skip_html {
                println!("report: {}", output.join("report.html").display());
            }
            println!("json: {}", output.join("report.json").display());
            println!(
                "cases={} passed={} failed={} unavailable={} duration_ms={}",
                run.summary.total,
                run.summary.passed,
                run.summary.failed,
                run.summary.unavailable,
                run.summary.duration_ms
            );
            if run.summary.failed > 0 || run.summary.unavailable > 0 {
                std::process::exit(2);
            }
        }
        Commands::Replay {
            spec,
            case,
            ios_device,
            android_device,
            out,
        } => {
            let loaded = LoadedSpec::load(&spec)?;
            let output = out.unwrap_or_else(|| timestamped_output("replay"));
            let options = runtime::RunOptions {
                output: output.clone(),
                case_ids: vec![case],
                ios_device,
                android_device,
                ..Default::default()
            };
            let run = runtime::run(&loaded, &options)?;
            println!("report: {}", output.join("report.html").display());
            if run.summary.failed > 0 || run.summary.unavailable > 0 {
                std::process::exit(2);
            }
        }
        Commands::Report { input, output } => {
            let run: deeplink_lab::RunReport = serde_json::from_slice(&fs::read(&input)?)?;
            let root = input.parent().unwrap_or_else(|| std::path::Path::new("."));
            fs::write(&output, report::render_html(&run, root)?)?;
            println!("wrote {}", output.display());
        }
        Commands::Gates {
            representative,
            repeatability,
            spec,
            format,
            output,
        } => {
            let loaded = LoadedSpec::load(spec)?;
            let representative: deeplink_lab::RunReport =
                serde_json::from_slice(&fs::read(representative)?)?;
            let repeated = repeatability
                .map(fs::read)
                .transpose()?
                .map(|bytes| serde_json::from_slice::<deeplink_lab::RunReport>(&bytes))
                .transpose()?;
            let result = gates::evaluate(
                &representative,
                repeated.as_ref(),
                &loaded.spec.gates,
                loaded.spec.metadata.integration_seconds,
            );
            let json = serde_json::to_string_pretty(&result)?;
            if let Some(path) = output {
                fs::write(path, format!("{json}\n"))?;
            }
            match format {
                OutputFormat::Json => println!("{json}"),
                OutputFormat::Human => print_gates(&result),
            }
            if !result.all_verified_gates_passed {
                std::process::exit(3);
            }
        }
        Commands::Schema { kind, output } => {
            let value = match kind {
                SchemaKind::Spec => schema::spec_schema(),
                SchemaKind::RunReport => schema::run_report_schema(),
                SchemaKind::GateReport => schema::gate_report_schema(),
            };
            let json = format!("{}\n", serde_json::to_string_pretty(&value)?);
            if let Some(path) = output {
                fs::write(path, json)?;
            } else {
                print!("{json}");
            }
        }
        Commands::Doctor { format } => {
            let report = doctor::inspect();
            match format {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                OutputFormat::Human => {
                    println!("static preflight: ready");
                    println!(
                        "iOS Simulator: {} — {}",
                        availability(report.ios_simulator.available),
                        report.ios_simulator.detail
                    );
                    println!(
                        "Android emulator: {} — {}",
                        availability(report.android_emulator.available),
                        report.android_emulator.detail
                    );
                    println!(
                        "Maestro: {} — {}",
                        availability(report.maestro.available),
                        report.maestro.detail
                    );
                    println!(
                        "Appium: {} — {}",
                        availability(report.appium.available),
                        report.appium.detail
                    );
                    println!("boundaries:");
                    for boundary in report.boundaries {
                        println!("- {boundary}");
                    }
                }
            }
        }
        Commands::Init { output } => {
            if output.exists() {
                bail!("refusing to overwrite {}", output.display());
            }
            fs::write(&output, include_str!("../examples/starter.yml"))
                .with_context(|| format!("failed to write {}", output.display()))?;
            println!("wrote {}", output.display());
        }
    }
    Ok(())
}

impl From<PlatformArg> for Platform {
    fn from(value: PlatformArg) -> Self {
        match value {
            PlatformArg::Ios => Self::Ios,
            PlatformArg::Android => Self::Android,
        }
    }
}

fn timestamped_output(prefix: &str) -> PathBuf {
    PathBuf::from("outputs").join(format!(
        "{}-{}",
        prefix,
        Utc::now().format("%Y%m%dT%H%M%SZ")
    ))
}

fn print_preflight(report: &deeplink_lab::model::PreflightReport) {
    for issue in &report.issues {
        println!(
            "{:?} {} {}{}",
            issue.severity,
            issue.code,
            issue
                .subject
                .as_ref()
                .map(|value| format!("[{value}] "))
                .unwrap_or_default(),
            issue.message
        );
    }
    println!(
        "valid={} errors={} warnings={}",
        report.valid, report.errors, report.warnings
    );
}

fn print_gates(report: &gates::GateReport) {
    for (name, gate) in [
        ("novelty", &report.novelty),
        ("repeatability", &report.repeatability),
        ("speed", &report.speed),
        ("integration", &report.integration),
        ("evidence", &report.evidence),
    ] {
        println!(
            "{name}: {:?} — {} (required: {})",
            gate.status, gate.observed, gate.required
        );
    }
    println!(
        "all_verified_gates_passed={}",
        report.all_verified_gates_passed
    );
}

fn availability(value: bool) -> &'static str {
    if value {
        "available"
    } else {
        "unavailable"
    }
}
