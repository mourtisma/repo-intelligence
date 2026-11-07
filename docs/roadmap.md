# Roadmap

> Status: Design. This sequences the build so that the hardest-to-change things
> (the JSON contract, the crate boundaries, determinism) are settled early and
> the easy-to-add things (more detectors, more Skills) come later. Nothing here
> is implemented.

## Guiding principle for sequencing

**Lock the contracts before the coverage.** The JSON schema and the crate
boundaries are expensive to change once Skills and detectors depend on them;
individual detectors are cheap to add at any time. So the roadmap front-loads the
boundary work and treats ecosystem coverage as an ongoing, parallelizable stream
rather than a phase.

Each phase below states its **goal**, its **exit criteria** (how we know it's
done), and the **key risks** it retires.

---

## Phase 0 — Workspace skeleton

**Goal:** the empty-but-correct shape from `workspace-layout.md`.

- Root virtual `Cargo.toml` with workspace inheritance, `rust-toolchain.toml`,
  committed `Cargo.lock`.
- Four crates (`json`, `core`, `detectors`, `cli`) that compile as empty shells
  with the dependency edges from the graph — and *only* those edges.
- CI: build, `clippy -D warnings`, `fmt --check`, and a **dependency-graph
  check** that fails if `core`/`detectors`/`json` gain a network dependency
  (protects the "engine is offline" invariant from `architecture.md` §5).

**Exit criteria:** `cargo build` and CI green; the dependency graph matches
`workspace-layout.md` §3 exactly.

**Risks retired:** accidental cyclic/loose dependencies; contract crate ending up
depending on the engine. Cheapest to enforce when the crates are empty.

---

## Phase 1 — The JSON contract (schema v0)

**Goal:** the `json` crate defines the full `Report` envelope and the finding /
diagnostic / confidence vocabulary, even for categories no detector fills yet.

- `SCHEMA_VERSION` constant; envelope (`schema_version`, `tool_version`,
  `analyzed_at`, `findings`, `diagnostics`).
- Emit a JSON Schema artifact into `schema/` from the Rust types.
- A round-trip test (serialize → deserialize → equal) and a stable-ordering test.

**Exit criteria:** a hand-authored example report validates against the emitted
schema; serialization is byte-stable across runs.

**Why this is Phase 1, before any detector:** every other component depends on
the report shape. Designing detectors first would let their incidental output
shape the contract by accident. We design the contract deliberately, then make
detectors fill it. Getting this wrong is the most expensive mistake available, so
it comes first while there are zero external consumers to break.

**Risk retired:** contract churn after Skills exist.

---

## Phase 2 — Core pipeline + determinism

**Goal:** `core` can scan a repo into a `RepoContext`, run a (still empty)
registry, run a synthesis pass, and emit a valid `Report`.

- Single deterministic traversal (sorted, symlink policy, ignore-file handling).
- `Detector` trait, `DetectorRegistry`, run pipeline, synthesis hook.
- Determinism harness: same fixture → byte-identical report across N runs and
  across machines (modulo the injected `analyzed_at`, which is excluded).

**Exit criteria:** an empty-detector run on a fixture produces a stable,
schema-valid report; the determinism harness passes.

**Why before detectors:** determinism must be a property of the *pipeline*, not
retrofitted per detector. Prove it with zero detectors so any later
non-determinism is unambiguously the fault of a specific new detector.

**Risk retired:** non-determinism creeping in and being hard to localize later.

---

## Phase 3 — First vertical slice: Cargo + git tags + CLI

**Goal:** one end-to-end path working, thin.

- `detectors`: the **Cargo** ecosystem/version-source detector and the **git-tag**
  version-source detector — enough to fully describe a Rust repo's versions.
- `cli`: `repo-intelligence analyze <path> --format json`, stdout=JSON only,
  stderr=logs, exit codes per `architecture.md` §6, timestamp/version injection.
- Golden tests for `rust-single-crate`, `rust-workspace`, and `messy`.

**Exit criteria:** running the CLI on this repo (itself a Rust workspace) emits a
correct report describing its own structure and versions; the `messy` fixture
produces diagnostics with exit 0.

**Why a narrow vertical slice before breadth:** it exercises *every layer and the
full contract* on real input, surfacing boundary problems (does the schema
actually express what a detector wants to say? does the CLI split work?) while
they are still cheap to fix. Breadth-first (all detectors, no CLI) would defer
that feedback.

**Risk retired:** integration gaps between layers; schema fields that look right
but can't actually represent detector output.

---

## Phase 4 — Universal Release Manager Skill (thin)

**Goal:** the first Skill, consuming the CLI, doing reasoning only.

- Skill under `skills/universal-release-manager/`.
- It invokes the CLI, parses the report, and: identifies version sources,
  detects release workflow presence, and **suggests** a semver bump and a
  changelog draft (reasoning — the engine supplies facts, the Skill judges).
- The Skill pins to schema **major** v0 and reads `schema_version` defensively.

**Exit criteria:** on a Rust repo, the Skill produces a coherent release plan and
changelog draft grounded entirely in the JSON — no re-scraping the repo.

**Why now:** validates the CLI↔Skill contract (`architecture.md` §7) and the
division of labor end to end. It also pressure-tests whether the report actually
contains what a reasoning Skill needs; gaps found here feed back as *additive*
schema bumps, not breaks.

**Risk retired:** the "is the JSON actually sufficient for reasoning?" question —
the core premise of the whole project.

---

## Phase 5 — Ecosystem breadth

**Goal:** cover the ecosystems named in `PROJECT.md`.

- Detectors for **npm/package.json**, **pyproject (pip/poetry)**, **Maven
  (pom.xml)**, **go.mod + tags**.
- CI-system detectors: **GitHub Actions**, then others.
- Release-tooling detectors: `semantic-release`, `changesets`, `goreleaser`,
  `cargo-release`.
- Cargo **feature flags** per ecosystem group activated here
  (`workspace-layout.md` §2.3).
- A fixture + `expected.json` per new detector; polyglot fixture exercises
  multi-project (`detector-architecture.md` §9).

**Why this is a stream, not a gated phase:** because the trait/registry design
makes each detector additive and isolated, these can land independently and in
parallel, in any order, without touching `core` or each other. This phase is
"turn the crank," which is the payoff of the earlier boundary investment.

**Exit criteria:** each `PROJECT.md` ecosystem has a detector + golden fixture;
the Universal Release Manager works across all of them unchanged (it reasons over
categories, not ecosystems).

---

## Phase 6 — Hardening and contract discipline

**Goal:** make the contract safe to depend on long-term.

- **Schema-diff check in CI:** compare the emitted schema against the last
  released one; a breaking diff without a major bump fails the build. This is
  what operationalizes the additive-minor / breaking-major rule from
  `architecture.md` §8.
- Performance pass: parallelize detection over the read-only context *if
  measured* to matter (the design already allows it without code changes).
- Broaden the messy/adversarial fixture corpus.
- Publish `repo-intelligence-json` (and possibly the CLI) so external consumers
  can deserialize reports.
- **Release automation from conventional commits:** merges into `dev` produce
  release candidates (`vX.Y.Z-rc.N`); merging `dev` into `main` produces the
  final `vX.Y.Z` from the same commits. The bump is computed from commits since
  the last final release: `feat` → minor, breaking (`!`, `BREAKING CHANGE:`, or
  `break:`) → major, everything else → patch. CI writes the version into
  `[workspace.package]` (so `tool_version` matches the tag) and creates the tag
  and GitHub release. Candidate tool: `semantic-release` (native prerelease
  branches); alternative: `git-cliff` plus a small workflow. Prerequisites: a
  manual baseline tag, since earlier commits are not conventional, and a
  conventional-title check on PRs. Open decisions:
  - during `0.x`, does a breaking change bump to `1.0.0`, or to the next minor
    (the usual pre-1.0 Cargo convention)?
  - should commits with no code change (`docs:`, `chore:`, `ci:`) create release
    candidates?

**Exit criteria:** a deliberately breaking schema PR is caught by CI; perf is
acceptable on a large real monorepo; the contract crate is publishable; a merge
into `dev` produces a correctly bumped release candidate, and merging `dev` into
`main` produces the matching final release.

---

## Phase 7 — Second Skill + Skill extraction

**Goal:** prove the "many Skills on one engine" premise and the Skill-independence
requirement.

- Implement one further Skill from `PROJECT.md`'s list (e.g. **Project
  Onboarding Assistant** — it needs structure/language/ecosystem facts the engine
  already produces, so it validates reuse without new engine work).
- **Extract** the Universal Release Manager Skill to its own repo (or make it
  installable standalone) to prove the co-location decision
  (`workspace-layout.md` §1) was reversible and that Skills install
  independently of the Rust build.

**Exit criteria:** two Skills run on the same engine install; at least one Skill
is installed from outside this repo with no engine rebuild.

**Risk retired:** hidden coupling between the engine and its first Skill; the
"Skills installable independently" requirement being aspirational rather than
real.

---

## What is deliberately deferred (and why)

- **Parallel detection** — deferred to Phase 6, done only if measured. Premature
  concurrency would complicate the pipeline before correctness and determinism
  are locked. The read-only-context design keeps this a safe deferral.
- **Crate-per-detector splitting** — deferred indefinitely; feature flags
  (Phase 5) cover the real need (subset builds) without the crate-count cost.
  Revisited only if a detector's dependencies become a burden on all consumers.
- **Config file / customization** — deferred until a concrete use case exists.
  Any config must be injected by the CLI, never read ambiently by the engine, to
  protect determinism (`architecture.md` §9).
- **Non-CLI front-ends (server, LSP)** — deferred; the thin-adapter design means
  these are new adapters over the same libraries when a need arises, not a
  rearchitecture.
- **Network-backed enrichment** (e.g. querying registries for latest versions) —
  deferred and, if ever added, must live *outside* the deterministic engine
  (in a Skill or a clearly-separated online tool), never inside `core`/`detectors`.

## Dependency map between phases

```
P0 ─▶ P1 ─▶ P2 ─▶ P3 ─▶ P4
                    │
                    └─▶ P5 (stream) ─▶ P6 ─▶ P7
```

- P1→P3 must be sequential: contract, then pipeline, then the first slice that
  proves both.
- P5 can begin as soon as P3's detector pattern exists and runs alongside P4.
- P6/P7 harden and generalize once the pattern is proven.

The shape of this roadmap is itself an architectural statement: **contracts and
determinism are sequenced early because they are irreversible; coverage and
Skills are parallel/streamed because the design made them cheap.**
