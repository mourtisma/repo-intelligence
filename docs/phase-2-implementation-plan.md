# Phase 2 — Implementation Plan

> Status: Ready to implement. Scope is defined by `roadmap.md` Phase 2,
> `architecture.md` §4–5, and `detector-architecture.md` §1–3, §5, §10. This plan
> turns those into exact, ordered tasks.
>
> Phase 2 delivers the **core pipeline + determinism**: `repo-intelligence-core`
> can scan a repository into a read-only `RepoContext`, run a (still empty)
> `DetectorRegistry` over it, run a synthesis pass, and assemble a valid `Report`
> using the Phase 1 `json` contract — **byte-identically across runs and
> machines**. No concrete detector, no CLI behavior, no ecosystem knowledge.

## 1. Phase Objectives

### What Phase 2 achieves

Phase 2 builds the **orchestration engine** in `core`: the machinery that turns a
directory on disk into a `Report`, with zero detectors registered. Concretely it
introduces, in `repo-intelligence-core`:

- A **single deterministic repository scan** into a shared, read-only
  `RepoContext` (`architecture.md` §4.1).
- The **`Detector` trait** and a **`DetectorRegistry`** (`detector-architecture.md`
  §2–3).
- The **run pipeline**: for each applicable detector, collect its output; then a
  **synthesis pass** over all findings; then **assemble a `Report`**
  (`detector-architecture.md` §3, §5).
- A **determinism harness** proving that the same fixture directory produces a
  byte-identical `Report` across N runs (and, structurally, across machines),
  excluding only the CLI-injected `analyzed_at` / `tool_version` — which the
  engine does not set at all in Phase 2.

### What problems it solves

- **Determinism becomes a property of the pipeline, not of each detector.** The
  single traversal point (§4.1) is where sorted ordering, symlink policy, and
  ignore-handling live. Prove it once, with zero detectors, so any later
  non-determinism is unambiguously a specific new detector's fault
  (`roadmap.md` Phase 2 rationale).
- **The detector extension contract gets fixed in code.** The `Detector` trait,
  the `applies()`/`detect()` split, and the "detectors are pure functions of an
  immutable `&RepoContext`" rule become compilable, testable reality rather than
  prose — so Phase 3's first real detector and Phase 5's hundredth are added the
  same way (`detector-architecture.md` §7).
- **The Phase 1 contract gets exercised end-to-end by a producer.** Phase 1
  proved the report *serializes* stably given a hand-authored value. Phase 2 is
  the first code that *produces* a `Report` programmatically, validating that the
  contract can actually be assembled from a scan + findings.

### Why it exists before detector implementation

`roadmap.md` sequences this deliberately (P1 → P2 → P3, strictly sequential):
determinism and the extension model are **irreversible** infrastructure. If a
detector were written first, its incidental needs would shape the `RepoContext`
and the trait by accident, and determinism would be retrofitted per-detector
instead of enforced centrally. Building the empty pipeline first means the
hardest-to-change things are settled while there is nothing depending on them,
and the first detector (Phase 3) drops into a proven harness.

## 2. Scope

### In scope (Phase 2)

- `RepoContext`: a read-only, in-memory model of the scanned repository —
  deterministically-ordered file listing, on-demand file-content reads, and the
  repository root. Populated by a **single traversal**.
- A **traversal** with: sorted, stable ordering; an explicit symlink policy;
  ignore-file / hidden-file handling; and defensive handling of unreadable
  entries (recorded as diagnostics, never panics).
- The **`Detector` trait** (`id`, `category`, `applies`, `detect`) and a
  **`DetectionOutput`** type carrying `Vec<Finding>` + `Vec<Diagnostic>`.
- The **`DetectorRegistry`**: an ordered collection of `Box<dyn Detector>` with a
  way to register detectors and iterate them. **Empty by default in `core`** —
  `core` never names a concrete detector.
- The **run pipeline**: gate each detector via `applies()`, run `detect()`,
  collect findings + diagnostics, run a **synthesis pass**, assemble a `Report`.
- The **synthesis pass** as an explicit, pure step over the full finding set —
  present and wired, even though with zero detectors it is an identity function
  in Phase 2 (see §3 for why it is included now, not deferred).
- **Deterministic ordering of findings and diagnostics** in the assembled report
  (the `json` crate preserves `Vec` order; Phase 2 owns *producing* a stable
  order — `phase-1-implementation-plan.md` §8).
- A **`fixtures/` directory** with a small number of **ecosystem-neutral** fixture
  repositories (plain files/dirs, no manifests) sufficient to exercise traversal
  ordering, nesting, ignore-handling, symlinks, and the empty-run path.
- A **determinism harness** (test) that runs the pipeline on a fixture N times and
  asserts byte-identical report output.
- New external dependencies enter `core` here: a traversal crate (`walkdir` or
  `ignore`) and whatever minimal support the context needs. `serde`/`serde_json`
  only if genuinely required by `core` (it should not be — `core` builds `json`
  types directly).

### Out of scope (deferred to the phase that owns it)

| Deferred item | Owning phase |
|---|---|
| Any concrete detector (Cargo, git-tags, npm, …) | Phase 3+ |
| Any ecosystem-specific parsing (`toml`, XML, `go.mod`, …) | Phase 3+ |
| `default_registry()` **populated** with real detectors | Phase 3 (the *builder location* is CLI-side; the *content* is Phase 3) |
| CLI `analyze` command, arg parsing, exit codes | Phase 3 |
| `analyzed_at` / `tool_version` **injection** (runtime values) | Phase 3 (CLI) |
| stdout/stderr discipline, `tracing` logging | Phase 3 |
| Golden **report** fixtures with `expected.json` per ecosystem | Phase 3 |
| "Likely primary" correlation *logic* inside synthesis | Phase 3+ (when >1 detector exists to correlate) |
| Parallel detector execution | Phase 6 (only if measured) |
| Config / ignore-pattern injection API | deferred until a concrete use case |
| Git metadata in the context (tags, HEAD, branches) | Phase 3 (git-tag detector's need drives it, added additively) |

**Rule:** if a component exists only to serve one future detector's need, it does
not belong in Phase 2. The context must be able to *grow* to serve detectors
additively (file reads are generic); it must not *pre-bake* any ecosystem's or
any specific detector's requirements. In particular, **no git integration in
Phase 2** — the git-tag detector arrives in Phase 3 and brings its context needs
with it as an additive extension.

**Abstractions we deliberately do NOT introduce yet:** no detector-dependency
DAG or scheduler (`detector-architecture.md` §5 rejects it); no caching/memoization
layer beyond reading each file at most once; no async; no plugin loading; no
context "views" or typed accessors per category; no error type hierarchy beyond
what diagnostics + a single fatal engine error require; no builder pattern for
`RepoContext` beyond a single constructor.

## 3. Required Components

Only what the empty pipeline genuinely needs. Each is justified against a
Phase 2 exit criterion or an architecture invariant; anything not so justified is
out of scope.

1. **`RepoContext` (read-only repository model).**
   - Holds: the repository **root path**, a **deterministically-ordered list of
     relative file paths**, and a means to **read a file's contents on demand**
     (reading each file at most once — `architecture.md` §4.1). Paths are
     repo-relative and **forward-slash normalized** at the boundary
     (`phase-1-implementation-plan.md` §8), so downstream `Finding.path` /
     `Provenance.sources` are stable across OSes.
   - Immutable after construction. Detectors receive `&RepoContext`.
   - *Why now:* it is the single-point-of-determinism the whole architecture
     rests on (§4.1). It must exist before any detector so detectors are pure.

2. **The single traversal (context builder).**
   - Walks the root **once**, producing the sorted file list. Applies the symlink
     policy and ignore/hidden-file handling. Converts unreadable entries into
     **diagnostics** rather than aborting (`architecture.md` §4.3).
   - *Why now:* traversal is the dominant source of non-determinism (ordering,
     symlinks, permissions). Centralizing it is the point of §4.1.

3. **`Detector` trait + `DetectorCategory` usage.**
   - `id() -> &'static str`, `category() -> DetectorCategory`,
     `applies(&RepoContext) -> bool`, `detect(&RepoContext) -> DetectionOutput`.
     `Send + Sync` (so Phase 6 parallelism needs no signature change).
   - Uses the `DetectorCategory` **from the `json` crate** — `core` does not
     redefine it.
   - *Why now:* it is the extension contract; fixing it in code is a primary goal.

4. **`DetectionOutput`.**
   - Carries `Vec<Finding>` + `Vec<Diagnostic>` (both `json` types). The return
     type of `detect()`.
   - *Why now:* `detector-architecture.md` §2 names it; it is the detector→pipeline
     interface. Trivial, but explicit beats a bare tuple for readability and
     future additive growth.

5. **`DetectorRegistry`.**
   - An ordered set of `Box<dyn Detector>` with registration + iteration. Empty in
     `core`. The **registration order is defined and stable** (it feeds ordering —
     see §6).
   - *Why now:* the pipeline iterates it; the ordering guarantee lives here.

6. **The run pipeline (the orchestrator).**
   - Input: `&RepoContext` + `&DetectorRegistry`. For each detector where
     `applies()` is true, call `detect()`, accumulate findings + diagnostics
     (plus any traversal diagnostics). Then run synthesis. Then assemble the
     `Report`. Produces a `Report` with **empty `tool_version` / `analyzed_at`**
     (or a documented placeholder) — the engine never sets those (§5, §7).
   - *Why now:* it is the thing Phase 2 exists to build.

7. **The synthesis pass (present, minimal).**
   - Signature: `findings + diagnostics → findings + diagnostics` (a pure
     function). In Phase 2 it is effectively identity (zero detectors → nothing to
     correlate).
   - *Why include it now rather than defer:* the architecture makes synthesis the
     **only** place cross-detector correlation may happen
     (`detector-architecture.md` §5). Wiring the *seam* now — an explicit,
     labeled, pure step the pipeline calls — means Phase 3 adds correlation
     *logic* inside an existing function rather than restructuring the pipeline.
     The seam is cheap and load-bearing; the *logic* is correctly deferred. This
     is the one "empty but present" component, and it is justified because its
     *absence* would invite a Phase 3 detector to correlate by calling another
     detector, which the architecture forbids.

8. **Deterministic result ordering.**
   - A defined, total, stable order for the final `findings` and `diagnostics`
     vectors (see §6). Owned by the pipeline (the `json` crate preserves order but
     does not impose one).
   - *Why now:* byte-stability across runs is an exit criterion; unordered
     accumulation from a registry + traversal is a determinism hazard.

9. **`fixtures/` (ecosystem-neutral).**
   - A handful of plain directory trees (nested files, a `.gitignore`-style ignore
     case, a symlink case if the platform supports it, an empty dir) — **no
     manifests, no ecosystem markers**. Enough to test traversal + the empty run.
   - *Why now:* the determinism harness needs real input; hand-authoring a
     `RepoContext` in a unit test covers detector purity, but only a real scan
     exercises traversal determinism.

10. **Determinism harness (test infrastructure).**
    - Runs the full pipeline on a fixture repeatedly and asserts byte-identical
      serialized output; also asserts stability of the file-listing order
      independently.
    - *Why now:* it is the Phase 2 exit gate made executable.

Nothing else. No config, no git, no logging, no parallelism, no per-category
context accessors.

## 4. Crate Responsibilities

Verified against `workspace-layout.md` §2–3. The dependency graph is unchanged;
only `core` gains code and dependencies.

| Crate | Phase 2 change | Responsibility in Phase 2 |
|---|---|---|
| `repo-intelligence-json` | **Unaffected** (no code change) | Still the leaf contract. `core` builds `Report`/`Finding`/`Diagnostic` from it. If — and only if — a genuinely contract-shaped need surfaces (it should not), it would be an additive minor bump; **the plan's expectation is zero changes to `json`.** |
| `repo-intelligence-core` | **All Phase 2 work lands here** | `RepoContext` + traversal; `Detector` trait; `DetectionOutput`; `DetectorRegistry`; run pipeline; synthesis seam; result ordering; determinism-enforcement. Gains a traversal dependency (`walkdir`/`ignore`). |
| `repo-intelligence-detectors` | **Unaffected** (stays an empty shell) | No concrete detector yet. It will implement `core::Detector` in Phase 3. Its `Cargo.toml` edges (`core`, `json`) are already correct from Phase 0. |
| `repo-intelligence-cli` | **Unaffected** (stays a stub) | Still just prints name/version. It will build a registry and call the pipeline in Phase 3. |

**Dependency-direction check (`workspace-layout.md` §3):** `core → json` only.
`core` must **not** depend on `detectors` (the inversion in §2.2). The
`DetectorRegistry` holds trait objects; concrete detectors are injected by a
caller (the CLI, in Phase 3) — `core` stays ignorant of them. The dependency-graph
guard (`scripts/check-engine-deps.sh`) must remain green; the **new traversal
dependency must not be on the network denylist** and must not pull a transitive
network/async dep.

**`workspace-layout.md` alignment notes:**
- §2.2 lists `core`'s owned concepts (Detector trait, RepoContext, registry,
  pipeline, determinism enforcement) — Phase 2 implements exactly that list, and
  nothing from §2.3/§2.4.
- §2.2 anticipates `walkdir`/`ignore` as `core`'s infrastructure deps — this plan
  picks one (see §6) and records the choice.

## 5. Pipeline Lifecycle

The full analysis lifecycle, with the architecture documents as source of truth
(`architecture.md` §4, `detector-architecture.md` §3):

```
Repository on disk (root path, provided by the caller)
        │
        │  single traversal (sorted, symlink policy, ignore handling;
        │  unreadable entries → diagnostics, never abort)
        ▼
RepoContext  (immutable: root + ordered relative paths + on-demand reads)
        │
        │  for each detector in the registry, in registration order:
        │      if applies(&ctx):  detect(&ctx) -> DetectionOutput
        │      (each detector sees only &RepoContext — pure, order-free)
        ▼
Collected findings + diagnostics
   (detector outputs ⊕ traversal diagnostics, accumulated)
        │
        │  synthesis pass: pure fn(findings, diagnostics) -> (findings, diagnostics)
        │  (Phase 2: identity — no detectors to correlate)
        ▼
Ordered findings + diagnostics
   (apply the deterministic total order — §6)
        │
        │  assemble Report (json crate):
        │    schema_version = SCHEMA_VERSION
        │    tool_version   = "" / unset  (CLI injects in Phase 3)
        │    analyzed_at    = "" / unset  (CLI injects in Phase 3)
        │    findings, diagnostics = the ordered vectors
        ▼
Report  (a valid, schema-conformant, byte-stable value)
```

Key lifecycle rules, each traceable to the architecture:

- **One traversal, up front.** Detectors never walk the filesystem
  (`architecture.md` §4.1; `detector-architecture.md` §1).
- **`applies()` gates `detect()`.** Skips obviously-irrelevant detectors and keeps
  output sparse (`detector-architecture.md` §2). With zero detectors this loop is
  empty but present.
- **No detector calls another detector.** Correlation is synthesis-only
  (`detector-architecture.md` §3, §5).
- **Errors are data.** A detector that hits malformed input pushes a diagnostic;
  the pipeline does not abort (`architecture.md` §4.3). A traversal problem on one
  entry is a diagnostic, not a failure. The **only** thing that aborts the run is
  an unusable root (does not exist / not a directory / unreadable), which surfaces
  as a fatal engine error to the caller — mapped to exit 2 by the CLI in Phase 3
  (`architecture.md` §6).
- **The engine injects no ambient values.** `tool_version` / `analyzed_at` are
  left empty by `core` and are excluded from determinism tests by construction —
  because `core` never sets them, there is nothing time-like to exclude
  (`architecture.md` §5).

## 6. Determinism Strategy

Determinism is the load-bearing Phase 2 property (`architecture.md` §5,
`roadmap.md` Phase 2 exit criterion). It is enforced structurally at four points:

### Stable ordering

- **Traversal order is sorted, not filesystem-native.** Filesystem enumeration
  order is unspecified and varies by OS/filesystem. The traversal produces a file
  list sorted by a **total order on the normalized relative path** (byte-wise on
  the forward-slash-normalized string). This is the single ordering decision the
  whole system inherits.
- **Registry order is registration order**, which is fixed and defined by the
  caller assembling the registry (Phase 3's CLI). In Phase 2 the registry is empty
  so this is moot, but the *guarantee* (iteration = registration order, no
  reordering, no hashing) is fixed now.
- **Final `findings` / `diagnostics` order is a defined total order.** Because
  detectors may emit findings in any internal order and the registry contributes
  in registration order, the pipeline applies a **stable total order** to the
  accumulated vectors before assembling the report. Recommended sort key, in
  order: (`path` with `None` last or first — pick one and pin it),
  (`category`), (`kind`), then a stable tiebreak (e.g. detector `id`, then
  original insertion index to make the sort **stable and total** even for
  otherwise-equal findings). The exact key is an implementation choice, but it
  **must be total** (no two distinct findings compare equal-and-unordered) so the
  output cannot depend on accumulation order. Document the chosen key; it becomes
  part of the observable contract that Phase 3 golden files will lock in.

### Stable serialization

- Inherited from Phase 1: struct field order is declaration order; enums serialize
  as strings; `serde_json` `preserve_order` is enabled; nothing is a `HashMap`.
- **`core` must not introduce a `HashMap`/`HashSet` anywhere its iteration order
  can reach the report** (e.g. deduplicating findings via a hash set then
  iterating it). If dedup/sets are needed, use a `BTreeMap`/`BTreeSet` or sort
  after collecting. This is the single most likely place a determinism bug enters
  in Phase 2.

### Repeatable outputs

- The determinism harness serializes the report from N independent pipeline runs
  on the same fixture and asserts byte-identical results. Because `core` sets no
  timestamp and does no I/O beyond reading files, there is no ambient input to
  vary between runs.

### Platform independence

- **Path normalization at the traversal boundary:** all stored/emitted paths are
  repo-relative and forward-slash, so a report from Windows matches one from Linux
  for the same tree.
- **Explicit symlink policy** (recommend: do not follow symlinks; record symlinked
  entries as files/skip with a diagnostic — pick one and pin it) so symlink
  handling can't diverge by platform default.
- **Byte-wise path sort** (not locale-aware collation) so ordering is identical
  regardless of system locale (`architecture.md` §5, "no locale-dependent
  formatting").
- **Line-ending / encoding neutrality:** the context returns raw file bytes (or
  reads lazily); it does not normalize content, so no platform-dependent text
  transformation occurs in `core`. Any parsing is a Phase 3 detector's job.

### Determinism risks (identified now, mitigations in §9)

- Filesystem enumeration order leaking through (mitigation: mandatory sort).
- A `HashMap` in `core` (mitigation: ban ordered-output hashing; use ordered
  collections; the review checklist flags it).
- Locale-aware string comparison (mitigation: byte-wise compare).
- Symlink cycles / following (mitigation: don't follow; or detect and diagnose).
- Non-UTF-8 paths (mitigation: define behavior — lossy-convert + diagnostic, or
  skip + diagnostic; pin one).

## 7. Public API Design

Prefer the **smallest surface that lets Phase 3 add a detector and a CLI without
touching `core`.** All new public items live in `repo-intelligence-core`.

| New public item | Kind | Why it must exist | Why this crate | Future-compat |
|---|---|---|---|---|
| `RepoContext` | struct | The read-only repo model detectors query (`architecture.md` §4.1). | `core` owns the scan (`workspace-layout.md` §2.2). | Fields kept **private**; expose narrow accessors (root, iterate paths, read a file) so the internal representation can grow (e.g. add git metadata in Phase 3) without a breaking change. |
| `RepoContext` accessors | methods | Detectors need: repo root, an ordered iterator over relative paths, and a "read this file" call. | Same. | Adding accessors later is additive. Start with the minimum three; do **not** add per-category or per-ecosystem views. |
| `Detector` | trait (`Send + Sync`) | The extension contract (`detector-architecture.md` §2). | `core` defines the trait; `detectors` implements it (the inversion, §2.2). | Trait method set is the hardest thing to change — keep to the four documented methods. `Send + Sync` now avoids a breaking bound-add for Phase 6 parallelism. |
| `DetectorCategory` | (re-export from `json`) | `Detector::category()` returns it. | Defined in `json` (Phase 1); `core` re-exports for ergonomics so implementers need one import. | Re-export, not redefinition — single source of truth. |
| `DetectionOutput` | struct | Return type of `detect()`: `findings` + `diagnostics`. | The detector→pipeline interface lives in `core`. | Public fields or a tiny constructor; additive to add fields later. Keep to the two documented vectors. |
| `DetectorRegistry` | struct | Holds and orders detectors; the pipeline iterates it; the CLI builds it. | `core` owns the pipeline. | Provide construction + registration + iteration only. No querying/filtering API until a consumer needs it. |
| The run entry point (e.g. `analyze(root) -> Result<Report, EngineError>` **or** `run(ctx, registry) -> Report`) | fn | The one call that produces a report. | `core` is the engine. | **Decide the seam deliberately** (see note). Keep it to one primary entry point. |
| `EngineError` | enum (small) | The single fatal condition (unusable root) the caller must distinguish from a valid-report-with-diagnostics. | `core` produces it; the CLI maps it to exit 2 (`architecture.md` §6). | Keep it minimal (e.g. `RootNotFound`, `RootNotADirectory`, `RootUnreadable`). Detector/traversal-entry problems are **diagnostics**, not variants. Additive to extend. |

**Note on the run entry point — the one real API-shape decision:** two viable
seams, both keep `core` ecosystem-neutral and detector-injected:

- **Option A (recommended): two functions.** A `scan(root) -> Result<RepoContext,
  EngineError>` and a `run(&RepoContext, &DetectorRegistry) -> Report`. This makes
  the context observable/testable on its own, lets detector unit tests build a
  `RepoContext` from a real scan or a synthetic one, and keeps each step small.
- **Option B: one function.** `analyze(root, &DetectorRegistry) -> Result<Report,
  EngineError>` that scans then runs. Simpler call site, but couples scan+run and
  makes it harder to feed a synthetic context in tests.

Recommend **A** — it is barely more surface (two small functions vs one) and it
directly serves the testing strategy (§8) and detector purity. The CLI in Phase 3
composes them.

**What is deliberately NOT public:** the traversal function/algorithm (internal),
the sort-key implementation (internal — the *ordering guarantee* is public via
docs + tests, the *code* is not), the synthesis function (internal to the
pipeline; it is not an extension point), and any file-cache internals of
`RepoContext`.

## 8. Testing Strategy

Phase 2 has real behavior (scanning, ordering, assembling), so tests are `#[test]`
functions plus fixture-backed integration tests. No golden *ecosystem* reports yet
(no detectors to produce them).

### Unit tests

- **`RepoContext` from a synthetic construction and from a real scan:** correct
  root, correct set of relative paths, paths forward-slash normalized, each file
  readable once with expected contents.
- **Traversal ordering:** given a known tree, the produced path list equals the
  expected **sorted** list — including nested dirs, so ordering is proven
  independent of filesystem enumeration.
- **Ignore/hidden handling:** entries matching the ignore policy are excluded
  (exact policy pinned by these tests).
- **Symlink policy:** a symlink is handled per the pinned policy (skipped/diagnosed
  or treated-as-file) — guarded so a platform without symlink support is skipped
  cleanly, not failed.
- **`Detector` purity via a fake detector:** a test-only detector implementing the
  trait, fed a synthetic `RepoContext`, returns the same `DetectionOutput` on
  repeated calls (`detector-architecture.md` §10 unit style). Proves the trait is
  usable and order-free.
- **Registry:** registration order preserved on iteration; empty registry yields
  an empty run.
- **`applies()` gating:** a fake detector whose `applies()` is false contributes
  nothing.
- **Result ordering:** given fake detectors emitting findings in a deliberately
  "wrong" order, the assembled report's `findings`/`diagnostics` are in the defined
  total order regardless of emission/registration order.
- **Errors-as-data:** an unreadable entry becomes a diagnostic and the run still
  completes with a valid report; an unusable **root** yields `EngineError`, not a
  report.

### Integration tests (fixture-backed)

- **Empty-detector run on each fixture** produces a **schema-valid, well-formed
  `Report`** with empty `findings` (registry is empty) and only traversal-derived
  diagnostics (ideally none for clean fixtures). This is the `roadmap.md` Phase 2
  exit criterion. Validate against the emitted schema the same way Phase 1 does
  (parse `schema/report.schema.json`, check structure) — reuse, do not duplicate,
  the Phase 1 approach.
- **End-to-end assembly:** `scan` → `run` on a fixture yields a report whose
  `schema_version` == `SCHEMA_VERSION` and whose `tool_version`/`analyzed_at` are
  the documented empty placeholder (engine sets nothing).

### Determinism tests (the exit gate)

- **N-run byte-stability:** run the full pipeline on a fixture N times; assert all
  serialized outputs are byte-identical (`roadmap.md` Phase 2 exit criterion).
- **Order-independence:** with fake detectors registered in different orders and
  emitting in different internal orders, assert the final serialized report is
  identical — proving the total-order sort, not accumulation order, decides output.
- **Path-normalization stability:** assert emitted paths are forward-slash and
  repo-relative (a structural proxy for cross-platform identity, since we can't run
  two OSes in one test).
- **"No `HashMap` in the output path" is enforced by the order-independence test**
  rather than by inspection — if a hash-ordered collection leaked in, the
  different-registration-order test would flake/fail.

## 9. Risks

### Over-engineering risks

- **Building a detector scheduler / dependency DAG.** *Mitigation:* explicitly
  out of scope; `detector-architecture.md` §5 already rejects it. The synthesis
  seam is the sanctioned correlation mechanism; keep it an identity function until
  Phase 3 has ≥2 detectors to correlate.
- **A "fat" context with typed per-category accessors or git metadata now.**
  *Mitigation:* context exposes only root + ordered paths + file read. Git and any
  richer view are added additively by the phase whose detector needs them (Phase
  3). Private fields make that growth non-breaking.
- **A caching/indexing layer.** *Mitigation:* "read each file at most once" is the
  only caching guarantee; implement it as the simplest memoized read, not a
  subsystem.
- **An error-type hierarchy.** *Mitigation:* one small `EngineError` for the fatal
  root case; everything else is a diagnostic.

### Abstraction risks

- **Generic parameters / associated types on `Detector`.** *Mitigation:* the
  architecture fixes it as a plain `dyn`-compatible trait (trait objects,
  `detector-architecture.md` §2). No generics — dynamic dispatch cost is
  irrelevant next to I/O.
- **Over-abstracting the pipeline into configurable stages.** *Mitigation:* the
  lifecycle is a fixed 4-step sequence (§5); implement it as such, not as a
  pluggable stage framework.

### Migration risks

- **The run-entry-point shape (§7) is hard to change once the CLI and detector
  tests depend on it.** *Mitigation:* choose Option A deliberately now, document
  it, and keep it to two small functions so a future change is localized.
- **The result-ordering key becomes observable via Phase 3 golden files.**
  *Mitigation:* choose a total, documented order in Phase 2 and pin it with tests
  *before* any golden file exists, so Phase 3 goldens lock in an intentional order,
  not an accidental one.
- **Symlink / non-UTF-8 / ignore policies are contract-shaping.** *Mitigation:*
  pin each policy explicitly with a test in Phase 2; a later change is then a
  visible, reviewed decision, not silent drift.

### Future detector compatibility risks

- **A future detector needs data the context can't provide (e.g. git tags).**
  *Mitigation:* this is expected and handled additively — the context grows a new
  accessor; existing detectors and the trait are untouched (`architecture.md` §4.1
  tradeoff, accepted). Phase 2's job is to make that growth additive, which private
  fields + narrow accessors guarantee.
- **A future detector wants to correlate with another.** *Mitigation:* it does so
  in synthesis, which already exists as a seam — no pipeline change needed.
- **Parallelism later.** *Mitigation:* `Detector: Send + Sync` and `&RepoContext`
  immutability are established now, so Phase 6 parallelization needs no signature
  or ownership change (`architecture.md` §9).

## 10. Task Breakdown

Ordered so each task compiles and is reviewable independently; `cargo build` +
`cargo test` + `clippy -D warnings` + `fmt --check` + the dependency guard green
after each. One commit per task recommended.

### Task 2.1 — Traversal dependency + guard verification
- **Objective:** add the chosen traversal crate (`ignore` recommended for built-in
  ignore-file handling, or `walkdir` for minimalism — pin the choice) to
  `[workspace.dependencies]` and to `core`'s `Cargo.toml`.
- **Dependencies:** none.
- **Acceptance:** `core` builds; `scripts/check-engine-deps.sh` is green (the new
  crate is not on the network denylist and pulls no network/async transitive dep);
  `core → json` remains the only internal edge.

### Task 2.2 — `RepoContext` type + accessors (no traversal yet)
- **Objective:** define `RepoContext` with private fields (root, ordered relative
  paths, file-read mechanism) and the minimal public accessors (root, ordered path
  iterator, read-file). Constructible from an explicit in-memory list for testing.
- **Dependencies:** 2.1.
- **Acceptance:** unit tests build a `RepoContext` from a synthetic path set and
  assert accessors return normalized, ordered paths and correct reads.

### Task 2.3 — The single traversal (populates `RepoContext`)
- **Objective:** implement `scan(root) -> Result<RepoContext, EngineError>`:
  one walk, byte-wise sorted paths, forward-slash normalization, symlink policy,
  ignore/hidden policy, unreadable-entry → diagnostic, unusable-root →
  `EngineError`.
- **Dependencies:** 2.2.
- **Acceptance:** fixture-backed tests assert exact sorted path lists for nested
  trees, ignore-exclusions, symlink policy (guarded), and that a missing/…root
  yields `EngineError`; scanning is repeatable (same list every run).

### Task 2.4 — `Detector` trait + `DetectionOutput` + `DetectorCategory` re-export
- **Objective:** define the `Detector` trait (`id`/`category`/`applies`/`detect`,
  `Send + Sync`), `DetectionOutput { findings, diagnostics }`, and re-export
  `DetectorCategory` from `json`.
- **Dependencies:** 2.2 (needs `&RepoContext` in signatures).
- **Acceptance:** a **test-only** fake detector implements the trait and compiles;
  a unit test calls `detect()` on a synthetic context and asserts stable output.

### Task 2.5 — `DetectorRegistry`
- **Objective:** an ordered registry of `Box<dyn Detector>` with registration +
  iteration in registration order. Empty by default.
- **Dependencies:** 2.4.
- **Acceptance:** unit tests assert registration order is preserved and an empty
  registry iterates to nothing.

### Task 2.6 — Result ordering + synthesis seam
- **Objective:** implement the deterministic total-order sort for
  findings/diagnostics and the pure synthesis function (identity in Phase 2), as
  internal pipeline helpers. Document the sort key.
- **Dependencies:** 2.4 (finding/diagnostic handling).
- **Acceptance:** unit tests feed unordered findings and assert the defined total
  order; synthesis is a pure pass-through verified by test.

### Task 2.7 — The run pipeline + `Report` assembly
- **Objective:** implement `run(&RepoContext, &DetectorRegistry) -> Report`: gate
  via `applies()`, run `detect()`, accumulate (+ traversal diagnostics), synthesize,
  order, assemble `Report` with `schema_version = SCHEMA_VERSION` and empty
  `tool_version`/`analyzed_at`.
- **Dependencies:** 2.3, 2.5, 2.6.
- **Acceptance:** an empty-registry run on a fixture yields a schema-valid `Report`
  with empty findings; a fake-detector run yields its findings in the defined
  order.

### Task 2.8 — Ecosystem-neutral fixtures
- **Objective:** create `fixtures/` with a small neutral set (nested tree, ignore
  case, symlink case if supported, empty dir). No manifests, no ecosystem markers.
- **Dependencies:** 2.3 (to know what traversal consumes).
- **Acceptance:** fixtures exist and are consumed by 2.3/2.7 tests; `workspace-
  layout.md` §1's `fixtures/` dir now has content justified by Phase 2.

### Task 2.9 — Determinism harness
- **Objective:** the N-run byte-stability test and the order-independence test
  (fake detectors in varied registration/emission order → identical output).
- **Dependencies:** 2.7, 2.8.
- **Acceptance:** both tests pass; deliberately introducing a `HashMap` in the
  output path (a throwaway experiment) is observed to break the order-independence
  test — confirming the guard actually guards. `roadmap.md` Phase 2 exit criterion
  met.

### Task 2.10 — Docs + guard + CI verification + reconciliation note
- **Objective:** document the run entry-point seam (§7 Option A), the pinned
  policies (symlink, ignore, non-UTF-8, sort key), and the empty
  `tool_version`/`analyzed_at` engine behavior in `core`'s module docs. Confirm CI
  (`fmt`/`clippy`/`build`/`test`/guard) green. Record any deviation from the
  architecture docs for the Phase 2 review.
- **Dependencies:** all prior.
- **Acceptance:** `cargo test --workspace` green in CI; guard green with the new
  traversal dep present; policies documented; deviations (if any) recorded.

### Task order summary

```
2.1 dep+guard
  └▶ 2.2 RepoContext ─▶ 2.3 traversal ─┐
                    └▶ 2.4 Detector+Output ─▶ 2.5 registry ─┐
                                          └▶ 2.6 ordering+synthesis ─┐
                                                                     ▼
                                        2.3 + 2.5 + 2.6 ─▶ 2.7 pipeline+assembly
                                                              └▶ 2.8 fixtures ─▶ 2.9 determinism harness ─▶ 2.10 docs+CI
```

## 11. Phase Completion Criteria

Phase 2 is complete when **all** hold — matching `roadmap.md` Phase 2 exit
criteria and the invariants above. Each is objective and testable.

- [ ] `repo-intelligence-core` defines `RepoContext` (immutable, read-only) built
      by a **single** traversal, with paths repo-relative, forward-slash
      normalized, and **byte-wise sorted**.
- [ ] `core` defines the `Detector` trait (`id`, `category`, `applies`, `detect`;
      `Send + Sync`), `DetectionOutput`, and `DetectorRegistry` — and **does not
      depend on `detectors`** (verified by the dependency guard).
- [ ] The run pipeline gates via `applies()`, runs `detect()`, accumulates findings
      + diagnostics, runs a pure synthesis pass, applies a **total, documented
      ordering**, and assembles a valid `Report`.
- [ ] An **empty-detector run** on each fixture produces a **schema-valid,
      well-formed `Report`** (empty `findings`; `schema_version` == `SCHEMA_VERSION`;
      `tool_version`/`analyzed_at` the documented empty placeholder set by the CLI
      later, not by the engine).
- [ ] The **determinism harness passes**: N runs on a fixture are byte-identical,
      and reports are identical under varied detector registration/emission order.
- [ ] Errors-as-data holds: an unreadable entry yields a diagnostic + a valid
      report; an unusable root yields `EngineError` (no report).
- [ ] Symlink, ignore/hidden, and non-UTF-8 policies are **pinned by tests** and
      documented.
- [ ] `json` is **unchanged**; `detectors` and `cli` remain the Phase 0/1 shells.
- [ ] The dependency-graph guard is green **with the new traversal dependency
      present** — no network/async dep entered any engine crate.
- [ ] CI (`fmt --check`, `clippy -D warnings`, `build`, `test`, guard) is green.
- [ ] Any deviation from the architecture documents is recorded for the Phase 2
      review (per the `phase-review` workflow).

The two **load-bearing** criteria are the **determinism harness** (it proves the
pipeline's central promise before any detector can muddy it) and the
**empty-run schema-valid report** (it proves the pipeline correctly produces the
Phase 1 contract end-to-end). The rest is structure that makes Phase 3's first
detector a drop-in.
```
