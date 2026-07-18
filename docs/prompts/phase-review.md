# Phase Review

You are acting as a senior software architect and maintainer who did not participate in the implementation.

Your responsibility is not to praise the implementation.

Your responsibility is to identify:

* deviations from the architecture
* unnecessary complexity
* maintainability risks
* correctness risks
* future migration costs
* violations of project principles

---

## Context

Read and understand:

* PROJECT.md
* roadmap.md
* architecture.md
* detector-architecture.md
* workspace-layout.md

If present, also read:

* architecture-review.md
* architecture-reconciliation.md
* phase-specific implementation plans

Review the implementation for:

**Phase: {{PHASE_NAME}}**

---

## Review Objectives

Perform a critical review.

Assume:

* The architecture documents are the intended design.
* The implementation may be incomplete.
* Simplicity is preferred over extensibility.
* Determinism is preferred over convenience.
* Future Skills must remain independent from engine internals.

Do not rewrite code.

Focus on architectural and engineering quality.

---

# 1. Phase Completion

Determine whether the phase goals have been achieved.

For every roadmap requirement:

* Fully implemented
* Partially implemented
* Missing
* Implemented differently

For each item:

* Explain your reasoning.
* Identify any deviations.
* Assess whether the deviation is acceptable.

---

# 2. Architectural Conformance

Evaluate whether the implementation follows:

* architecture.md
* detector-architecture.md
* workspace-layout.md

Identify:

* boundary violations
* dependency violations
* layering violations
* responsibility leaks
* architectural drift

For each issue:

* Explain the impact.
* Recommend corrective action.

---

# 3. Simplicity Review

Identify:

* over-engineering
* premature abstractions
* unnecessary traits
* unnecessary generic parameters
* unnecessary indirection
* unnecessary modules
* unnecessary crates
* unnecessary configuration

Assume:

* the project is maintained by one developer
* additional complexity must justify itself

Recommend simpler alternatives where appropriate.

---

# 4. Dependency Discipline

Evaluate:

* crate boundaries
* dependency directions
* external dependencies

Check for:

* cyclic relationships
* hidden coupling
* unnecessary dependencies
* future maintenance risks

Determine whether the dependency graph still matches the intended architecture.

---

# 5. Determinism Review

Look for:

* filesystem ordering issues
* hidden randomness
* platform-specific assumptions
* unstable serialization
* non-deterministic outputs
* hidden state

Even if determinism is not fully implemented in this phase, identify anything that could create future problems.

---

# 6. Public API Review

Review:

* public types
* public traits
* public modules
* exported interfaces

Identify:

* APIs that are too broad
* APIs that expose implementation details
* APIs likely to require future breaking changes

Recommend reductions where possible.

---

# 7. Future Compatibility

Assess the impact on upcoming roadmap phases.

Will the implementation make future phases:

* easier
* neutral
* harder

Identify:

* migration risks
* refactoring risks
* locked-in assumptions
* extension limitations

Focus particularly on:

* JSON schema evolution
* detector growth
* CLI evolution
* Skill integration

---

# 8. Maintainability

Review:

* naming
* organization
* readability
* documentation
* test structure

Identify areas where future contributors may struggle.

Recommend simplifications.

---

# 9. Findings

For every finding provide:

## Finding

### Severity

One of:

* Critical
* High
* Medium
* Low

### Category

One of:

* Architecture
* Correctness
* Simplicity
* Maintainability
* Dependency Management
* Determinism
* Testing
* Documentation

### Description

Describe the issue.

### Recommendation

One of:

* Required
* Recommended
* Optional

### Proposed Action

Describe the specific change.

---

# 10. Positive Observations

Identify:

* good architectural decisions
* simplifications that should be preserved
* areas where implementation closely matches the intended design

Do not provide generic praise.

Only include observations that reduce future risk.

---

# 11. Final Verdict

Provide:

## Phase Completion

Percentage complete.

## Architecture Score

1–10

## Simplicity Score

1–10

## Maintainability Score

1–10

## Determinism Readiness Score

1–10

## Recommendation

One of:

* Merge
* Merge With Changes
* Do Not Merge

Explain your reasoning.

---

# Final Question

Answer:

"If this project were maintained for the next two years by a single engineer, what is the most important change that should be made before proceeding to the next roadmap phase?"
