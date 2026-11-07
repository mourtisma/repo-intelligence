# Architecture Generation

> Part 1 of a three-stage, multi-model architecture workflow:
> **[generation]** → [review](./architecture-review.md) →
> [reconciliation](./architecture-reconciliation.md).

## Purpose

The generation stage produces the *first coherent draft* of the architecture
documents from the project vision alone. It is where the load-bearing decisions
are made and, crucially, where each decision is written down **with its tradeoff
and the alternative it beat**. The output is deliberately opinionated — a strong
draft is easier to challenge than a hedged one.

## Model

Use the **strongest available reasoning model** for this stage. In this project
that was **Claude Opus 4.8 (1M context)**.

**Why the strongest model here:** generation sets the frame every later stage
works within. The crate boundaries, the JSON contract shape, and the
"Rust-analyzes / Claude-reasons / JSON-between-them" spine are expensive to
change once written; spending the most capable model on the stage that fixes
them is the cheapest place to buy quality.

## Inputs

- `PROJECT.md` — the vision, goals, non-goals, suggested crates, design
  principles. This is the sole source of intent.
- The existing repository state (to ground the docs in reality rather than
  inventing structure that conflicts with what exists).

## The prompt (verbatim shape used)

```
Read PROJECT.md.

Act as the lead software architect.

Do not write any code.

Produce the following documents:
1. docs/architecture.md
2. docs/workspace-layout.md
3. docs/detector-architecture.md
4. docs/roadmap.md

Focus on:
- long-term maintainability
- Rust workspace design
- detector extensibility
- CLI/Skill interaction
- JSON contracts

For every major decision, explain tradeoffs and alternatives.

Do not implement anything yet.
```

## What makes this prompt work

- **"Act as the lead software architect"** sets the altitude: system-level
  decisions and their rationale, not code.
- **"Do not write any code" / "Do not implement anything yet"** keeps the output
  as reviewable prose. Code at this stage would anchor decisions before they are
  challenged.
- **An explicit focus list** steers the model toward the axes that actually
  matter for this project (the JSON boundary, extensibility) rather than a
  generic architecture essay.
- **"For every major decision, explain tradeoffs and alternatives"** is the most
  important instruction. It forces the draft to *expose its own decision points*,
  which is precisely what the review stage attacks. A draft that hides its
  reasoning cannot be effectively challenged.
- **Naming the four output files** produces a set of interlocking documents
  (shared crate names, one JSON envelope, one detector trait) rather than four
  disconnected essays.

## Expected output

Four mutually-consistent documents where every significant choice is stated as
*decision → tradeoff → rejected alternative*. The consistency across files
(same crate names, same contract, same trait) is intentional and is what lets
the review stage reason about the system as a whole.

## Handoff to the review stage

The generation output is treated as a **strong proposal, not a settled truth**.
It is handed, unmodified, to a *different* model for adversarial review. The
tradeoff/alternative annotations become the review stage's attack surface: the
reviewer can ask "was the rejected alternative actually worse?" for every
decision, because every decision named its alternative.

See [architecture-review.md](./architecture-review.md).
