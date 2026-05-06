# Program-ID Reconciliation Runbook

> Fix the `Anchor.toml` ↔ `deployments/devnet.json` drift *once*, so every
> downstream proof can succeed its SEC-13 `verifierAddress == ID.to_bytes()`
> check. Running `scripts/check_program_ids.py` at HEAD currently exits 1
> with the details; this doc is how to drive it back to 0.

---

## Canonical IDs (source of truth = `Anchor.toml`)

| Program            | Program ID                                       |
|--------------------|--------------------------------------------------|
| `zk_verifier`      | `DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb`   |
| `issuer_registry`  | `5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx`   |
| `schema_registry`  | `4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1`   |

These match the `declare_id!()` macros in each `programs/<name>/src/lib.rs`,
match what the circuits and the TS SDK assume, and are locked to cluster
`localnet` and `devnet` in `Anchor.toml`.

**The old devnet deployment** (`FhtE…XEgr` / `6ewr…Laeo` / `2oma…pAsH`,
deployed 2026-04-04) is stale and must be retired.

---

## Procedure (once per stale cluster)

Execute from `nix develop` so `solana --version` and `anchor --version` are
pinned correctly.

### 1. Authenticate as the current upgrade authority

```bash
solana config set --url devnet
solana config set --keypair <path to the keypair that owns the stale programs>
solana address            # should print Hz4uJrCLqs9rqMHJyvD9tqWgSNc92TjsBYANUUNxLWWv
solana balance            # ≥ 6 SOL recommended for close + redeploy
```

### 2. Close the stale programs, recovering the rent

```bash
# Close each stale program (rent goes back to the authority).
solana program close FhtEvsUwxRRT8nf3uaxK6iAefFqvqCYimc9vqnH5XEgr \
  --lamports-recipient Hz4uJrCLqs9rqMHJyvD9tqWgSNc92TjsBYANUUNxLWWv
solana program close 6ewriDVJLTaeBG7AsjuobAfyhDWfCz681RDWMFq5Laeo \
  --lamports-recipient Hz4uJrCLqs9rqMHJyvD9tqWgSNc92TjsBYANUUNxLWWv
solana program close 2oma2yU2vfSYNR8sPDq5JvRbrtetyk5wuuGzwGYupAsH \
  --lamports-recipient Hz4uJrCLqs9rqMHJyvD9tqWgSNc92TjsBYANUUNxLWWv
```

*Safety:* `solana program close` is **irreversible** and bricks the on-chain
account. Only run it when you're sure no live client depends on those IDs.
For SolID today the only live state is the devnet programs themselves plus
their PDAs — those are superseded by the redeploy below.

### 3. Verify the keypairs under `target/deploy/` match the canonical IDs

Anchor derives each program ID from `target/deploy/<name>-keypair.json`.
These files are *not* checked in (they're authority secrets), so they have
to be reconstructed locally from the keys `Anchor.toml` declares.

```bash
# Abort if the keypairs on disk don't match Anchor.toml.
anchor keys list
# Expected lines:
#   zk_verifier:     DcyezhHYGwFTZCeb3BMJbQHFh7EyQMx8WCrKDNLbarb
#   issuer_registry: 5fxhJ1uKBtsVGq17xuVDapcTALZprNVU8Ar9mFHVijMx
#   schema_registry: 4ZCrxVBKpko7xUSrLq7zZzd87xGEKFSxFm3JG6j3CmF1
```

If any ID differs, the keypair on disk is wrong. Restore the canonical
keypairs from your secrets store — **do not** run `anchor keys sync`, that
would overwrite `Anchor.toml` and silently change the canonical ID.

### 4. Redeploy

```bash
export SOLID_KEYPAIR_PATH="$HOME/.config/solana/solid-devnet-admin.json"
NO_DNA=1 anchor build --no-idl
npm run build:idl

solana program deploy target/deploy/schema_registry.so \
  --program-id target/deploy/schema_registry-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 10
solana program deploy target/deploy/zk_verifier.so \
  --program-id target/deploy/zk_verifier-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 10
solana program deploy target/deploy/issuer_registry.so \
  --program-id target/deploy/issuer_registry-keypair.json \
  --upgrade-authority "$SOLID_KEYPAIR_PATH" \
  --keypair "$SOLID_KEYPAIR_PATH" \
  --fee-payer "$SOLID_KEYPAIR_PATH" \
  --url devnet --use-rpc --max-sign-attempts 20
```

### 5. Regenerate the manifest

```bash
python3 scripts/regen_devnet_manifest.py > deployments/devnet.json
```

(That script reads program IDs from `Anchor.toml`, fetches upgrade authority
and IDL metadata via `solana program show`, and writes the JSON in the same
shape as today's manifest. See `scripts/regen_devnet_manifest.py`.)

### 6. Re-upload the verification key

The VK-storage PDA is seeded with `(b"vk-storage", verifier_config_pubkey)`,
and the verifier_config PDA is seeded inside the `zk_verifier` program —
i.e., it depends on the program ID. Because the program ID changed, the
existing VK PDA is stranded. Re-upload:

```bash
pnpm --filter scripts ts-node scripts/store_vk.ts \
  --cluster devnet \
  --vk-json circuits/build/compound_query_vk.json
```

### 7. Re-run the consistency gate

```bash
python3 scripts/check_program_ids.py
# → "Program IDs consistent across Anchor.toml, declare_id!, and deployments/."
```

If this prints anything else, **do not** proceed to E2E — the circuit's
`verifierAddress` input will not agree with the on-chain verifier and every
proof will fail `InvalidVerifierAddress`.

---

## Why the script is the hard gate

`scripts/check_program_ids.py` is wired into CI (see `.github/workflows/ci.yml`,
job `program_id_consistency`). A PR that updates `Anchor.toml` without updating
`deployments/<cluster>.json` (or vice-versa) will fail CI. That failure is the
enforcement mechanism: documentation rots, but a CI gate does not.
