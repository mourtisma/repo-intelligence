# Phase 0 — Implementation Plan

> Status: Ready to implement. Scope is defined by `roadmap.md` Phase 0 and
> `workspace-layout.md`. This plan turns those into exact, ordered tasks.
>
> Phase 0 delivers the **empty-but-correct** Rust workspace: four crates that
> compile as shells, wired with *only* the dependency edges from the design, and
> CI that enforces the structural invariants. No analysis logic, no schema types,
> no traits.

## 1. Scope discipline (read this first)

Phase 0 is a skeleton. The single biggest risk is scope creep — pulling work
that belongs to later phases into this one. To keep the plan honest:

**In scope (Phase 0):**

- Root virtual workspace manifest with shared configuration.
- Four crate shells: `json`, `core`, `detectors`, `cli`.
- The exact dependency edges from `workspace-layout.md` §3, and no others.
- `rust-toolchain.toml`, committed `Cargo.lock`, `.gitignore` for build output.
- CI: build, `fmt --check`, `clippy -D warnings`, and a dependency-graph guard.

**Explicitly out of scope (deferred to the phase that owns them):**

| Deferred item                              | Owning phase |
|--------------------------------------------|--------------|
| `Report` / `Finding` / `Diagnostic` types, `SCHEMA_VERSION` | Phase 1 |
| JSON Schema emission into `schema/`        | Phase 1      |
| `Detector` trait, `RepoContext`, `DetectorRegistry`, pipeline | Phase 2 |
| Any concrete detector (Cargo, git-tags…)   | Phase 3      |
| `analyze` subcommand, arg parsing beyond a stub, exit-code logic | Phase 3 |
| `fixtures/` sample repos + golden tests    | Phase 2/3    |
| `skills/` contents                         | Phase 4      |
| MSRV *enforcement* tooling, schema-diff CI | later        |

**Rule:** if a task requires inventing a type, trait, or field that a later
phase is supposed to design, it does not belong in Phase 0. The crates should
compile with essentially empty `lib.rs`/`main.rs` files. We are proving the
*shape and the guardrails*, not any behavior.

**Abstractions we deliberately do NOT introduce yet:** no `default_registry()`
builder, no re-export facades, no trait definitions, no feature-flag matrix in
`detectors`, no error type. Each of those is added by the phase that first needs
it. Adding them now would be speculative and would have to be re-designed once
the consuming code exists.

## 2. Directories created in Phase 0

Only what is needed to compile and to make the intended shape visible:

```
Cargo.toml                     # NEW: virtual workspace manifest
Cargo.lock                     # NEW: committed
rust-toolchain.toml            # NEW
.gitignore                     # UPDATE: ensure /target ignored
crates/
├── json/       { Cargo.toml, src/lib.rs }        # NEW shell
├── core/       { Cargo.toml, src/lib.rs }        # NEW shell
├── detectors/  { Cargo.toml, src/lib.rs }        # NEW shell
└── cli/        { Cargo.toml, src/main.rs }        # NEW shell
.github/workflows/ci.yml       # NEW
```

`skills/`, `fixtures/`, and `schema/` from `workspace-layout.md` are **not**
created in Phase 0 — they have no content yet and empty dirs add noise. They are
created by the phase that first fills them. (Optionally a single
`.gitkeep`-style placeholder if we want the top-level map visible early; treat as
cosmetic, not required.)

## 3. Crate creation order

Create crates **bottom-up along the dependency graph** so every crate compiles
the moment it is added and the graph is never in a broken intermediate state:

```
1. json        (leaf — no internal deps)
2. core        (deps: json)
3. detectors   (deps: core, json)
4. cli         (deps: core, detectors, json)
```

**Why this order:** it mirrors `workspace-layout.md` §3 exactly and means each
step is independently buildable. If we created `cli` first it couldn't compile
until its dependencies existed; bottom-up keeps `cargo build` green at every
commit, which also makes the work bisectable.

## 4. Exact tasks (ordered)

Each task is small and independently verifiable. Recommended: one commit per
task, `cargo build` green after each.

### Task 0.1 — Root workspace manifest

- Create root `Cargo.toml` as a **virtual manifest**: `[workspace]` with
  `resolver = "2"`, `members = ["crates/*"]`, and no `[package]`.
- Add `[workspace.package]` with the shared metadata inherited by all crates:
  `version` (start at `0.0.0`), `edition`, `license`, `rust-version` (MSRV),
  `repository`. These are the single source of truth per `workspace-layout.md`
  §4.
- Add `[workspace.dependencies]` declaring the shared external crates so
  versions are pinned once. Keep this list **minimal** — only what the Phase 0
  shells actually use:
  - `serde` (with `derive`) — `json` will need it in Phase 1; declaring it now in
    the workspace table is fine, but do **not** add it as a crate dependency
    until a crate actually uses it. *(Judgment call: declaring-but-not-using in
    the workspace table is acceptable; pulling it into `json/Cargo.toml` before
    Phase 1 is scope creep. See §7.)*
  - `clap` — same treatment; the `cli` will use it in Phase 3.
- **Acceptance:** `cargo metadata` succeeds and lists a workspace with no members
  yet (or errors cleanly asking for members — expected until Task 0.2+).

### Task 0.2 — `json` crate shell (leaf)

- `crates/json/Cargo.toml`: `[package]` inheriting workspace fields
  (`version.workspace = true`, etc.), `name = "repo-intelligence-json"`. **No
  dependencies.**
- `crates/json/src/lib.rs`: empty (a module doc comment stating its role as the
  JSON contract crate; nothing else). No types yet — those are Phase 1.
- **Acceptance:** `cargo build -p repo-intelligence-json` succeeds; the crate has
  zero dependencies in `cargo tree -p repo-intelligence-json`.

### Task 0.3 — `core` crate shell

- `crates/core/Cargo.toml`: `name = "repo-intelligence-core"`, inherits workspace
  fields, depends on `repo-intelligence-json` via **path** dependency
  (`repo-intelligence-json = { path = "../json" }`). No external deps yet
  (`walkdir`/`ignore` arrive in Phase 2).
- `crates/core/src/lib.rs`: empty with a role doc comment. No trait, no
  `RepoContext` — those are Phase 2.
- **Acceptance:** `cargo build -p repo-intelligence-core` succeeds; `cargo tree`
  shows exactly one internal edge: `core → json`.

### Task 0.4 — `detectors` crate shell

- `crates/detectors/Cargo.toml`: `name = "repo-intelligence-detectors"`, inherits
  workspace fields, path deps on `core` and `json`. No parser crates yet — those
  arrive with real detectors in Phase 3+. **No feature-flag section yet** (Phase
  5 owns it).
- `crates/detectors/src/lib.rs`: empty with a role doc comment. No
  `default_registry()` — that is added in Phase 3 when there is something to
  register.
- **Acceptance:** `cargo build -p repo-intelligence-detectors` succeeds; internal
  edges are exactly `detectors → core` and `detectors → json`.

### Task 0.5 — `cli` crate shell (binary)

- `crates/cli/Cargo.toml`: `name = "repo-intelligence-cli"`, inherits workspace
  fields, `[[bin]]` named `repo-intelligence`, path deps on `core`, `detectors`,
  `json`. No `clap`/`tracing` yet — those arrive in Phase 3.
- `crates/cli/src/main.rs`: a `fn main()` that does the absolute minimum to prove
  the binary links and the layering is exercised. It must **not** implement any
  CLI behavior (no arg parsing, no analysis). A single line printing the tool
  name/version to stderr, exiting 0, is enough. It should reference each library
  crate at least trivially (e.g. `use` them) only if that can be done without
  inventing API; otherwise a bare `main` is fine and the path deps alone prove
  the edges.
- **Acceptance:** `cargo build -p repo-intelligence-cli` succeeds; `cargo run -p
  repo-intelligence-cli` exits 0; `cargo tree -p repo-intelligence-cli` shows all
  three internal edges and no others.

### Task 0.6 — Toolchain pin

- Create `rust-toolchain.toml` pinning the `channel` (a specific stable version,
  e.g. the current stable at bootstrap) and listing components `rustfmt` and
  `clippy` so CI and contributors get them automatically.
- Set the same version as `rust-version` (MSRV) in `[workspace.package]` for now;
  they can diverge later, but starting them equal is the simplest correct state.
- **Acceptance:** `rustc --version` inside the repo matches the pinned channel;
  `cargo fmt` and `cargo clippy` are available without manual `rustup component
  add`.

### Task 0.7 — Lockfile and gitignore

- Run a build to generate `Cargo.lock`; **commit it** (`workspace-layout.md` §5 —
  the workspace produces a binary, so we want reproducible builds).
- Ensure `.gitignore` ignores `/target` (verify the existing one; add if
  missing). Do **not** ignore `Cargo.lock`.
- **Acceptance:** `Cargo.lock` is tracked by git; `/target` is not.

### Task 0.8 — CI workflow

- Create `.github/workflows/ci.yml` running on push and PR. One job (matrix not
  needed in Phase 0) that:
  1. Checks out the repo.
  2. Installs the toolchain from `rust-toolchain.toml` (honored automatically by
     the standard Rust CI action) including `rustfmt` and `clippy`.
  3. Caches the Cargo registry and `target` for speed (optional but cheap).
  4. Runs, and **fails the build on any failure**:
     - `cargo fmt --all -- --check`
     - `cargo clippy --workspace --all-targets -- -D warnings`
     - `cargo build --workspace`
     - `cargo test --workspace` (passes trivially now — no tests yet — but wires
       the step so Phase 1+ tests are enforced automatically)
     - the dependency-graph guard (Task 0.9).
- **Acceptance:** CI is green on a clean checkout; deliberately introducing a
  formatting error or a `#[warn]`-triggering line makes CI red.

### Task 0.9 — Dependency-graph / offline-engine guard

This is the one Phase-0-specific *check* (not just a build) and it protects the
architecture's central invariant: `json`, `core`, and `detectors` must never
gain a **network** dependency (`architecture.md` §5, `roadmap.md` Phase 0).

- Implement as a CI step, kept as simple as possible. Two complementary,
  low-tech approaches — pick the simpler that works:
  - **Denylist via `cargo tree`:** run `cargo tree -p <engine-crate> --edges
    normal` for `json`, `core`, `detectors` and grep the output for a small
    denylist of known network/async-runtime crates (`reqwest`, `hyper`,
    `tokio` with net features, `ureq`, `curl`, `openssl`-for-networking, etc.).
    Any hit fails the step. Crude but transparent and needs no new tooling.
  - **`cargo-deny` (if we accept one dev tool):** a `deny.toml` `[bans]` section
    listing forbidden crates for the engine crates. More robust and also gives us
    license/advisory checks for free, at the cost of adding a tool to CI.
- **Recommendation for Phase 0:** start with the **`cargo tree` + grep denylist**
  — zero new dependencies, obvious behavior, and it is trivially correct while the
  crates are empty (the denylist should currently match *nothing*). Revisit
  `cargo-deny` in Phase 6 hardening when there is more surface to police.
- Also assert **no cycles and correct edges**: a small check that `cargo tree`
  for each crate shows only the allowed internal edges from §3. Even a grep for
  unexpected `repo-intelligence-*` edges (e.g. `core` must not depend on
  `detectors`) is enough for now.
- **Acceptance:** the guard passes on the current empty graph; a test PR that
  adds `reqwest` to `core`, or a `core → detectors` dependency, makes the guard
  fail.

## 5. Crate-by-crate dependency summary (what Phase 0 actually wires)

| Crate                          | Internal deps (path)      | External deps in Phase 0 | Deferred external deps      |
|--------------------------------|---------------------------|--------------------------|-----------------------------|
| `repo-intelligence-json`       | —                         | none                     | `serde`, `schemars` (P1)    |
| `repo-intelligence-core`       | `json`                    | none                     | `walkdir`/`ignore` (P2)     |
| `repo-intelligence-detectors`  | `core`, `json`            | none                     | `toml`, `serde_json`… (P3+) |
| `repo-intelligence-cli`        | `core`, `detectors`, `json` | none                   | `clap`, `tracing` (P3)      |

The point of the "Deferred" column: Phase 0 intentionally ships crates with **no
external dependencies at all**. This makes the offline-engine guard trivially
green and makes the first appearance of any external dependency a reviewable
event tied to the phase that justifies it.

## 6. CI setup details

- **Trigger:** `push` to any branch and `pull_request`.
- **Toolchain:** pinned by `rust-toolchain.toml`; CI must not override it with a
  different channel, so the same compiler that developers use is what gates
  merges — consistent builds matter for a determinism-focused tool.
- **Failure policy:** every step is blocking. `clippy` runs with `-D warnings`
  per `workspace-layout.md` §5 (aggressive lint posture). `fmt --check` prevents
  style drift from day one, before there is enough code for it to be painful.
- **Determinism note:** no determinism/golden tests exist in Phase 0 (there is no
  output yet). The `cargo test` step is wired now so those tests are automatically
  gated the moment Phase 1/2 add them — we set up the harness slot, not the tests.
- **Kept out of Phase 0 CI:** cross-platform matrix, release/publish workflows,
  schema-diff check (Phase 6), coverage. Adding them now is speculative; they
  attach cleanly later.

## 7. Test strategy for Phase 0

Phase 0 has **no runtime behavior to unit-test**, so the "tests" are structural
and are enforced by CI rather than by `#[test]` functions:

1. **It compiles.** `cargo build --workspace` is the primary correctness signal
   for a skeleton. A shell that doesn't build is the only way Phase 0 can be
   "wrong" in behavior.
2. **The graph is exactly right.** The dependency-graph guard (Task 0.9) is the
   real test of Phase 0's *intent*: the edges match `workspace-layout.md` §3, no
   cycles, no forbidden network deps in the engine crates. This is where the
   architectural invariant is actually verified.
3. **Lint/format cleanliness.** `clippy -D warnings` and `fmt --check` gate every
   change. Establishing this on empty crates means the standard is set before any
   real code is written.
4. **The binary links and runs.** `cargo run -p repo-intelligence-cli` exits 0 —
   proves the top of the dependency graph is wired end to end.

**No golden tests, no unit tests of types, no fixture repos in Phase 0** — there
is nothing to assert about output because there is no output. Introducing test
scaffolding for types that don't exist yet would be speculative work that Phase
1/2 would have to redo. The one forward-looking move is wiring the `cargo test`
CI step (Task 0.8) so later phases' tests are enforced automatically without a CI
change.

**A note on the `serde`/`clap` judgment call (Task 0.1):** declaring shared
external crates in `[workspace.dependencies]` without any crate consuming them
yet is a gray area. The plan's stance: it is acceptable to *declare* them
centrally now (it documents intent and pins versions once), but a crate must not
add them to its own `[dependencies]` until the phase that uses them. If we prefer
maximum minimalism, omit them from the workspace table entirely until Phase 1 —
either is defensible; the plan leans toward declaring them to avoid a churny
edit in Phase 1, while keeping the crates themselves dependency-free.

## 8. Acceptance criteria for Phase 0 (exit gate)

Phase 0 is complete when **all** of the following hold — matching `roadmap.md`
Phase 0 exit criteria:

- [ ] Root virtual `Cargo.toml` with `[workspace.package]` and
      `[workspace.dependencies]`; `resolver = "2"`; members = the four crates.
- [ ] `rust-toolchain.toml` pins the channel + `rustfmt`/`clippy`; MSRV declared.
- [ ] `Cargo.lock` is committed; `/target` is gitignored.
- [ ] Four crate shells build: `cargo build --workspace` is green.
- [ ] `cargo run -p repo-intelligence-cli` exits 0.
- [ ] Internal dependency edges match `workspace-layout.md` §3 **exactly**:
      `json` has none; `core → json`; `detectors → {core, json}`;
      `cli → {core, detectors, json}`. No cycles, no extra edges (esp. no
      `core → detectors`).
- [ ] Engine crates (`json`, `core`, `detectors`) have **no external
      dependencies** and the offline-engine guard passes.
- [ ] CI runs `fmt --check`, `clippy -D warnings`, `build`, `test`, and the
      dependency-graph guard, and is green.
- [ ] A deliberately-bad test PR (adds `reqwest` to `core`, or a `core →
      detectors` edge, or a fmt violation) makes CI **red** — proving the guards
      actually guard.

The last checkbox is the most important: Phase 0's value is the *enforced
invariants*, so we verify the guards fail when they should, not just that they
pass on a clean tree.

## 9. Task dependency order (summary)

```
0.1 root manifest
  └▶ 0.2 json ──▶ 0.3 core ──▶ 0.4 detectors ──▶ 0.5 cli
0.6 toolchain pin        (independent; do early so fmt/clippy exist)
0.7 lockfile + gitignore (after crates exist so lock is populated)
0.8 CI workflow          (after 0.5–0.7; needs crates + toolchain)
  └▶ 0.9 dependency-graph guard (a step within/alongside CI)
```

Crates are strictly bottom-up (0.2 → 0.5). Toolchain (0.6) can be done first so
`fmt`/`clippy` are present while writing shells. CI (0.8/0.9) comes last because
it gates everything the earlier tasks produced.
