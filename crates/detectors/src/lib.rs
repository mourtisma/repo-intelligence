//! Detector implementations: concrete checks that inspect a repository through
//! `repo-intelligence-core` and report findings in the JSON contract.
//!
//! Only `default_registry()` is public. Detector types stay private, so adding a
//! detector means adding a module and one registration line.
//!
//! # Kind catalogue (part of the contract; pinned by golden files)
//!
//! | kind | category | detector | path | `data` keys |
//! |---|---|---|---|---|
//! | `cargo-workspace` | ecosystem | `cargo` | root manifest | `members`, `member_patterns`, `exclude` |
//! | `cargo-package` | ecosystem | `cargo` | manifest | `name`, `workspace` |
//! | `cargo-toml` | version-source | `cargo-version` | manifest | `name`, `version`, `field`, `inherited_from` |
//! | `git-tag` | version-source | `git-tags` | `null` | `name`, `version`, `prefix`, `annotated`, `commit` |

mod cargo;
mod git_tags;

use repo_intelligence_core::DetectorRegistry;

/// The Phase 3 detectors, in registration order.
pub fn default_registry() -> DetectorRegistry {
    let mut registry = DetectorRegistry::new();
    registry.register(cargo::CargoPackages);
    registry.register(cargo::CargoVersions);
    registry.register(git_tags::GitTags);
    registry
}
