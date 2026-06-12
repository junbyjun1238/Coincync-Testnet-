# Cycle 02 Finding #1 — Peer flap every ~75s on stable IPv6 WAN link

**Status:** root-caused (NAT / TCP-keepalive), fix in progress.
**Severity:** medium. Stable enough that chain sync still progresses;
operationally noisy (CPU burn on repeated Noise handshakes, log
volume); blocks tx propagation when the disconnect happens to
overlap with a block-relay window.
**Builds affected:** `v1.0.11-fleet` (`b8571c8`) — likely all
earlier versions too; the missing TCP keepalive sockopts have been
absent since the network layer was first written.

## Updates

- **2026-06-11:** Initial draft from cycle 02 observation.
- **2026-06-11 (later):** Loopback reproduction test ran for
  145+ seconds with **zero disconnects** between two v1.0.11
  binaries connected via IPv6 loopback (`[::1]`). Same binary,
  same noise wrapper, same code path — but no flap. Confirms
  the flap is **not application-layer**; it's NAT/router state
  expiration on the WAN path. Fix shifts from "investigate
  source" to "set TCP keepalive on outbound P2P sockets."

## TL;DR

Two v1.0.11 nodes connected over IPv6 on an otherwise-healthy link
disconnect and reconnect every ~10 seconds. Every cycle is:

```text
Noise handshake succeeded with [peer]:28080 (remote key: ...)
    ... 10-20s pass ...
Peer disconnected (noise stream closed): IO error: unexpected end of file
    ... 10s pass ...
Noise handshake succeeded with [peer]:28080 (remote key: ...)
```

Symmetric (both sides report the disconnect).

## Symptom

Repeating cycle visible in the log; sample timestamps from cycle 02:

```text
01:24:15.020 — Peer disconnected (noise stream closed)
01:24:24.859 — Noise handshake succeeded with 82.66.194.28:28080
01:24:44.980 — Peer disconnected
01:24:54.859 — Noise handshake succeeded with [IPv6]:28080
01:25:04.849 — Peer disconnected
01:25:14.884 — Noise handshake succeeded with 82.66.194.28
... (continues indefinitely)
```

Note also: the address alternates between IPv4 and IPv6 because
addnode supplied both. Each reconnect picks one (operator's view
is "barns has two known addresses, both reachable, both being
tried").

## Discovery path

Found during Cycle 02 live test. Was checking why chain sync was
slow (h=3 after several minutes) and noticed the flap pattern
when filtering out `GetHeaders` log spam.

## Hypotheses, ranked by likelihood (post-loopback-test)

1. **CONFIRMED — TCP keepalive not set on outbound P2P
   sockets.** Without `TCP_KEEPIDLE` / `TCP_KEEPINTVL`, idle
   connections die when an intermediate NAT/firewall expires
   its state. Consumer routers default to short timeouts
   (60-300s for established TCP). With our P2P traffic
   pattern of "GetHeaders every 500ms + nothing else for
   minutes" during low activity, a router can expire the
   state between two GetHeaders if the gap exceeds its idle
   timeout. The Noise stream then sees EOF from the
   no-longer-routed peer. Cycle 02 measured ~75s flap interval
   (not 10s as initially estimated) which is consistent with
   a 60-90s NAT idle timeout.
2. *(ruled out)* Noise framing self-timeout. Loopback test
   shows the same noise wrapper sustained 145+ seconds without
   close. If a wrapper-internal timeout existed it would also
   fire on loopback.
3. *(ruled out)* Graceful-close-from-barns. We see the same
   EOF symmetrically from operator's side, and the loopback
   test (both sides controlled by us) showed no
   close-and-reconnect, so this isn't a peer-logic event.

## What we know

- Cycle is stable enough that headers sync completes (we reached
  h=3 on the testnet from barns over the flapping).
- Both IPv4 and IPv6 paths exhibit the same behavior; alternation
  between them on reconnect suggests the addnode replay path.
- Operator's network is Cox CGNAT (Cycle 01 confirmed); barns'
  network is unknown but probably residential French ISP. Either
  could be the NAT culprit.
- No `RST` in logs — only `EOF`. Suggests application-layer close,
  not TCP reset.

## Verification methodology when investigating

1. Capture pcap on both ends during a single cycle. Look at:
   - Whether SYN-keepalive frames are sent or absent.
   - Whether the close is `FIN` (clean) or `RST` (forced).
   - Whether the close is preceded by any P2P-protocol message.
2. Check the noise/p2p wrapper source for any timeout that could
   fire on idle.
3. Check whether the same cycle exhibits on a localhost
   loopback test (operator runs two local nodes against each
   other). If localhost reproduces, NAT is ruled out.
4. Check whether the cycle stops once IBD completes and tx
   traffic flows.

## Impact

- **Operator UX:** Visible log noise. Operator dashboard
  shows peer count flapping 0↔1.
- **Privacy:** Reconnect cycles burn through Noise key
  exchange; no information leak per se (the remote key
  fingerprint stays constant), but the timing pattern is
  observable to anyone on the path.
- **Mining:** Rig refuses to mine while peer flap means
  `synced=false`, so visible knock-on effect: in cycle 02
  the rig WAITED through several flap cycles before
  considering us synced.

## Fix candidates (not yet implemented)

- **Set TCP keepalive on outbound P2P sockets** with reasonable
  intervals (e.g., `KEEPIDLE=60`, `KEEPINTVL=15`, `KEEPCNT=4`).
  This is a one-line change in `src/network/node.rs`'s connect
  path. Cheapest fix.
- **Send periodic Ping P2P messages** during idle periods.
  Heavier but more portable across noise wrappers.
- **Investigate noise wrapper timeout** before adding either of
  the above — fix the root cause if it's application-layer.

## Follow-ups

- [ ] Reproduce on localhost (rules out NAT)
- [ ] pcap capture during a single cycle
- [ ] Audit `src/network/p2p/` for any timeout that could fire
- [ ] Apply TCP keepalive fix and re-test against barns
- [ ] Backport fix to v1.0.13/14 once root-caused
