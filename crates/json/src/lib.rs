//! JSON contract crate: defines the stable, versioned report schema shared
//! between the analysis engine and its consumers.
//!
//! # Versioning
//!
//! This crate uses semantic versioning (major.minor.patch). The versioning rule:
//! - **Additive changes bump minor:** new optional fields, new `DetectorCategory`
//!   variants, new finding kinds, new diagnostic severities. Consumers reading
//!   schema version v0.N must tolerate future v0.M where M >= N.
//! - **Breaking changes bump major:** renaming/removing fields, changing field
//!   meaning or type, making optional fields required, removing enum variants.
//! - **Tool version is separate:** `tool_version` in reports is the CLI/engine
//!   release, injected at runtime. Many tool versions may share one schema version.
//!
//! During `0.x` (pre-1.0), we adopt the discipline above rather than relying on
//! semver's technical permission for breaking changes in minors. This ensures
//! external consumers (Skills, third parties) can safely pin to schema v0.x.

pub const SCHEMA_VERSION: &str = "0.1.0";

mod confidence;
pub use confidence::Confidence;
