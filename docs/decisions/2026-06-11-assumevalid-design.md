# AssumeValid for CoinCync — design (DRAFT)

**Status:** DRAFT — idea tiers LOCKED 2026-06-11 (section 8 short-list
confirmed); section-7 decision points partially resolved by those
selections; remaining open items called out below.
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

```text
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

**RESOLVED 2026-06-11 by ⭐ 8.14** (per-network defaults):
testnet → aggressive default (faster iteration, lower stakes);
mainnet → conservative default (safer floor). The single-default
question is reframed as a per-network split.

### 7.2 Default disabled vs enabled at first release

If disabled, v1.0.14 ships with no actual speedup until constants
get populated. Alternative: pick a testnet block height now (we have
one running) and ship enabled.

**STILL OPEN.** Tier confirmation didn't resolve this. Three
sub-options:

  - **a.** Ship v1.0.14 disabled; populate constants in v1.0.15
    after the v1.0.11-fleet branch has been in operation for
    weeks. Safer; no speedup for v1.0.14 testers.
  - **b.** Ship v1.0.14 enabled with a testnet constant taken from
    the current local v1.0.11 chain's tip. Feature ships
    self-demonstrating; risks shipping the wrong hash if v1.0.11
    needs a re-spin.
  - **c.** Ship the mechanism enabled with a deliberately-stale
    AV (say H=100, well-mined-over) — proves the mechanism works
    end-to-end on real chain history without anchoring to a tip
    that might shift.

My instinct: **(c)** — proves the mechanism, doesn't bake in a
hash that could turn out wrong. <!-- OPERATOR: pick a/b/c -->

### 7.3 `--assumevalid-deep` as opt-in flag

vs always-on aggressive (Bitcoin style). Operator who wants safety
floors must explicitly opt out.

**PARTIALLY RESOLVED 2026-06-11 by ⭐ 8.14.** Per-network default
means the flag's *role* depends on which network you're on. On
testnet, it's redundant (default already aggressive). On mainnet,
it's the opt-in for aggressive mode. Net: the flag exists for
mainnet operator use; on testnet it's a no-op.

### 7.4 Hard hash-mismatch abort vs warn-and-continue

Bitcoin warns. I'm proposing abort because "you're on a fork without
realizing it" is a worse outcome for a privacy chain.

**STILL OPEN.** Not implicitly resolved by section-8 selections.
My recommendation stands: hard abort + privacy-aware error
message that doesn't leak which peer fed the bad data (per §10.4).
<!-- OPERATOR: confirm hard-abort, or flip to warn-and-continue? -->

### 7.5 Activation height for the initial testnet AssumeValid value

Do you want to anchor to the current local v1.0.11 chain at some
recent height, or wait until the public testnet has soaked for N
days post-v1.0.11-release?

**DEFERRED until 7.2 is resolved** (a/b/c above). If 7.2 lands on
**a**, this question doesn't apply yet. If on **b**, we need the
hash of a current testnet tip. If on **c**, we just pick H=100
or similar. <!-- OPERATOR: comes into focus once 7.2 is picked -->

### 7.6 Should `--assumevalid-deep` even exist for v1.0.14?

vs ship just the conservative-only path and add aggressive later.

**RESOLVED 2026-06-11 by ⭐ 8.14** (per-network defaults):
aggressive mode is the testnet DEFAULT, which means it has to
exist. The `--assumevalid-deep` flag becomes meaningful on
mainnet, where it's an opt-in override of the conservative
default. The flag also becomes the testnet opt-out for "I want
mainnet-level safety even on testnet."

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

**RESOLVED 2026-06-11 by ⭐ 8.10** (snapshot tip declares an AV
hint): we couple, but only by treating the snapshot's tip as a
PER-INSTALL effective AV, not a project-level AV. Hardcoded
constants are unaffected. A user who already trusted the snapshot
URL gets the historical-verification skip naturally without
forcing the project-level trust floor to weaken.

### 7.8 Single hash vs sequence of checkpoint hashes

Instead of a single AssumeValid hash, ship a sequence of
(height, hash) pairs. More expensive in code but harder for an
attacker to forge a chain that matches at exactly one point.

**RESOLVED 2026-06-11 by ⭐ 8.4 + 8.15** (sliding-window +
append-only ratchet): the data structure becomes a sequence of
checkpoints (8.15 append-only ratchet), AND the trusted window
slides forward as the chain grows (8.4 sliding-window). Each
release appends; never modifies. Combined effect: the chain is
anchored at multiple historic heights AND every binary picks up
trust for blocks `tip − K` regardless of release age.

### 7.9 Anything else I haven't surfaced?

<!-- OPERATOR: -->

---

## 8. Idea bucket — 17 design variants for review

Captured 2026-06-11 from a brainstorm pass. Each idea is described
in two-to-four sentences: what it is, what it buys, what it costs.
The recommendation column captures which ideas I think are worth
folding into the v1.0.14 mechanism vs which are speculative /
future-CIP material. Decision on each lands in section 7 above
once the operator weighs in.

Status legend:
  ⭐ = strong recommendation, fold into v1.0.14
  ◐ = worth doing, possibly v1.0.15
  ○ = speculative, may belong in its own CIP

### Trust-model alternatives

**8.1 ⭐ Multi-source consensus checkpoint.** Instead of one
hardcoded hash, fetch *published* (height, hash) tuples from N
independent sources at startup (peer gossip + `.well-known` + a
maintainer-signed manifest). Require k-of-n agreement before
activating AssumeValid. *Cost:* more code, slower startup. *Buys:*
no single point of trust failure. *CoinCync angle:* the published
sources can reuse the same `.well-known/` infrastructure we built
for DNS-seed fallback in `7b003b0` — same trust anchor, same
operational footprint.

**8.2 ◐ Maintainer-signed checkpoint manifest.** Ship a maintainer
pubkey in the binary; fetch `.well-known/coincync-checkpoints.json`
(signed); verify signature before trusting. *Buys:* compromised CDN
can't poison checkpoints. *CoinCync angle:* couples to the existing
`MAINTAINERS.md` bus-factor framework — checkpoint-signing is a
natural maintainer responsibility we'd add to the role description.

**8.3 ○ Time-locked checkpoint.** A checkpoint published at T₀
becomes trustable only at T₀ + 14 days. *Buys:* network reaction
window — if a checkpoint is wrong, somebody notices and shouts
before binaries start trusting it. *Cost:* slightly inconvenient
for fresh installs in the first 2 weeks of a release.

### Mechanism variants

**8.4 ⭐ Sliding-window AssumeValid.** Instead of a single fixed
height, the trusted window is `tip − K` blocks. *Buys:* an
out-of-date node STILL benefits because its window slides forward
as it catches up; avoids the "AV constant goes stale between
releases" problem. *Math:* K=10,000 means anything older than ~3.5
days at 30s blocks gets the speedup. *CoinCync angle:* combines
naturally with our 30-second block time — the math gives natural
units (days, not just block heights).

**8.5 ○ Layered crypto skipping by depth.** Per-transaction
decision: based on the tx's depth from tip, skip CLSAG (deepest),
skip BP+ (very deep), skip balance proof (only at the assumevalid
floor). Smooth fade-in rather than binary on/off. *Cost:* more
complex. *Buys:* matches "trust scales with how settled the block
is" intuition.

**8.6 ◐ Coinbase-only skipping.** Below AV, skip validation **only
for coinbase txs** (no ring sigs, no BP+ on those by definition).
*Buys:* tiny speedup, ~zero attack surface — coinbase structure
is heavily restricted. *Use:* a minimal "let's get the mechanism
shipped" version. Good prototype before expanding to general
skipping.

**8.7 ○ Probabilistic spot-check.** Even below AV, randomly
fully-verify 1 in N blocks. *Buys:* statistical detection of forged
chains — if even 1% of below-AV blocks get audited, a tampered
chain shows up quickly. *Cost:* speedup is (N-1)/N. The check is
invisible to the operator and cheap to add.

### Privacy-specific (CoinCync-unique)

**8.8 ⭐ Key-image uniqueness ALWAYS verified, even below AV.**
Even if we skip ring signature math, key-image-unique check is what
defines double-spend prevention. *Cost:* sub-microsecond per
lookup. *Defensible posture:* "we skip the crypto-math-proof of
ownership but still enforce the no-double-spend property at the
database level." *CoinCync angle:* this is fundamentally a
privacy-chain concept that has no Bitcoin equivalent — Bitcoin's
double-spend prevention comes from signature verification itself,
ours comes from key-image set membership, so they decouple in
ways Bitcoin's design doesn't.

**8.9 ○ Curve-point sanity ALWAYS verified, even below AV.**
Stealth output P, key images, commitments — verifying they're
on-curve is sub-microsecond per element. Skipping doesn't help
speed but keeping it catches "obvious garbage" early. *Worth
keeping for defense in depth.*

### Integration with what we've already shipped

**8.10 ⭐ Snapshot tip declares an AssumeValid hint.** The
`snapshot-fetch` metadata JSON (see `e127a01`) includes a
`(tip_height, tip_hash)` pair. After extraction, the node uses
that as its effective AssumeValid — for THIS chain only, for THIS
install. *Buys:* couples cleanly — a user who trusted the snapshot
URL has already accepted the trust assumption.

**8.11 ◐ AssumeValid as snapshot precondition.** Refuse
`snapshot-fetch` if the snapshot's tip is BELOW the hardcoded
AssumeValid. *Buys:* prevents downgrade attacks via stale-but-
correct snapshots. Makes AV the floor for what snapshots can
advertise.

**8.12 ⭐ AV health visible on `ibd-status`.** The diagnostic
subcommand from `48faf06` gets a new line: "AssumeValid: enabled
@ H=14328 (verified ✓)" or "AssumeValid: disabled (paranoid mode)"
or "AssumeValid: pending — chain not yet at AV height". *Buys:*
operators can audit at a glance which validation mode their fleet
is using.

### Operator-experience

**8.13 ◐ Visible validation-profile audit trail.** Log line every
N blocks during IBD: "Block H validated [full | conservative-skip
| aggressive-skip | spot-check]". *Buys:* lets operators see when
AV kicked in/out. Helps debug "is my node validating what I think
it is?" *CoinCync angle:* must be privacy-aware — log block-level
profile, NOT per-tx (per-tx would leak which txs got skipped).

**8.14 ⭐ Per-network defaults.** Testnet defaults to **aggressive**
(faster iteration, lower stakes); mainnet defaults to
**conservative** (safer floor, slower but firmer). *Codifies
different risk postures by network.*

### Governance hooks

**8.15 ⭐ AssumeValid ratchet (append-only).** Each release ADDS a
new (height, hash) but never modifies or removes old ones. The
constants table grows; never shrinks. *Buys:* audit trail in git
history by construction. Catches the "someone tampered with the AV
constant" case at code review time — the diff would *remove* a
line, which is the suspicious case.

**8.16 ◐ N-of-M maintainer signatures to update AV.** Adding a new
AV constant requires N-of-M signatures from the `MAINTAINERS.md`
set. *Buys:* closes the "single rogue maintainer poisons the
chain" attack. Reuses the existing bus-factor framework.

### Crucible-integration

**8.17 ⭐ Crucible exercise: ship a binary with deliberately wrong
AV hash.** Before locking a mainnet AV, run a Crucible cycle that
distributes a binary with the WRONG hash to testers; verify the
safety check (section 3 above) fires for all of them. *Buys:*
empirical proof the abort path works before staking real users'
chains on it. *CoinCync angle:* leverages the v1.0.13 Crucible
automation (`1b6da66`) — bundle is one command.

### My short-list (fold into v1.0.14 mechanism)

⭐ 8.1, 8.4, 8.8, 8.10, 8.12, 8.14, 8.15, 8.17 — eight items.
Together they form a coherent v1.0.14 AssumeValid that's:

  - multi-source (8.1) so no single trust point
  - sliding-window (8.4) so it doesn't go stale between releases
  - privacy-safe (8.8) — key-image check never skipped
  - snapshot-aware (8.10) — closes the loop with `e127a01`
  - observable (8.12) — ibd-status shows the active mode
  - per-network calibrated (8.14)
  - append-only governed (8.15)
  - empirically tested via Crucible (8.17) before lock

The remaining ◐ items (8.2, 8.6, 8.11, 8.13, 8.16) are v1.0.15
candidates — they extend the shape but aren't load-bearing for
the v1.0.14 ship.

The ○ items (8.3, 8.5, 8.7, 8.9) are either speculative or
defense-in-depth additions that could go either way.

<!-- OPERATOR: react to the short-list. Move items between tiers,
add new ones, kill ideas you don't like. -->

---

## 9. Working notes

Drop scratch / context / links here as the design evolves.

- **2026-06-11:** doc started. Sketched threat model + two-level
  activation + hard safety check + constants shape + CLI surface +
  3-commit plan. All decision points still open.
- **2026-06-11:** added 17-idea bucket (section 8) + CoinCync-
  uniqueness analysis (section 10). Short-list of 8 ⭐ items
  proposed for v1.0.14 fold-in.

---

## 10. What makes this uniquely CoinCync (not a Bitcoin port)

The temptation when designing a feature with a Bitcoin precedent is
to port the Bitcoin design and call it done. For AssumeValid that
would be: copy Bitcoin Core's flag, hardcode a hash, skip script
verification below it. Done in 50 lines. **That's not what we're
building**, and this section captures why — so that future
reviewers can re-derive the choices we made rather than wondering
why we didn't take the obvious path.

### 10.1 The cost profile is different

Bitcoin's expensive validation work is concentrated in *signature
verification* (a single Secp256k1 curve multiplication per input).
Roughly equal cost per input regardless of tx shape.

CoinCync's expensive validation work is concentrated in
**Bulletproofs+ range proofs** (~5–10 ms per output), distantly
followed by CLSAG ring signature verification (~1 ms per input),
with balance proofs as a rounding error.

This matters because Bitcoin's `-assumevalid` is binary: skip all
script verification, or skip none. The cost classes are too
similar to bother with intermediate states. **CoinCync has
meaningfully distinct cost classes**, which is why the
conservative-vs-aggressive split (section 2) is even worth
considering. We get to pick *which* expensive thing to skip;
Bitcoin doesn't.

### 10.2 The trust property being skipped is different

Bitcoin's AssumeValid skips "does this signature prove ownership"
— a fungibility / property-rights check. The threats it would
otherwise catch are signature forgery and (with Segwit witness
data) malleability shenanigans.

CoinCync's AssumeValid would skip:

- "Does this CLSAG prove ownership of the spend authority for one
  of the ring members?" — same flavor as Bitcoin.
- "Does this BP+ prove the output amount is in `[0, 2^64)`?" —
  this has no Bitcoin equivalent because Bitcoin amounts are
  cleartext integers.
- "Does the balance proof show `Σ inputs = Σ outputs`?" — this also
  has no Bitcoin equivalent.

The last two are **inflation defenses**, not ownership defenses.
Skipping them is qualitatively different from skipping sig checks.
Our conservative default (sig-only) preserves the inflation floor;
Bitcoin's aggressive-by-default doesn't have inflation-floor
analog because the floor is a cheap integer check that's free to
keep.

This is why we land on **conservative-default + aggressive-opt-in**
rather than Bitcoin's reverse: the inflation floor matters more in
a confidential-amounts chain.

### 10.3 Key-image uniqueness is the privacy-defining check

Bitcoin's double-spend defense IS signature verification — same
mechanism, two purposes. Skip sig verification and you've lost
double-spend defense too.

CoinCync's double-spend defense is **key-image set membership**
(a database lookup), entirely separate from signature math. We can
skip CLSAG verification entirely and still enforce no-double-spend
at the database level. **Idea 8.8 is fundamentally a CoinCync
concept**; there's no Bitcoin equivalent because the design space
doesn't exist there.

This decouples "ownership proof" from "double-spend prevention"
in a way that gives us much safer defaults than Bitcoin would. We
can be aggressive about skipping signature math without weakening
the chain's double-spend defense — Bitcoin can't.

### 10.4 Privacy-aware error handling

When a Bitcoin node detects an AssumeValid mismatch, it logs the
problem with peer addresses, block hashes, full context. That's
fine for Bitcoin — there's no privacy expectation about which
peer fed you which block.

For CoinCync, the error path needs to **not leak which peer told
us the bad data**. A nation-state running a poisoned peer could
use error reports to detect "this IP just rejected my poisoned
chain — they're a sophisticated user." We log the height and the
hash mismatch, but not the peer identity, by default. Per-peer
identification is gated behind an explicit `--log-peer-on-mismatch`
flag for operators who are explicitly OK with that disclosure.

### 10.5 Fleet-as-trust-anchor is a CoinCync pattern

The `fleet.toml` ↔ `.well-known/` pattern we built in v1.0.13–14
is unique to our deployment shape. Bitcoin has no fleet in this
sense — it has a developer-author trust assumption plus a global
DNS-seed network.

For us, the project-operated fleet *is* the trust anchor. Idea 8.1
(multi-source consensus checkpoint) leverages this directly: the
fleet's own nodes publish their tip hashes to `.well-known/`, and
a fresh node reaches k-of-n agreement across the fleet's
publications. This is a load-bearing design choice — it means our
trust model is "trust the fleet" rather than "trust one
hardcoded constant", which scales more gracefully as the project
grows.

### 10.6 The Crucible program enables empirical verification

Idea 8.17 (ship a binary with the wrong AV hash to Crucible
testers, verify the abort fires) is impossible in the Bitcoin
world because Bitcoin doesn't have a structured testing program
of independent operators willing to run experimental binaries.

We do. The v1.0.13 Crucible automation (`1b6da66`) makes the
exercise mechanically cheap: one bundle command, distribution
through Discord, results in a `docs/crucible/cycle-NN/` writeup.
That's a CoinCync-specific testing capability that the design
should explicitly take advantage of.

### 10.7 Pre-mainnet timing lets us design AV in rather than retrofit

Bitcoin Core added AssumeValid in 2017 — eight years after launch.
By then the chain had a deep history and AV had to retrofit onto
running consensus.

CoinCync adds AssumeValid pre-mainnet. **We get to design AV into
the launch state** rather than back-fitting. The hash gets baked
in at GA based on testnet maturity; v1.0.0 of mainnet ships
already AV-aware. This means we don't have the "old binaries
without AV, new binaries with AV" version-skew problem Bitcoin
navigated for years.

### 10.8 Per-network calibration

Bitcoin has mainnet, testnet, regtest, signet. Each has different
trust profiles in practice but Bitcoin Core doesn't formalize
that — `-assumevalid` is one flag, one default, regardless of
network.

For us, idea 8.14 (per-network defaults) codifies what's
intuitively true: testnet operators want speed for iteration, so
aggressive default; mainnet operators want safety floors, so
conservative default. We make the network-dependent default
explicit in code rather than leaving it for the operator to set
correctly. This is small but worth doing.

### Summary

Five things make this design CoinCync-specific:

1. **Distinct cost classes** (BP+ vs CLSAG vs balance) enable
   meaningful conservative/aggressive split (§10.1)
2. **Inflation-floor preservation** drives our default toward
   conservative, opposite of Bitcoin's default (§10.2)
3. **Key-image decoupled from signatures** lets us skip aggressively
   without losing double-spend defense (§10.3, idea 8.8)
4. **Fleet-as-trust-anchor** lets multi-source consensus checkpoints
   reuse our existing infrastructure (§10.5, idea 8.1)
5. **Crucible program** lets us empirically verify the safety check
   before mainnet lock (§10.6, idea 8.17)

If a future reviewer asks "why didn't you just port Bitcoin's
design?" — the answer is "because those five points would have
been wrong for our threat model and our deployment shape."
