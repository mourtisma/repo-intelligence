# Architecture Reconciliation

> Part 3 of a three-stage, multi-model architecture workflow:
> [generation](./architecture-generation.md) →
> [review](./architecture-review.md) → **[reconciliation]**.

## Purpose

The reconciliation stage **weighs the review's challenges against the original
design and produces the updated documents**. It is the only stage that edits
files. It decides, per challenge, whether to *accept* (change the design),
*reject* (keep the design, and record why the challenge did not win), or
*synthesize* (a third option better than both).

This is where the multi-model disagreement is resolved into a single coherent
design — not by deferring to either model, but by adjudicating.

## Model

Switch **back to the strong reasoning model** used for generation — here,
**Claude Opus 4.8 (1M context)** (`/model Opus` / the session default).

**Why back to the strong model:** adjudicating between a design and its critique
is harder than either generating or critiquing. It requires holding the whole
system in mind, judging which objections actually threaten the load-bearing
properties, and integrating accepted changes without breaking cross-document
consistency. That is the most demanding reasoning in the loop, so it gets the
most capable model.

**Why not just let the reviewer's model reconcile:** the reviewer optimized for
finding problems; it is primed to over-weight its own objections. The strong
model, which authored the coherent draft, is better positioned to tell a
genuine flaw from a plausible-but-wrong critique — and it has the original
design intent in view.

## Inputs

- The four documents (generation output).
- The critique (review output).
- `PROJECT.md`, as the tie-breaker: when the design and the critique conflict,
  the original intent decides.

## The prompt (shape)

```
Read PROJECT.md, the four docs in docs/, and the review critique.

Act as the lead architect making final decisions. You own the design.

For each point the reviewer raised:
- Decide: ACCEPT (change the design), REJECT (keep it), or SYNTHESIZE (a better
  third option).
- If you REJECT, the design already explains why — make sure that reasoning is
  explicit in the doc so the same objection isn't re-litigated later.
- If you ACCEPT or SYNTHESIZE, update the affected documents AND keep all four
  consistent (crate names, JSON contract, detector trait must still line up).

Then apply the changes directly to the files. Preserve the
decision → tradeoff → alternative format throughout. Do not write code.
```

## What makes this prompt work

- **"You own the design"** re-establishes a single decision-maker. Without it,
  the model tends to defer to the critique and rubber-stamp every objection,
  which would just replace the generator's judgment with the reviewer's.
- **The explicit ACCEPT / REJECT / SYNTHESIZE verdict** forces a decision on
  every point rather than a vague blend. It also makes the reasoning auditable:
  a reader can later see *why* a challenged decision stands.
- **"If you REJECT … make the reasoning explicit in the doc"** turns a survived
  challenge into durable documentation. The design gets *stronger* from
  rejected critiques, not just from accepted ones — the objection and its
  rebuttal are now written down, so the next reviewer (human or model) doesn't
  re-open a settled question.
- **"Keep all four consistent"** guards the property that makes the document set
  valuable: shared crate names, one JSON contract, one detector trait. A change
  to the contract in one file that isn't propagated to the others would silently
  break the design's coherence.
- **"Apply the changes directly to the files"** — unlike the first two stages,
  this stage mutates the docs. It is the write stage.

## Expected output

The four architecture documents, updated in place, still mutually consistent and
still in decision → tradeoff → alternative form — now hardened by an
independent review and an explicit adjudication of each objection.

## The loop

This is not strictly one-shot. The three stages form a cycle that can repeat:

```
   generation (strong model)
        │
        ▼
   review (different model)  ◀──────────┐
        │                               │
        ▼                               │  (optional: re-review the
   reconciliation (strong model) ───────┘   reconciled docs)
        │
        ▼
   settled design (or another round)
```

A second review pass on the reconciled documents is worthwhile when
reconciliation made large structural changes — the new design deserves the same
adversarial scrutiny the first draft got. Stop when a review round produces only
minor or rejected objections: that is the signal the design has stabilized.

## Why this three-stage, two-model process at all

- **Independence beats self-review.** One model critiquing its own work shares
  its own blind spots. A second, different model surfaces objections the author
  model rationalized away.
- **The disagreement is the product.** Where the two models agree, the decision
  is well-supported. Where they disagree, there is a real decision point worth a
  human's (or the strong model's) explicit adjudication.
- **Roles are matched to model strengths.** The strongest model frames and
  adjudicates (the hardest reasoning); a different model challenges (breadth and
  independence). Neither model gets to be both author and unchecked judge.
- **The paper trail compounds.** Because every stage records decisions,
  tradeoffs, and — after reconciliation — why surviving objections were
  rejected, the design becomes progressively harder to erode by accident and
  easier for a newcomer to understand *why* it is the way it is.
