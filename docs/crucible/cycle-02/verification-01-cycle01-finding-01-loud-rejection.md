# Cycle 02 Verification #1 — Cycle 01 Finding #1 fix holds (loud rejection banner)

**Verifies:** [Cycle 01 Finding #1](../cycle-01/finding-01-silent-mempool-eviction.md)
— "silent mempool eviction" — observability arm of the fix.
**Status:** confirmed. The improvement is real and visible.
**Builds under test:** rig built from `c:/dev/coincync` (uncommitted
v1.0.12-consensus-refresh WIP) submitting to `v1.0.11-fleet` node
(`b8571c8`).

## The reproduction scenario

Cycle 01 Finding #1's underlying class is "version mismatch between
the wallet/rig that constructs a thing and the node that validates
it." Cycle 01 caught this for the wallet+node case (barns submitted
a v1.0.10-wallet-built tx to a v1.0.11 node, got silent eviction).

Cycle 02 reproduced the same class accidentally for the rig+node
case: operator's local rig was built from the WIP v1.0.12 consensus
work (graduated 11→13→16 ring-size ramp + other changes); local
node is v1.0.11. The rig produced blocks following v1.0.12 rules;
the node rejected them.

## What we saw

The rejection was **immediately loud and operator-readable**:

```text
@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
@    BLOCK REJECTED AS INVALID                            @
@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
  The daemon refused our submitted block as invalid.
  This is NOT a normal race-lost rejection.

  Daemon error: POST submit_block to http://127.0.0.1:28081

  Possible causes:
    - Rig and daemon disagree on consensus rules (version skew)
    - Rig built a malformed template (bug)
    - Daemon corruption (try restarting the daemon)
    - Wrong network (rig on mainnet target, daemon on testnet, or vice versa)

  If this fires more than a few times in a row, stop the rig
  and investigate before you produce a long chain of bad blocks.
```

**Compare to Cycle 01 Finding #1** (the silent-eviction case):
- tx was accepted by mempool
- 60 seconds later it was silently evicted by `shadow_evict_invalid`
- the only operator-visible signal was the absence of confirmation
- root cause not obvious without log archaeology

## What the fix shipped

Cycle 01's fix (commit `7358775`) had two arms:

1. **Mechanism arm:** `add_with_chain` now calls the full
   `chain.validate_transaction()` at admission, not just the
   per-key-image fast check. Catches the bug at the right layer.
2. **Observability arm:** rejection paths print human-readable
   diagnostics (the rig banner above; the mempool path
   logs the specific consensus rule that failed).

Cycle 02 confirms the observability arm works in a different
context (rig+node mismatch, not wallet+node mismatch). The
operator running this test immediately understood what happened
and how to fix it.

## Cycle 02 impact

The Cycle 01 "make rejections loud" investment is paying off
in unrelated bug classes. Worth carrying that style forward —
when in doubt, prefer a noisy error with enumerated causes
over a single-line silent failure.

## Worth carrying forward

When designing future error paths (e.g. AssumeValid hash
mismatch — see [v1.0.14 ROADMAP item 2](../../decisions/2026-06-11-assumevalid-design.md)
§ 7.4), use the same template:

- one-line summary that's grep-friendly
- explicit "this is NOT a normal race / drop / transient" callout
  when the error is structural
- enumerated possible causes (not a single guess)
- recommended next action

The AV mismatch path's "Either you're on a fork, or your binary
is for a different network" follows this template directly. The
template's track record now spans both the wallet+node case
(Cycle 01) and the rig+node case (this verification).
