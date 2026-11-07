# Architecture

> Status: Design. Nothing in this document is implemented yet. This is the
> reference the first implementation should be measured against.

## 1. Purpose and scope

Repo Intelligence analyzes software repositories and emits **structured,
deterministic JSON** describing what a repository *is*: its structure,
languages, ecosystems, version sources, CI/CD systems, and release tooling.

It does **not** make decisions. It does not bump versions, open PRs, or run
releases. Those are the job of Claude Code Skills reasoning over the JSON, or of
the existing tools those Skills orchestrate.

The single most important architectural constraint flows from the design
principles in `PROJECT.md`:

- **Rust performs deterministic analysis.** Given the same repository bytes,
  the engine produces byte-identical JSON. No network calls, no clocks, no
  randomness, no LLM calls inside the engine.
- **Claude performs reasoning.** Anything requiring judgment (which version bump
  is appropriate, what a changelog should say) lives in the Skill.
- **JSON is the contract between them.** The engine and the Skills are coupled
  *only* through a versioned JSON schema, never through code.

This boundary is the spine of the whole system. Every other decision below is
downstream of keeping it clean.

## 2. System overview

```
┌─────────────────────────────────────────────────────────────┐
│  Claude Code Skill  (e.g. Universal Release Manager)          │
│  - reasoning, judgment, prose, user interaction               │
│  - orchestrates existing release/CI tooling                   │
└───────────────▲───────────────────────────┬──────────────────┘
                │  reads JSON (stdout)       │  invokes
                │                            ▼
        ┌───────┴────────────────────────────────────────┐
        │  repo-intelligence CLI  (thin adapter)          │
        │  - argument parsing, process I/O, exit codes    │
        │  - serializes core results to the JSON contract │
        └───────▲─────────────────────────────────────────┘
                │  calls Rust API
        ┌───────┴─────────────────────────────────────────┐
        │  Engine (Rust library crates)                    │
        │                                                  │
        │   core  ──drives──▶ detectors ──emit──▶ findings │
        │    │                                       │     │
        │    └──────────── json (schema) ◀───────────┘     │
        └──────────────────────────────────────────────────┘
                │  reads (read-only)
                ▼
        ┌───────────────────────┐
        │  Repository on disk    │
        └───────────────────────┘
```

Data flows in exactly one direction: **filesystem → engine → JSON → Skill →
action**. Nothing downstream ever reaches back up. The engine never knows a
Skill exists; the Skill never knows how detection works.

## 3. The layered model

Four responsibilities, mapped onto four crates (detailed in
`workspace-layout.md`):

| Layer      | Crate                         | Responsibility                                        | May depend on                |
|------------|-------------------------------|-------------------------------------------------------|------------------------------|
| Contract   | `repo-intelligence-json`      | Serde types + schema version for the JSON report      | (nothing internal)           |
| Domain     | `repo-intelligence-core`      | Domain types, traits, the detection pipeline/registry | `json`                       |
| Capability | `repo-intelligence-detectors` | Concrete detectors (Cargo, npm, git tags, GH Actions) | `core`, `json`               |
| Adapter    | `repo-intelligence-cli`       | Process boundary: args, I/O, exit codes, serialization| `core`, `detectors`, `json`  |

Dependencies point strictly downward. `json` at the bottom is deliberate: the
contract is the most stable thing in the system and must not depend on detection
logic that changes often.

### Why the contract is its own crate, not a module in `core`

**Decision:** `repo-intelligence-json` is a standalone crate with no internal
dependencies.

**Tradeoff:** It adds a crate and a small amount of ceremony (a fourth
`Cargo.toml`, re-exports) versus just putting the serde types in `core`.

**Why it wins:** The JSON schema is the *public API of the entire product*.
Skills, and eventually third parties, depend on it. Isolating it means:

- It can be versioned and released on its own cadence.
- It can be published to crates.io independently so external tools can
  deserialize reports without pulling in the detection engine.
- Changes to it are visible in review as changes to *that crate*, making
  contract breakage impossible to sneak in as a side effect of a detector tweak.

**Alternative considered — schema types inside `core`:** simpler at first, but
it welds the stable contract to the volatile domain logic. Any Skill or external
consumer would have to depend on the whole engine to read a report. Rejected.

**Alternative considered — a JSON Schema (`.json`) file as the source of
truth, with Rust types generated from it:** attractive for language-neutral
consumers, but code-generation tooling adds build complexity and the generated
types are awkward to hand-tune. **Chosen middle path:** Rust serde types are the
source of truth, and we *emit* a JSON Schema artifact from them (via `schemars`
or equivalent) as a build output. Consumers who want language-neutral validation
get the schema; we keep ergonomic hand-written Rust types. See
`json` section in `workspace-layout.md`.

## 4. The core pipeline

`core` owns the orchestration. At a high level:

```
scan repository → build a shared read-only context
      │
      ▼
run each registered detector against the context   (independent, parallelizable)
      │
      ▼
collect findings + diagnostics
      │
      ▼
assemble the Report (json types)
```

Key design choices:

### 4.1 A shared, read-only `RepoContext`

**Decision:** The engine walks the repository filesystem **once**, up front,
into a `RepoContext` that detectors query. Detectors do not each re-walk the
tree.

**Why:** Filesystem walking is the dominant cost and the main source of
non-determinism (ordering, symlinks, permission errors). Doing it once, in one
place, means:

- Determinism is enforced in a single location (sorted traversal, explicit
  symlink policy, ignore-file handling).
- Detectors are cheap and pure: given a context, they compute findings.
- We can cache file contents once instead of N detectors reading `Cargo.toml`.

**Tradeoff:** The context must anticipate what detectors need (file listing,
on-demand file reads, parsed `.gitignore`, maybe git metadata). If a detector
needs something the context can't provide, the context grows. We accept a
context that is slightly "fat" in exchange for detector purity and single-point
determinism.

**Alternative — each detector walks what it needs:** maximally decoupled, but
duplicates the hardest-to-get-right code (traversal, ignore semantics) across
every detector and makes determinism a per-detector concern. Rejected.

### 4.2 Detectors are independent and order-free

Detectors must not depend on each other's *execution order*. Each takes the
context and produces findings. This is what makes the pipeline parallelizable
and testable in isolation, and it is elaborated in `detector-architecture.md`.

Where one conclusion genuinely depends on another (e.g. "release tooling" wants
to know the detected ecosystem), we resolve it in a **synthesis pass** over all
findings *after* detection, not by having detectors call each other. Detection
produces facts; synthesis correlates them.

### 4.3 Errors are data, not failures

**Decision:** A detector failing (malformed `Cargo.toml`, unreadable file) does
**not** abort the run. It records a **diagnostic** in the report and the run
continues.

**Why:** Real repositories are messy. A tool that dies on the first bad file is
useless on exactly the repos that most need analysis. Partial results with
explicit diagnostics are far more valuable than an all-or-nothing crash. This
also keeps the JSON contract honest: consumers always get a well-formed report,
and can see what the engine couldn't determine.

**Tradeoff:** Every detector must be written defensively and every "I couldn't
figure this out" must be representable in the schema. That is a real cost paid on
the contract side (a `diagnostics` array, confidence levels) — see the JSON
contract section below.

## 5. Determinism as an enforced property

Determinism is not a nice-to-have; it is what lets Skills, tests, and caches
trust the output. The architecture enforces it structurally:

- **Single traversal point** with sorted, stable ordering (see 4.1).
- **No ambient inputs in the engine.** No wall-clock time, no environment
  variables read for logic, no locale-dependent formatting, no RNG. Anything
  time-like (e.g. "analyzed at") is injected by the *CLI adapter*, never read
  inside `core`/`detectors`, and is excluded from any determinism test.
- **No network.** The engine crates must not have HTTP/network dependencies.
  This is enforceable in CI by checking the dependency tree.
- **Stable serialization.** Map-like structures are emitted as sorted arrays or
  ordered maps so JSON output is byte-stable.

A golden-file test suite (fixture repos → expected JSON) is the regression net.
Because the engine is deterministic and offline, these tests are fast and
reliable.

## 6. The CLI as a thin adapter

**Decision:** The CLI contains *no analysis logic*. It parses arguments,
resolves the target path, calls the engine, injects environment facts (the
`analyzed_at` timestamp, tool version), serializes the `Report` to JSON on
stdout, writes diagnostics/logs to stderr, and maps outcomes to exit codes.

**Why:** Keeping business logic out of the CLI means the engine is fully usable
as a library (future GUI, LSP, server, or another CLI) and the CLI stays
trivially testable. It also honors the `PROJECT.md` mandate that "business logic
should live in Rust [libraries]" and the CLI merely exposes it.

**The stdout/stderr split is part of the contract:** stdout is *only* the JSON
report, so a Skill can pipe it straight into a parser. All human-readable
logging, progress, and warnings go to stderr. Violating this (printing a warning
to stdout) breaks every Skill silently, so it is a first-class rule.

**Exit codes** communicate coarse outcome to the Skill without it having to
parse JSON just to know if the run succeeded:

| Code | Meaning                                                        |
|------|----------------------------------------------------------------|
| 0    | Analysis completed; report on stdout (may contain diagnostics) |
| 1    | Usage error (bad arguments, path does not exist)               |
| 2    | Fatal engine error; no valid report produced                   |

A detector-level problem is **exit 0 + a diagnostic in the JSON**, not a nonzero
exit. Nonzero is reserved for "you cannot trust stdout as a report."

## 7. CLI ↔ Skill interaction

The Skill is deliberately thin. Its contract with the CLI:

1. **The Skill invokes one documented command** (e.g.
   `repo-intelligence analyze <path> --format json`) and reads stdout.
2. **The Skill parses JSON and reasons.** It never scrapes human text, never
   re-derives facts the engine already provides, and never shells into the repo
   to double-check the engine.
3. **The Skill depends on the schema version**, not on the CLI's internal
   behavior. It reads `schema_version` from the report and refuses/adapts if it
   sees a major version it doesn't understand (see §8).
4. **Skills are independently installable.** A Skill declares which CLI version
   range and schema major version it needs. Multiple Skills can share one engine
   install.

This gives us a clean division of labor that matches the models' strengths:
Rust guarantees the *facts* are correct and reproducible; Claude handles the
*judgment* (which bump, what the changelog says, how to sequence a release) and
the *messy real world* (asking the user, adapting to an unusual repo).

**Why not richer coupling (e.g. the Skill calling many fine-grained
subcommands)?** A single "analyze → one report" call is easier to version, test,
and cache than a chatty protocol, and it keeps the reasoning in one place. We
add subcommands only when a Skill genuinely needs a *different* analysis, not to
decompose one analysis. (See `roadmap.md` for how `--only`/scoping is added
without breaking this.)

## 8. Versioning and compatibility strategy

Two version numbers matter, and they are intentionally separate:

- **Schema version** (`schema_version` in every report): the shape of the JSON
  contract. Uses semantic versioning. **Additive changes** (new optional field,
  new detector emitting a new finding kind) bump *minor*. **Breaking changes**
  (renamed/removed field, changed meaning) bump *major*.
- **Tool version**: the CLI/engine release. Independent of schema version; many
  tool releases can share one schema version.

**Rule:** Skills pin to a schema *major* version and tolerate minor bumps. This
is only safe if we hold the line that minor = additive. Enforcing that is a
review discipline plus a schema-diff check in CI (see `roadmap.md`).

**Why separate the two versions:** it lets us ship engine improvements (new
ecosystems, bug fixes, speedups) constantly without forcing Skill updates, while
still having a hard signal when we must break consumers. Coupling them would mean
every engine patch looks like a potential contract change to a Skill. Rejected.

## 9. Cross-cutting concerns

- **Logging/observability:** structured logging to stderr (e.g. `tracing`),
  behind a verbosity flag. Never on stdout. Off by default so the JSON path
  stays clean.
- **Performance:** detectors are independent, so the pipeline can run them
  concurrently over the shared `RepoContext`. We start single-threaded for
  simplicity and correctness, and parallelize once there is a measured need,
  because the read-only-context design already makes it safe to do so later
  without touching detector code.
- **Configuration:** the engine should work with zero configuration. Any future
  config (ignore patterns, enabled detectors) is passed *into* the engine as an
  explicit argument by the CLI — never read from ambient env/files inside the
  engine, to preserve determinism.
- **Security posture:** the engine only *reads* the repository and never
  executes code found in it (no running build scripts, no evaluating configs).
  This is both a safety property and a determinism property.

## 10. What this architecture optimizes for

In priority order:

1. **A clean, versioned JSON boundary** — because it is what lets the Rust engine
   and the Claude Skills evolve independently, which is the entire premise.
2. **Determinism** — because it is what makes the JSON trustworthy and testable.
3. **Detector extensibility** — because ecosystem coverage is the product's
   growth axis (see `detector-architecture.md`).
4. **Long-term maintainability** — strict layering, thin adapters, errors-as-data,
   and independent detectors all serve the goal of a codebase that a new
   contributor can extend by adding one file, without understanding the whole.

Explicitly *not* optimized for: raw first-version speed, minimal crate count, or
supporting in-engine reasoning. Those are traded away deliberately in favor of
the boundary and the extensibility.
