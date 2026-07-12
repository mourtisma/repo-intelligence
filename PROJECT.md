# Repo Intelligence

## Vision

Repo Intelligence is a repository analysis platform designed for Claude Code.

Its purpose is to help developers understand, maintain, and automate work across software repositories regardless of language, framework, or release tooling.

The platform consists of:

1. A Rust-powered repository intelligence engine.
2. A CLI exposing repository analysis capabilities.
3. One or more Claude Code Skills that consume the CLI output.

The first Skill is Universal Release Manager.

---

## Goals

* Detect repository structure automatically.
* Detect languages and ecosystems.
* Detect version sources.
* Detect CI/CD systems.
* Detect release tooling.
* Produce structured JSON reports.
* Enable Claude Code Skills to reason over repository metadata.
* Support multiple ecosystems through a plugin-style detector architecture.

---

## Non-Goals

* Replace existing release tools.
* Replace package managers.
* Replace CI/CD systems.

Repo Intelligence should orchestrate and augment existing tooling rather than reimplementing it.

---

## Initial Feature: Universal Release Manager

The first Claude Code Skill should:

* Inspect repositories.
* Discover version sources.
* Detect release workflows.
* Suggest semantic version bumps.
* Generate release plans.
* Generate changelog drafts.
* Support multiple ecosystems.

Examples include:

* Rust (Cargo.toml)
* Python (pyproject.toml)
* Node.js (package.json)
* Java (pom.xml)
* Go (go.mod and tags)

---

## Architecture

The project should be organized as a Rust workspace.

Suggested crates:

* repo-intelligence-core
* repo-intelligence-detectors
* repo-intelligence-json
* repo-intelligence-cli

The Claude Code Skill should remain thin and consume structured CLI output.

Business logic should live in Rust.

---

## Design Principles

* Claude performs reasoning.
* Rust performs deterministic analysis.
* JSON is the interface between both.
* Skills should be installable independently.
* The system should be extensible for future Skills.

---

## Future Skills

Potential future Skills:

* Architecture Reviewer
* Dependency Modernizer
* CI Failure Analyzer
* Project Onboarding Assistant

All future Skills should build upon the same repository intelligence engine.
