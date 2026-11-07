//! Cargo detectors: the packages and workspaces (`cargo`, ecosystem), and the
//! declared versions (`cargo-version`, version-source). Two detectors because a
//! detector may emit only its own category (`core`'s identity rule). Both use
//! the same parser in `manifest.rs`.

mod manifest;

use repo_intelligence_core::{DetectionOutput, Detector, DetectorCategory, RepoContext};
use repo_intelligence_json::{Confidence, Diagnostic, Finding, Provenance, Severity};
use serde_json::{json, Value};

use manifest::{Declared, Manifest, Model, Problem};

const CARGO: &str = "cargo";
const CARGO_VERSION: &str = "cargo-version";

fn has_manifest(ctx: &RepoContext) -> bool {
    ctx.paths().any(manifest::is_manifest_path)
}

fn to_diagnostic(problem: Problem, detector: &str) -> Diagnostic {
    Diagnostic {
        severity: problem.severity,
        message: problem.message,
        detector: detector.to_string(),
        path: Some(problem.path),
    }
}

fn finding(
    category: DetectorCategory,
    kind: &str,
    detector: &str,
    path: Option<&str>,
    confidence: Confidence,
    data: Value,
    sources: Vec<String>,
) -> Finding {
    Finding {
        category,
        kind: kind.to_string(),
        path: path.map(str::to_string),
        confidence,
        data,
        provenance: Provenance {
            detector: detector.to_string(),
            sources,
        },
    }
}

/// Packages and workspace roots (`cargo`, ecosystem). Owns all parse-level
/// diagnostics, so `cargo-version` does not repeat them.
pub(crate) struct CargoPackages;

impl Detector for CargoPackages {
    fn id(&self) -> &'static str {
        CARGO
    }

    fn category(&self) -> DetectorCategory {
        DetectorCategory::Ecosystem
    }

    fn applies(&self, ctx: &RepoContext) -> bool {
        has_manifest(ctx)
    }

    fn detect(&self, ctx: &RepoContext) -> DetectionOutput {
        let model = manifest::load(ctx);
        let mut findings = Vec::new();
        let mut diagnostics: Vec<Diagnostic> = model
            .problems
            .iter()
            .cloned()
            .map(|p| to_diagnostic(p, CARGO))
            .collect();

        for m in &model.manifests {
            let Some(package) = &m.package else { continue };
            if package.name.is_none() {
                diagnostics.push(to_diagnostic(
                    manifest::warning(&m.path, "package has no name"),
                    CARGO,
                ));
            }
            let workspace = model.member_of.get(&m.path).cloned();
            let mut sources = vec![m.path.clone()];
            if let Some(root) = workspace.as_ref().filter(|r| **r != m.path) {
                sources.push(root.clone());
            }
            findings.push(finding(
                DetectorCategory::Ecosystem,
                "cargo-package",
                CARGO,
                Some(&m.path),
                Confidence::High,
                json!({ "name": package.name, "workspace": workspace }),
                sources,
            ));
        }

        for root in model.manifests.iter().filter(|m| m.workspace.is_some()) {
            let ws = root
                .workspace
                .as_ref()
                .expect("filtered to workspace roots");
            let members: Vec<&str> = model
                .manifests
                .iter()
                .filter(|m| model.member_of.get(&m.path) == Some(&root.path))
                .map(|m| m.path.as_str())
                .collect();
            findings.push(finding(
                DetectorCategory::Ecosystem,
                "cargo-workspace",
                CARGO,
                Some(&root.path),
                Confidence::High,
                json!({
                    "members": members,
                    "member_patterns": ws.members,
                    "exclude": ws.exclude,
                }),
                vec![root.path.clone()],
            ));
        }

        DetectionOutput::new(findings, diagnostics)
    }
}

/// Declared versions (`cargo-version`, version-source). One `cargo-toml` finding
/// per declared version, each anchored to the file that declares it.
pub(crate) struct CargoVersions;

impl Detector for CargoVersions {
    fn id(&self) -> &'static str {
        CARGO_VERSION
    }

    fn category(&self) -> DetectorCategory {
        DetectorCategory::VersionSource
    }

    fn applies(&self, ctx: &RepoContext) -> bool {
        has_manifest(ctx)
    }

    fn detect(&self, ctx: &RepoContext) -> DetectionOutput {
        let model = manifest::load(ctx);
        let mut findings = Vec::new();
        let mut diagnostics = Vec::new();

        for m in &model.manifests {
            if let Some(package) = &m.package {
                package_version(&model, m, package, &mut findings, &mut diagnostics);
            }
            if let Some(ws) = &m.workspace {
                workspace_version(m, &ws.package_version, &mut findings, &mut diagnostics);
            }
        }

        DetectionOutput::new(findings, diagnostics)
    }
}

fn package_version(
    model: &Model,
    m: &Manifest,
    package: &manifest::PackageDecl,
    findings: &mut Vec<Finding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = package.name.as_deref();
    match &package.version {
        Declared::Absent => {}
        Declared::WrongType => {
            diagnostics.push(warning(&m.path, "version has an unsupported type"))
        }
        Declared::Literal(version) => {
            check_semver(&m.path, version, diagnostics);
            findings.push(
                VersionSource {
                    path: &m.path,
                    name,
                    version: Some(version),
                    field: "package.version",
                    inherited_from: None,
                    sources: vec![m.path.clone()],
                }
                .finding(),
            );
        }
        Declared::Inherited => {
            let root = model.root_of(m);
            let inherited = root.and_then(|r| match &r.workspace.as_ref()?.package_version {
                Declared::Literal(v) => Some(v.as_str()),
                _ => None,
            });
            let root_path = root.map(|r| r.path.as_str());
            let mut sources = vec![m.path.clone()];
            sources.extend(root_path.filter(|r| *r != m.path).map(str::to_string));
            if inherited.is_none() {
                diagnostics.push(warning(&m.path, "inherited version cannot be resolved"));
            }
            // An invalid inherited version is reported once, at the root that
            // declares it (`workspace_version`); here it only lowers confidence.
            findings.push(
                VersionSource {
                    path: &m.path,
                    name,
                    version: inherited,
                    field: "workspace.package.version",
                    inherited_from: root_path,
                    sources,
                }
                .finding(),
            );
        }
    }
}

fn workspace_version(
    root: &Manifest,
    declared: &Declared,
    findings: &mut Vec<Finding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match declared {
        Declared::Absent => {}
        Declared::Literal(version) => {
            check_semver(&root.path, version, diagnostics);
            findings.push(
                VersionSource {
                    path: &root.path,
                    name: None,
                    version: Some(version),
                    field: "workspace.package.version",
                    inherited_from: None,
                    sources: vec![root.path.clone()],
                }
                .finding(),
            );
        }
        Declared::WrongType | Declared::Inherited => {
            diagnostics.push(warning(&root.path, "version has an unsupported type"));
        }
    }
}

/// Warns, at the declaring file, if `version` is not valid semver.
fn check_semver(path: &str, version: &str, diagnostics: &mut Vec<Diagnostic>) {
    if semver::Version::parse(version).is_err() {
        diagnostics.push(warning(path, "version is not valid semver"));
    }
}

/// One declared or inherited version, before it becomes a `cargo-toml` finding.
struct VersionSource<'a> {
    path: &'a str,
    name: Option<&'a str>,
    /// `None` when an inherited version cannot be resolved.
    version: Option<&'a str>,
    field: &'static str,
    inherited_from: Option<&'a str>,
    sources: Vec<String>,
}

impl VersionSource<'_> {
    /// High confidence for valid semver, Low otherwise (including unresolved).
    fn finding(self) -> Finding {
        let valid = self
            .version
            .is_some_and(|v| semver::Version::parse(v).is_ok());
        finding(
            DetectorCategory::VersionSource,
            "cargo-toml",
            CARGO_VERSION,
            Some(self.path),
            if valid {
                Confidence::High
            } else {
                Confidence::Low
            },
            json!({
                "name": self.name,
                "version": self.version,
                "field": self.field,
                "inherited_from": self.inherited_from,
            }),
            self.sources,
        )
    }
}

fn warning(path: &str, message: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message: message.to_string(),
        detector: CARGO_VERSION.to_string(),
        path: Some(path.to_string()),
    }
}
