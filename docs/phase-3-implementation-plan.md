# Phase 3 — Implementation Plan

> Status: Implemented (see §13 for results and deviations). Scope is defined by `roadmap.md` Phase 3,
> `architecture.md` §4–9, `detector-architecture.md`, and `workspace-layout.md`
> §2–6. This plan is written against the **actual** Phase 0–2 code, not the
> earlier plans; where they differ, the code wins and the difference is called out.
>
> Phase 3 delivers the **first vertical slice**: a Cargo detector pair, a git-tag
> detector, and a working `repo-intelligence analyze <path> --format json` CLI.
> Running it on a real Rust repository produces a schema-conformant,
> deterministic report describing the repo's packages, workspaces, and versions.

## 1. Phase Objectives

### What Phase 3 achieves

One end-to-end path, thin: **disk → scan → detectors → report → stdout**. After
Phase 3, a user (and, in Phase 4, a Skill) can run:

```
repo-intelligence analyze <path> --format json
```

and receive a report that lists every Cargo package and workspace in the repo,
every declared or inherited version with the file it comes from, and every
version-like git tag.

### How it builds on Phases 0–2 (as actually implemented)

| Phase | What exists today | How Phase 3 uses it |
|---|---|---|
| 0 | Workspace, four crates, CI (`fmt`/`clippy`/`build`/`test`), `scripts/check-engine-deps.sh` | Unchanged. CI gains one step (§10, Task 3.11). |
| 1 | `json` crate: `Report`, `Finding`, `Diagnostic`, `Confidence`, `DetectorCategory`, `SCHEMA_VERSION = "0.1.0"`, committed `schema/report.schema.json` | Reused as-is. **No schema change** (§8). |
| 2 | `core`: `scan()`, `RepoContext` (`root`, `paths`, `read`, `diagnostics`), `Detector`, `DetectionOutput`, `DetectorRegistry`, `run()`, `EngineError`; identity enforcement; total result order; `fixtures/nested`; `resolver = "3"` | Reused. Extended additively with git tag metadata and two small API fixes carried over from the Phase 2 review (Task 3.0). |

Facts about the Phase 2 implementation that shape this plan:

- **`scan()` excludes `.git` and lists dotfiles.** Detectors therefore cannot see
  git data through `paths()`. Git metadata must be collected by `core` and exposed
  through a new `RepoContext` accessor (§5). This is the additive growth the
  Phase 2 plan anticipated ("Git metadata in the context … Phase 3").
- **`run()` rejects findings whose `category` differs from `detector.category()`.**
  A single "Cargo ecosystem/version-source detector" cannot emit both
  `ecosystem` and `version-source` findings. Phase 3 therefore uses **two Cargo
  detectors sharing one parser** (§4.1).
- **`run()` overwrites each diagnostic's `detector` with the emitting detector's
  `id()`**, and ordering is fixed by `pipeline.rs`. Detectors do not need to sort.
- **Phase 2 review items N1–N3 are still open** (`DetectionOutput` literal
  construction, duplicate ids, unvalidated detector paths). Phase 3 is the last
  cheap moment to close them, because it introduces the first detectors.
- **The Phase 2 work is committed** (`374d878`, on
  `feat/bootstrap-repo-intelligence`), so Phase 3 diffs are reviewable on their
  own.

### New capabilities

- Git tag enumeration in `core` (annotated vs lightweight, peeled commit).
- `detectors`: `cargo` (ecosystem), `cargo-version` (version-source),
  `git-tags` (version-source), and `default_registry()`.
- `cli`: argument parsing, exit codes, `tool_version` / `analyzed_at` injection,
  JSON on stdout, errors on stderr.
- Golden reports for `rust-single-crate`, `rust-workspace`, and `messy`.

### What counts as a successful end-to-end analysis

1. `repo-intelligence analyze . --format json`, run on this repository, exits 0.
   Its report lists the four `crates/*` packages as members of the root
   workspace, each with version `0.0.0` inherited from `workspace.package.version`,
   plus any version-like tags.
2. The `messy` fixture produces diagnostics and still exits 0.
3. Golden reports match byte-for-byte, except `analyzed_at`.

## 2. Scope

### In scope (from `roadmap.md` Phase 3)

- **Cargo detection:** single packages, workspaces (root and members), virtual
  manifests, `members`/`exclude` globs, literal and inherited
  (`version.workspace = true`) versions, missing versions, invalid manifests.
- **Git tag detection:** enumerate tags, classify version-like tags, distinguish
  annotated from lightweight, report the peeled commit.
- **Version source discovery:** every place a version is declared is a separate
  `version-source` finding with a path or ref name and provenance.
- **Orchestration:** `detectors::default_registry()` wired into the CLI; the
  Phase 2 `scan` → `run` pipeline is unchanged.
- **CLI:** `repo-intelligence analyze <path> --format json`; stdout is only the
  JSON report; stderr carries errors; exit codes per `architecture.md` §6;
  runtime injection of `tool_version` and `analyzed_at`.
- **Golden tests:** `rust-single-crate`, `rust-workspace`, `messy`.
- **Phase 2 carry-over:** review items N1–N3 and `RepoContext::contains` (Task 3.0),
  because the first detectors depend on them.

### Out of scope

| Excluded | Owning phase |
|---|---|
| Any other ecosystem (npm, Python, Maven, Go) | Phase 5 |
| Language, CI, and release-tooling detectors | Phase 5 |
| Semver bump recommendations, release plans, changelogs | Phase 4 (Skill reasoning) |
| GitHub release automation | Not on the roadmap for the engine |
| The Claude Code Skill | Phase 4 |
| Synthesis logic (e.g. linking `foo-v1.2.0` tags to crate `foo`, "likely primary") | Deferred; the Skill does this in Phase 4 (§11, D6) |
| `tracing`, a verbosity flag, other CLI subcommands or flags | Added when there is something to log (§7) |
| Per-ecosystem Cargo feature flags on `detectors` | Phase 5, when a second ecosystem exists |
| Tag dates, tag messages, branches, HEAD | Additive later, if Phase 4 needs them |
| Path-dependency auto-membership in workspaces | Known limitation (§6) |
| Schema tightening for `data` payloads | Phase 5/6 per `phase-1-implementation-plan.md` §5.3 |

## 3. Existing Architecture and Crate Responsibilities

No new crates. The dependency graph from `workspace-layout.md` §3 is unchanged.

| Crate | Phase 3 change | New dependencies |
|---|---|---|
| `json` | **None.** | None |
| `core` | Task 3.0 API fixes; new private `git` module; `GitTag` type; `RepoContext::git_tags()`, `RepoContext::contains()`, `RepoContext::with_git_tags()`; `scan()` collects tags. | None. Uses `std::process::Command` to run the system `git`. |
| `detectors` | `cargo` module (two detectors and a shared private manifest parser), `git_tags` module, `default_registry()`. | `toml`, `semver`, `globset` (`globset` is already in the lockfile via `ignore`). |
| `cli` | `analyze` command, exit-code mapping, injection, RFC 3339 formatter, golden tests. | `clap` (already declared in `[workspace.dependencies]`). |

**Public surface added:**

- `core`: `GitTag` (`#[non_exhaustive]`, with a constructor), `RepoContext::git_tags`,
  `RepoContext::contains`, `RepoContext::with_git_tags`, and
  `DetectionOutput::new` (Task 3.0).
- `detectors`: **only** `default_registry()`. The detector types stay `pub(crate)`.
  Nothing outside the crate needs to name them, and keeping them private keeps
  the extension workflow ("add a module, register it") from leaking into the API.
- `cli`: nothing (it is a binary).

**Rules respected:**

- `core` names no concrete detector.
- Detectors read only through `&RepoContext`.
- Only `cli` touches process I/O, environment, and the clock.
- `core`'s one subprocess call (`git`) is part of the single scan step, not a
  detector (§5, D1).

## 4. Detector Design

All three detectors live in `crates/detectors/src/`. Each is a unit struct
implementing `core::Detector` and builds its output with `DetectionOutput::new`.

### 4.1 Cargo: two detectors, one parser

**Why two detectors.** `run()` keeps a finding only if its `category` equals the
emitting detector's `category()`. The roadmap's single "Cargo
ecosystem/version-source detector" would need two categories. Options:

- **(A, chosen)** Two detectors in one module, `cargo` (ecosystem) and
  `cargo-version` (version-source), sharing a private `manifest` parser. Each is
  small, and each category stays honest.
- **(B)** One `version-source` detector that folds workspace and package facts
  into version-source `data`. It is simpler, but it loses the "this is a Cargo
  repo" ecosystem fact that `detector-architecture.md` §8 lists, and it overloads
  the version-source kind.
- **(C)** Let a detector declare several categories. This changes the `core`
  trait and reverses a reviewed Phase 2 decision. Rejected:
  `detector-architecture.md` §7 treats core edits for a detector as suspect.

Parsing twice is cheap. `RepoContext::read` memoizes file bytes, and TOML parsing
of a few manifests is negligible next to the scan.

**Shared parser (`cargo/manifest.rs`, private).** It parses each `Cargo.toml`
found via `ctx.paths()`, matched by file name exactly. It returns, per manifest:

- the repo-relative path;
- `package`: name, plus a version declared as a literal string, as
  `{ workspace = true }`, absent, or invalid;
- `workspace`: `members` patterns, `exclude` patterns, and the
  `workspace.package.version` literal if present;
- an optional `package.workspace` override path.

Failures become a `ParseProblem` with the path, a fixed message, and an optional
line and column. Line and column are computed from the TOML error's byte span.
**The TOML error's `Display` text is never used**, because it embeds file
excerpts and multi-line formatting. This keeps diagnostic text stable, following
the same rule as the Phase 2 F1 fix.

**Workspace resolution (shared).**

1. Every manifest with a `[workspace]` table is a **workspace root**.
2. A package's workspace root is:
   - the explicit `package.workspace` path, resolved relative to the manifest,
     if present;
   - otherwise the nearest ancestor-or-self manifest with `[workspace]`.
3. The package is a **member** only if one of these holds:
   - its directory, relative to the root, matches a `members` pattern and no
     `exclude` prefix;
   - it is the root manifest's own `[package]`.
4. Patterns are compiled with `globset` using `literal_separator(true)`, so `*`
   does not cross `/`. This matches Cargo's `glob` semantics for typical
   patterns (`crates/*`).
5. A non-member package has `workspace: null`.

**Detector `cargo`** (category `ecosystem`):

- `applies`: any path whose file name is `Cargo.toml`.
- **`cargo-workspace` findings**, one per workspace root:
  - `path`: the root manifest.
  - `data`: `{ "members": [member manifest paths, sorted], "member_patterns": [raw],
    "exclude": [raw] }`.
  - `confidence`: High.
  - `sources`: the root manifest.
- **`cargo-package` findings**, one per `[package]`:
  - `path`: the manifest.
  - `data`: `{ "name": "...", "workspace": "<root manifest path>" | null }`.
  - `confidence`: High.
  - `sources`: the manifest, plus the root manifest when it is a member.
- **Diagnostics** (this detector owns all parse-level diagnostics, so the version
  detector does not duplicate them):
  - **Invalid TOML:** `error` severity, `path` = manifest, message
    `"invalid TOML at line L, column C"`.
  - **Not UTF-8:** `error` severity, message `"manifest is not valid UTF-8"`.
  - **`[package]` without `name`:** `warning` severity,
    message `"package has no name"`.
  - **Unmatched `members` pattern:** a pattern that matches no manifest directory
    gets a `warning`, `"workspace member pattern matches no Cargo.toml: <pattern>"`.
- **Ecosystem leakage guard:** everything Cargo-specific (TOML keys, glob rules)
  stays in `detectors/src/cargo/`. `core` gains no Cargo knowledge.

**Detector `cargo-version`** (category `version-source`):

- `applies`: same as `cargo`.
- Every finding uses kind `cargo-toml`. The `data` keys are always present, using
  `null` when absent, so the shape is invariant:
  `{ "name", "version", "field", "inherited_from" }`.

| Case | `path` | `data` | Confidence | Diagnostic |
|---|---|---|---|---|
| Literal `package.version` | member manifest | `name`, `version`, `field: "package.version"`, `inherited_from: null` | High | None |
| `version.workspace = true`, root has `workspace.package.version` | member manifest | resolved `version`, `field: "workspace.package.version"`, `inherited_from: <root manifest>` | High | None |
| `version.workspace = true`, unresolvable | member manifest | `version: null`, `field: "workspace.package.version"`, `inherited_from: <root>` or `null` | Low | `warning`: `"inherited version cannot be resolved"` |
| `workspace.package.version` on a root | root manifest | `name: null`, `version`, `field: "workspace.package.version"`, `inherited_from: null` | High | None |
| `version` not valid semver (e.g. `"1.0"`) | manifest | raw string in `version` | Low | `warning`: `"version is not valid semver"` |
| `version` has the wrong type (e.g. an integer) | manifest | No finding | — | `warning`: `"version has an unsupported type"` |
| No `version` key | — | No finding. Cargo treats a missing version as an implicit `0.0.0`, unpublishable package; that is not a version source. | — | None |
| Unparseable manifest | — | Skipped silently; `cargo` already reported it | — | None |

`sources` holds the member manifest, plus the root manifest when the version is
inherited. **Multiple version sources** are expected and reported independently;
the engine never picks a "primary" version (`detector-architecture.md` §6).

### 4.2 Detector `git-tags` (category `version-source`)

- **Input:** `ctx.git_tags()` (§5). The detector never runs `git` itself.
- **`applies`:** `ctx.git_tags().is_some()`, meaning the root is a readable git
  repository.
- **Output:** one finding of kind `git-tag` per **version-like** tag:
  - `path`: `null`, since a tag is repo-global.
  - `sources`: `[]` (see §8, limitation 1).
  - `data`: `{ "name": "v1.2.0", "version": "1.2.0", "prefix": null | "repo-intelligence-core", "annotated": true, "commit": "<hex>" | null }`.
- **Version-like classification** (deterministic, documented in the module).
  Try these in order and stop at the first success:
  1. The whole name, with an optional leading `v` removed, parses with
     `semver::Version::parse`. The tag is version-like with **High** confidence
     and `prefix: null`. Examples: `v1.2.3`, `1.2.3`, `v2.0.0-rc.1`.
  2. Otherwise, scan separators `-`, `/`, `@` from left to right. At the first
     separator where the remainder (optional leading `v` removed) parses as
     semver, the tag is version-like with **Medium** confidence. `prefix` is the
     text before that separator. Examples: `repo-intelligence-core-v0.1.0`
     (cargo-release's default workspace format), `core/v1.0.0`, `pkg@1.0.0`.
  3. Otherwise, the tag is **not version-like** and emits nothing.
- **Not every tag is a release:**
  - Non-version tags (`nightly`, `before-refactor`, `v1.2`, `v01.2.3`) produce
    no finding and no diagnostic. They are valid git state, not errors.
  - Two tags naming the same version (`v1.0.0` and `1.0.0`) both appear. The
    Skill arbitrates (`detector-architecture.md` §6).
- **`commit`** is the peeled commit id. It is `null` when the tag points at a
  non-commit object, such as a tree, a blob, or a tag-of-tag deeper than one
  level (§5).
- **Diagnostics:** none from the detector. Git-level problems are scan
  diagnostics (§5).

### 4.3 Registration

```rust
pub fn default_registry() -> DetectorRegistry  // in repo-intelligence-detectors
```

It registers `cargo`, `cargo-version`, and `git-tags`, in that order.
Registration order does not affect output (Phase 2 total order), but it is fixed
for readability. Duplicate ids panic once Task 3.0 is done.

## 5. Git Integration

### Decision D1: run the system `git` binary from `core::scan`

| Option | Verdict |
|---|---|
| **System `git`**, `for-each-ref` (chosen) | It is existing tooling, so no new dependency. It handles loose refs, `packed-refs`, the reftable format, packfiles, worktrees, and `.git` files (submodules). Annotated vs lightweight is one format field. |
| Hand-parse `.git/refs/tags` and `packed-refs` | No subprocess, but it **cannot tell annotated from lightweight** for loose refs without decompressing objects and parsing packfiles, and it breaks on reftable repositories (git ≥ 2.45). Rejected. |
| `gix` (gitoxide) | Pure Rust and full-featured, but a very large dependency tree with a fast-moving MSRV. Disproportionate for one `for-each-ref`. Rejected for now; it is the migration path if a subprocess becomes a problem. |
| `git2` (libgit2) | C build. Its default features pull `openssl-sys`, which is on the guard's denylist. Rejected. |

**Why in `core`, not in the detector:** detectors must be pure functions of
`&RepoContext` (`detector-architecture.md` §1). Collecting tags during `scan`
keeps detectors pure and unit-testable with synthetic contexts. It also keeps the
"single point of environment contact" property of the scan.

### How a repository is detected

`<root>/.git` exists, as a directory or as a file (a gitfile from a worktree or
submodule). There is no upward search: scanning a subdirectory of a repository
yields no git data. This matches Phase 2's `parents(false)` policy.

To prevent git's own upward discovery, git runs against the **canonical** root,
and `GIT_CEILING_DIRECTORIES` is set to the canonical parent. Git ignores
relative and symlinked ceiling entries. Before this was fixed (Phase 3 review,
F1), a relative root with a broken `.git` inside another repository reported
that outer repository's tags. A regression test covers it.

Each `git` call is bounded by a 30-second timeout. A timeout produces the warning
`"git timed out; tags not collected"`. This is the one case where output can
depend on machine speed.

### How tags are enumerated

```
git -C <root> -c core.fsmonitor=false for-each-ref \
    --format=%(refname)%00%(objecttype)%00%(*objecttype)%00%(objectname)%00%(*objectname) \
    refs/tags
```

**Hardened environment.** The engine must not read ambient configuration
(`architecture.md` §5):

- Call `env_clear()`, then set only:
  - `PATH`, needed to locate `git`; this is the single documented ambient input;
  - `GIT_CONFIG_NOSYSTEM=1`;
  - `GIT_CONFIG_GLOBAL=` the null device (`/dev/null` on Unix, `NUL` on Windows);
  - `GIT_CEILING_DIRECTORIES`;
  - `LC_ALL=C`;
  - `GIT_TERMINAL_PROMPT=0`.
- Repository-local config is still read by git. It cannot change `for-each-ref`
  output with an explicit `--format`.
- `core.fsmonitor=false` ensures no configured helper program runs. This
  preserves the "engine never executes code found in the repo" posture
  (`architecture.md` §9).

**Parsing.** Each output line is `refname, type, peeled type, id, peeled id`,
separated by NUL. Ref names cannot contain NUL or newline.

- `name` = `refname` without the `refs/tags/` prefix.
- `objecttype == "tag"` → annotated. `objecttype == "commit"` → lightweight,
  with `commit = objectname`.
- For annotated tags, `commit = *objectname` if `*objecttype == "commit"`,
  otherwise `null`. `%(*…)` peels one level, so a tag-of-tag yields `null`
  (documented, rare).
- Tags are sorted byte-wise by name. Ids are stored as git prints them (SHA-1 or
  SHA-256 hex).

**Shallow clones.** `git rev-parse --is-shallow-repository` runs with the same
hardening. If it prints `true`, add a `warning` scan diagnostic:
`"shallow clone: tag list may be incomplete"`. CI checkouts are often shallow and
tagless, which a release Skill must know about.

### Failure handling

All of these are scan diagnostics with the `scanner` detector id and fixed
messages. The tool never echoes git's stderr, which carries absolute paths and
localized text.

| Situation | `git_tags()` | Diagnostic |
|---|---|---|
| No `<root>/.git` | `None` | None. It is not a repository; `git-tags` doesn't apply. |
| `.git` present, `git` not on `PATH` | `None` | `warning`: `"git not found; tags not collected"` |
| `.git` present, git exits non-zero (corrupt repository, "dubious ownership", unsupported format) | `None` | `warning`: `"git metadata unreadable; tags not collected"` |
| Repository with no tags | `Some(&[])` | None |
| Unparseable output line (should not happen) | That line is skipped | `warning`: `"unrecognized git output line skipped"` |

"Dubious ownership" (`safe.directory`) is reported, not bypassed. Passing
`-c safe.directory=*` would silence a git security check. That is a decision for
the user, not for the engine.

### Core API additions

```rust
#[non_exhaustive]
pub struct GitTag { pub name: String, pub annotated: bool, pub commit: Option<String> }
impl GitTag { pub fn new(name, annotated, commit) -> Self }

impl RepoContext {
    pub fn git_tags(&self) -> Option<&[GitTag]>;            // None = no readable repo
    pub fn with_git_tags(self, tags: Vec<GitTag>) -> Self;  // synthetic contexts (tests)
    pub fn contains(&self, path: &str) -> bool;             // Task 3.0
}
```

## 6. Cargo Integration

Covered in detail in §4.1. Edge-case policy summary:

| Case | Behavior |
|---|---|
| Single package, no `[workspace]` | One `cargo-package` with `workspace: null`. One `cargo-toml` version source if a version is declared. |
| Root package that is also a workspace (`[package]` + `[workspace]`) | `cargo-workspace` plus `cargo-package`. The package is a member of its own workspace. |
| Virtual manifest (`[workspace]` only) | `cargo-workspace` only, plus a `cargo-toml` for `workspace.package.version` if present. |
| Nested workspace (a member dir has its own `[workspace]`) | Nearest ancestor-or-self root wins, matching Cargo's lookup. Each root gets its own finding. |
| Packages under a workspace dir but not matched by `members` (e.g. this repo's `fixtures/*`) | `workspace: null`. Cargo would also make these path-dependency members or reject them; path-dependency auto-membership is **not modeled** in Phase 3 (documented limitation). |
| `members` glob matches a directory with no `Cargo.toml` | Not a member; `warning` diagnostic. |
| Inherited version, root lacks `workspace.package.version` | Low-confidence finding with `version: null`, plus a `warning`. |
| Missing version | No version source (implicit `0.0.0`); no diagnostic. |
| Invalid TOML or non-UTF-8 manifest | `error` diagnostic from `cargo`; manifest skipped by both detectors. Exit stays 0. |
| Multiple manifests or versions | All reported, path-anchored. None is chosen as primary. |
| `Cargo.toml` inside ignored dirs (`target/`) | Never seen. Phase 2's `.gitignore` handling excludes them. |

**This repository's own report:** add `exclude = ["fixtures"]` to the root
`[workspace]`. This is Cargo-correct, because it stops Cargo from treating
fixture packages as stray members. The self-analysis then states explicitly
that `fixtures/*` are not members. Fixture manifests, including `messy`'s broken
ones, still appear in the self-report with their diagnostics. That is truthful
output, and it remains exit 0.

## 7. CLI Integration

### Syntax

```
repo-intelligence analyze <PATH> [--format json]
```

- **`<PATH>` is required.** Explicit over magic; the roadmap form names it.
  Relative paths are passed through unchanged; Phase 2 verified that `.`, `./`,
  and `../x` scan identically. The report contains no absolute paths, so there
  is nothing to canonicalize.
- **`--format`** is a `clap::ValueEnum` with the single value `json`, defaulting
  to `json`. The exact roadmap command works. Adding `--format` later formats is
  additive.
- No other flags. There is no `-v` because nothing is logged yet: `tracing` is
  deferred until there are log events. This is a recorded deviation from
  `workspace-layout.md` §2.4.

### Behavior

1. Parse arguments with `Cli::try_parse()`, not `parse()`. Clap's default exit
   code for usage errors is **2**, which would collide with "fatal engine error".
   - On `--help` or `--version`: print to stdout and exit 0.
   - On any other parse error: print to stderr and exit **1**.
2. `core::scan(path)`. Map `EngineError`:
   - `RootNotFound` or `RootNotADirectory` → stderr message, exit **1**
     ("path does not exist" is a usage error per `architecture.md` §6).
   - `RootUnreadable` → exit **2**.
3. `core::run(&ctx, &detectors::default_registry())`, wrapped in
   `std::panic::catch_unwind`. A panicking detector → stderr
   `"internal error: analysis panicked"`, exit **2**, nothing on stdout. This
   gives panic isolation at the adapter, for the whole run, without changing
   `core`.
4. Inject the runtime values:
   - `tool_version = env!("CARGO_PKG_VERSION")`;
   - `analyzed_at` = current UTC time as `YYYY-MM-DDTHH:MM:SSZ` (RFC 3339, second
     precision). A ~20-line civil-date formatter in `cli`, unit-tested against
     known instants (epoch, 2024-02-29, 2026-11-01); no `chrono`, `time`, or
     `humantime`.
5. Serialize with `serde_json::to_string_pretty` plus a trailing newline. Write
   to a locked stdout and flush. A write failure (e.g. a closed pipe) → stderr
   message, exit **2**.

Exit codes are exactly `architecture.md` §6:

- **0:** report written. It may contain diagnostics, including all `messy`
  cases.
- **1:** usage error. This includes a nonexistent path or a path that is not a
  directory.
- **2:** fatal; stdout is not a report.

**stdout carries only the report.** Every other byte goes to stderr. The one
conventional exception is `--help`/`--version` output.

## 8. JSON Contract Integration

All output uses existing `json` types. **`SCHEMA_VERSION` stays `0.1.0`, and
`schema/report.schema.json` is unchanged.** New `kind` strings are not schema
changes (`phase-1-implementation-plan.md` §5.2).

| Kind | Category | Detector | `path` | `data` keys (always present) |
|---|---|---|---|---|
| `cargo-workspace` | `ecosystem` | `cargo` | root manifest | `members`, `member_patterns`, `exclude` |
| `cargo-package` | `ecosystem` | `cargo` | manifest | `name`, `workspace` |
| `cargo-toml` | `version-source` | `cargo-version` | manifest | `name`, `version`, `field`, `inherited_from` |
| `git-tag` | `version-source` | `git-tags` | `null` | `name`, `version`, `prefix`, `annotated`, `commit` |

The catalogue is documented in `detectors/src/lib.rs` and pinned by the golden
files. `json`'s hand-written `example_report.json` uses illustrative kinds; it
stays untouched because `json` is unchanged.

**Consistency:**

- Every `data` object has a fixed key set, built with `serde_json::json!` in
  declared key order. Arrays are sorted byte-wise.
- Diagnostics use fixed phrases and repo-relative paths. No OS or library error
  text, matching the Phase 2 F1 rule.
- Ordering is entirely the Phase 2 total order (`crates/core/src/pipeline.rs`).
  Findings sort by `path` (with `null` last), then `category`, `kind`,
  `provenance.detector`, `confidence` (ascending: Low, Medium, High),
  `provenance.sources`, and finally `data` as compact JSON.
  - `git-tag` findings have `path: null`, so they come after every Cargo finding.
  - Among `git-tag` findings, **confidence decides before name**. Medium
    (prefixed) tags come before High (plain) tags. Within one confidence level,
    `data` decides, and its first key is `name`, so tags sort byte-wise by name.
  - Expected `git-tag` order in the `rust-workspace` git-overlay golden:
    `alpha-v2.1.0` (Medium), then `v0.1.0`, then `v0.2.0` (both High).
    `nightly` is absent.
  - The same rule applies to `cargo-toml` findings that share a path: a Low
    finding comes before a High one.

**Contract limitations found (none blocking; no change proposed now):**

1. **`Provenance.sources` can only name files.** A git tag comes from a ref, not
   a file in the listing, and reftable repositories have no per-tag file at all.
   Phase 3 uses `sources: []` and carries the ref in `data.name`. The smallest
   future fix is an **additive** optional `Provenance.refs: Vec<String>` (a minor
   bump), but only if Phase 4 shows the Skill needs it. The source comment
   "may be empty for synthesized facts" should be widened to "for facts not
   derived from files" when `json` is next touched. That is documentation only.
2. **There is no `info` severity.** Shallow-clone and symlink notices use
   `warning`. This is acceptable; an `info` level would be an additive minor bump
   if Phase 4 shows warnings are too noisy.

If implementation finds a result the contract cannot express, stop and record it
here. Do not change `json` inside Phase 3 without an explicit, reviewed version
bump.

## 9. Testing Strategy

**Isolation rules (all tests):**

- **No developer state:**
  - No test reads the developer's repository, `~/.gitconfig`, or the system
    git config.
  - Git test repositories are created in `tempfile` directories by a helper that
    runs `git` with the same hardened environment as `core`.
- **Deterministic commit ids:** the helper also sets fixed
  `GIT_AUTHOR_NAME`/`EMAIL`/`DATE`, `GIT_COMMITTER_*`, and
  `-c init.defaultBranch=main`, so commit ids are identical on every machine.
- **Missing `git`:** if `git` is not installed, git-dependent tests print a skip
  note and return. CI (Ubuntu) has `git`, so they always run there.

**Unit tests (`detectors`, synthetic `RepoContext` via `from_paths` +
`with_git_tags` + tempdir files):**

- **manifest parser:**
  - literal / inherited / missing / wrong-type versions;
  - `package.workspace` override;
  - invalid TOML → line and column;
  - non-UTF-8 bytes.
- **`cargo`:**
  - single package;
  - virtual workspace;
  - root package + workspace;
  - nested workspace;
  - glob members and `exclude`;
  - a non-member under a workspace dir;
  - an unmatched member pattern;
  - parse diagnostics appear exactly once.
- **`cargo-version`:** every row of the §4.1 table, including the
  invalid-semver and unresolvable-inheritance diagnostics.
- **`git-tags`:**
  - the classification table (exact, `v`-prefixed, prerelease, `crate-v1.2.3`,
    `core/v1.0.0`, `pkg@1.0.0`, `1.2.3-1.2.3`, `nightly`, `v1.2`, `v01.2.3`);
  - annotated vs lightweight;
  - `commit: null`;
  - `applies()` is false when `git_tags()` is `None`.

**`core` tests (real temp git repos):**

- lightweight and annotated tags, with the peeled commit equal to the commit id;
- no `.git` → `None`, no diagnostic;
- `.git` in a parent but not at the root → `None`;
- a shallow clone (`git clone --depth 1 file://…`) → warning;
- `PATH` without `git` → `None` plus the "git not found" warning, simulated by
  having `core`'s runner take the executable path as a parameter;
- a corrupt `.git` → the "unreadable" warning;
- the output is independent of a hostile `HOME/.gitconfig`: the test writes a
  config with `tag.sort` and an alias and asserts identical results.

**Fixture-backed integration (`detectors/tests/`):** `scan` + `run` over each
committed fixture with `default_registry()`, asserting the finding kinds and
diagnostics per fixture.

**Committed fixtures (`fixtures/`, no `.git` directories: git cannot commit
them):**

- **`rust-single-crate/`:** `Cargo.toml` (`[package] name = "single"`,
  `version = "0.1.0"`) and `src/lib.rs`.
- **`rust-workspace/`:**
  - root virtual manifest with `members = ["crates/*"]`, `exclude = ["crates/skip"]`,
    and `[workspace.package] version = "2.1.0"`;
  - `crates/alpha` (inherits the version);
  - `crates/beta` (literal `0.3.0`);
  - `crates/skip` (excluded, with its own version);
  - `tools/helper` (outside `members`).
- **`messy/`:**
  - root workspace with `members = ["crates/*", "missing/*"]`;
  - `crates/broken` (invalid TOML);
  - `crates/badver` (`version = "1.0"`);
  - `crates/orphan` (`version.workspace = true`, but the root has no
    `workspace.package.version`);
  - `crates/binary` (a non-UTF-8 `Cargo.toml`, committed with
    `.gitattributes` `binary`);
  - `crates/noname` (`[package]` without a name).

**Golden tests (`cli/tests/golden.rs`, through the real binary via
`env!("CARGO_BIN_EXE_repo-intelligence")`; no `assert_cmd`):**

- **Plain:** each of the three fixtures is analyzed and compared byte-for-byte
  with `fixtures/<name>/expected.json`, after replacing `analyzed_at` with a
  fixed placeholder. The test separately asserts that the real `analyzed_at`
  matches the RFC 3339 shape and that `tool_version == CARGO_PKG_VERSION`.
- **With git:** `rust-single-crate` and `rust-workspace` are copied into tempdirs
  and given deterministic histories and tags:
  - `v0.1.0`, lightweight;
  - `v0.2.0`, annotated;
  - `alpha-v2.1.0`, annotated;
  - `nightly`.
  They are compared with `fixtures/<name>/expected.git.json`. This proves the
  git path end to end with stable commit ids.
- **Regeneration:** an `#[ignore]`d `regenerate_golden` test rewrites the
  expected files. This mirrors Phase 1's `regenerate_schema`. Golden diffs get
  code-level review (`workspace-layout.md` §6).

**CLI tests (`cli/tests/cli.rs`):**

- exit 0 on a valid fixture, and stdout parses as a `json::Report`;
- `messy` → exit 0, diagnostics present, and stdout is still valid JSON;
- nonexistent path → exit 1, empty stdout, non-empty stderr;
- a file path → exit 1;
- unknown flag → exit 1 (not clap's 2);
- `--format yaml` → exit 1;
- `--help` → exit 0;
- an unreadable root (Unix, non-root user) → exit 2, empty stdout;
- the RFC 3339 formatter unit tests.

**Determinism:**

- Run each golden case twice and assert identical bytes, except `analyzed_at`.
- Analyze the same git-overlaid fixture at two different tempdir locations and
  assert identical bytes. This extends the Phase 2 cross-location proxy to the
  git path.

## 10. Task Breakdown

Each task ends with `cargo fmt --check`, `cargo clippy --workspace --all-targets
-D warnings`, `cargo test --workspace`, and `scripts/check-engine-deps.sh` green.
Plan one commit per task.

### Task 3.0: Close the Phase 2 detector-boundary items
- **Objective:**
  - `DetectionOutput`: `#[non_exhaustive]` plus `DetectionOutput::new` (N1).
  - `DetectorRegistry::register` panics on a duplicate id (N3).
  - `run()` rejects a detector finding if its `path`, or any entry in
    `sources`, is not a valid repo-relative path (N2). The rule works on
    `/`-separated components, never on substrings:
    - it must not be empty;
    - it must not start with `/` or `\`;
    - its first component must not be a drive prefix (a letter followed by `:`);
    - no component may be empty, `.`, or `..`, which also rules out `a//b` and
      trailing slashes.

    A `\` *inside* a component is allowed, because `scan` lists such Unix file
    names and detectors may anchor to them. A `..` inside a name (`a..b.txt`) is
    also allowed. Rejection uses the existing identity-mismatch diagnostic
    mechanism. `path: None` and empty `sources` stay allowed.
  - Add the no-absolute-paths / no-OS-text rule to the `Detector` docs.
  - Add `RepoContext::contains`.
- **Files:** `crates/core/src/{detector,pipeline,context}.rs`, core tests.
- **Depends on:** none. Phase 2 is committed (`374d878`).
- **Acceptance:**
  - Existing tests pass after switching to the constructor.
  - New tests cover:
    - a duplicate-id panic;
    - rejection of `/abs`, `\abs`, `C:/x`, `a/../b`, `./a`, `a//b`, `a/`, `""`;
    - acceptance of `a..b.txt`, `dir\name.txt`, and every path `scan` lists;
    - `contains`.

### Task 3.1: Git tag metadata in `core`
- **Objective:**
  - A private `git.rs` with the hardened runner; the executable path is a
    parameter so tests can simulate a missing `git`.
  - `GitTag`, `git_tags()`, and `with_git_tags()`.
  - `scan()` collects tags and the shallow warning.
  - Git test helper in `crates/core/tests/common/`.
- **Files:** `crates/core/src/{git,context,lib}.rs`, `crates/core/tests/git.rs`.
- **Depends on:** 3.0.
- **Acceptance:** every row of the §5 failure table, plus the hostile-global-config
  test; the guard is still green (no new dependencies).
- **Tests:** §9 "core tests".

### Task 3.2: `detectors` scaffolding
- **Objective:**
  - Add `toml`, `semver`, and `globset` to `[workspace.dependencies]` and to
    `detectors`. Resolver 3 picks MSRV-compatible versions.
  - Add an empty `default_registry()` and the kind catalogue doc in `lib.rs`.
  - Add `exclude = ["fixtures"]` to the root workspace.
- **Files:** `Cargo.toml`, `Cargo.lock`, `crates/detectors/{Cargo.toml,src/lib.rs}`.
- **Depends on:** 3.0.
- **Acceptance:** builds on 1.85; guard green; `cargo tree -p repo-intelligence-detectors`
  shows no network crate.

### Task 3.3: Cargo manifest parser
- **Objective:** private `cargo/manifest.rs` and workspace resolution (§4.1).
- **Depends on:** 3.2.
- **Acceptance / tests:** parser and resolution unit tests (§9), including line
  and column for invalid TOML and glob members with `exclude`.

### Task 3.4: Detector `cargo`
- **Objective:** `cargo-workspace` and `cargo-package` findings plus parse
  diagnostics. Register it.
- **Depends on:** 3.3.
- **Acceptance / tests:** the `cargo` unit tests in §9; parse diagnostics are
  emitted once.

### Task 3.5: Detector `cargo-version`
- **Objective:** `cargo-toml` findings per the §4.1 table. Register it.
- **Depends on:** 3.3.
- **Acceptance / tests:** one test per table row.

### Task 3.6: Detector `git-tags`
- **Objective:** classification and `git-tag` findings (§4.2). Register it.
- **Depends on:** 3.1, 3.2.
- **Acceptance / tests:** the classification table, annotated vs lightweight,
  and `applies()`, all on synthetic contexts.

### Task 3.7: Fixtures and detector integration tests
- **Objective:** add `fixtures/rust-single-crate`, `rust-workspace`, and `messy`
  (§9). Add `detectors/tests/fixtures.rs` running `scan` + `run` +
  `default_registry()`.
- **Depends on:** 3.4–3.6.
- **Acceptance:**
  - Each fixture yields exactly the expected kinds and diagnostics.
  - `messy` produces only diagnostics and findings, never a panic.

### Task 3.8: CLI `analyze`
- **Objective:**
  - `clap` derive with `try_parse` and exit-code mapping.
  - `catch_unwind`.
  - Injection, including the RFC 3339 formatter.
  - Pretty JSON on stdout.
  - Replace the Phase 0 stub `main`.
- **Files:** `crates/cli/{Cargo.toml,src/main.rs}` (plus `src/time.rs` if
  `main.rs` exceeds ~150 lines).
- **Depends on:** 3.4–3.6.
- **Acceptance / tests:** `cli/tests/cli.rs` (§9).

### Task 3.9: Golden tests
- **Objective:**
  - `cli/tests/golden.rs`: plain and git-overlay cases.
  - The `regenerate_golden` ignored test.
  - Commit the `expected.json` and `expected.git.json` files after reviewing
    them by hand against §4–§6.
- **Depends on:** 3.7, 3.8.
- **Acceptance:**
  - Goldens match.
  - Two runs and two locations give identical bytes.
  - Each expected file was reviewed line by line, and each finding is traced
    back to a fixture file.

### Task 3.10: Self-analysis acceptance
- **Objective:** run `cargo run -p repo-intelligence-cli -- analyze . --format json`
  on this repository and check the report against §1's success definition. This
  is a manual check plus a CI step, **not** a `cargo test`, because tests must
  not depend on the developer's repository.
- **Depends on:** 3.8.
- **Acceptance:**
  - Exit 0.
  - Four `crates/*` members, each with `0.0.0` inherited from the root.
  - Fixture packages listed with `workspace: null`.
  - `messy` diagnostics present.
  - Tags (if any) classified per §4.2.

### Task 3.11: CI, guards, docs, deviations
- **Objective:**
  - **CI steps:**
    - run the CLI on the checkout and require exit 0 and parseable JSON
      (`python3 -m json.tool`);
    - fetch tags in checkout (`fetch-depth: 0`) so the git path is exercised.
  - **Process-use guard:** extend `check-engine-deps.sh` with a grep check that
    `std::process` appears in engine crates only in `crates/core/src/git.rs`.
  - **Docs:** update this plan's status, record deviations (§11), tick §12.
- **Depends on:** all.
- **Acceptance:** CI green; the guard fails when `Command` is added to a detector
  (checked once by hand).

```
3.0 ─▶ 3.1 ─────────────────────▶ 3.6 ─┐
   └─▶ 3.2 ─▶ 3.3 ─▶ 3.4 ──────────────┼─▶ 3.7 ─┐
                  └─▶ 3.5 ─────────────┘        ├─▶ 3.9 ─▶ 3.10 ─▶ 3.11
                                   3.4–3.6 ─▶ 3.8 ┘
```

## 11. Risks and Design Decisions

### Decisions

| ID | Decision | Alternatives | Why |
|---|---|---|---|
| D1 | System `git` via a hardened subprocess, inside `core::scan` | Hand-parsed refs; `gix`; `git2` | §5. Correct for packs, reftable, worktrees, and annotated tags; zero new dependencies; detectors stay pure. |
| D2 | Two Cargo detectors sharing one parser | One mixed detector; multi-category trait | §4.1. Respects Phase 2's category enforcement without changing `core`. |
| D3 | `toml` crate, generic `toml::Table` | `cargo metadata`; `cargo_toml` crate | `cargo metadata` executes `cargo`, may hit the network, and fails on broken manifests, which are exactly the cases to report. `cargo_toml` adds weight and opinions. A generic table gives full control over diagnostics. |
| D4 | `semver` crate for version validity and tag classification | A hand-written parser | Small, dependency-free, and canonical (Cargo uses it). Hand parsing invites edge-case drift. |
| D5 | Only version-like tags become findings | Emit every tag | Non-release tags are not version sources. Emitting thousands of tags bloats reports. A count of non-version tags is an additive later option. |
| D6 | Synthesis stays the identity in Phase 3 | Link tag prefixes to crates; mark the "primary" version | Correlation is judgment and is ambiguous (prefix conventions vary). Phase 4's Skill does it from path-anchored facts. Revisit when a second consumer needs it. |
| D7 | `git-tag` uses `sources: []` | Fake `.git/refs/...` paths; a schema change now | Fake paths are wrong for packed refs and reftable. A schema change is premature (§8). |
| D8 | No `tracing` yet | Add `tracing` + `-v` now | Nothing to log. A dependency without a use is speculative. |
| D9 | Hand-written RFC 3339 formatter | `humantime`, `time`, `chrono` | About 20 lines, unit-tested, no dependency. Swap in `humantime` if it grows. |
| D10 | Golden files at `fixtures/<name>/expected*.json`; whole-report goldens in `cli` tests | Goldens in a test directory | Follows `workspace-layout.md` §6 exactly. |

### Risks

- **Architecture: `core` now spawns a process.**
  - **Risk:** it erodes "no ambient inputs".
  - **Mitigation:** one private module; `env_clear()` plus an explicit
    allowlist; a guard grep (Task 3.11); `PATH` is the only documented ambient
    input; failures degrade to a diagnostic.
  - **Exit path:** `gix` behind the same `git_tags()` accessor; nothing outside
    `core` changes.
- **Git: repository-local config is still honored.** Mitigated by `--format`
  (output is format-defined) and `core.fsmonitor=false`. Hooks do not run for
  `for-each-ref`.
- **Git: CI and shallow clones.** Tags are often absent there. Reported via the
  shallow-clone warning; the Skill must treat missing tags as "unknown", not
  "no releases".
- **Git: tag-of-tag peeling** gives `commit: null`. Documented; rare.
- **Cargo edge cases:**
  - Path-dependency auto-membership is not modeled.
  - Exotic globs (`**`, character classes) follow `globset`, which may differ
    from Cargo's `glob` in corner cases.
  - Both are documented limitations with tests pinning current behavior.
- **Schema compatibility:** none expected. Golden files will freeze the kind
  names and `data` shapes, so they **must** be reviewed before commit (Task 3.9).
  Renaming a kind after Phase 4 is a breaking change for the Skill.
- **Platform:**
  - The null device and `env_clear()` semantics differ on Windows: `git` may
    need `SystemRoot`. The runner handles Unix and Windows explicitly, but
    Windows is untested because CI is Ubuntu-only. This matches the Phase 2
    residual risk.
  - The non-UTF-8 fixture is a committed binary file, so it is portable.
- **Dependencies:**
  - `toml`, `semver`, and `clap` are new direct dependencies; `globset` is
    already present. All are offline crates.
  - The MSRV is protected by `resolver = "3"`.
  - The guard and `cargo tree` checks stay in Task 3.2's acceptance.
- **Over-engineering:**
  - **No** detector feature flags.
  - **No** config file.
  - **No** synthesis logic.
  - **No** `tracing`.
  - **No** public detector types.
  - **No** git abstraction trait, only one private function behind an accessor.
  - Each of these is deferred until a second use exists.
- **Self-analysis noise:** fixture packages and `messy` diagnostics appear in
  this repo's own report. This is accepted as truthful. `exclude = ["fixtures"]`
  makes their non-membership explicit.

## 12. Phase Completion Criteria

Phase 3 is complete when **all** of the following hold:

- [x] **Single crate:** `repo-intelligence analyze fixtures/rust-single-crate --format json`
      exits 0 and matches `expected.json`. The report contains one
      `cargo-package` and one `cargo-toml` (`0.1.0`, `package.version`, High).
- [x] **Workspace:** `analyze fixtures/rust-workspace --format json` exits 0 and
      matches `expected.json`. The report shows:
      - the workspace members;
      - the inherited `2.1.0` with `inherited_from`;
      - `beta`'s literal `0.3.0`;
      - `skip` and `helper` as non-members.
- [x] **Git:** the git-overlay goldens match. Lightweight and annotated tags are
      distinguished, commit ids are correct, `alpha-v2.1.0` has
      `prefix: "alpha"` and Medium confidence, and `nightly` is absent.
- [x] **Messy:** `analyze fixtures/messy --format json` exits 0 and matches
      `expected.json`. Diagnostics are reported for:
      - invalid TOML (with line and column);
      - a non-UTF-8 manifest;
      - an invalid-semver version;
      - unresolvable inheritance;
      - a missing name;
      - an unmatched member pattern.
- [x] **Self-analysis:** on this repository, the report shows the four
      `crates/*` packages as members inheriting `0.0.0` (Task 3.10), and it runs
      in CI.
- [x] **Schema:** every report deserializes as `json::Report`. `SCHEMA_VERSION`
      is `0.1.0`, and `schema/report.schema.json` and the `json` crate are
      unchanged.
- [x] **Determinism:** reports are byte-identical across two runs and across two
      tempdir locations (except `analyzed_at`), including the git path.
- [x] **Isolation:** no test reads the developer's repository or global/system
      git config; the hostile-`~/.gitconfig` test passes.
- [x] **Exit codes:** match `architecture.md` §6 in the CLI tests, including
      unknown flag → 1 and nonexistent path → 1.
- [x] **Boundaries:**
      - `core` names no detector;
      - detectors read only through `RepoContext`;
      - `std::process` appears only in `core/src/git.rs` (guard green);
      - `detectors` exports only `default_registry()`.
- [x] **Phase 2 carry-over:** N1–N3 are closed (Task 3.0).
- [x] **CI:** `fmt --check`, `clippy -D warnings`, `build`, `test`, the
      dependency/process guard, and the self-analysis step are all green.
- [x] **Deviations recorded:**
      - two Cargo detectors;
      - git subprocess in `core`;
      - no `tracing`;
      - no feature flags;
      - `sources: []` for tags.

## 13. Implementation Results and Deviations

**Verified (`cargo fmt --check`, `clippy -D warnings`, `build`, `test --workspace`,
`check-engine-deps.sh` all green):**

- Task 3.0: `DetectionOutput` is `#[non_exhaustive]` with `new()`. Duplicate ids
  panic. Findings with non-repo-relative `path` or `sources` are rejected with a
  diagnostic. `RepoContext::contains` added.
- Task 3.1: git tags collected in `core` through a hardened `git` invocation
  (`crates/core/src/git.rs`). Tests cover lightweight and annotated tags, tags
  naming a tree, shallow clones, subdirectories, identical ids across locations,
  a missing binary, a corrupt `.git`, a hostile `HOME/.gitconfig`, and the
  environment allowlist.
- Tasks 3.2–3.6: detectors `cargo`, `cargo-version`, `git-tags`, and
  `default_registry()`. Parser and membership logic are in `cargo/manifest.rs`.
- Task 3.7: fixtures `rust-single-crate`, `rust-workspace`, `messy`. Detector
  integration tests check exact findings and diagnostics per fixture.
- Task 3.8: `analyze <PATH> [--format json]` with exit codes 0/1/2. Tests in
  `crates/cli/tests/cli.rs`.
- Task 3.9: goldens `expected.json` (3 fixtures) and `expected.git.json` (2
  fixtures with a fixed history). Regeneration is stable: running it twice gives
  identical files. Only `tool_version` and `analyzed_at` are normalized.
- Task 3.10: self-analysis on this repository lists `crates/{cli,core,detectors,json}`
  as members of the root workspace, each inheriting `0.0.0`.
- Task 3.11: the CI workflow checks out with `fetch-depth: 0` and runs the
  self-analysis step. `check-engine-deps.sh` rejects `std::process` outside
  `crates/core/src/git.rs`. Checked by planting a `Command` in a detector: the
  guard reported FAIL.
- Schema: `schema/report.schema.json` and `crates/json` are unchanged.

**Deviations from the plan:**

1. `fixtures/` contains only the three committed inputs, plus `nested` from
   Phase 2. The git-overlay repositories are built in tempdirs at test time, since
   git cannot commit a nested `.git`.
2. Golden git history skips `expected*` files when copying a fixture. Otherwise
   the golden file would be part of the tree it describes, and regenerating
   would change every commit id. Found by the first regeneration run.
3. Goldens normalize `tool_version` as well as `analyzed_at`, so a version bump
   does not require regenerating goldens. The separate test
   `runtime_values_are_injected` checks the real value.
4. Added diagnostic `package.workspace does not name a workspace root`. The plan
   did not list it, but it is the only way to report a bad explicit override.
5. Plan row "a `members` glob matches a directory with no `Cargo.toml`": not
   implemented as its own diagnostic. A pattern that matches no manifest directory
   gets one warning (`... matches no Cargo.toml: <pattern>`), which covers the
   practical case.
6. `package.workspace` override resolution treats a target outside the repository
   as "not a workspace root" (same diagnostic).
7. The panic hook is not replaced. The default message reaches stderr, followed
   by our `internal error` line. stdout stays clean either way.
8. `exclude = ["fixtures"]` is in the root `[workspace]` as planned. Cargo
   itself does not require it for these fixtures, so it documents intent.
9. Added a 30-second timeout per `git` call (§5), with the warning
   `"git timed out; tags not collected"`.
10. An invalid `workspace.package.version` is warned about once, at the root
    that declares it. Inheriting members get a Low-confidence finding without
    repeating the warning.

**Phase 3 review fixes (F1–F9):**

- **F1:** git runs on the canonical root, with a canonical ceiling. Regression
  test `broken_dot_git_under_relative_root_does_not_leak_parent_tags`. It was
  confirmed to fail without the fix (it reported the parent's tags).
- **F2:** Cargo semantics pinned in `crates/detectors/tests/cargo_edge_cases.rs`:
  - a root package that is also a workspace;
  - nested workspaces;
  - valid and invalid `package.workspace` overrides;
  - wrong-type and invalid inherited versions;
  - a workspace below the repo root with explicit members and `exclude`;
  - globs not crossing directories.
- **F3:** see deviation 10.
- **F4:** git-dependent tests skip with a note when `git` is missing
  (`regenerate_golden` instead fails, since it cannot produce correct output).
  Test helpers use the platform null device.
- **F5:** detector diagnostics with non-repo-relative paths are now replaced by
  an `error` diagnostic with no path, so N2 is fully closed (test
  `detector_diagnostics_with_non_repo_relative_paths_are_replaced`). §5 is
  corrected for F1.
- **F6:**
  - Timeout added (deviation 9; unit test `slow_git_times_out_with_a_warning`).
  - The review's Windows concern was wrong: since Rust 1.58, `Command::new` does
    not search the current directory on Windows, so a planted `git.exe` there
    is not run. No change was needed.
- **F7:** accepted risk. The `messy` golden pins the `toml` crate's error
  position, and the golden test's doc says a moved position after a `toml`
  upgrade is an expected, reviewable diff.
- **F8:** `RepoContext::from_paths` and `with_git_tags` are `#[doc(hidden)]`. The
  eight-argument helper is replaced by a `VersionSource` struct, and the
  `clippy` allow is gone.
- **F9:** decision deferred to Phase 4, recorded here. The self-analysis report
  includes the `messy` fixture's `error` diagnostics, which is truthful. The
  Phase 4 Skill must decide which paths it reasons about (e.g. ignore
  `fixtures/`). If that needs engine support, it is an additive scoping option,
  not a change to Phase 3 output.

**Not verified (gaps, not claimed as tested):**

- `catch_unwind` at the CLI boundary: no test panics a detector to exercise it.
- `git` failure messages for "dubious ownership" and "unrecognized output line":
  the code paths exist but have no test.
- `git` config files in the repository itself (`.git/config`): git reads these,
  as the plan states. Not tested.
- Windows and macOS: CI is Ubuntu-only. Canonical paths (`\\?\` stripping),
  `SystemRoot`, and the null device are untested there.
- The remote CI run has not been executed. Only the commands were run locally.
- Commits: none. The plan says one commit per task; nothing was committed.

**Checklist:** every §12 item is ticked, except that the CI bullet is verified
only locally (see above).

