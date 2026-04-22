# docs/archive/ -- HISTORICAL. DO NOT USE.

Every file in this directory predates the 2026-04-22 consolidation and
is kept only for provenance. None of it is current.

Canonical sources as of 2026-04-23:

| If you want to know...                       | Read this                                                |
|----------------------------------------------|----------------------------------------------------------|
| ...the current security findings             | `sec/SECURITY_REGISTRY.md`                                |
| ...the canonical dated audit snapshots       | `sec/audits/`                                             |
| ...why the protocol is designed this way     | `adr/INDEX.md` + individual `adr/NNNN-*.md`               |
| ...what is done, left, how we will ship it   | `plan/IMPLEMENTATION_PLAN.md`                             |
| ...the current post-remediation audit        | `docs/POST_REMEDIATION_AUDIT.md`                          |
| ...the canonical component contracts         | `docs/MODULE_CONTRACTS.md`                                |
| ...the hard invariants                       | `CLAUDE.md`                                               |

## Why these files are archived

- `SOLID_ARCHITECTURAL_DECISIONS.md`, `SOLID_COMPREHENSIVE_REVIEW.md`,
  `SOLID_DEEP_AUDIT_2026_04.md`, `SOLID_FINAL_AUDIT_2026_04.md`,
  `SOLID_INFRA_IMPROVEMENTS_STATUS.md`, `SOLID_INFRA_MANIFESTO.md`,
  `SOLID_POST_REMEDIATION_AUDIT_2026_04.md`, `SOLID_SECURITY_AUDIT.md`:
  earlier audit and review artifacts, superseded by the April 2026
  consolidated work under `sec/audits/` and `docs/POST_REMEDIATION_AUDIT.md`.
- `infra_roadmap.md`: claims Light Protocol CPI and Bloom filter items
  as shipped; both were removed in v0.2 per ADR-0003 / ADR-0007.
- `e2e_run_guide.md`: publishes stale devnet program IDs retired by
  `docs/PROGRAM_ID_RECONCILIATION.md`; prescribes wrong toolchain
  versions; references `initNullifierBloom()` which no longer exists.
- `e2e_test_results.md`: snapshot of a test run that no longer
  corresponds to the current harness.
- `solid_protocol_terminal_manifesto.md`: marketing copy; not engineering.

## Rule

Do not cite anything in this directory in a PR, an audit, or an ADR.
If you need the historical context, cite it with the `docs/archive/`
prefix explicitly and the phrase "historical".
