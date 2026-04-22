# sec/

Dedicated security directory. Everything security-relevant lives here so nothing
gets lost between audit cycles or scattered across `docs/`.

## Why this folder exists

- Security findings were previously split between `security_audit.md` (root),
  `solid_protocol_system_audit_2026_04_21.md` (root), `docs/SOLID_SECURITY_AUDIT.md`,
  `docs/SOLID_DEEP_AUDIT_2026_04.md`, `docs/SOLID_FINAL_AUDIT_2026_04.md`,
  `docs/SOLID_POST_REMEDIATION_AUDIT_2026_04.md`, `docs/POST_REMEDIATION_AUDIT.md`,
  `docs/SOLID_COMPREHENSIVE_REVIEW.md`, and `docs/IMPROVEMENTS_ROADMAP.md`. That is
  nine files with overlapping scope and no single source of truth for "what is
  currently open."
- `sec/` consolidates the living tracker into exactly one file
  (`SECURITY_REGISTRY.md`) and keeps point-in-time audit snapshots under
  `sec/audits/`. Snapshots are immutable; the registry is the living index.

## Layout

```
sec/
  README.md                -- this file
  SECURITY_REGISTRY.md     -- LIVING canonical tracker. All findings. One doc.
  audits/
    2026-04-22_v0.3_comprehensive_audit.md
    <future dated snapshots>
```

## Workflow

### Running a new audit

1. Write the full report as a new file under `sec/audits/` using the naming
   convention `YYYY-MM-DD_<protocol-version>_<slug>.md`. This file is
   immutable after the audit closes -- it is the historical record.
2. For every new finding, append an entry to `SECURITY_REGISTRY.md` with a new
   stable ID `SOLID-SEC-NNN` (monotonically increasing, never reused).
3. For every finding that carries over from a prior audit, update the status
   in the registry. Do not delete.
4. Update the registry's `Last audit` field and the summary table counts.

### Fixing a finding

1. The fix PR must reference the `SOLID-SEC-NNN` ID in the title or body.
2. When the fix is merged, flip the status in `SECURITY_REGISTRY.md` from
   `Open` or `In Progress` to `Fixed`. Record the commit hash and the
   regression test that gates it.
3. On the next audit, an independent verifier flips `Fixed` to `Verified`
   (or reopens with a new note). `Verified -> Closed` after one full audit
   cycle with no regression.

### Status lifecycle

```
Open -> In Progress -> Fixed -> Verified -> Closed
                         |         |
                         v         v
                       Won't Fix (with written justification)
```

- `Open`: finding exists, no active fix.
- `In Progress`: PR in flight.
- `Fixed`: merged, regression test green.
- `Verified`: independent auditor confirmed on next cycle.
- `Closed`: one full audit cycle post-verification with no regression.
- `Won't Fix`: acceptance of risk. Must include justification and the
  compensating control, signed off by protocol authority.

## Severity definitions

- **CRITICAL**: soundness break, loss of funds, universal forgery, silent
  corruption of identity state. Blocks any further deployment.
- **HIGH**: narrow soundness gap, material privacy loss, governance takeover,
  or availability loss under adversarial conditions. Blocks external audit
  close-out.
- **MEDIUM**: hardening gap, defense-in-depth absence, operational foot-gun.
  Blocks production release.
- **LOW**: code hygiene, minor misconfiguration, style-level integer handling.
- **INFO**: informational, non-actionable, or documentation drift.

## Conventions

- Plain ASCII. No emoji, no unicode box-drawing (per `CLAUDE.md:97-98`).
- Every finding cites `file:line` or a range.
- IDs are stable. Once issued, never reused even if the finding is rejected.
- Severity at time-of-discovery is frozen in the audit snapshot. The
  registry's current severity may change only with a written note in the
  finding's history.

## Hard rule

If a new security issue surfaces -- from external audit, bug bounty, incident
response, internal review, or a user report -- it goes here first. No
security-material note may be added to any file under `docs/` until the
corresponding `SOLID-SEC-NNN` entry exists in `SECURITY_REGISTRY.md`.
