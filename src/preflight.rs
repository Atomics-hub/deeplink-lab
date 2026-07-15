use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use plist::Value as PlistValue;
use regex::Regex;
use serde_json::Value;
use url::Url;

use crate::model::{
    AppSpec, ExpectedTarget, Platform, PreflightIssue, PreflightReport, Severity, SourceContext,
};
use crate::spec::LoadedSpec;

const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";

pub fn validate(loaded: &LoadedSpec) -> PreflightReport {
    let mut issues = Vec::new();
    let spec = &loaded.spec;

    if spec.version != 1 {
        issue(
            &mut issues,
            Severity::Error,
            "spec.unsupported_version",
            format!("spec version {} is unsupported; expected 1", spec.version),
            None,
        );
    }
    if spec.apps.is_empty() {
        issue(
            &mut issues,
            Severity::Error,
            "spec.no_apps",
            "at least one app is required",
            None,
        );
    }
    if spec.cases.is_empty() {
        issue(
            &mut issues,
            Severity::Error,
            "spec.no_cases",
            "at least one case is required",
            None,
        );
    }

    validate_cases(loaded, &mut issues);
    validate_apps(loaded, &mut issues);
    validate_associations(loaded, &mut issues);

    let errors = issues
        .iter()
        .filter(|item| item.severity == Severity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|item| item.severity == Severity::Warning)
        .count();
    PreflightReport {
        schema_version: 1,
        spec_version: spec.version,
        valid: errors == 0,
        errors,
        warnings,
        issues,
    }
}

fn validate_cases(loaded: &LoadedSpec, issues: &mut Vec<PreflightIssue>) {
    let spec = &loaded.spec;
    let id_pattern = Regex::new(r"^[a-z0-9][a-z0-9._-]*$").expect("valid regex");
    let mut seen = BTreeSet::new();
    for case in &spec.cases {
        let subject = Some(format!("case:{}", case.id));
        if !seen.insert(case.id.clone()) {
            issue(
                issues,
                Severity::Error,
                "case.duplicate_id",
                "case id is duplicated",
                subject.clone(),
            );
        }
        if !id_pattern.is_match(&case.id) {
            issue(
                issues,
                Severity::Error,
                "case.invalid_id",
                "case id must be lowercase and contain only letters, digits, '.', '_' or '-'",
                subject.clone(),
            );
        }
        let Some(app) = spec.apps.get(&case.app) else {
            issue(
                issues,
                Severity::Error,
                "case.unknown_app",
                format!("case references unknown app '{}'", case.app),
                subject,
            );
            continue;
        };
        match Url::parse(&case.link) {
            Ok(url) => {
                if matches!(url.scheme(), "http" | "https") && url.host_str().is_none() {
                    issue(
                        issues,
                        Severity::Error,
                        "case.link_missing_host",
                        "HTTP(S) link has no host",
                        subject.clone(),
                    );
                }
                if !matches!(url.scheme(), "http" | "https") {
                    validate_custom_scheme(loaded, case, app, url.scheme(), issues);
                }
            }
            Err(error) => issue(
                issues,
                Severity::Error,
                "case.invalid_link",
                format!("link is not a valid absolute URL: {error}"),
                subject.clone(),
            ),
        }
        if case.expected.target == ExpectedTarget::App && case.expected.destination.is_none() {
            issue(
                issues,
                Severity::Error,
                "case.destination_required",
                "app expectations require an exact destination",
                subject.clone(),
            );
        }
        if case.expected.target != ExpectedTarget::App && case.runtime_failure_class.is_some() {
            issue(
                issues,
                Severity::Warning,
                "case.failure_class_without_app_destination",
                "runtimeFailureClass is most useful on an app destination assertion",
                subject.clone(),
            );
        }
        if case.source.context != SourceContext::Direct
            && (case.source.page_url.is_none() || case.source.tap_text.is_none())
        {
            issue(
                issues,
                Severity::Error,
                "case.browser_source_incomplete",
                "browser and controlled-page sources require pageUrl and tapText",
                subject.clone(),
            );
        }
        if app.platform == Platform::Ios && case.source.context == SourceContext::Chrome {
            issue(
                issues,
                Severity::Info,
                "case.chrome_requires_install",
                "Chrome context on iOS requires Chrome and Maestro on the selected simulator",
                subject,
            );
        }
    }
}

fn validate_custom_scheme(
    loaded: &LoadedSpec,
    case: &crate::model::CaseSpec,
    app: &AppSpec,
    scheme: &str,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("case:{}", case.id));
    let configured = match app.platform {
        Platform::Ios => app.info_plist.as_ref().is_some_and(|path| {
            PlistValue::from_file(loaded.resolve(path))
                .ok()
                .is_some_and(|value| {
                    value
                        .as_dictionary()
                        .and_then(|dict| dict.get("CFBundleURLTypes"))
                        .and_then(PlistValue::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(PlistValue::as_dictionary)
                        .filter_map(|dict| dict.get("CFBundleURLSchemes"))
                        .filter_map(PlistValue::as_array)
                        .flatten()
                        .any(|item| item.as_string() == Some(scheme))
                })
        }),
        Platform::Android => app.manifest.as_ref().is_some_and(|path| {
            fs::read_to_string(loaded.resolve(path))
                .ok()
                .is_some_and(|raw| {
                    roxmltree::Document::parse(&raw)
                        .ok()
                        .is_some_and(|document| {
                            document.descendants().any(|node| {
                                node.has_tag_name("data")
                                    && node.attribute((ANDROID_NS, "scheme")) == Some(scheme)
                            })
                        })
                })
        }),
    };
    if !configured {
        issue(
            issues,
            Severity::Error,
            "case.custom_scheme_not_declared",
            format!("scheme '{scheme}' is not declared by the app configuration"),
            subject,
        );
    }
}

fn validate_apps(loaded: &LoadedSpec, issues: &mut Vec<PreflightIssue>) {
    for (name, app) in &loaded.spec.apps {
        let subject = Some(format!("app:{name}"));
        if app.app_id.trim().is_empty() {
            issue(
                issues,
                Severity::Error,
                "app.missing_id",
                "appId cannot be empty",
                subject.clone(),
            );
        }
        if !loaded.resolve(&app.artifact).exists() {
            issue(
                issues,
                Severity::Warning,
                "app.artifact_missing",
                format!(
                    "runtime artifact does not exist yet: {}",
                    app.artifact.display()
                ),
                subject.clone(),
            );
        }
        match app.platform {
            Platform::Ios => validate_ios_app(loaded, name, app, issues),
            Platform::Android => validate_android_app(loaded, name, app, issues),
        }
    }
}

fn validate_ios_app(
    loaded: &LoadedSpec,
    name: &str,
    app: &AppSpec,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("app:{name}"));
    if app.team_id.is_none() {
        issue(
            issues,
            Severity::Warning,
            "ios.team_id_missing",
            "teamId is required to cross-check AASA app identifiers",
            subject.clone(),
        );
    }
    if let Some(path) = &app.entitlements {
        let path = loaded.resolve(path);
        if let Err(error) = PlistValue::from_file(&path) {
            issue(
                issues,
                Severity::Error,
                "ios.entitlements_invalid",
                format!("cannot parse {}: {error}", portable(&path, &loaded.root)),
                subject.clone(),
            );
        }
    } else {
        issue(
            issues,
            Severity::Warning,
            "ios.entitlements_missing",
            "no entitlements file was provided",
            subject.clone(),
        );
    }
    if let Some(path) = &app.info_plist {
        let path = loaded.resolve(path);
        if let Err(error) = PlistValue::from_file(&path) {
            issue(
                issues,
                Severity::Error,
                "ios.info_plist_invalid",
                format!("cannot parse {}: {error}", portable(&path, &loaded.root)),
                subject,
            );
        }
    }
}

fn validate_android_app(
    loaded: &LoadedSpec,
    name: &str,
    app: &AppSpec,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("app:{name}"));
    let Some(manifest) = &app.manifest else {
        issue(
            issues,
            Severity::Warning,
            "android.manifest_missing",
            "no AndroidManifest.xml was provided",
            subject,
        );
        return;
    };
    let path = loaded.resolve(manifest);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) => {
            issue(
                issues,
                Severity::Error,
                "android.manifest_unreadable",
                format!("cannot read {}: {error}", portable(&path, &loaded.root)),
                subject,
            );
            return;
        }
    };
    if raw.contains("${") {
        issue(
            issues,
            Severity::Error,
            "android.unresolved_placeholder",
            "manifest contains an unresolved ${...} placeholder",
            subject.clone(),
        );
    }
    let document = match roxmltree::Document::parse(&raw) {
        Ok(document) => document,
        Err(error) => {
            issue(
                issues,
                Severity::Error,
                "android.manifest_invalid_xml",
                format!("manifest XML is invalid: {error}"),
                subject,
            );
            return;
        }
    };
    let has_view_filter = document
        .descendants()
        .filter(|node| node.has_tag_name("intent-filter"))
        .any(|filter| {
            let has_view = filter.descendants().any(|node| {
                node.has_tag_name("action")
                    && node.attribute((ANDROID_NS, "name")) == Some("android.intent.action.VIEW")
            });
            let has_browsable = filter.descendants().any(|node| {
                node.has_tag_name("category")
                    && node.attribute((ANDROID_NS, "name"))
                        == Some("android.intent.category.BROWSABLE")
            });
            has_view && has_browsable
        });
    if !has_view_filter {
        issue(
            issues,
            Severity::Error,
            "android.no_browsable_view_filter",
            "manifest has no VIEW plus BROWSABLE intent filter",
            subject,
        );
    }
}

fn validate_associations(loaded: &LoadedSpec, issues: &mut Vec<PreflightIssue>) {
    let mut domains = BTreeMap::new();
    for association in &loaded.spec.associations {
        if domains
            .insert(association.domain.clone(), association)
            .is_some()
        {
            issue(
                issues,
                Severity::Error,
                "association.duplicate_domain",
                "association domain is duplicated",
                Some(format!("domain:{}", association.domain)),
            );
        }
        if association.domain.contains("/") || association.domain.contains("://") {
            issue(
                issues,
                Severity::Error,
                "association.invalid_domain",
                "domain must be a bare host without scheme or path",
                Some(format!("domain:{}", association.domain)),
            );
        }
        if let Some(path) = &association.aasa {
            validate_aasa(loaded, association.domain.as_str(), path, issues);
        }
        if let Some(path) = &association.assetlinks {
            validate_assetlinks(loaded, association.domain.as_str(), path, issues);
        }
        for app in loaded.spec.apps.values() {
            match app.platform {
                Platform::Ios if association.aasa.is_some() => {
                    cross_check_ios(loaded, app, association, issues)
                }
                Platform::Android if association.assetlinks.is_some() => {
                    cross_check_android(loaded, app, association, issues)
                }
                _ => {}
            }
        }
    }

    for case in &loaded.spec.cases {
        let Ok(url) = Url::parse(&case.link) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        let Some(host) = url.host_str() else { continue };
        let Some(association) = domains.get(host) else {
            issue(
                issues,
                Severity::Error,
                "association.domain_missing",
                format!("no association entry covers HTTPS host '{host}'"),
                Some(format!("case:{}", case.id)),
            );
            continue;
        };
        let Some(app) = loaded.spec.apps.get(&case.app) else {
            continue;
        };
        match app.platform {
            Platform::Ios => cross_check_ios(loaded, app, association, issues),
            Platform::Android => cross_check_android(loaded, app, association, issues),
        }
    }
}

fn validate_aasa(
    loaded: &LoadedSpec,
    domain: &str,
    relative: &Path,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("aasa:{domain}"));
    let path = loaded.resolve(relative);
    if relative.extension().and_then(|value| value.to_str()) == Some("json") {
        issue(
            issues,
            Severity::Warning,
            "aasa.json_extension",
            "AASA should be served without a .json filename extension",
            subject.clone(),
        );
    }
    let raw = match fs::read(&path) {
        Ok(raw) => raw,
        Err(error) => {
            issue(
                issues,
                Severity::Error,
                "aasa.unreadable",
                format!("cannot read {}: {error}", portable(&path, &loaded.root)),
                subject,
            );
            return;
        }
    };
    if raw.len() > 128 * 1024 {
        issue(
            issues,
            Severity::Error,
            "aasa.too_large",
            "uncompressed AASA exceeds Apple's 128 KB limit",
            subject.clone(),
        );
    }
    match serde_json::from_slice::<Value>(&raw) {
        Ok(value) => {
            let details = value.pointer("/applinks/details").and_then(Value::as_array);
            if details.is_none_or(|items| items.is_empty()) {
                issue(
                    issues,
                    Severity::Error,
                    "aasa.missing_details",
                    "AASA has no applinks.details entries",
                    subject,
                );
            }
        }
        Err(error) => issue(
            issues,
            Severity::Error,
            "aasa.invalid_json",
            format!("AASA JSON is invalid: {error}"),
            subject,
        ),
    }
}

fn validate_assetlinks(
    loaded: &LoadedSpec,
    domain: &str,
    relative: &Path,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("assetlinks:{domain}"));
    let path = loaded.resolve(relative);
    let raw = match fs::read(&path) {
        Ok(raw) => raw,
        Err(error) => {
            issue(
                issues,
                Severity::Error,
                "assetlinks.unreadable",
                format!("cannot read {}: {error}", portable(&path, &loaded.root)),
                subject,
            );
            return;
        }
    };
    match serde_json::from_slice::<Value>(&raw) {
        Ok(Value::Array(items)) if !items.is_empty() => {
            let fingerprint =
                Regex::new(r"^(?:[0-9A-F]{2}:){31}[0-9A-F]{2}$").expect("valid regex");
            for value in items
                .iter()
                .filter_map(|item| item.pointer("/target/sha256_cert_fingerprints"))
            {
                if let Some(values) = value.as_array() {
                    for value in values.iter().filter_map(Value::as_str) {
                        if !fingerprint.is_match(value) {
                            issue(
                                issues,
                                Severity::Error,
                                "assetlinks.invalid_fingerprint",
                                "SHA-256 certificate fingerprint must be 32 uppercase colon-separated bytes",
                                subject.clone(),
                            );
                        }
                    }
                }
            }
        }
        Ok(_) => issue(
            issues,
            Severity::Error,
            "assetlinks.invalid_root",
            "assetlinks.json must be a non-empty JSON array",
            subject,
        ),
        Err(error) => issue(
            issues,
            Severity::Error,
            "assetlinks.invalid_json",
            format!("assetlinks JSON is invalid: {error}"),
            subject,
        ),
    }
}

fn cross_check_ios(
    loaded: &LoadedSpec,
    app: &AppSpec,
    association: &crate::model::AssociationSpec,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("app:{}", app.app_id));
    if association.aasa.is_none() {
        issue(
            issues,
            Severity::Error,
            "ios.aasa_missing",
            "HTTPS case has no AASA fixture",
            subject.clone(),
        );
    }
    if let (Some(team), Some(aasa)) = (&app.team_id, &association.aasa) {
        let path = loaded.resolve(aasa);
        if let Ok(raw) = fs::read(&path) {
            if let Ok(value) = serde_json::from_slice::<Value>(&raw) {
                let expected = format!("{team}.{}", app.app_id);
                let found = value
                    .pointer("/applinks/details")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .any(|detail| {
                        detail.get("appID").and_then(Value::as_str) == Some(expected.as_str())
                            || detail
                                .get("appIDs")
                                .and_then(Value::as_array)
                                .is_some_and(|ids| {
                                    ids.iter().any(|id| id.as_str() == Some(expected.as_str()))
                                })
                    });
                if !found {
                    issue(
                        issues,
                        Severity::Error,
                        "ios.aasa_app_id_mismatch",
                        format!("AASA does not declare '{expected}'"),
                        subject.clone(),
                    );
                }
            }
        }
    }
    if let Some(entitlements) = &app.entitlements {
        let path = loaded.resolve(entitlements);
        if let Ok(value) = PlistValue::from_file(path) {
            let expected = format!("applinks:{}", association.domain);
            let found = value
                .as_dictionary()
                .and_then(|dict| dict.get("com.apple.developer.associated-domains"))
                .and_then(PlistValue::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.as_string() == Some(expected.as_str()))
                });
            if !found {
                issue(
                    issues,
                    Severity::Error,
                    "ios.entitlement_domain_missing",
                    format!("entitlements do not contain '{expected}'"),
                    subject,
                );
            }
        }
    }
}

fn cross_check_android(
    loaded: &LoadedSpec,
    app: &AppSpec,
    association: &crate::model::AssociationSpec,
    issues: &mut Vec<PreflightIssue>,
) {
    let subject = Some(format!("app:{}", app.app_id));
    let Some(assetlinks) = &association.assetlinks else {
        issue(
            issues,
            Severity::Error,
            "android.assetlinks_missing",
            "HTTPS case has no assetlinks fixture",
            subject,
        );
        return;
    };
    let path = loaded.resolve(assetlinks);
    if let Ok(raw) = fs::read(path) {
        if let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(&raw) {
            let matched = items.iter().any(|item| {
                item.pointer("/target/package_name").and_then(Value::as_str)
                    == Some(app.app_id.as_str())
                    && item
                        .get("relation")
                        .and_then(Value::as_array)
                        .is_some_and(|relations| {
                            relations.iter().any(|relation| {
                                relation.as_str()
                                    == Some("delegate_permission/common.handle_all_urls")
                            })
                        })
            });
            if !matched {
                issue(
                    issues,
                    Severity::Error,
                    "android.assetlinks_package_mismatch",
                    format!("assetlinks does not authorize '{}'", app.app_id),
                    subject,
                );
            }
        }
    }
}

fn portable(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn issue(
    issues: &mut Vec<PreflightIssue>,
    severity: Severity,
    code: impl Into<String>,
    message: impl Into<String>,
    subject: Option<String>,
) {
    issues.push(PreflightIssue {
        severity,
        code: code.into(),
        message: message.into(),
        subject,
    });
}
