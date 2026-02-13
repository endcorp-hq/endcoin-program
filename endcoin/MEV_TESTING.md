# MEV Regression Test Suite

This file documents the automated checks added for MEV mitigation coverage.

## What is covered

### Trident fuzzing
`/Users/andrew/Documents/projects/endcoin-program/endcoin/trident-tests/fuzz_0/test_fuzz.rs`

The fuzz harness now exercises and asserts:
- Oracle-gated minting (`DepositLiquidity` fails when oracle value is missing).
- Whitelist-gated reward claims (`UnauthorizedClaimer` is enforced).
- Canonical reserve enforcement (`InvalidPoolAccount` on spoofed reserve accounts).
- Fee update cooldown (`ParameterRecentlyUpdated` blocks same-slot swaps).
- Oracle staleness checks (`OracleFeedStale` blocks stale SST swaps).
- Price-impact cap (`PriceImpactTooHigh` blocks oversized swaps).
- Happy-path swap still succeeds after all guards.

### Surfpool/localnet integration tests
`/Users/andrew/Documents/projects/endcoin-program/endcoin/tests/localnet_mev_guards.ts`

Integration tests assert:
- Unauthorized claims are blocked by whitelist authority.
- Non-canonical reserve accounts are rejected.
- Same-transaction `update_fee -> swap` is blocked (cooldown).
- Same-transaction `pull_feed -> swap` is blocked (cooldown).
- Oversized swaps are blocked by the price-impact guard.

## Commands

From `/Users/andrew/Documents/projects/endcoin-program/endcoin`:

```bash
# Program + unit tests
anchor build
cargo test -p endcoin --lib

# Trident fuzz suite
cd trident-tests
cargo test -q
cargo run --quiet --bin fuzz_0

# Surfpool/localnet suites
cd ..
yarn localnet:smoke
yarn localnet:mev
# or both:
yarn localnet:test
```

## Notes
- `yarn localnet:*` requires a reachable local validator (`http://127.0.0.1:8899`) and a wallet keypair.
- `scripts/localnet/common.ts` now falls back to a local provider if `ANCHOR_PROVIDER_URL` is not set.
