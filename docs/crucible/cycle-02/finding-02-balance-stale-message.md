# Cycle 02 Finding #2 — `wallet balance` prints stale "doesn't persist" message

**Status:** open. Trivial fix.
**Severity:** low (UX-only). Operator confusion, no data loss.
**Builds affected:** `v1.0.11-fleet` (`b8571c8`).

## TL;DR

`coincync-wallet balance` always prints:

```text
(P1 note: balance computed from wallet file state only.
 The file doesn't persist UTXOs yet — P2 work will add a
 real chain-scan-via-RPC path and UTXO materialisation.)
```

This message dates from when wallet UTXO persistence wasn't
implemented. **It is now implemented.** `coincync-wallet scan`
writes a `<wallet>.utxos` sidecar that contains exactly the
materialised UTXOs the message says don't exist.

So `balance` and `scan` are out of sync: `scan` reports the
truth, `balance` claims the truth doesn't exist yet.

## Symptom from Cycle 02 testing

```text
$ wallet scan
Scanned:        4 blocks
Found outputs:  3
Balance total:  149.9998 CYNC
UTXO count:     3
UTXOs persisted to ".../test.wallet.utxos"

$ wallet balance
Wallet label:    default
Scanned height:  3
(P1 note: balance computed from wallet file state only.
 The file doesn't persist UTXOs yet — P2 work will add a
 real chain-scan-via-RPC path and UTXO materialisation.)
```

`balance` says no persistence; `scan` ran in the same session and
persisted 3 UTXOs.

## Root cause

`balance`'s code path reads only the encrypted wallet file and
prints the P1 stub. It was never updated to read the `.utxos`
sidecar that `scan` writes.

## Fix

`balance` should:

1. Look for the `<wallet>.utxos` sidecar.
2. If present: deserialize, sum unspent amounts, print balance
   + UTXO count.
3. If absent: print the P1 note (preserving the message for
   the genuine "haven't scanned yet" case).

Estimated ~20-30 lines of Rust in `src/bin/wallet.rs`. No
consensus implications, no critical_files.lock touch.

## Verification once fixed

```text
$ wallet balance
Balance:    149.9998 CYNC
UTXOs:      3
Scanned:    block 3
```

## Follow-ups

- [ ] Implement the sidecar-read path in `balance`
- [ ] Keep the P1 fallback for the "no scan ever ran" case
- [ ] Add a one-line note to the wallet docs explaining the
      scan → balance flow
- [ ] Audit other wallet subcommands that print similar stub
      messages for similar staleness
