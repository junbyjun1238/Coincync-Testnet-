# Crucible Cycle 02 — 2026-06-11

**Status:** in progress.
**Operator:** ghostrider1092 (local) + barns1253 (remote, France).
**Builds under test:** `v1.0.11-fleet` HEAD (`b8571c8`).
**Mesh shape:** 2-node isolated testnet, operator dials out to barns
over IPv6 (CGNAT inversion from Cycle 01 Finding #2).
**Network reachability:** IPv6 `2a01:e0a:c53:63d0:225:90ff:febc:989a`
+ IPv4 `82.66.194.28`, both port 28080. Both reach.

## Goal

Validate that the four Cycle 01 fixes hold in a real 2-node mesh
against an external tester, and surface anything that didn't
get caught in single-node testing.

## Test sequence

1. **Connect** — operator's v1.0.11 node `--no-peers --addnode <barns>:28080`
   isolated to barns only.
2. **Mine** — local solo-rig produces blocks paying a fresh testnet
   wallet (`tCYNC3Z3sPbZssh5dvvL5dzycR5EkYxNKYcxWNvnB8QCcvYsAfQu3pLEMvSmpVb16fzFSYwGtWEx1VFutN642Do5iK1exTxXy5Xq`).
3. **Sync** — observe barns' blocks arrive over P2P.
4. **Send** — once coinbase matures (≥10 blocks deep), build a
   privacy tx paying barns' wallet `tCYNC3YyMB6Cjz6cXQ6hyLVYf3A3cLDpL5JwDkW9UkYZWeUUJkL5FjVCiaUGqf4HJX8fuQ7C8dfkRr8WQ5rr8iSv4x1HQfRbT9ek`.
5. **Verify** — barns confirms receipt in his wallet scan.

## Findings & verifications

- [Finding #1](finding-01-peer-flap.md) — peer flap every ~10s
  (Noise stream close → reconnect cycle, both sides).
- [Finding #2](finding-02-balance-stale-message.md) — `wallet balance`
  prints "UTXOs don't persist" P1 note even when `scan` has persisted
  them to the `.utxos` sidecar.
- [Verification #1](verification-01-cycle01-finding-01-loud-rejection.md) —
  Cycle 01 Finding #1's observability improvement holds: v1.0.12-rig
  vs v1.0.11-node now produces a LOUD ASCII banner rejection (vs
  Cycle 01's silent eviction). Improvement confirmed.

## Open in-flight observations

The chain has progressed slowly (h=3 after ~10 minutes of test).
Mining solo-rig refuses to mine while not-yet-synced (correct
behavior — won't mine to a private fork). EMERGENCY-TIER-3
recovery fired at 01:28:17 — chain hadn't advanced past h=3 for
300s — which is the Cycle 01 Finding #3 recovery path firing as
designed. So far an additional small observation: the recovery
fires but doesn't immediately get the chain unstuck; subsequent
peer-flap cycle does the actual unsticking.

## Test partial-pause

As of writing this README, the test is paused awaiting
coinbase maturity (chain at h=3, need h≥11). The send-tx leg
of the test continues once h=11 reaches.
