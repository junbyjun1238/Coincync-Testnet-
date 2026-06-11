# DNS-seed fallback layers

v1.0.14 expanded the bootstrap-discovery path. A fresh node now has
**three** layers it tries in order before giving up:

```
   ┌─────────────────────────────────────────────────────────┐
   │ 1. DNS                                                   │
   │    Resolve seed1.coincync.network, seed2.…, seed3.…      │
   │    A + AAAA records on two independent providers.        │
   └─────────────────────────────────────────────────────────┘
                              ↓ (all failed)
   ┌─────────────────────────────────────────────────────────┐
   │ 2. Hardcoded fallback                                   │
   │    Compiled-in list from fleet.toml. Frozen at build.    │
   └─────────────────────────────────────────────────────────┘
                              ↓ (empty / all unreachable)
   ┌─────────────────────────────────────────────────────────┐
   │ 3. HTTPS .well-known                                    │
   │    Fetch the published seed list from a stable URL.      │
   │    Network-specific, JSON, served via TLS.               │
   └─────────────────────────────────────────────────────────┘
```

This doc covers layer 3 — the new HTTPS fallback added in v1.0.14.
Layer 1 is OS resolver (or DNS-over-TCP through SOCKS5 when Tor is
active). Layer 2 lives in `src/network/dns_seeds.rs::*_FALLBACK`
arrays (derived from `fleet.toml`, see
`docs/operations/incidents/2026-06-06-fleet-recovery.md` for why
that derivation matters).

## Why a third layer

The 2026-06-06 fleet recovery surfaced the failure mode this layer
covers: an old binary in the wild whose hardcoded fallback table
references decommissioned boxes. DNS resolves fine but the
hardcoded IPs no longer route. The binary has no escape — it can't
update itself at runtime, and asking the operator to rebuild is a
bar the average tester won't clear.

The HTTPS layer threads that needle. The published file is
regenerated whenever `fleet.toml` changes (via the
`gen-manifests.py` pipeline shipped in v1.0.13). Any binary —
even a year-old one — can reach the current seed set via a
single HTTPS GET, **as long as it can reach the public internet
and resolve a single canonical hostname**.

The single hostname is the trust anchor. We pay the operational
cost of keeping it pointing at a maintained origin in exchange for
the universal-fallback property.

## Endpoint URLs

```
Testnet: https://coincync.network/.well-known/coincync-seeds-testnet.json
Mainnet: https://coincync.network/.well-known/coincync-seeds-mainnet.json
Regtest: not applicable (no public seeds)
```

The path follows RFC 8615 `/.well-known/` conventions so future
discovery layers (light-wallet config, explorer URLs, etc.) can
sibling-mount under the same prefix.

## JSON schema

```json
{
  "schema_version": "1.0",
  "network":        "testnet",
  "generated":      "2026-06-11T20:14:33Z",
  "peers": [
    "66.135.23.193:28080",
    "140.82.57.168:28080",
    "207.148.111.76:28080"
  ]
}
```

Required fields:

| Field | Type | Description |
| --- | --- | --- |
| `schema_version` | string | Major.minor. Loader rejects a mismatch on major. |
| `network` | string | `"mainnet"` / `"testnet"`. Must match the requesting node's network. |
| `peers` | array of strings | Each entry is `"ip:port"` or `"ip"` (defaults to network's P2P port). |

Optional:

| Field | Type | Description |
| --- | --- | --- |
| `generated` | RFC 3339 timestamp | When the file was emitted; for operator diagnostics. |
| `notes` | string | Human-readable comments. Loader ignores. |

Peers can be IPv4 or IPv6. IPv6 in `"ip:port"` form uses the bracketed
form: `"[2001:db8::1]:28080"`.

## Privacy properties

The fallback only runs when DNS and hardcoded layers BOTH fail, which
is rare. Even so:

- **No fallback over Tor.** When a SOCKS5 proxy is active, this layer
  is skipped. The TLS handshake leaks the client IP to the HTTPS
  origin; if the user is on Tor they explicitly opted into avoiding
  that. The privacy posture is to fail closed: better to have no
  peers than to leak.
- **No request payload.** The HTTP GET carries no identifying
  cookies / auth / fingerprintable headers. The `User-Agent` is the
  bare `coincync-node/<version>` string — same shape every fresh
  install emits, so it's not differentiating.
- **No origin-side stickiness.** The origin doesn't issue
  Set-Cookie, doesn't serve user-specific content. Refresh on every
  call.

## Operator setup

The published file is generated from `fleet.toml` so the file's
contents stay in lockstep with the in-binary fallback list. The
v1.0.13 manifest-generator pipeline gets a new target in a
follow-up commit:

```bash
python3 scripts/gen-manifests.py well-known
# writes deploy/landing/well-known/coincync-seeds-{mainnet,testnet}.json
```

Operator deployment then rsyncs `deploy/landing/well-known/*` to the
canonical landing host's `/var/www/.well-known/` directory. The
landing host already serves the `coincync.network` apex over TLS.

The CI `fleet-manifest-drift` lane (added in v1.0.13) is extended
in the same follow-up so a hand-edit of the published JSON
gets caught the same way a hand-edit of `prometheus.yml` does.

## Code location

- `src/network/dns_seeds.rs::fetch_https_seeds()` — the loader
- `src/network/dns_seeds.rs::https_seeds_url()` — network-specific URL
- `src/network/dns_seeds.rs::resolve_seeds_inner()` — call site (after layer 2)

## Failure modes the layer DOES NOT cover

This layer covers "DNS broken + hardcoded list stale". It does not
cover:

- TLS hostname blocked by the network. A captive-portal or
  state-firewall scenario. Falls through to no-peers; operator must
  intervene.
- Origin server compromised. The HTTPS origin is a single point of
  trust. The published file is signed in a future enhancement (CIP
  to be drafted in v1.0.15) so a compromised origin can't redirect
  honest nodes to attacker-controlled peers undetected.
- An attacker DNS-poisons the well-known hostname itself.
  Mitigated by HTTPS — TLS hostname verification catches this if
  the CA chain holds.

These are followups, tracked in the v1.0.14 ROADMAP item:
"DNS-seed resilience".
