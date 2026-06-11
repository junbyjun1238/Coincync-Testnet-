# AssumeValid for CoinCync — design (DRAFT)

**Status:** DRAFT — under collaborative design.
**Authors:** ghostrider1092 + Claude.
**Started:** 2026-06-11.
**Targets:** v1.0.14 IBD-speedups track, item 2.

This document is the workspace for the AssumeValid design. Decision
points are marked `<!-- OPERATOR: -->` and intended to be filled in
during review. The bottom section is reserved for design additions
that don't fit the original outline — the operator (or any
maintainer) should add their own ideas there freely; we'll fold them
into the structure once we know what they're.

When this doc reaches consensus, it splits into two:

- `docs/architecture/ASSUMEVALID.md` — operator + maintainer reference
- `docs/decisions/2026-06-XX-assumevalid-rationale.md` — frozen
  rationale + decision record

Until then, this single file holds everything.

---

## 1. Threat-model frame

AssumeValid is a **speed enhancement, not a defense weakening**. The
argument is "every block at H ≤ AssumeValid_height has already been
validated by thousands of peers; a fresh node doesn't need to re-do
that work." Two things follow:

1. AssumeValid only affects IBD validation cost. Every block ABOVE
   AssumeValid_height is fully validated like today.
2. AssumeValid IS a trust assumption about the canonical chain. We
   need to be honest about what it costs.

This is the same frame Bitcoin Core uses for `-assumevalid` and
Monero for `--fast-sync`. CoinCync's privacy posture doesn't change
the frame — it changes the LIST of things we're trusting.

## 2. Two activation levels (proposed)

| Level | Below assumevalid_height, skip... | Speedup (est.) | Trust assumption |
|---|---|---|---|
| **Conservative** (default) | CLSAG signatures only | ~15–30% | "Forged sigs never landed on the canonical chain" |
| **Aggressive** (opt-in: `--assumevalid-deep`) | CLSAG + BP+ range proofs + balance proofs | ~80–90% | "No exploitable crypto violations on the canonical chain" |

CoinCync's tx validation cost breaks down roughly:

- CLSAG verify per input: ~1ms
- Bulletproofs+ per output: ~5–10ms
- Balance proof: <1ms

So conservative skips the smaller cost; aggressive skips the dominant
cost. The conservative-default + aggressive-opt-in pattern matches
Bitcoin Core's `-assumevalid` posture (which defaults to skipping
all script verification — equivalent to our aggressive mode — but
with the safety net that Bitcoin has clear-text amounts so "no
amount overflow" is a cheap integer check, while ours needs
cryptographic verification).

<!-- OPERATOR: are two levels right, or is one (just conservative, or
just aggressive) cleaner? -->

## 3. The hard safety check

When the node reaches `ASSUMEVALID_HEIGHT` during sync, it **MUST**
verify the block at that height has hash == `ASSUMEVALID_HASH`. On
mismatch:

- Reject the chain
- Refuse to start
- Operator-facing error: "your chain at H=N doesn't match the
  published assumevalid hash; either you're on a fork or your binary
  is wrong"

Without this check, an attacker could send any chain that "looks"
right under the cheap-only validators; the hash anchor catches that.

<!-- OPERATOR: hard abort vs Bitcoin's warn-and-continue? My
recommendation is hard abort because "you're on a fork without
realizing it" is worse for a privacy chain than "you got an error
and had to re-sync." -->

## 4. Constants shape

```rust
// constants.rs (testnet/mainnet conditional)
pub const ASSUMEVALID_HEIGHT: u64 = 0;        // 0 = disabled
pub const ASSUMEVALID_HASH:   [u8; 32] = [0; 32];
```

Default at first release: **disabled** (height=0 + zero hash).
Populated after the chain has matured for some weeks AND been
audited. For mainnet GA in October, this gets populated AT mainnet
launch time using a height the chain reached during the pre-GA
testnet phase. Until then the mechanism exists but is inert —
operators can override via `--assumevalid-hash <hex>` for testing.

<!-- OPERATOR: do we want the constants to be (height, hash) singletons,
or a SEQUENCE of (height, hash) pairs? See decision point 8 below. -->

## 5. CLI surface

```
--assumevalid-hash <hex>     Override hardcoded hash (testing / custom networks)
--no-assumevalid             Verify every block fully (paranoid / audit mode)
--assumevalid-deep           Aggressive mode: skip BP+ and balance proof too
```

Default behavior matches conservative mode when constants are set;
disabled when they aren't.

<!-- OPERATOR: any flag names you'd rather rename? --no-assumevalid
could also be --paranoid or --full-validation. Bikeshed welcome. -->

## 6. Multi-commit implementation plan

| # | Commit | What |
|---|---|---|
| 1 | **Design doc** | this file + `docs/architecture/ASSUMEVALID.md`. Pure documentation, no code, no `critical_files.lock` touch. Lets you review the design before any consensus-adjacent code lands. |
| 2 | **Mechanism** | Add constants + `validate_transaction_assumevalid()` variant in `consensus/validation.rs` + the `chain.rs` gate + checkpoint hash assertion. Requires `critical_files.lock` refresh on `validation.rs` and `constants.rs`. |
| 3 | **CLI flags** | Add the three flags. End-to-end test with a synthetic chain. |

Splitting this way means: if you stop after commit 1, the doc is
still useful and nothing is broken. The doc is the place we want
explicit review/sign-off before committing the mechanism.

## 7. Decision points (need OPERATOR judgment before commit 2 lands)

These are the open questions that need explicit answers in this
file before the mechanism commit ships.

### 7.1 Conservative vs aggressive as default

Bitcoin defaults to aggressive (skip everything). Are we okay being
more conservative, accepting smaller speedups for the safety floor
of keeping inflation checks?

<!-- OPERATOR: -->

### 7.2 Default disabled vs enabled at first release

If disabled, v1.0.14 ships with no actual speedup until constants
get populated. Alternative: pick a testnet block height now (we have
one running) and ship enabled.

<!-- OPERATOR: -->

### 7.3 `--assumevalid-deep` as opt-in flag

vs always-on aggressive (Bitcoin style). Operator who wants safety
floors must explicitly opt out.

<!-- OPERATOR: -->

### 7.4 Hard hash-mismatch abort vs warn-and-continue

Bitcoin warns. I'm proposing abort because "you're on a fork without
realizing it" is a worse outcome for a privacy chain.

<!-- OPERATOR: -->

### 7.5 Activation height for the initial testnet AssumeValid value

Do you want to anchor to the current local v1.0.11 chain at some
recent height, or wait until the public testnet has soaked for N
days post-v1.0.11-release?

<!-- OPERATOR: -->

### 7.6 Should `--assumevalid-deep` even exist for v1.0.14?

vs ship just the conservative-only path and add aggressive later.

<!-- OPERATOR: -->

### 7.7 Snapshot ↔ AssumeValid coupling

Should snapshots (the `snapshot-fetch` subcommand) **implicitly**
populate AssumeValid based on the snapshot's tip metadata, or stay
strictly separate?

Arguments for coupling: a user who fetched a snapshot has already
trusted the URL host; carrying that trust forward into validation
seems consistent.

Arguments against coupling: the trust assumption for AssumeValid
("the canonical chain validated by thousands of peers") is stronger
than the trust assumption for a snapshot ("this URL host gave me a
chain I can resync from"). Mixing them weakens the AssumeValid
floor.

<!-- OPERATOR: -->

### 7.8 Single hash vs sequence of checkpoint hashes

Instead of a single AssumeValid hash, ship a sequence of
(height, hash) pairs. More expensive in code but harder for an
attacker to forge a chain that matches at exactly one point.

<!-- OPERATOR: -->

### 7.9 Anything else I haven't surfaced?

<!-- OPERATOR: -->

---

## 8. Operator-added designs

This section is reserved for design ideas that don't fit the
sections above. Add anything here in any form — bullet points,
free-form prose, code sketches, half-baked thoughts. We'll fold
them into the structure once we see what shape they're.

The only thing that matters at this stage is capturing the ideas.
Don't worry about scope: some might belong in AssumeValid; some
might be their own CIP; some might be v1.0.15 / v2.0 territory.
We'll sort after.

### 8.1 [your idea here]

<!-- OPERATOR: drop ideas — single thread of consciousness or
bullet list, whichever's natural. -->

### 8.2 [your idea here]

<!-- OPERATOR: -->

### 8.3 [your idea here]

<!-- OPERATOR: -->

---

## 9. Working notes

Drop scratch / context / links here as the design evolves.

- 2026-06-11: doc started. Sketched threat model + two-level activation
  + hard safety check + constants shape + CLI surface + 3-commit plan.
  All decision points still open.
