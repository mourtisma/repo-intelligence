# Detector Architecture

> Status: Design. The `Detector` trait and its detectors are not implemented.
> This document defines the extension model so the first detector and the
> hundredth are added the same way.

## 1. What a detector is

A **detector** answers one focused question about a repository by inspecting the
already-scanned `RepoContext` and emitting **findings** and, when it hits
trouble, **diagnostics**. Examples:

- "Is there a Cargo workspace, and what crates/versions does it contain?"
- "Does this repo use GitHub Actions, and is there a release workflow?"
- "What git tags exist, and do they look like semver version sources?"

A detector is a **pure function of the context**: same context in → same
findings out. No network, no clock, no filesystem walking of its own (it queries
the context), no dependence on other detectors' execution.

This purity is the load-bearing property. Everything below exists to preserve it
while making detectors easy to add.

## 2. The core trait

The exact signature is an implementation detail, but the shape is fixed by the
architecture:

```rust
// in repo-intelligence-core

pub trait Detector: Send + Sync {
    /// Stable identifier, e.g. "cargo", "npm", "git-tags", "github-actions".
    /// Appears in diagnostics and (optionally) provenance. Never changes once shipped.
    fn id(&self) -> &'static str;

    /// Which category of question this answers (structure, language, ecosystem,
    /// version-source, ci, release-tooling). Drives how findings map into the report.
    fn category(&self) -> DetectorCategory;

    /// Cheap gate: should this detector even run against this context?
    /// e.g. the Cargo detector returns false if no Cargo.toml exists anywhere.
    /// Lets us skip parsing work and keeps findings sparse.
    fn applies(&self, ctx: &RepoContext) -> bool;

    /// The actual analysis. Returns findings and diagnostics; must not panic on
    /// malformed input — it reports a diagnostic instead.
    fn detect(&self, ctx: &RepoContext) -> DetectionOutput;
}
```

Where `DetectionOutput` carries `Vec<Finding>` and `Vec<Diagnostic>` (both from
the `json` crate's vocabulary, or core types that map to it).

### Why a trait object registry rather than an enum of detectors

**Decision:** detectors are trait objects (`Box<dyn Detector>`) held in a
`DetectorRegistry`.

**Tradeoff:** dynamic dispatch has a tiny runtime cost and you lose exhaustive
`match`-over-all-detectors at compile time.

**Why it wins:** adding a detector is *adding a type that implements a trait and
registering it* — it never requires editing a central `enum` or `match`. That is
the entire extensibility story. The dispatch cost is irrelevant next to
filesystem I/O.

**Alternative — an enum `Detector { Cargo, Npm, ... }` with a big match:**
compile-time exhaustiveness is nice, but every new ecosystem edits the shared
enum and every match on it, creating merge contention and a central bottleneck.
Rejected: it optimizes for a guarantee we don't need at the cost of the growth
axis we care about most.

### Why `applies()` is separate from `detect()`

Splitting a cheap gate from the real work lets the pipeline skip detectors that
obviously don't apply (no `go.mod` → skip the Go detector) without paying parsing
cost, and keeps the output sparse and honest — a detector that doesn't apply
contributes nothing rather than a pile of "not found" findings.

## 3. The registry and pipeline

```
DetectorRegistry (built by the CLI from detectors::default_registry())
        │
        ▼
for each detector where applies(ctx):        ← independent, order-free, parallelizable
        run detect(ctx) → findings + diagnostics
        │
        ▼
collect all findings + diagnostics
        │
        ▼
SYNTHESIS PASS (see §5): correlate findings across detectors
        │
        ▼
assemble Report (json crate)
```

- The registry is assembled once (by the CLI, so `core` stays ignorant of
  concrete detectors — see `workspace-layout.md` §2.4).
- Detection is **embarrassingly parallel** by construction because detectors
  share only an immutable `&RepoContext`. We may run serially first for
  simplicity; the design does not need to change to parallelize later.
- **No detector calls another detector.** Cross-detector reasoning happens only
  in synthesis. This is what keeps detectors independently testable and
  order-free.

## 4. Findings: the unit of output

A **finding** is one structured fact with provenance and confidence. The
vocabulary lives in the `json` crate. Conceptually each finding has:

- **category** — structure / language / ecosystem / version-source / ci /
  release-tooling.
- **kind** — the specific fact type within a category (e.g. version-source kinds:
  `cargo-toml`, `package-json`, `pyproject`, `git-tag`, `pom-xml`).
- **data** — the typed payload for that kind (path, value, parsed fields).
- **confidence** — see §6.
- **provenance** — which detector produced it and from which file(s), so a Skill
  (or a human) can trace any claim back to a byte on disk.

**Why findings carry provenance and confidence rather than being bare values:**
the engine's job is to give a reasoning agent *trustworthy, traceable* facts.
"Version 1.4.0 from `crates/foo/Cargo.toml`, high confidence" lets a Skill make a
grounded decision and explain it. A bare `"1.4.0"` forces the Skill to re-verify.
This costs schema surface and detector effort, and it is worth it — traceability
is the whole point of putting deterministic analysis under a reasoning layer.

## 5. Synthesis: cross-detector correlation without coupling

Some report-level conclusions depend on multiple detectors:

- "This is a Rust project" combines the language detector and the Cargo
  ecosystem detector.
- "The release tooling is `cargo-release`" wants to know both the CI findings and
  the ecosystem.

**Decision:** these correlations run in a **synthesis pass** over the full
finding set *after* all detectors finish — not by one detector calling another.

**Why:** if detectors depended on each other's outputs, we'd reintroduce
execution-order coupling and destroy independent testability. Synthesis keeps the
dependency in one clearly-labeled place that operates on *data* (findings), not on
*detectors*. Synthesis is itself a pure function: findings in → enriched report
out.

**Tradeoff:** two-phase (detect, then synthesize) is slightly more machinery than
letting detectors chat. We accept it because it protects the properties (purity,
order-independence, testability) that make the system maintainable at scale.

**Alternative — a dependency DAG between detectors:** more flexible, but it turns
the pipeline into a scheduler with ordering constraints, makes a single detector
un-runnable in isolation, and is far harder to reason about. Rejected until a
real need appears; even then, synthesis-over-findings likely covers it.

## 6. Confidence and ambiguity

Real repos are ambiguous: a `package.json` might be tooling config in a Rust
repo; a `pom.xml` might be a leftover. Detectors must not pretend certainty.

**Decision:** every finding carries a **confidence** (`High` / `Medium` / `Low`,
or a small bounded scale). Detectors assign it from concrete signals (a
`Cargo.toml` with a `[package]` and `version` → high; a stray manifest with no
corroboration → low).

**Why an explicit scale rather than a boolean or a raw float:**

- A boolean ("detected: yes/no") throws away exactly the information a reasoning
  Skill needs to arbitrate conflicts.
- A raw `0.0–1.0` float invites false precision and non-determinism (what makes
  something 0.73 vs 0.71?) and is hard to keep stable across versions.
- A small enum is deterministic, explainable, and stable — a detector can
  document *why* a signal is High vs Low, and that rule is testable.

**Tradeoff:** enums are coarse; occasionally a signal sits between buckets. We
prefer coarse-and-stable over fine-and-arbitrary, because Skills reason better
over a handful of well-defined levels than over opaque numbers.

Conflicting findings (two detectors claim different primary ecosystems) are
**not** resolved by the engine into a single winner. The engine emits both, with
their confidences and provenance, and lets the synthesis pass mark a *likely
primary* while preserving the alternatives. Judgment stays with the Skill; the
engine stays honest.

## 7. Adding a new detector — the intended workflow

The measure of this design is how little a contributor must touch:

1. Add `crates/detectors/src/<ecosystem>.rs` with a type implementing `Detector`.
2. Implement `id`, `category`, `applies`, `detect`. Parse defensively; on
   malformed input, push a `Diagnostic`, never panic.
3. If it introduces a new finding *kind*, add that kind to the `json` crate
   (a schema change → additive → minor schema bump).
4. Register it in `detectors::default_registry()`.
5. Add a fixture under `fixtures/` and its `expected.json`; the golden test
   covers it.

No edits to `core`, no central enum, no touching other detectors. A new
ecosystem is an additive change end to end. **That property is the goal of this
entire document** — if a future change makes adding a detector require editing
`core` or a shared match, that change is suspect.

## 8. Detector categories and the first set

Mapped to the goals in `PROJECT.md`:

| Category         | First detectors                                              |
|------------------|--------------------------------------------------------------|
| structure        | workspace/monorepo layout, project roots                     |
| language         | file-extension + manifest-based language identification      |
| ecosystem        | cargo, npm/yarn/pnpm, pip/poetry (pyproject), maven (pom), go |
| version-source   | Cargo.toml, package.json, pyproject.toml, pom.xml, git tags  |
| ci               | GitHub Actions, GitLab CI, (extensible)                      |
| release-tooling  | cargo-release, semantic-release, changesets, goreleaser, etc.|

These directly serve the Universal Release Manager: version sources + release
tooling + CI + ecosystem are exactly the facts a release Skill reasons over.

## 9. Handling multi-project / polyglot repositories

A repo may contain several projects (a workspace, a monorepo with a Rust service
and a Node frontend). Detectors must not assume a single project at the root.

**Decision:** findings are anchored to **paths**, not to "the repo." The
structure detector identifies project roots; other detectors emit findings scoped
to the path they came from. The report can therefore describe *N* projects in one
repo.

**Why:** polyglot and monorepo layouts are common and are precisely where
"understand this repo" is hardest and most valuable. Baking single-project
assumptions in early would force a painful rewrite. Path-anchored findings cost a
little more schema and detector care now and avoid that cliff.

**Tradeoff:** the Skill must handle "which project am I releasing?" This is
correct division of labor — that *is* a judgment call, and judgment belongs to
the Skill. The engine supplies the enumerated, path-anchored facts.

## 10. Testing detectors

- **Unit:** feed a detector a synthetic `RepoContext` and assert findings —
  possible only because detectors are pure and order-free.
- **Golden:** fixture repo → full expected report (see `workspace-layout.md` §6).
- **Adversarial/messy:** malformed manifests must yield diagnostics and *not*
  crash or produce a nonzero exit — the errors-as-data contract from
  `architecture.md` §4.3.

Because detectors are isolated and deterministic, these tests are fast, exact,
and don't require a real repo or network.

## 11. Summary of detector decisions

| Decision                    | Chosen                                   | Rejected alternative                    |
|-----------------------------|------------------------------------------|-----------------------------------------|
| Extension mechanism         | Trait objects + registry                 | Central enum + match                    |
| Cross-detector logic        | Post-hoc synthesis over findings         | Detector-to-detector calls / DAG        |
| Purity                      | Pure fn of read-only `RepoContext`       | Detectors walk FS / call network        |
| Output unit                 | Findings w/ provenance + confidence      | Bare values / boolean detected          |
| Confidence representation   | Small enum scale                         | Boolean, or raw 0–1 float               |
| Failure handling            | Diagnostic + continue (errors as data)   | Abort run on first bad file             |
| Multi-project               | Path-anchored findings                   | Single-project-at-root assumption       |
| Gate                        | Separate `applies()` before `detect()`   | Always run everything                   |
