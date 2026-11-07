//! Parses `Cargo.toml` manifests from the scanned listing and resolves workspace
//! membership. Private to the `cargo` detectors. Both detectors call `load`, which
//! is cheap compared to the scan.
//!
//! Diagnostic text is fixed: TOML parser messages are never used, because they
//! include source excerpts (the Phase 2 F1 rule).

use std::collections::{BTreeMap, BTreeSet};

use globset::{GlobBuilder, GlobMatcher};
use repo_intelligence_core::RepoContext;
use repo_intelligence_json::Severity;
use toml::Value;

/// A version as declared in one place in a manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Declared {
    Literal(String),
    /// `version.workspace = true` (or `version = { workspace = true }`).
    Inherited,
    Absent,
    /// Present, but not a string or an inheritance table.
    WrongType,
}

#[derive(Clone, Debug)]
pub(super) struct PackageDecl {
    /// `None` when `name` is missing or not a string.
    pub name: Option<String>,
    pub version: Declared,
    /// `package.workspace`, a path to the workspace root manifest's directory.
    pub workspace_override: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct WorkspaceDecl {
    pub members: Vec<String>,
    pub exclude: Vec<String>,
    /// `workspace.package.version`.
    pub package_version: Declared,
}

#[derive(Clone, Debug)]
pub(super) struct Manifest {
    /// Repo-relative path, e.g. `crates/alpha/Cargo.toml`.
    pub path: String,
    /// Repo-relative directory, `""` for the repository root.
    pub dir: String,
    pub package: Option<PackageDecl>,
    pub workspace: Option<WorkspaceDecl>,
}

/// A problem with a manifest. The `cargo` detector reports these as diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Problem {
    pub path: String,
    pub severity: Severity,
    pub message: String,
}

pub(super) struct Model {
    /// Sorted by path.
    pub manifests: Vec<Manifest>,
    pub problems: Vec<Problem>,
    /// Package manifest path -> path of the workspace root it belongs to.
    /// Absent for packages that are not members of any workspace.
    pub member_of: BTreeMap<String, String>,
}

impl Model {
    pub fn root_of(&self, manifest: &Manifest) -> Option<&Manifest> {
        let root = self.member_of.get(&manifest.path)?;
        self.manifests.iter().find(|m| &m.path == root)
    }
}

pub(super) fn is_manifest_path(path: &str) -> bool {
    path == "Cargo.toml" || path.ends_with("/Cargo.toml")
}

pub(super) fn load(ctx: &RepoContext) -> Model {
    let mut problems = Vec::new();
    let mut manifests = Vec::new();

    for path in ctx.paths().filter(|p| is_manifest_path(p)) {
        let Ok(bytes) = ctx.read(path) else {
            problems.push(problem(path, "manifest is unreadable"));
            continue;
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            problems.push(problem(path, "manifest is not valid UTF-8"));
            continue;
        };
        match text.parse::<toml::Table>() {
            Ok(table) => manifests.push(parse_manifest(path, &table)),
            Err(err) => {
                let (line, column) = err
                    .span()
                    .map_or((1, 1), |span| line_column(text, span.start));
                problems.push(problem(
                    path,
                    &format!("invalid TOML at line {line}, column {column}"),
                ));
            }
        }
    }

    let member_of = resolve_membership(&manifests, &mut problems);
    Model {
        manifests,
        problems,
        member_of,
    }
}

/// An error: the manifest cannot be used.
fn problem(path: &str, message: &str) -> Problem {
    Problem {
        path: path.to_string(),
        severity: Severity::Error,
        message: message.to_string(),
    }
}

/// A warning: analysis continues with the manifest or pattern ignored.
pub(super) fn warning(path: &str, message: &str) -> Problem {
    Problem {
        severity: Severity::Warning,
        ..problem(path, message)
    }
}

/// 1-based line and column (in characters) of a byte offset.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.chars().rev().take_while(|&c| c != '\n').count() + 1;
    (line, column)
}

fn parse_manifest(path: &str, table: &toml::Table) -> Manifest {
    let dir = match path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    };
    let package = table
        .get("package")
        .and_then(Value::as_table)
        .map(|t| PackageDecl {
            name: t.get("name").and_then(Value::as_str).map(str::to_string),
            version: declared(t.get("version")),
            workspace_override: t
                .get("workspace")
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    let workspace = table
        .get("workspace")
        .and_then(Value::as_table)
        .map(|t| WorkspaceDecl {
            members: string_list(t.get("members")),
            exclude: string_list(t.get("exclude")),
            package_version: t
                .get("package")
                .and_then(Value::as_table)
                .map_or(Declared::Absent, |p| declared(p.get("version"))),
        });
    Manifest {
        path: path.to_string(),
        dir,
        package,
        workspace,
    }
}

fn declared(value: Option<&Value>) -> Declared {
    match value {
        None => Declared::Absent,
        Some(Value::String(s)) => Declared::Literal(s.clone()),
        Some(Value::Table(t)) if t.get("workspace") == Some(&Value::Boolean(true)) => {
            Declared::Inherited
        }
        Some(_) => Declared::WrongType,
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Joins `rel` onto `dir` and normalizes `.` and `..`. Returns `None` if the
/// result would leave the repository.
fn join_dir(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// The directory of `dir` relative to `root_dir`, if it is inside it.
fn relative_to(root_dir: &str, dir: &str) -> Option<String> {
    if root_dir.is_empty() {
        return Some(dir.to_string());
    }
    dir.strip_prefix(root_dir)?
        .strip_prefix('/')
        .map(str::to_string)
        .or_else(|| (dir == root_dir).then(String::new))
}

fn parent_dir(dir: &str) -> String {
    dir.rsplit_once('/')
        .map_or(String::new(), |(parent, _)| parent.to_string())
}

fn manifest_in(dir: &str) -> String {
    if dir.is_empty() {
        "Cargo.toml".to_string()
    } else {
        format!("{dir}/Cargo.toml")
    }
}

/// A compiled member or exclude pattern, kept with its source text for diagnostics.
struct Pattern {
    source: String,
    matcher: GlobMatcher,
}

/// Compiles patterns. `*` stops at `/`, which matches Cargo's behavior for the
/// patterns that appear in practice (`crates/*`). Invalid patterns are reported
/// and skipped.
fn compile(patterns: &[String], root: &str, problems: &mut Vec<Problem>) -> Vec<Pattern> {
    patterns
        .iter()
        .filter_map(
            |source| match GlobBuilder::new(source).literal_separator(true).build() {
                Ok(glob) => Some(Pattern {
                    source: source.clone(),
                    matcher: glob.compile_matcher(),
                }),
                Err(_) => {
                    problems.push(warning(
                        root,
                        &format!("invalid workspace member pattern: {source}"),
                    ));
                    None
                }
            },
        )
        .collect()
}

/// Decides, for every package manifest, which workspace root (if any) it belongs to.
///
/// 1. A package with `package.workspace = "<dir>"` belongs to that root, which must
///    be a workspace root. Otherwise this is a problem.
/// 2. Otherwise it belongs to the nearest ancestor-or-self manifest with a
///    `[workspace]` table, as Cargo does.
/// 3. A root's own package is always a member. Any other package is a member only
///    if its directory matches a `members` pattern and no `exclude` pattern.
///
/// Also reports `members` patterns that match no manifest directory.
fn resolve_membership(
    manifests: &[Manifest],
    problems: &mut Vec<Problem>,
) -> BTreeMap<String, String> {
    let roots: BTreeSet<&str> = manifests
        .iter()
        .filter(|m| m.workspace.is_some())
        .map(|m| m.path.as_str())
        .collect();

    // Owner of each package, computed once so each problem is reported once.
    let mut owners: BTreeMap<&str, String> = BTreeMap::new();
    for manifest in manifests.iter().filter(|m| m.package.is_some()) {
        let explicit = manifest
            .package
            .as_ref()
            .and_then(|p| p.workspace_override.as_deref());
        let owner = match explicit {
            Some(rel) => join_dir(&manifest.dir, rel)
                .map(|dir| manifest_in(&dir))
                .filter(|target| roots.contains(target.as_str())),
            None => nearest_root(&manifest.dir, &roots),
        };
        match owner {
            Some(owner) => {
                owners.insert(&manifest.path, owner);
            }
            None if explicit.is_some() => problems.push(warning(
                &manifest.path,
                "package.workspace does not name a workspace root",
            )),
            None => {}
        }
    }

    let mut member_of = BTreeMap::new();
    for root in manifests.iter().filter(|m| m.workspace.is_some()) {
        let ws = root
            .workspace
            .as_ref()
            .expect("filtered to workspace roots");
        let members = compile(&ws.members, &root.path, problems);
        let excludes = compile(&ws.exclude, &root.path, problems);

        for pattern in &members {
            let matched = manifests.iter().any(|m| {
                relative_to(&root.dir, &m.dir).is_some_and(|rel| pattern.matcher.is_match(&rel))
            });
            if !matched {
                problems.push(warning(
                    &root.path,
                    &format!(
                        "workspace member pattern matches no Cargo.toml: {}",
                        pattern.source
                    ),
                ));
            }
        }

        for manifest in manifests.iter().filter(|m| m.package.is_some()) {
            if owners.get(manifest.path.as_str()) != Some(&root.path) {
                continue;
            }
            let is_member = manifest.path == root.path
                || relative_to(&root.dir, &manifest.dir).is_some_and(|rel| {
                    members.iter().any(|p| p.matcher.is_match(&rel))
                        && !excludes.iter().any(|p| p.matcher.is_match(&rel))
                });
            if is_member {
                member_of.insert(manifest.path.clone(), root.path.clone());
            }
        }
    }
    member_of
}

fn nearest_root(dir: &str, roots: &BTreeSet<&str>) -> Option<String> {
    let mut current = dir.to_string();
    loop {
        let candidate = manifest_in(&current);
        if roots.contains(candidate.as_str()) {
            return Some(candidate);
        }
        if current.is_empty() {
            return None;
        }
        current = parent_dir(&current);
    }
}
