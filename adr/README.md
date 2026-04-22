# adr/

Architecture Decision Records. One file per consequential design
decision. Tracks why the protocol looks the way it does, so future
work can evaluate changes against the original intent instead of
reverse-engineering it from code.

## Why this folder exists

Design rationale was previously scattered across `CLAUDE.md`, headers
in `docs/architecture.md`, audit docs, and implementation comments.
A reader had to triangulate to answer "why did we pick SPL Account
Compression over Light Protocol?" or "why is the nullifier
five-element, not three?". Without an authoritative log, every future
contributor re-litigates decisions already settled.

The ADR pattern (Michael Nygard, 2011) is the industry-standard answer:
one file per decision, immutable once accepted, superseded rather than
edited. `adr/` is the protocol's decision log.

## Layout

```
adr/
  README.md               -- this file
  INDEX.md                -- LIVING index of every ADR
  0001-use-groth16-over-alt-bn128-for-on-chain-verification.md
  0002-use-babyjubjub-and-poseidon-for-issuance-signing.md
  ...
```

## Conventions

- Filename: `NNNN-kebab-case-title.md` with a zero-padded 4-digit ID.
- IDs are monotonically allocated. Never reused, even if the ADR is
  rejected.
- Plain ASCII. No emoji, no unicode box-drawing.
- Each ADR uses the same template (below).

## Template

```markdown
# ADR NNNN: <title>

- **Status:** Proposed | Accepted | Deprecated | Superseded by ADR-XXXX
- **Date:** YYYY-MM-DD
- **Deciders:** <who signed off>
- **Affects:** <components>

## Context

What problem, what constraints, what forces are in play.

## Decision

The chosen approach. Written in present tense, imperative voice.

## Consequences

- Positive: what we get.
- Negative: what we give up, what is now harder.
- Neutral: follow-on work this creates or forecloses.

## Alternatives considered

Each with one line of why-not.

## References

- file:line citations
- related SOLID-SEC-NNN
- related CLAUDE.md invariants
```

## Workflow

### Adding a new ADR

1. Allocate the next ID by inspecting `adr/INDEX.md` and incrementing.
2. Write the file using the template above.
3. Append a one-line pointer to `adr/INDEX.md` under the right phase.
4. Status starts at `Proposed`. Flip to `Accepted` when the decision
   lands in code.

### Superseding an ADR

1. Do not edit the old ADR except to change its `Status` to
   `Superseded by ADR-XXXX`.
2. Write a new ADR explaining the new decision; reference the old one
   in its `## References`.
3. Update `adr/INDEX.md` to reflect the new status.

### Reviewing a change

If a proposed code change contradicts an `Accepted` ADR, the change
must either (a) be rejected, or (b) include a new ADR that supersedes
the old one. No change quietly violates an accepted decision.

## Hard rule

If a design decision is load-bearing for security, scalability, or
compatibility, it gets an ADR before it gets code. "We have always
done it this way" is not an argument; the ADR is.
