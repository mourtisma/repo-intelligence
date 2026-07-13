# Architecture Review

> Part 2 of a three-stage, multi-model architecture workflow:
> [generation](./architecture-generation.md) → **[review]** →
> [reconciliation](./architecture-reconciliation.md).

## Purpose

The review stage **adversarially challenges** the generated architecture. Its
job is not to agree, polish, or extend — it is to find the weak decisions, the
hidden assumptions, and the places where the rejected alternative was actually
the better choice. It produces a *critique*, not new documents.

## Model

Use a **different model** from the one that generated the draft. In this project
generation used Opus 4.8; review used **Claude Sonnet** (switched via
`/model Sonnet`).

**Why a different model:** a model reviewing its own output tends to rationalize
its earlier choices — it shares the same blind spots that produced them. A
different model brings independent priors and is far more likely to push back on
a decision the author model found "obvious." The disagreement between models is
the signal we are mining. Using a second model is a cheap approximation of a
second architect on the review.

The *model class matters less than the independence*. A smaller/faster model is
fine and even desirable here: the review stage is about breadth of challenge and
a fresh perspective, not about out-reasoning the generator. The final synthesis
happens back on the strong model in stage 3.

## Inputs

- The four documents from the generation stage, **unmodified**.
- `PROJECT.md`, so the reviewer checks decisions against the original intent,
  not just internal consistency.

## The prompt (shape)

```
Read PROJECT.md and the four documents in docs/.

Act as a skeptical senior architect doing a design review. Do NOT rewrite the
documents and do NOT write code.

For every major decision, challenge it:
- Is the stated tradeoff honest, or does it undersell the cost?
- Was the rejected alternative actually the better choice?
- What assumption is load-bearing but unstated?
- Where will this hurt at 10x the scale / 10x the detectors / a second Skill?

Produce a critique: a list of decisions you would push back on, each with your
reasoning and a concrete counter-proposal. Flag anything you think is fine, too,
so the next stage knows what NOT to touch.
```

## What makes this prompt work

- **"Act as a skeptical senior architect"** and **"challenge it"** invert the
  default helpful-agreement bias. The reviewer is rewarded for disagreement, not
  consensus.
- **"Do NOT rewrite the documents"** keeps the stages cleanly separated. Review
  produces *arguments*; only reconciliation edits files. Mixing them would let a
  single model both critique and "fix," collapsing the independence we are
  paying for.
- **The four framed questions** (honest tradeoff / better alternative / unstated
  assumption / behavior at scale) direct the critique at exactly the parts the
  generation prompt was told to expose. They target decisions, not prose.
- **"Flag anything you think is fine"** is important: it tells the reconciliation
  stage which decisions survived scrutiny, so they are *not* re-opened. A review
  that only lists problems leaves everything else ambiguous.
- **A concrete counter-proposal per objection** forces the critique to be
  actionable. "I don't like X" is noise; "replace the confidence enum with X
  because Y" is something reconciliation can weigh and accept or reject.

## Expected output

A structured critique: per-decision pushback with reasoning and a counter-
proposal, plus an explicit "these are fine, leave them" list. No edited
documents, no code.

## How the output is used

The critique is **advisory, not authoritative**. It is a set of arguments to be
weighed in stage 3 by the strong model, which may accept a challenge, reject it,
or find a third option. The reviewer does not get the last word — deliberately,
because a second model challenging is valuable, but letting it unilaterally
override the primary design would just swap one single-model opinion for another.

See [architecture-reconciliation.md](./architecture-reconciliation.md).
