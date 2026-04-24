# ADR 0008: Chunked VK upload with sequential `next_vk_chunk` cursor

- **Status:** Accepted (to be revised in Phase 2 per SOLID-SEC-006)
- **Date:** v0.3 remediation (formalized 2026-04-22)
- **Deciders:** founding team
- **Affects:** `programs/zk-verifier/src/lib.rs` (`store_verification_key`,
  `VerifierConfig`, `VkStorage`)

## Context

A Groth16 verifying key for the batch circuit (31 public inputs in
v0.5, 32 after ADR-0014 in v0.6; VK size grew by one G1 entry to
roughly 2.55KB) exceeds Solana's per-transaction data limit of 1232
bytes after the header, so the VK cannot be uploaded atomically.

The chunk upload scheme must (a) allow uploading chunks in multiple
transactions, (b) block out-of-order chunk writes, (c) prevent a
caller from partially overwriting an already-initialized VK, and
(d) support future rotation without re-deploying the program.

## Decision

`VerifierConfig` tracks the next chunk index the authority is
allowed to write:

```rust
pub struct VerifierConfig {
    pub authority: Pubkey,
    pub proof_count: u64,
    pub vk_initialized: bool,
    pub paused: bool,
    pub bump: u8,
    pub next_vk_chunk: u16,
}
```

`store_verification_key(chunk_index, chunk_data, is_final_chunk)`:

- Requires `chunk_index == config.next_vk_chunk`.
- Chunk 0 overwrites `vk_storage.data`; later chunks append.
- Increments `next_vk_chunk` monotonically.
- Flips `vk_initialized = true` when `is_final_chunk`.

`VerifierConfig::SPACE = 49` pins the layout (the doc-drift gap
that had this as 45 was closed in SEC-042; `next_vk_chunk` is a
u16 and `timestamp_skew_seconds` is a u32 from SEC-005); any change
must bump the constant. The constraint is called out in CLAUDE.md
under "Hard invariants". SEC-006 is expected to grow the struct
further (`vk_finalized` + `vk_generation`); the `SPACE` constant
moves with it.

## Consequences

- **Positive.** Write-once-per-generation by construction (chunk
  re-write impossible after `next_vk_chunk > 0`). Order-enforcement
  prevents partial-state races.
- **Negative.** No freeze flag. Authority can finalize a truncated
  VK with small `nr_ic` -- see SOLID-SEC-006, which Phase 2 closes
  by adding `vk_frozen` + `vk_generation` and requiring the program
  be paused for any post-gen-0 write.
- **Negative.** No VK versioning. A new VK upload immediately
  invalidates every in-flight proof. Phase 2's VK generation counter
  introduces a grace window.
- **Neutral.** Storage PDA is `vk-storage` seeded under
  `verifier_config`. Size is capped at `VK_MAX_BYTES` at compile
  time; the parser enforces `nr_ic <= 32` (`VkBuf::parse`).

## Alternatives considered

- **Compressed on-chain via SPL AC.** Rejected: the VK is accessed
  on every verify and must live in a direct-readable account.
- **Upload via program loader buffer.** Rejected: couples VK
  lifecycle to the program upgrade path, preventing routine rotation.

## References

- `programs/zk-verifier/src/lib.rs:82-130` (handler)
- `programs/zk-verifier/src/lib.rs:530-565` (VerifierConfig, VkStorage)
- `programs/zk-verifier/src/lib.rs:338-383` (VkBuf::parse)
- `CLAUDE.md:26-29`
- SOLID-SEC-006
