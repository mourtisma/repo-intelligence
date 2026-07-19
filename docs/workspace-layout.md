# Workspace Layout

> Status: Design. No crates exist yet. This document specifies the intended
> Rust workspace so the first `cargo new` follows a deliberate structure rather
> than an accidental one.

## 1. Top-level shape

A single Cargo **workspace** with a virtual manifest at the root (the root
`Cargo.toml` has no `[package]`, only `[workspace]`). All crates live under
`crates/`.

```
repo-intelligence/
├── Cargo.toml                 # [workspace] virtual manifest
├── Cargo.lock                 # committed (see §5)
├── rust-toolchain.toml        # pinned toolchain (see §5)
├── README.md
├── PROJECT.md
├── LICENSE
├── docs/
│   ├── architecture.md
│   ├── workspace-layout.md
│   ├── detector-architecture.md
│   └── roadmap.md
├── crates/
│   ├── json/                  # repo-intelligence-json
│   ├── core/                  # repo-intelligence-core
│   ├── detectors/             # repo-intelligence-detectors
│   └── cli/                   # repo-intelligence-cli
├── skills/
│   └── universal-release-manager/   # Claude Code Skill (not a Rust crate)
├── fixtures/                  # sample repos for golden tests (see §6)
└── schema/                    # emitted JSON Schema artifacts (see json crate)
```

### Why `crates/` subdirectory instead of flat crates at the root

**Decision:** all library/binary crates live under `crates/`.

**Tradeoff:** paths are one level deeper (`crates/core` vs `core`).

**Why it wins:** the root stays legible. `docs/`, `skills/`, `fixtures/`, and
`schema/` are peers of the code, not buried among four crate directories. New
contributors see the top-level map at a glance. This is a well-worn convention
(ripgrep, cargo itself) and costs almost nothing.

**Alternative — flat layout:** fewer path segments, but the repo root becomes a
grab-bag of crate dirs mixed with docs/fixtures. Rejected for readability.

### Why the Skill lives in-repo under `skills/`

**Decision:** the Universal Release Manager Skill lives in the same repo, under
`skills/`, not in a separate repository.

**Tradeoff:** the repo now contains two languages (Rust + the Skill's
Markdown/config) and two release cadences.

**Why it wins now:** during bootstrap the CLI and its first Skill co-evolve
tightly; keeping them together makes it trivial to keep the JSON contract, the
CLI, and the Skill's expectations in sync in a single PR. The `skills/`
directory makes the boundary explicit.

**Migration path:** `PROJECT.md` requires Skills be "installable independently."
Co-location does not violate that — a Skill is installed by copying its
directory, not by depending on the Rust build. When a Skill matures or a third
party owns one, it can graduate to its own repo with no change to the engine.
This is called out in `roadmap.md`. We choose reversible co-location over
premature separation.

## 2. The crates

Naming: the crate *package* names carry the `repo-intelligence-` prefix
(publishable, unambiguous on crates.io); the *directory* names are short
(`crates/core`) for ergonomics. Library names may be short too (`use
repo_intelligence_core as core;` is a per-consumer choice).

### 2.1 `repo-intelligence-json`  (`crates/json`)

**Role:** the JSON contract. The stable, public data types that make up a
report, plus the `SCHEMA_VERSION` constant.

- Contains **only** serde-serializable data types and their (de)serialization.
- **No logic, no I/O, no detection.** No dependency on any other internal crate.
- Depends on: `serde`, `serde_json`, and a schema-emitting crate (e.g.
  `schemars`) to generate the language-neutral JSON Schema artifact into
  `schema/`.
- Exposes: `Report` (the envelope: `schema_version`, `tool_version`,
  `analyzed_at`, `findings`, `diagnostics`), `Finding`, `Diagnostic`,
  `Confidence`, `DetectorCategory`, `Provenance`, and `Severity`. There are
  **no** distinct top-level types for `RepoStructure`, `Language`,
  `Ecosystem`, `VersionSource`, `CiSystem`, or `ReleaseTooling` — these are
  represented as `Finding.category`/`kind` values (a closed `DetectorCategory`
  enum plus an open `kind` string), not as separate envelope sections. This
  reconciliation (a flat, path-anchored `findings` array over typed sections)
  is decided and justified in `phase-1-implementation-plan.md` §4.2 and §9.
  Exact shapes are defined by the JSON contract (see `architecture.md` §6–8
  and `detector-architecture.md`).

**Why a leaf crate with zero internal deps:** the contract is the most stable
and most widely depended-upon artifact. Making it a leaf guarantees a Skill,
test, or third-party tool can deserialize a report by depending on this one
small crate alone, never pulling in the engine. It also makes contract changes
reviewable in isolation.

**Alternative — put types in `core`:** rejected in `architecture.md` §3.

### 2.2 `repo-intelligence-core`  (`crates/core`)

**Role:** the domain and the engine. Owns:

- The `Detector` trait (see `detector-architecture.md`).
- The `RepoContext` (single read-only repository scan).
- The `DetectorRegistry` and the run pipeline (drive detectors → collect
  findings → synthesis pass → assemble `Report`).
- Determinism enforcement (sorted traversal, ignore handling, no ambient inputs).

Depends on: `repo-intelligence-json` (to build the report), and infrastructure
crates (`walkdir`/`ignore` for traversal, `serde` where needed). **Does not
depend on `detectors`** — detectors depend on core, never the reverse.

**Why core does not depend on detectors:** if `core` knew about concrete
detectors, adding a detector would touch core, and the dependency graph would
have a cycle-prone shape. Instead, core defines the *trait* and *registry*;
concrete detectors register themselves. This is the inversion that makes
extensibility cheap (see §3 below and `detector-architecture.md`).

### 2.3 `repo-intelligence-detectors`  (`crates/detectors`)

**Role:** all concrete detectors — Cargo, npm/package.json, pyproject, pom.xml,
go.mod, git tags, GitHub Actions, etc. Each detector implements the `core`
`Detector` trait and knows how to parse its ecosystem's manifests.

Depends on: `core` (trait + context), `json` (finding types), and per-detector
parsing crates (`toml`, `serde_json`, `serde_yaml`, `roxmltree` for XML/pom,
etc.).

**Why one detectors crate rather than one crate per detector (initially):**

- **Tradeoff:** a single crate means all detectors compile together and share a
  version; you can't publish "just the Cargo detector."
- **Why it wins now:** a crate-per-detector explosion (a dozen tiny crates)
  multiplies `Cargo.toml` maintenance, versioning, and CI matrix cost for no
  current benefit. Internally, detectors are still isolated *modules*
  (`detectors/src/cargo.rs`, `detectors/src/npm.rs`), so the code-level
  extensibility ("add a file") is preserved.
- **Migration path:** if a detector grows heavy dependencies that we don't want
  to force on all consumers (e.g. a detector needing a large parser), it can be
  split into its own crate later behind a Cargo **feature flag**. We design the
  module boundaries now so that split is mechanical. See `roadmap.md`.

**Feature flags:** the detectors crate should expose Cargo features per
ecosystem group (e.g. `rust`, `node`, `python`, `jvm`, `go`, `ci`) so a
size-sensitive consumer can compile a subset. Default features enable the common
set. This is the pressure valve that gives most of the benefit of crate-splitting
without the crate-count cost.

### 2.4 `repo-intelligence-cli`  (`crates/cli`)

**Role:** the binary. The only crate that touches process I/O, arguments, exit
codes, stdout/stderr discipline, and environment injection (timestamp, tool
version).

Depends on: `core`, `detectors`, `json`, plus `clap` (args) and `tracing` +
`tracing-subscriber` (stderr logging).

**Why the CLI is separate from a hypothetical `lib`:** the binary crate stays
tiny and untested-heavy-logic-free; all reusable behavior is in the libraries.
This is what makes a future second front-end (server, LSP) a matter of writing a
new adapter, not refactoring the engine.

**Registration wiring:** the CLI is where concrete detectors from the
`detectors` crate are registered into the `core` registry (e.g. a
`detectors::default_registry()` builder the CLI calls). This keeps `core`
ignorant of concrete detectors while giving the binary one obvious place that
assembles the full set.

## 3. The dependency graph

```
        cli
       / | \
      /  |  \
detectors |  |
     \   |  /
      \  | /
      core
       |
      json      (leaf; no internal deps)
```

- Edges point from dependent to dependency.
- **No cycles.** Enforced by the structure: `json` depends on nothing internal;
  `core` depends only on `json`; `detectors` on `core`+`json`; `cli` on all.
- Adding a detector touches **only** `detectors` (and one registration line the
  CLI already has). Adding a report field touches **only** `json` (+ whoever
  populates it). This localization is the payoff of the layering.

## 4. Shared workspace configuration

Use Cargo's `[workspace.package]` and `[workspace.dependencies]` (workspace
inheritance) so version, edition, license, and common dependency versions are
declared **once** at the root and inherited by each crate.

**Why:** prevents version drift across crates (e.g. two different `serde`
versions), centralizes the edition/MSRV bump to one place, and keeps each
crate's `Cargo.toml` short.

**Tradeoff:** a change to a shared dependency version is a root-level change that
affects everyone at once. That is acceptable and usually desirable for a
single-team workspace; it is exactly the coupling we want here.

## 5. Toolchain, lockfile, and hygiene

- **`rust-toolchain.toml`** pins the toolchain channel so every contributor and
  CI runner builds with the same compiler. Reproducible builds matter for a tool
  whose whole value is determinism.
- **Commit `Cargo.lock`.** This workspace produces a binary (the CLI), and we
  want reproducible builds and reproducible golden-test output. Libraries alone
  wouldn't commit it, but the binary tips the decision.
- **MSRV policy:** declare a Minimum Supported Rust Version in
  `[workspace.package]` and check it in CI. Keeps the door open for packagers.
- **Lints:** deny warnings in CI; enable `clippy` at workspace level. A
  deterministic-analysis tool benefits from an aggressive lint posture.

## 6. Fixtures and tests layout

```
fixtures/
├── rust-single-crate/        # minimal Cargo project
├── rust-workspace/           # multi-crate workspace
├── node-package/             # package.json
├── python-pyproject/         # pyproject.toml
├── go-modules/               # go.mod + tags
├── polyglot/                 # deliberately mixed repo
└── messy/                    # malformed manifests → exercises diagnostics path
```

- Each fixture has a committed **expected report** (`expected.json`) that golden
  tests compare against. Because the engine is deterministic and offline, these
  comparisons are exact.
- Golden tests live in the crate that owns the behavior: detector-specific
  fixtures under `detectors` tests, whole-report/CLI behavior under `cli` tests.
- A dedicated "messy" fixture ensures the errors-as-data path (diagnostics, still
  exit 0) stays covered — the failure mode we most want to protect.

**Why golden files over hand-written assertions:** a report is a large
structure; asserting field-by-field is tedious and under-covers. A golden file
captures the *entire* output, so any accidental change (including ordering
regressions that would break determinism) shows up as a diff in review.

**Tradeoff:** golden files must be regenerated when the schema legitimately
changes, and a careless "just update the goldens" can mask a real regression. We
mitigate with a review rule: golden-file diffs get the same scrutiny as code, and
schema changes must be accompanied by an intentional schema-version bump.

## 7. Summary of layout decisions

| Decision                          | Chosen                          | Main alternative rejected            |
|-----------------------------------|---------------------------------|--------------------------------------|
| Workspace vs many repos           | Single workspace                | Repo-per-crate (premature)           |
| Crate dirs location               | `crates/`                       | Flat at root (cluttered)             |
| Contract crate                    | Standalone `json` leaf          | Types inside `core`                  |
| Detectors granularity             | One crate, modules + features   | Crate-per-detector (crate explosion) |
| Skill location                    | In-repo `skills/`               | Separate repo (premature)            |
| Shared config                     | Workspace inheritance           | Per-crate duplicated versions        |
| `Cargo.lock`                      | Committed                       | Ignored (breaks reproducibility)     |
