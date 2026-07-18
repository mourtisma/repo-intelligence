# Phase 1 — Implementation Plan

> Status: Ready to implement. Scope is defined by `roadmap.md` Phase 1,
> `architecture.md` §6–8, and `detector-architecture.md` §4–6. This plan turns
> those into exact, ordered tasks.
>
> Phase 1 delivers the **JSON contract (schema v0)**: the `repo-intelligence-json`
> crate defines the full `Report` envelope and the finding / diagnostic /
> confidence vocabulary — including categories no detector fills yet — plus a
> committed JSON Schema artifact, round-trip tests, and byte-stable
> serialization. No detection logic, no pipeline, no `RepoContext`, no CLI
> behavior.

## 1. Scope discipline (read this first)

Phase 1 designs the single most expensive-to-change artifact in the system: the
JSON contract is *the public API of the entire product* (`architecture.md` §3).
Every later component — pipeline, detectors, CLI, Skills, third parties —
depends on its shape. The roadmap sequences it before any detector precisely so
that no detector's incidental output shapes the contract by accident
(`roadmap.md` Phase 1). Two failure modes are equally dangerous and this plan
guards against both:

1. **Under-designing** — shipping a contract so thin that Phase 3's first real
   detector can't express what it found, forcing an early breaking change.
2. **Over-designing** — inventing fields, kinds, and structure for detectors
   that do not exist yet, welding speculative shape into a contract that is
   costly to remove.

The discipline that resolves the tension: **design the envelope and the
vocabulary completely; design the per-kind payloads minimally.** The parts that
are expensive to change later (envelope shape, the finding/diagnostic/confidence
model, the extension mechanism, versioning) are settled now, deliberately. The
parts that are cheap and additive later (concrete finding payloads for specific
ecosystems) are left as an *open, forward-compatible shape*, not enumerated.

**In scope (Phase 1):**

- The `Report` envelope: `schema_version`, `tool_version`, `analyzed_at`,
  `findings`, `diagnostics`.
- The `Finding` model: category, kind, path anchoring, typed data, confidence,
  provenance.
- The `Diagnostic` model: severity, message, detector attribution, optional path.
- The `Confidence` model: a small bounded enum.
- The `DetectorCategory` vocabulary as a serde enum (structure, language,
  ecosystem, version-source, ci, release-tooling).
- The `SCHEMA_VERSION` constant and the versioning rule it encodes.
- Emission of a JSON Schema artifact into `schema/` from the Rust types.
- Round-trip and stable-ordering tests; a hand-authored example report that
  validates against the emitted schema.
- The first external dependencies enter the `json` crate here: `serde`,
  `serde_json`, and a schema-emitting crate (`schemars` or equivalent).

**Explicitly out of scope (deferred to the phase that owns it):**

| Deferred item                                                     | Owning phase |
|-------------------------------------------------------------------|--------------|
| `Detector` trait, `RepoContext`, `DetectorRegistry`, pipeline     | Phase 2      |
| `DetectionOutput` (core type that maps to findings/diagnostics)   | Phase 2      |
| Synthesis pass / "likely primary" correlation logic               | Phase 2      |
| Any concrete detector or ecosystem-specific finding payload       | Phase 3+     |
| Cargo/npm/pyproject/pom/go-specific fields                        | Phase 3+     |
| `fixtures/` sample repos + golden tests                           | Phase 2/3    |
| CLI `analyze`, timestamp/version injection at runtime             | Phase 3      |
| Schema-diff check in CI (breaking-change detector)                | Phase 6      |
| Publishing `repo-intelligence-json` to crates.io                  | Phase 6      |

**Rule:** if a field or kind exists only to serve one ecosystem's detector that
Phase 3+ will write, it does not belong in Phase 1. The contract must be able to
*carry* that detector's output additively later; it must not *pre-declare* it.

**Abstractions we deliberately do NOT introduce yet:** no builder types, no
`Report::new()` convenience constructors beyond what tests need, no per-category
sub-structs mirroring `RepoStructure` / `Ecosystem` / `VersionSource` as
distinct top-level report sections (see §4.2 for why the flat `findings` array is
chosen over typed sections), no confidence arithmetic, no severity beyond what
diagnostics need. Each is added by the phase that first has a consumer for it.

## 2. Directories and files created in Phase 1

```
crates/json/
├── Cargo.toml            # UPDATE: add serde, serde_json, schemars deps
└── src/
    ├── lib.rs            # UPDATE: module wiring, re-exports, SCHEMA_VERSION
    ├── report.rs         # NEW: Report envelope
    ├── finding.rs        # NEW: Finding, DetectorCategory, Provenance, data payload
    ├── diagnostic.rs     # NEW: Diagnostic, Severity
    └── confidence.rs     # NEW: Confidence
schema/
└── report.schema.json    # NEW: emitted JSON Schema artifact (see §7)
tests/  (in crates/json/tests/)
├── round_trip.rs         # NEW: serialize → deserialize → equal
├── stability.rs          # NEW: byte-stable ordering across runs
└── example_validates.rs  # NEW: example report validates against emitted schema
```

The `schema/` directory (from `workspace-layout.md` §1) is created **now**,
because Phase 1 is the first phase with content to put in it. `fixtures/` and
`skills/` remain uncreated — they have no Phase 1 content.

Module split rationale: one file per model keeps each concept reviewable in
isolation, which matters most for the contract crate where "what changed in the
schema" must be legible in a diff (`architecture.md` §3). The split is by
*concept*, not by *ecosystem* — there is no `cargo.rs` in the `json` crate, ever.

## 3. The versioning model (design this first)

Settle the version rule before writing any type, because it constrains every
subsequent decision.

- **`SCHEMA_VERSION`** is a constant in the `json` crate and is emitted as
  `schema_version` in every report. It uses semantic versioning
  (`architecture.md` §8). Phase 1 ships **`0.1.0`** (see note below on `0.x`).
- **Additive changes bump minor:** a new optional field, a new
  `DetectorCategory` variant, a new finding *kind*, a new diagnostic severity.
  Skills pin to a schema **major** and must tolerate minors
  (`architecture.md` §8, `roadmap.md` Phase 4).
- **Breaking changes bump major:** renaming/removing a field, changing a field's
  meaning or type, making an optional field required, removing an enum variant.
- **Tool version is separate.** `tool_version` is the CLI/engine release,
  injected by the CLI at runtime (`architecture.md` §6, §8). The `json` crate
  does **not** know the tool version; it only defines the *field*. Many tool
  versions share one schema version.

**Note on `0.x`:** during `0.x`, semver technically permits breaking changes in
minors. We do **not** rely on that. The rule above (additive=minor,
breaking=major, even pre-1.0) is adopted deliberately so the discipline Skills
depend on (`roadmap.md` Phase 4: "pins to schema major v0 and reads
`schema_version` defensively") is real from day one. `0.1.0` signals "stable
shape, not yet frozen surface"; the moment an external consumer exists (Phase 4),
the same rule already governs.

**What makes minor-is-additive enforceable** is deferred to Phase 6 (schema-diff
CI). Phase 1's job is to make the *rule* explicit and to make the emitted schema
artifact exist so that a diff is *possible*. We do not build the diff checker now.

## 4. The envelope (`report.rs`)

### 4.1 Fields

`Report` is the top-level type. Exactly five fields, in this order:

| Field            | Type                | Notes                                                        |
|------------------|---------------------|--------------------------------------------------------------|
| `schema_version` | `String`            | e.g. `"0.1.0"`. Serialized from `SCHEMA_VERSION`.            |
| `tool_version`   | `String`            | Injected by the CLI (`architecture.md` §6). `json` defines the field, never populates it. |
| `analyzed_at`    | `String`            | RFC 3339 timestamp, injected by the CLI. **Excluded from determinism tests** (`architecture.md` §5). String, not a chrono type, to keep `json` free of a time dependency and to keep the contract format-explicit. |
| `findings`       | `Vec<Finding>`      | The facts. Empty array is valid (an empty-detector run — the Phase 2 exit criterion — still produces a well-formed report). |
| `diagnostics`    | `Vec<Diagnostic>`   | The "couldn't determine" record (`architecture.md` §4.3). Empty array is valid. |

Both `findings` and `diagnostics` are always present (never `null`, never
omitted) — a consumer never has to distinguish "absent" from "empty." This is a
contract-honesty decision: the report shape is invariant regardless of outcome.

### 4.2 Why a flat `findings` array, not typed report sections

**Decision:** the report carries one `Vec<Finding>` where each finding declares
its `category`, rather than typed top-level sections (`structure: RepoStructure`,
`ecosystems: Vec<Ecosystem>`, `version_sources: Vec<VersionSource>`, …) as the
`workspace-layout.md` §2.1 *exposes*-list might first suggest.

**Why it wins:**

- **Additivity without breakage.** A new category or kind is a new variant /
  string, appended to a homogeneous array — a minor bump. Typed sections would
  mean adding a new top-level field per category and per ecosystem, and every
  ecosystem-specific shape would tempt a Cargo-specific or Node-specific field
  into the envelope. The flat array is the structural expression of "avoid
  Cargo-specific fields" (the Phase 1 brief) and of the additive-minor rule.
- **Path-anchored polyglot support falls out for free.** `detector-architecture.md`
  §9 requires findings anchored to *paths*, not to "the repo," so one report can
  describe N projects. A flat array of path-anchored findings does this
  natively; typed singleton sections fight it.
- **Provenance and confidence are uniform.** Every fact carries the same
  metadata (§5) because every fact is the same `Finding` type.

**Tradeoff:** consumers filter the array by `category`/`kind` instead of reading
a typed field. That filtering is trivial and is exactly the reasoning step a
Skill does anyway. The named types from `workspace-layout.md` §2.1
(`RepoStructure`, `Ecosystem`, `VersionSource`, `CiSystem`, `ReleaseTooling`)
are reframed as **finding categories/kinds**, not as separate envelope sections.
This is a reconciliation with `workspace-layout.md` and is called out in §9.

**Alternative — typed sections per category:** more self-documenting at first
glance, but it hard-codes the category set into the envelope and pushes
ecosystem-specific fields upward. Rejected: it optimizes for readability of v0
at the cost of the additive-evolution property that is the whole point of Phase 1.

## 5. The finding model (`finding.rs`)

A `Finding` is one structured fact with provenance and confidence
(`detector-architecture.md` §4). Fields:

| Field        | Type                 | Notes                                                                 |
|--------------|----------------------|-----------------------------------------------------------------------|
| `category`   | `DetectorCategory`   | Enum (§5.1). Which class of question this answers.                     |
| `kind`       | `String`             | The specific fact type within a category (§5.2). A **string**, not an enum — this is the key extensibility decision. |
| `path`       | `Option<String>`     | Repo-relative path this fact is anchored to (`detector-architecture.md` §9). `None` for genuinely repo-global facts. Always forward-slash normalized (§8). |
| `confidence` | `Confidence`         | §6.                                                                   |
| `data`       | `serde_json::Value`  | The typed-per-kind payload, carried as an open JSON object (§5.3).     |
| `provenance` | `Provenance`         | Which detector produced it, from which file(s) (§5.4).                 |

### 5.1 `DetectorCategory` — a closed enum, defined fully now

The six categories are known from the architecture and are stable
(`detector-architecture.md` §8, `architecture.md` §6). Define all six as serde
enum variants now, even though Phase 3 fills only two:

`structure`, `language`, `ecosystem`, `version-source`, `ci`, `release-tooling`.

- Serialized as kebab-case strings (§8) so JSON is human-legible and
  language-neutral.
- Adding a category later is a **minor** bump (additive). We define the full set
  now because it is small, known, and stable — enumerating it is documentation,
  not speculation. This is the one place we *do* enumerate, because the category
  axis is fixed by the architecture, unlike the kind/payload axis which grows per
  ecosystem.

### 5.2 `kind` is a `String`, not an enum — the load-bearing extensibility choice

**Decision:** `kind` (e.g. `"cargo-toml"`, `"package-json"`, `"git-tag"`,
`"github-actions"`) is a free-form (but convention-governed) string, not a
Rust enum.

**Why:** adding a new ecosystem detector (Phase 5's "turn the crank" stream)
must be an **additive** change end to end (`detector-architecture.md` §7). If
`kind` were an enum in the `json` crate, every new detector would edit the
contract crate and force a schema change *and* a `json`-crate change in lockstep
— re-coupling the volatile detector layer to the stable contract, the exact
coupling `architecture.md` §3 rejects. A string `kind` lets Phase 5 add
`"go-mod"` by writing a detector and documenting the kind, touching `json` only
if a *new category* is needed.

**Tradeoff:** no compile-time exhaustiveness over kinds, and typos in `kind` are
not caught by the type system. We mitigate with:

- A documented kind-naming convention (kebab-case, ecosystem-prefixed where
  natural, e.g. `cargo-toml`, `pyproject-toml`).
- The golden tests (Phase 3+) pin exact kind strings per fixture, catching drift.

This mirrors the trait-object-over-enum decision in `detector-architecture.md`
§2: we accept losing compile-time exhaustiveness to buy the additive-growth
property that is the product's growth axis.

### 5.3 `data`: an open JSON payload, minimal in v0

**Decision:** the per-kind typed payload is carried as `serde_json::Value` (an
open JSON object), **not** as a Rust enum of per-kind structs in v0.

**Why:** the same additivity argument as `kind`. A per-kind struct enum would
put every ecosystem's payload shape into the `json` crate, making each new
detector a contract change and tempting Cargo-specific fields into the crate the
Phase 1 brief says to keep ecosystem-neutral. An open `data` object lets a
detector attach `{ "version": "1.4.0" }` or `{ "name": "foo", "members": [...] }`
without the contract crate knowing those shapes.

**Tradeoff — this is the sharpest tradeoff in Phase 1:** an open `data` object is
*less* self-validating. The JSON Schema (§7) can describe `data` only loosely
(an object). We accept this in v0 deliberately because:

- v0 has **zero** detectors, so there is no payload shape to validate yet.
- Locking payload shapes now would be pure speculation about Phase 3+ detectors.
- The path from here is additive and non-breaking: when detectors exist and
  payload shapes stabilize (Phase 5/6), we can *tighten* the schema for specific
  kinds via a `oneOf`-on-`kind` refinement, which is an additive schema change
  (it constrains previously-unconstrained data — a compatibility judgment made in
  Phase 6, not now).

**What v0 does specify about `data`:** it is always a JSON **object** (never a
bare scalar or array at the top level), so every kind has a place to grow fields
additively. This one constraint is worth stating in the schema.

### 5.4 `Provenance`

Minimal: which detector, from which files.

| Field     | Type          | Notes                                                        |
|-----------|---------------|-------------------------------------------------------------|
| `detector`| `String`      | The detector's stable `id` (`detector-architecture.md` §2). |
| `sources` | `Vec<String>` | Repo-relative, slash-normalized paths the fact was derived from. May be empty for synthesized facts (Phase 2). |

Provenance is what makes a fact *traceable to a byte on disk*
(`detector-architecture.md` §4) — the reason findings aren't bare values. Kept to
two fields; line numbers / byte offsets are deferred until a detector needs them
(additive).

## 6. The confidence model (`confidence.rs`)

**Decision:** `Confidence` is a small closed enum: `High`, `Medium`, `Low`.
Serialized as lowercase strings (`"high"`, `"medium"`, `"low"`).

**Why an enum, not a bool or float:** taken directly from
`detector-architecture.md` §6 — a boolean throws away the arbitration
information a Skill needs; a raw `0.0–1.0` float invites false precision and
non-determinism ("why 0.73?") and is hard to keep stable across versions. A small
enum is deterministic, explainable, and stable.

**Ordering:** the enum's variant order (`High` > `Medium` > `Low`) is meaningful;
if we derive `Ord`, `High` must be the greatest. Serialization is by string, not
by ordinal, so reordering variants would not silently change JSON — but adding a
variant between them would, so any future scale change is a **major** bump. We
ship exactly three levels; the "small bounded scale" language in
`detector-architecture.md` §6 permits later refinement, but that would be
breaking, so three is the committed v0 surface.

## 7. Schema generation (`schema/report.schema.json`)

**Decision:** Rust serde types are the source of truth; we *emit* a JSON Schema
artifact from them (`architecture.md` §3, "chosen middle path"), via `schemars`
(or equivalent). We do **not** hand-author the schema or generate Rust from a
schema.

Mechanics:

- Derive `JsonSchema` alongside `Serialize`/`Deserialize` on every public type.
- Provide a **deterministic way to (re)generate** `schema/report.schema.json`.
  Options, pick the simpler that keeps `cli` uninvolved:
  - A small `#[test]` in the `json` crate that regenerates the schema and asserts
    the committed file matches (fails CI if the artifact is stale). This makes
    "schema drifted from types" a test failure, needs no new binary, and keeps
    the artifact reviewable in diffs.
  - Alternatively an example / xtask that writes the file, with a CI check that
    `git diff --exit-code schema/` is clean. Heavier; the test approach is
    preferred for Phase 1.
- The emitted schema must itself be **byte-stable** (§8): schemars output order
  must be deterministic across runs, or the "assert committed file matches" test
  flakes. Verify this as part of Task 1.6.

**The schema artifact is committed** so external/language-neutral consumers get
validation without a Rust build (`architecture.md` §3), and so Phase 6's
schema-diff check has a prior artifact to diff against. Committing it now is what
makes the versioning rule (§3) operational later.

**`data` in the schema:** typed loosely as `"type": "object"` in v0 per §5.3.
The schema documents the envelope, finding metadata, category enum, confidence
enum, and diagnostic shape precisely; it documents `data` payloads only as
"object." This honestly reflects that v0 has no payload shapes to validate.

## 8. Serialization stability (determinism)

The report must be **byte-stable across runs** (`architecture.md` §5,
`roadmap.md` Phase 1 exit criterion), excluding the injected `analyzed_at`.
Concrete rules for Phase 1:

- **No map-like fields with nondeterministic iteration.** Nothing in the v0
  envelope is a `HashMap`. `data` is a `serde_json::Value`; when a detector
  eventually builds one, object key order must be stable — prefer
  `serde_json`'s `preserve_order` feature (backed by an ordered map) **or**
  document that detectors must emit sorted keys. Decide this now (recommend
  enabling `preserve_order`) so Phase 3 detectors inherit stability rather than
  retrofitting it.
- **Field order is struct-declaration order** (serde default) — stable by
  construction. The field orders in §4.1 and §5 are therefore part of the
  contract's byte output and should not be reordered casually.
- **Ordering of `findings` / `diagnostics` is the producer's responsibility**
  (Phase 2's pipeline sorts them deterministically). The `json` crate does not
  sort; it preserves `Vec` order. Phase 1 only guarantees that *given the same
  `Vec`, the bytes are identical*. State this boundary explicitly so Phase 2 owns
  the traversal-ordering guarantee (`architecture.md` §4.1).
- **String enums, not numeric** — category, confidence, severity all serialize
  as stable strings, immune to variant-reordering changing output.
- **Path normalization:** all path fields (`Finding.path`, `Provenance.sources`,
  `Diagnostic.path`) are forward-slash, repo-relative. The `json` crate defines
  this as a documented invariant; enforcement (converting OS paths) is the
  producer's job in Phase 2. Documenting it here prevents a Windows/Unix
  divergence from becoming a contract ambiguity later.

## 9. Reconciliation with `workspace-layout.md` §2.1

`workspace-layout.md` §2.1 lists the `json` crate as exposing `Report`,
`RepoStructure`, `Language`, `Ecosystem`, `VersionSource`, `CiSystem`,
`ReleaseTooling`, `Finding`, `Diagnostic`, `Confidence`, and the envelope. This
plan **reframes** the middle group:

- `RepoStructure`, `Language`, `Ecosystem`, `VersionSource`, `CiSystem`,
  `ReleaseTooling` are **not** shipped as distinct top-level types in v0. They
  are represented as **finding categories** (§5.1) whose specifics live in
  `kind` + `data`.
- **Why the deviation:** shipping them as typed envelope sections would
  contradict both the "keep the contract minimal" and "avoid Cargo-specific
  fields" instructions for Phase 1, and would break the additive-evolution
  property (§4.2). `workspace-layout.md` §2.1 says exact shapes are "defined by
  the JSON contract" — this plan *is* that definition, and it resolves the
  named-types list into the category/kind model.
- **Is the deviation acceptable:** yes, and it should be recorded. It keeps the
  envelope ecosystem-neutral and additive. If a future need for a typed
  convenience view arises, it can be added *in a consumer* or as an additive
  helper, without changing the wire contract. This is exactly the kind of
  architecture-vs-implementation reconciliation the `phase-review` /
  `architecture-reconciliation` prompts exist to capture; flag it in the Phase 1
  review.

## 10. Dependencies added in Phase 1

The first external dependencies enter the workspace here. Each is a reviewable
event (`phase-0-implementation-plan.md` §5).

| Crate         | Where          | Why                                                        |
|---------------|----------------|-----------------------------------------------------------|
| `serde`       | `json`         | (De)serialization derive. Already in `[workspace.dependencies]`. |
| `serde_json`  | `json`         | JSON (de)serialization + `Value` for open `data` payloads. Add to workspace table. Enable `preserve_order` (§8). |
| `schemars`    | `json`         | Emit the JSON Schema artifact from Rust types (§7). Add to workspace table. |

- All three are **offline, no-network** crates — the dependency-graph guard
  (`scripts/check-engine-deps.sh`) must stay green. Verify after adding: none is
  on the denylist and none pulls in a networked transitive dep. This is the first
  real exercise of the guard against actual dependencies.
- Add `serde_json` and `schemars` to the root `[workspace.dependencies]` table so
  versions are pinned once (`workspace-layout.md` §4), then consume them in
  `crates/json/Cargo.toml`.
- `core`, `detectors` remain dependency-free in Phase 1 — only `json` grows.

## 11. Exact tasks (ordered)

Each task is small and independently verifiable. Recommended: one commit per
task, `cargo build` + `cargo test` green after each.

### Task 1.1 — Versioning constant and rule
- Add `SCHEMA_VERSION` to `json` (`"0.1.0"`) and a module doc comment stating the
  additive-minor / breaking-major rule (§3), including the `0.x` stance.
- **Acceptance:** the constant exists and is re-exported from `lib.rs`.

### Task 1.2 — Confidence model
- `confidence.rs`: the three-level enum, kebab/lowercase serde, `JsonSchema`
  derive, `Ord` with `High` greatest.
- **Acceptance:** unit test asserts each variant's exact serialized string.

### Task 1.3 — Diagnostic model
- `diagnostic.rs`: `Diagnostic { severity, message, detector, path: Option<_> }`
  and a `Severity` enum (start minimal: `Error`, `Warning`; add levels additively
  later). Kebab-case serde, `JsonSchema` derive.
- **Acceptance:** unit test asserts serialized shape; `path` omitted vs `null`
  decision documented (recommend `skip_serializing_if = "Option::is_none"` only
  if we accept it as *the* contract — otherwise always emit; §4.1 favors
  invariant shape, so prefer emitting `null`). Pick one and pin it in a test.

### Task 1.4 — Finding model
- `finding.rs`: `DetectorCategory` (six variants, kebab-case), `Provenance`,
  `Finding { category, kind, path, confidence, data, provenance }`. `data` is
  `serde_json::Value`. `JsonSchema` derive on all.
- **Acceptance:** unit test round-trips a finding with a non-trivial `data`
  object and asserts key order is preserved.

### Task 1.5 — Report envelope
- `report.rs`: `Report` with the five fields in the §4.1 order. `findings` /
  `diagnostics` always present.
- `lib.rs`: wire modules, re-export the public surface, document that
  `tool_version` / `analyzed_at` are CLI-injected and not populated here.
- **Acceptance:** an empty `Report` (no findings, no diagnostics) serializes to a
  well-formed object with empty arrays.

### Task 1.6 — Schema emission
- Add `schemars`; derive `JsonSchema` everywhere; add the regenerate-and-assert
  test (§7) that writes/compares `schema/report.schema.json`.
- Create `schema/` and commit the artifact.
- **Acceptance:** the schema test passes; re-running it produces a byte-identical
  file (stability of the emitter itself).

### Task 1.7 — Example report + validation test
- Hand-author an example report (a couple of findings across two categories, one
  diagnostic) as a committed JSON fixture.
- Test that it (a) deserializes into `Report` and (b) validates against the
  emitted schema (using a JSON Schema validator crate, dev-dependency only).
- **Acceptance:** `roadmap.md` Phase 1 exit criterion met — "a hand-authored
  example report validates against the emitted schema."

### Task 1.8 — Round-trip and stability tests
- `round_trip.rs`: `serialize → deserialize → assert_eq` on the example.
- `stability.rs`: serialize the same `Report` value N times, assert byte-identical
  output (excluding nothing, since the test value has fixed `analyzed_at`); assert
  that permuting HashMap-free structures is a non-issue by construction.
- **Acceptance:** `roadmap.md` Phase 1 exit criterion met — "serialization is
  byte-stable across runs."

### Task 1.9 — Guard + CI verification
- Run `scripts/check-engine-deps.sh`; confirm `serde`/`serde_json`/`schemars`
  introduce no denylisted transitive deps and the `json` crate still has no
  network dependency.
- Confirm `cargo test --workspace` runs the new tests in CI (the step already
  exists from Phase 0).
- **Acceptance:** guard green with real dependencies present; CI green.

## 12. Task dependency order (summary)

```
1.1 version ─┐
1.2 confidence ─┤
1.3 diagnostic ─┼─▶ 1.4 finding ─▶ 1.5 report ─▶ 1.6 schema ─▶ 1.7 example+validate
                │                                              └▶ 1.8 round-trip+stability
                └────────────────────────────────────────────────▶ 1.9 guard+CI (last)
```

Confidence and diagnostic are leaves (no deps on other new types); finding needs
confidence; report needs finding+diagnostic; schema needs all types; the tests
need the schema. The guard/CI check comes last, once real dependencies are wired.

## 13. Test strategy for Phase 1

Phase 1 has real, testable behavior (serialization), so tests are `#[test]`
functions, not just structural CI checks:

1. **Round-trip fidelity.** Every public type survives serialize → deserialize
   unchanged. Catches accidental lossy fields.
2. **Byte-stable serialization.** The same value serializes identically across
   runs (`architecture.md` §5). This is the Phase 1 slice of the determinism
   guarantee — proved on the *contract* before the *pipeline* (Phase 2) and
   *detectors* (Phase 3) inherit it. Locating it here means any later
   non-determinism is unambiguously the pipeline's or a detector's fault, not the
   serializer's.
3. **Schema ↔ types agreement.** The emitted schema matches the committed
   artifact (staleness = test failure), and a hand-authored example validates
   against it. This is what makes the schema artifact trustworthy for external
   consumers and for Phase 6's diff check.
4. **Enum string stability.** Each `DetectorCategory` / `Confidence` / `Severity`
   variant asserts its exact serialized string, so a rename (a breaking change)
   cannot slip in as an incidental edit — it fails a test and forces the major
   bump the rule (§3) demands.

**No golden *report* fixtures, no detector tests, no pipeline tests in Phase 1**
— there is no detector and no traversal to produce a report. The example report
in Task 1.7 is hand-authored precisely because nothing generates one yet.
Introducing fixture-repo golden tests now would be Phase 2/3 work done
speculatively.

## 14. Acceptance criteria for Phase 1 (exit gate)

Phase 1 is complete when **all** hold — matching `roadmap.md` Phase 1 exit
criteria:

- [ ] `SCHEMA_VERSION` exists (`0.1.0`) and the additive-minor / breaking-major
      rule is documented in the crate.
- [ ] The full envelope (`schema_version`, `tool_version`, `analyzed_at`,
      `findings`, `diagnostics`) is defined, with `findings` / `diagnostics`
      always-present arrays.
- [ ] The `Finding` model exists with category (closed six-variant enum), `kind`
      (string), `path` (optional), `confidence`, open `data` object, and
      `Provenance` — with **no ecosystem-specific fields**.
- [ ] The `Diagnostic` and `Confidence` models exist as specified.
- [ ] `schema/report.schema.json` is committed and is regenerable to a
      byte-identical file; a staleness test guards it.
- [ ] A hand-authored example report deserializes and validates against the
      emitted schema.
- [ ] Round-trip and byte-stability tests pass and are wired into
      `cargo test --workspace` / CI.
- [ ] The dependency-graph guard is green with `serde` / `serde_json` /
      `schemars` present — the engine `json` crate has no network dependency.
- [ ] `core` and `detectors` remain dependency-free; only `json` gained deps.
- [ ] The reconciliation with `workspace-layout.md` §2.1 (§9 above) is recorded
      for the Phase 1 review.

The two load-bearing criteria are the **byte-stability test** (it proves the
determinism spine on the contract) and the **schema-validates-example test** (it
proves the artifact external consumers will depend on is real and correct). The
rest is shape.

## 15. Notes on keeping the contract minimal and future-proof (the brief's core)

The Phase 1 brief asks for a minimal contract that supports Rust, Python, Node,
Java, and Go **without breaking changes** and **without Cargo-specific fields**.
The design achieves this through three deliberate choices, each recorded above:

1. **`category` is a closed enum; `kind` and `data` are open.** The fixed axis
   (six categories) is enumerated; the growing axis (per-ecosystem kinds and
   payloads) is open. Adding Go or Java support is: new `kind` strings + `data`
   shapes, zero envelope change — a minor bump at most (§5.2, §5.3).
2. **No ecosystem lives in the envelope.** There is no `cargo`, `npm`, `maven`,
   `go` field anywhere in the contract. Ecosystem identity is a `kind` string on
   an `ecosystem`-category finding. This is the structural guarantee against
   Cargo-specificity (§4.2, §9).
3. **Path-anchored findings from day one.** Because findings carry `path`, a
   polyglot repo (a Go service beside a Node frontend beside a Rust workspace) is
   already expressible in v0 — the hardest and most valuable case
   (`detector-architecture.md` §9) needs no future contract change to support.

The result: every ecosystem in the brief is a Phase 5 "turn the crank" addition
against an unchanged v0 envelope, which is exactly the payoff `roadmap.md`
front-loads this phase to buy.
