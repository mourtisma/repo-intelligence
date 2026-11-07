# repo-intelligence

Repository intelligence for Claude Code: inspect projects, manage releases, analyze dependencies, and automate developer workflows across ecosystems.

> **Status: early preview (0.1).** Today, `repo-intelligence` analyzes **Rust/Cargo
> repositories and their git tags** and prints a structured JSON report. The
> Universal Release Manager Skill for Claude Code, and support for other
> ecosystems, are planned (see [`docs/roadmap.md`](docs/roadmap.md)).

## Install

Requirements:

- **Rust 1.85 or newer.** Older toolchains fail with a misleading
  `could not find 'repo-intelligence-cli'` error. Run `rustup update stable` first.
- **git 2.32 or newer** on `PATH`, for git tag detection. Without git, the report
  still works and says that tags were not collected.

```sh
cargo install --locked --git https://github.com/mourtisma/repo-intelligence \
  --tag v0.1.0 repo-intelligence-cli
```

Replace the tag with the latest release tag. This installs the `repo-intelligence` binary.

## Usage

```sh
repo-intelligence analyze <PATH> [--format json]
```

`<PATH>` is the repository root. stdout carries **only** the JSON report, so it
can be piped straight into a parser. Errors go to stderr.

Example, for a single-crate repository:

```json
{
  "schema_version": "0.1.0",
  "tool_version": "0.1.0-rc.1",
  "analyzed_at": "2026-11-07T17:07:03Z",
  "findings": [
    {
      "category": "ecosystem",
      "kind": "cargo-package",
      "path": "Cargo.toml",
      "confidence": "high",
      "data": { "name": "single", "workspace": null },
      "provenance": { "detector": "cargo", "sources": ["Cargo.toml"] }
    },
    {
      "category": "version-source",
      "kind": "cargo-toml",
      "path": "Cargo.toml",
      "confidence": "high",
      "data": {
        "name": "single",
        "version": "0.1.0",
        "field": "package.version",
        "inherited_from": null
      },
      "provenance": { "detector": "cargo-version", "sources": ["Cargo.toml"] }
    }
  ],
  "diagnostics": []
}
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Report written to stdout. It may contain diagnostics. |
| 1 | Usage error: bad arguments, or the path does not exist or is not a directory. |
| 2 | Fatal error: no report was produced. |

## What the report contains

- `schema_version`: the version of the JSON contract. The schema is in
  [`schema/report.schema.json`](schema/report.schema.json). Minor bumps only add things.
- `tool_version` and `analyzed_at`: the tool release and an RFC 3339 UTC timestamp.
- `findings`: facts about the repository, each with a `confidence` and a
  `provenance` naming the detector and the files it read.
- `diagnostics`: what could not be determined, such as an invalid manifest. A
  diagnostic never stops the analysis.

Finding kinds in this release:

| Kind | Category | Anchored to | `data` |
|---|---|---|---|
| `cargo-workspace` | ecosystem | root `Cargo.toml` | `members`, `member_patterns`, `exclude` |
| `cargo-package` | ecosystem | package `Cargo.toml` | `name`, `workspace` |
| `cargo-toml` | version-source | the file declaring the version | `name`, `version`, `field`, `inherited_from` |
| `git-tag` | version-source | repository (`path: null`) | `name`, `version`, `prefix`, `annotated`, `commit` |

Every place a version is declared is reported separately, including versions
inherited from `[workspace.package]`. Git tags are reported only if they look like
versions (`v1.2.3`, `1.2.3`, or a prefixed form such as `my-crate-v1.2.3`). A tag
is not assumed to be a release.

The same repository always produces the same report, apart from `analyzed_at`.

## Limitations

- Rust/Cargo only for now.
- Git tags are read only when `<PATH>` is the repository root. In a shallow
  clone, the report warns that the tag list may be incomplete.
- Workspace members added only through path dependencies are not detected. Only
  `members` and `exclude` patterns are used.
- Tested on Linux only.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-engine-deps.sh
```

Design documents are in [`docs/`](docs/): start with
[`architecture.md`](docs/architecture.md) and [`roadmap.md`](docs/roadmap.md).

## License

MIT. See [`LICENSE`](LICENSE).
