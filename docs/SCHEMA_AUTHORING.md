# Schema Authoring

Schemas are the shared contract between DAO, issuer, holder, and verifier.
For devnet, schema registration can stay open, but every public launch schema
must be canonical and visible in the manifest/indexer.

## Schema Requirements

Each schema needs:

- name
- version
- category
- exactly eight field slots for the current circuit
- field names
- field types
- allowed predicates
- canonical schema hash
- schema PDA
- credential tree address
- credential tree depth matching the batch circuit (`20` for the current
  devnet smoke path)

## Current Circuit Shape

The current batch circuit supports:

- up to four credentials in one proof
- up to four predicates
- eight numeric field slots per schema
- operators: `EQ`, `NE`, `GT`, `GTE`, `LT`, `LTE`
- compound logic: `AND` or `OR`

## Recommended Devnet Schemas

- `basic_identity_v2` (current live smoke schema; depth-20 tree)
- `basic_identity_v1`
- `vaccination_v1`
- `product_cert_v1`
- `dao_membership_v1`
- `accredited_investor_v1`

## Making Schemas More Powerful

The current circuit is numeric-first. To support richer real-world schemas:

- use enum dictionaries for strings
- use timestamp fields for time windows
- use country/region numeric codes
- use bitsets for boolean capabilities
- use hash commitments for long text or document IDs
- publish codecs next to schema metadata

Future circuit upgrades can add:

- string dictionary membership
- set membership
- date ranges with calendar-aware codecs
- selective disclosure of exact field values
- schema-specific predicate plugins

## Registration Gate

A schema is public-devnet ready only when:

- schema JSON is committed
- hash is reproducible
- schema is registered on-chain
- tree is created/bound with depth 20
- manifest includes schema metadata
- indexer serves it
- wallet preview renders field names
- verifier SDK can encode requirements against it
