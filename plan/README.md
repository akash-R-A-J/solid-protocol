# plan/

Implementation planning. Where the forward-looking phased execution
plan lives, so "what's done, what's left, how we ship the rest"
has one answer in one place.

## Why this folder exists

- `sec/` tracks findings (what is wrong, or might be).
- `adr/` tracks decisions (why the system looks the way it does).
- `plan/` tracks execution (how we get from the current state to a
  production system).

Without a consolidated plan, execution drifts: findings sit in the
registry without an owner or a phase, decisions in `adr/` sit
disconnected from delivery milestones, and the team re-answers
"what are we doing next?" at every standup.

## Layout

```
plan/
  README.md                 -- this file
  IMPLEMENTATION_PLAN.md    -- LIVING plan. Single source of truth
                               for scope, phasing, and acceptance.
```

## Workflow

### Starting a phase

1. Pre-flight: every phase-1 item references a `SOLID-SEC-NNN` or
   `ADR-NNNN`. If a scope item has no reference, add one first.
2. Entry criteria in the plan must be green before work starts.

### Closing a phase

1. Exit criteria must all be green.
2. A dated audit snapshot goes to `sec/audits/` confirming the
   registry states for every in-scope `SOLID-SEC-NNN` have flipped
   to `Fixed` (or `Won't Fix` with justification).
3. The plan is updated to reflect the next phase's entry criteria.

### Living updates

- Status tables in `IMPLEMENTATION_PLAN.md` are updated on every
  close-out. Nothing else in the plan changes silently.
- Scope additions go through the same rigor: if the addition is a
  new finding, it appears in `sec/SECURITY_REGISTRY.md` first.

## Cadence

- End of each phase: a dated audit snapshot in `sec/audits/`.
- Mid-phase: weekly updates to the `Status` column in the plan's
  scope tables. No prose changes mid-phase without a superseding
  ADR or a new audit snapshot.

## Hard rule

There is exactly one implementation plan. If a conflicting
"roadmap" or "milestone doc" shows up anywhere else, that doc must
either (a) be archived, or (b) be a downstream derivation of this
plan clearly labelled as such. `docs/IMPROVEMENTS_ROADMAP.md` is
being migrated into the registry + this plan during Phase 1
(SOLID-SEC-027).
