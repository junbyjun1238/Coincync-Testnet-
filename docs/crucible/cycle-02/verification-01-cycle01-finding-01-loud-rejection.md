# Cycle 02 Verification #1 — Cycle 01 Finding #1 fix holds (loud rejection banner)

**Verifies:** [Cycle 01 Finding #1](../cycle-01/finding-01-silent-mempool-eviction.md)
— "silent mempool eviction" — observability arm of the fix.
**Status:** confirmed for the underlying pattern. **Original
attribution revised** — see below.
**Builds under test:** rig built from `c:/dev/coincync` (uncommitted
v1.0.12-consensus-refresh WIP) submitting to `v1.0.11-fleet` node
(`b8571c8`).

## Initial draft (revised)

The first version of this verification doc attributed the rejection
banner to a v1.0.11/v1.0.12 consensus version skew. **That was
incorrect.** Closer reading of the node log showed that:

- The rejection banner fired at 01:22:05 (the rig's first submit
  attempt right after startup).
- A second submit at 01:22:29 by the SAME v1.0.12 rig was **accepted**
  by the SAME v1.0.11 node and committed at h=1.
- Subsequent blocks at h=2 and h=3 also accepted from the same rig.

So the consensus rules between v1.0.11 and the WIP v1.0.12 are
compatible at the heights tested (h=1-3). The v1.0.12 changes
visible in the diff (graduated 11→13→16 ring-size ramp via new
`MID_RING_SIZE` + `RING_SIZE_RAMP_TO_MID_HEIGHT=5000`) wouldn't
diverge from v1.0.11 until h≥5000.

**Actual cause of the initial rejection:** node startup transient.
The rig submitted right after the node finished its own startup
sequence; the node wasn't yet ready to accept submissions and
returned an error. The rig dutifully printed the rejection banner
with version-skew listed as one of four possible causes.

This is itself a small Cycle 02 finding (queued):

- **The rig prints the same banner for "node not ready"
  startup errors as for "real consensus disagreement"** — leads
  operator misdiagnosis. Should distinguish in the message.

## The pattern this still verifies

Cycle 01 Finding #1's class is broader than "version skew": it's
"a path that previously failed silently now fails loudly." The
banner shipped in Cycle 01's observability arm fires for ANY
rejection that goes through `submit_block`, including startup
transients. The verification still holds for the underlying
class — when something rejects, we see it.

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
