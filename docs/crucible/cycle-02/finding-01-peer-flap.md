# Cycle 02 Finding #1 — Peer flap every ~10s on stable IPv6 link

**Status:** open. Cause not yet root-caused.
**Severity:** medium. Stable enough that chain sync still progresses;
operationally noisy (CPU burn on repeated Noise handshakes, log
volume).
**Builds affected:** `v1.0.11-fleet` (`b8571c8`) — possibly earlier;
not yet checked against v1.0.13/14.

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

## Hypotheses, ranked by likelihood

1. **KeepAlive not set on the TCP socket.** Without `TCP_KEEPIDLE`
   / `TCP_KEEPINTVL`, idle connections die when an intermediate
   NAT/firewall expires its state. Default Linux behavior is
   `tcp_keepalive_time = 7200s` (2h), so this alone doesn't
   explain a 10-second cycle. But if barns' router has a short
   UDP/TCP idle timeout (some consumer routers default to
   30-60s for non-established connections) AND we send no
   keepalive frames, this could fire repeatedly during low
   traffic. The handshake itself looks healthy.
2. **Noise framing layer has a self-timeout we're not aware of.**
   If our Noise wrapper expects a heartbeat frame within N
   seconds and one doesn't arrive (because we're stuck on
   IBD-GetHeaders polling with no other traffic), the wrapper
   could close the stream itself.
3. **Barns' node sends a graceful close that we interpret as
   a network error.** "IO error: unexpected end of file" suggests
   the peer closed cleanly (FIN packet) and we read the EOF.
   Could mean barns has logic that closes peers under some
   condition we're hitting.

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
