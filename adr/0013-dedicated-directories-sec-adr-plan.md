# ADR 0013: Dedicated `sec/`, `adr/`, and `plan/` directories

- **Status:** Accepted
- **Date:** 2026-04-22
- **Deciders:** developer_other@eterna.dev (protocol owner)
- **Affects:** repository organization; all future audit, decision,
  and planning artifacts

## Context

As of 2026-04-22, security and planning content was fragmented:

- Three audit artifacts at the repo root
  (`security_audit.md`, `solid_protocol_system_audit_2026_04_21.md`,
  and the earlier `solid_system_audit_2026_04_22.md`).
- Seven more under `docs/` (`docs/SOLID_*.md`,
  `docs/POST_REMEDIATION_AUDIT.md`, `docs/IMPROVEMENTS_ROADMAP.md`).
- Design rationale scattered across `CLAUDE.md`, doc headers, and
  audit prose.
- No consolidated forward plan; `docs/IMPROVEMENTS_ROADMAP.md` was
  stale and contradicted `docs/POST_REMEDIATION_AUDIT.md`.

The user asked for three things: (1) one dedicated place for
security so findings cannot be lost between cycles, (2) one
dedicated place for design decisions so rationale does not
re-litigate itself, (3) one dedicated place for a forward plan that
ties findings to phased execution.

## Decision

Three top-level directories:

- **`sec/`** -- security tracker + audit snapshots.
  - `sec/SECURITY_REGISTRY.md`: living canonical tracker. Every
    finding has a stable `SOLID-SEC-NNN` ID. Never deleted.
  - `sec/audits/`: immutable dated snapshots of each audit pass.
  - `sec/README.md`: workflow, severity, status lifecycle.
- **`adr/`** -- architecture decision records.
  - `adr/INDEX.md`: living index.
  - `adr/NNNN-*.md`: one file per decision, immutable once
    `Accepted`.
  - `adr/README.md`: template, workflow, hard rule.
- **`plan/`** -- implementation plan.
  - `plan/IMPLEMENTATION_PLAN.md`: living phased plan.
  - `plan/README.md`: purpose and cadence.

Hard rule for each directory:

- No security-material note in `docs/` without a `SOLID-SEC-NNN`
  entry in `sec/SECURITY_REGISTRY.md`.
- No code change that contradicts an `Accepted` ADR without a
  superseding ADR.
- No claim of "done" outside `plan/IMPLEMENTATION_PLAN.md`'s
  status table.

Historical audit artifacts in `docs/SOLID_*.md`,
`docs/POST_REMEDIATION_AUDIT.md`, and the deprecated roadmap are
tracked for archival relocation under `docs/archive/` in Phase 1
(SOLID-SEC-027 scope).

## Consequences

- **Positive.** Single source of truth for each dimension. Audit
  cycles append rather than overwrite. Future contributors know
  exactly where to look and exactly where to add.
- **Negative.** Three more top-level directories to maintain.
  Discipline required: any new audit MUST append to the registry,
  not fragment a new location.
- **Neutral.** The existing `docs/` becomes user-facing documentation
  (architecture overview, how-to guides, key management). Internal
  engineering + security + planning lives under `sec/`, `adr/`,
  `plan/`.

## Alternatives considered

- **Keep everything in `docs/`.** Rejected: was already producing
  fragmentation and duplicate canonical claims.
- **Single `meta/` directory with subfolders.** Rejected: adds a
  level of nesting for no benefit; three top-level dirs are easier
  to discover.
- **External wiki (GitHub Pages / Notion).** Rejected: audit content
  must live with the code for provenance and git-history auditing.

## References

- `sec/README.md`
- `adr/README.md`
- `plan/README.md`
- `sec/SECURITY_REGISTRY.md` (SOLID-SEC-027 for doc-drift archival)
