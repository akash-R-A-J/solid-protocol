# keys/localnet/

Tracked program keypairs for localnet (and the canonical devnet IDs they
also serve). Anchor reads these to bake the program ID into the BPF
binary at `anchor build` time and to authorise the deploy CPI at
`anchor deploy` time.

## Files

- `zk_verifier-keypair.json`     -> `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`
- `issuer_registry-keypair.json` -> `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`
- `schema_registry-keypair.json` -> `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`

These three pubkeys are the canonical program IDs declared in
`Anchor.toml`, `declare_id!()` in each program crate, and
`deployments/devnet.json`. CI's `scripts/check_program_ids.py`
enforces the invariant.

## Why the keypairs are tracked

`anchor deploy` regenerates `target/deploy/*-keypair.json` from
scratch on every `anchor build` if the file is missing, which
silently rotates the on-chain program ID and breaks every
hard-coded canonical reference (owner-checks, ADR-0014 issuer-tree
binding, the verifier's two-layer guard, etc.). Committing the
keypairs at this tracked path is the only way to make the canonical
set durable across machines and clean checkouts.

These are localnet/devnet test keys. They are NOT mainnet upgrade
authorities; rotating them is harmless. Mainnet program IDs and
upgrade authorities are managed separately and never live here.

## Devnet rollout (TBD — do not skip this when promoting)

Today the same three keypairs serve localnet AND the IDs recorded in
`deployments/devnet.json`. That is fine while devnet is operator-only
test infrastructure, but it is NOT the long-term posture: the moment
external integrators rely on the devnet program IDs (or the deployed
artefacts hold non-trivial state), the upgrade authority for those
programs must move out of a tracked git path and into a separate,
access-controlled key custody (Squads multisig / hardware wallet / DAO
threshold PDA — same posture mainnet will eventually use).

When that day comes, the migration shape is:
1. Rotate the devnet IDs (Addendum procedure in
   `adr/0004-three-program-split.md`) so the tracked `keys/localnet/*`
   set no longer matches anything live on devnet.
2. Move the devnet upgrade-authority keypair out of git into the
   chosen custody scheme.
3. Update `deployments/devnet.json` to point at the new IDs and
   document the upgrade-authority address (not the keypair) there.
4. CI's `scripts/check_program_ids.py` keeps enforcing the
   `Anchor.toml` / `declare_id!` / `deployments/<cluster>.json`
   invariant per cluster.

Until that migration ships, treat any state on devnet as disposable.

## Usage

`anchor deploy --provider.cluster localnet` and the helper scripts
under `scripts/` resolve the keypairs by walking these locations in
order:

1. `keys/localnet/<program>-keypair.json` (this directory).
2. `target/deploy/<program>-keypair.json` (Anchor's default; only used
   if step 1 is absent).

If you ever need to cut a fresh ID set (e.g. for a parallel test
cluster), run `solana-keygen new -o keys/localnet/<program>-keypair.json`,
then re-run the rotation procedure documented in
`adr/0004-three-program-split.md` "Addendum: 2026-04-25 ID rotation".
