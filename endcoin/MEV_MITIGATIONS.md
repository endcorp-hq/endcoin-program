# Endcoin MEV / Extraction Mitigations (Draft)

This file captures concrete mitigation ideas for the issues identified in the swap and minting flows. Each section lists:
- **Issue**
- **Potential mitigation**
- **Concepts to read** (with sources)

---

## 1) Pool reserve account spoofing (swap accepts arbitrary token accounts)
**Issue**
- Swap only checks that `pool_account_a/b` are owned by the pool authority and have the right mint. It does **not** prove these are the *canonical* pool reserve accounts. An attacker can pass arbitrary PDA‑owned accounts and drain the real pool by skewing reserves.

**Potential mitigation**
- Store `reserve_a` and `reserve_b` in `Pool` at creation.
- Enforce `#[account(address = pool.reserve_a)]` / `#[account(address = pool.reserve_b)]` in swap.
- Alternatively (or additionally), enforce canonical ATA addresses using Anchor’s `associated_token` constraints for `(pool_authority, mint)`.
- Keep PDA bump + seeds consistent and validated for the pool authority.

**Concepts to read**
- Anchor account constraints: `address`, `owner`, `has_one`
- Associated Token Account (ATA) derivation and usage

Sources:
- https://www.anchor-lang.com/docs/account-constraints
- https://www.anchor-lang.com/docs/tokens/basics/create-token-account

---

## 2) Permissionless minting + user‑supplied temperature
**Issue**
- `deposit_liquidity` mints to **caller‑supplied** accounts with no owner constraint, and it takes `mean_temp` as a parameter. Any user can mint to arbitrary accounts and choose a temperature to maximize issuance.

**Potential mitigation**
- Remove `mean_temp` from the instruction; read SST from a trusted on‑chain source.
- Enforce staleness/cooldown on SST updates (e.g., must be updated once per epoch, cannot be used if too new).
- Gate minting to an admin/oracle authority, or enforce a program‑controlled schedule (epoch PDA or cron‑like state).
- Restrict pool token accounts to canonical ATAs (see item #1) so minting can’t be redirected.

**Concepts to read**
- ATA constraints for deterministic reserve accounts
- Oracle / TWAP concepts for manipulation‑resistance

Sources:
- https://www.anchor-lang.com/docs/tokens/basics/create-token-account
- https://docs.uniswap.org/contracts/v2/concepts/core-concepts/oracles

---

## 3) Permissionless reward claiming (no entitlement checks)
**Issue**
- `claim_reward` has no entitlement logic; any signer can claim arbitrary amounts if the vault has balance. This enables direct vault draining and MEV copy‑cats.

**Potential mitigation**
- Implement entitlement accounting: reward index per LP share, checkpointed balances, or per‑period accrual.
- Alternatively use a Merkle distributor or a dedicated reward program that validates proofs.
- If temporary, restrict claiming to admin or a distributor authority.

**Concepts to read**
- Reward distribution patterns (index / accumulator models)
- Anchor account constraints for authorized claim flows

Sources:
- https://www.anchor-lang.com/docs/account-constraints

---

## 4) Admin‑timed SST/fee updates (extractable value)
**Issue**
- Admin can update SST or fees just before a large swap and tilt pricing or take outsized fees.

**Potential mitigation**
- Add a timelock/cooldown window for updates.
- Store `last_update_slot` and require swaps to use values older than N slots.
- Use staged updates (commit → apply after delay).
- Require multisig for admin actions.

**Concepts to read**
- MEV and transaction ordering
- Governance timelocks

Sources:
- https://writings.flashbots.net/frontrunning-mev-crisis

---

## 5) Classic sandwich risk around swaps
**Issue**
- AMM swaps are inherently sandwichable. Searchers can front‑run and back‑run large swaps to capture slippage.

**Potential mitigation**
- Encourage tight `min_output_amount` on client side.
- Add max price‑impact guards or TWAP‑based checks on the program side.
- Consider private orderflow / bundling in deployment environments (if available).

**Concepts to read**
- MEV taxonomy
- TWAP‑based oracle checks

Sources:
- https://writings.flashbots.net/frontrunning-mev-crisis
- https://docs.uniswap.org/contracts/v2/concepts/core-concepts/oracles

---

## 6) Fee rounding & micro‑trade gaming
**Issue**
- Fee calculation is `floor(amount * bps / 10_000)`. Splitting trades can reduce or eliminate fees.

**Potential mitigation**
- Enforce a minimum fee of 1 base unit for non‑zero trades.
- Accumulate fractional fees in a separate counter and apply once it crosses 1 unit.
- Increase fee precision (e.g., use higher‑precision fee denominator).

**Concepts to read**
- AMM fee accounting practices

Sources:
- https://docs.uniswap.org/contracts/v2/concepts/protocol-overview/how-uniswap-works

---

## 7) PDA canonicality & address validation
**Issue**
- PDAs are used extensively; if bumps and seeds aren’t validated consistently, accounts can be spoofed.

**Potential mitigation**
- Store canonical PDAs in state where practical; enforce `address = ...` constraints.
- Ensure PDA derivations use the same seeds everywhere.

**Concepts to read**
- Solana PDA derivation & canonical bump

Sources:
- https://solana.com/docs/core/pda

---

## Notes
- This is a mitigation sketch. I can turn any item into a concrete patch plan with exact Anchor constraints and state changes.
