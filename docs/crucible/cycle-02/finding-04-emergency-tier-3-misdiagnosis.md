# Cycle 02 Finding #4 — `EMERGENCY-TIER-3` fires repeatedly when nobody is mining

**Status:** open. Misdiagnosis: real condition (chain not advancing) but
wrong root cause (recovery treats it as "sync engine pathology" when
actually no peer is producing blocks).
**Severity:** medium. Wasted recovery effort + misleading log line. Not
data-corrupting.
**Builds affected:** `v1.0.11-fleet` (`b8571c8`).

## TL;DR

`EMERGENCY-TIER-3` fired six times in 25 minutes during cycle 02
testing while the chain sat at h=3 with no mining happening on
either end of the test mesh. Each firing logged:

```text
Sync EMERGENCY-TIER-3 #N: chain has not advanced past height 3 for
420s (>= 300s threshold) despite sync engine reporting non-stalled
state. This indicates an orphan-fetch cascade or similar pathology
where the engine is internally busy but making no real progress.
Forcing aggressive reset: clear address tried-list, drop expired
orphans, reset headers-request timeout. If this fires repeatedly,
operator may need to wipe + reimport snapshot.
```

But the actual problem wasn't an orphan-fetch cascade. **Neither
node was mining**, so there were no new blocks to receive. The
recovery's "clear tried-list, drop orphans, reset headers timeout"
did nothing useful and reran every ~4 minutes.

## Discovery path

Cycle 02 test mesh: operator's v1.0.11 node + barns' v1.0.11 node.
Operator's solo rig refused to mine because `synced=false`
(safety: don't produce blocks on a private fork — Cycle 01
context). Presumably barns' rig has the same safety check and
also refused. **Mutual refusal deadlock.**

Chain stuck at h=3 → 300s elapsed → EMERGENCY-TIER-3 fires →
nothing actually changes → 420s later, fires again → etc.

Confirmed by setting `COINCYNC_RIG_SKIP_SYNC_CHECK=1` on the
operator's rig: chain immediately advanced from h=3 to h=119+
within minutes. The recovery hadn't been the unblocker; an
operator manually bypassing the sync gate was.

## Root cause

The EMERGENCY-TIER-3 trigger condition is "chain hasn't advanced
in N seconds despite non-stalled sync state." It assumes the
reason is internal-to-sync (orphan-fetch cascade, header request
timeout, etc.). It doesn't distinguish that case from the simpler
"no peer is producing blocks to receive."

When mining is happening, the assumption is correct: a stalled
sync engine despite live blocks IS sync pathology. But in a
test mesh where mining is gated on the same `synced` flag the
recovery is trying to fix, you get a deadlock the recovery
can't see.

## Impact

In production:

- Single-node minority operator with no mining and no incoming
  blocks (e.g. all peers temporarily down): repeated TIER-3
  firings burn log volume and CPU on no-op resets.
- Test mesh of 2 nodes that both refuse to mine: deadlock; the
  recovery actively misleads the operator who thinks "the
  recovery is firing, my chain must be broken in some
  recoverable way" when the actual answer is "tell someone to
  start mining."

In cycle 02 specifically:

- 6 firings, 25 minutes wasted on the test
- 12 lines of red ERROR log per firing
- Operator had to root-cause manually via code reading

## Fix candidates

### Fix A — Distinguish "no peers producing" from "engine stuck"

Add a check before firing: "have we received any block headers
from any peer in the last N seconds?" If yes AND we're stuck,
that's engine pathology — fire TIER-3. If no, that's no-peer-
producing — log a different message (e.g. "no peer has produced
blocks; this is normal if mining is paused").

Implementation: track `last_header_received` timestamp across
all peers; check before TIER-3 trigger.

### Fix B — Distinguish in the error message itself

Cheaper: keep firing TIER-3 (the resets are harmless), but
update the message to enumerate both possibilities. Something
like:

```text
chain has not advanced past height N for 300s. This usually
means one of:
  - all peers stopped mining (most common in small test meshes)
  - sync engine internally busy (orphan-fetch cascade, etc.)
Recovery resets sync state in case of the latter. If you're
on a test mesh, check whether any peer is actively producing
blocks.
```

Doesn't fix the wasted firings but stops misleading the operator.

### Fix C — Both

Implement A for the firing logic, B for the message. The
message change is one-line and worth doing regardless.

## Verification (once fixed)

Cycle 02's scenario reproduces deterministically:

1. Spin up two v1.0.11 nodes, point them at each other.
2. Don't start a rig on either.
3. Observe whether TIER-3 fires.

After fix A: TIER-3 should not fire because no headers are
arriving (= no peer producing). After fix B: if it fires, the
message should mention the test-mesh case.

## Follow-ups

- [ ] Pick fix A vs A+B (recommendation: A+B; cheap message
      improvement is no-brainer)
- [ ] Add a regression test that simulates the no-mining case
      and asserts TIER-3 doesn't fire (or fires with the right
      message)
- [ ] Backport to v1.0.13/14 once landed
