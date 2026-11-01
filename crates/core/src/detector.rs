//! The detector extension contract: the `Detector` trait, its output type, and the
//! ordered `DetectorRegistry`. `core` never names a concrete detector.

use repo_intelligence_json::{Diagnostic, Finding};

use crate::context::RepoContext;

pub use repo_intelligence_json::DetectorCategory;

/// What a detector returns from `detect()`: findings plus diagnostics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectionOutput {
    pub findings: Vec<Finding>,
    pub diagnostics: Vec<Diagnostic>,
}

/// A detector: a pure function of an immutable `RepoContext`.
///
/// Detectors must not walk the filesystem (use `ctx.paths()` / `ctx.read()`),
/// must not call other detectors, and must not depend on call order.
pub trait Detector: Send + Sync {
    /// Stable identifier, recorded in provenance and diagnostics.
    fn id(&self) -> &'static str;
    /// The category of question this detector answers.
    fn category(&self) -> DetectorCategory;
    /// Cheap gate: when false, `detect()` is not called.
    fn applies(&self, ctx: &RepoContext) -> bool;
    /// Produces findings and diagnostics for `ctx`.
    fn detect(&self, ctx: &RepoContext) -> DetectionOutput;
}

/// Ordered collection of detectors. Iteration yields registration order.
#[derive(Default)]
pub struct DetectorRegistry {
    detectors: Vec<Box<dyn Detector>>,
}

impl DetectorRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `detector` after all previously registered detectors.
    pub fn register<D: Detector + 'static>(&mut self, detector: D) {
        self.detectors.push(Box::new(detector));
    }

    /// Detectors in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Detector> {
        self.detectors.iter().map(|d| d.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Named(&'static str, bool);

    impl Detector for Named {
        fn id(&self) -> &'static str {
            self.0
        }
        fn category(&self) -> DetectorCategory {
            DetectorCategory::Structure
        }
        fn applies(&self, _ctx: &RepoContext) -> bool {
            self.1
        }
        fn detect(&self, _ctx: &RepoContext) -> DetectionOutput {
            DetectionOutput::default()
        }
    }

    #[test]
    fn registry_preserves_registration_order() {
        let mut reg = DetectorRegistry::new();
        reg.register(Named("b", true));
        reg.register(Named("a", true));
        let ids: Vec<&str> = reg.iter().map(|d| d.id()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn empty_registry_iterates_to_nothing() {
        assert_eq!(DetectorRegistry::new().iter().count(), 0);
    }

    #[test]
    fn detection_output_is_stable_on_repeated_calls() {
        let ctx = RepoContext::from_paths("/r", ["a.txt"]);
        let d = Named("fake", true);
        assert_eq!(d.detect(&ctx), d.detect(&ctx));
    }
}
