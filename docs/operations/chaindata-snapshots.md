# Chaindata snapshots — operator + user guide

v1.0.14 ships a `coincync-node snapshot-fetch <URL>` subcommand that
downloads, verifies, and extracts a published chaindata tarball as
an IBD speedup. A fresh node that boots from a snapshot skips
validating every block from genesis — for testnet that's tens of
thousands of blocks, on a fresh laptop ~30 minutes saved; on mainnet
post-GA it'll scale into hours.

## Trust model

The URL host is the trust anchor.

**What the verification catches:**

- Tampering in transit (CDN cache poisoning, mid-path MitM) — caught
  by SHA256 against the sidecar file. The fetcher refuses to extract
  on mismatch.
- Wrong network (mainnet tarball downloaded for testnet) — caught by
  the metadata sidecar (see schema below).
- Truncated downloads (partial bytes interpreted as a valid archive) —
  caught because partial tar/gzip data fails to decode.

**What the verification does NOT catch:**

- A malicious URL host serving a real but non-canonical chain. The
  signature math holds, the SHA matches, but the chain itself is a
  fork that no honest peer agrees with. Post-extraction, the node
  validates every NEW block against consensus rules — if the snapshot
  landed on a non-canonical fork, the node will detect the mismatch
  the moment its peer mesh disagrees, and will reorganize. Worst case:
  a small IBD window's worth of wasted time.

This trust posture matches Bitcoin Core's `assumevalid` and Monero's
fast-sync. Snapshots are a SPEED enhancement, not a SAFETY
weakening; canonical-chain enforcement remains at the consensus
validator below.

## URL conventions

Published snapshots live under the apex host:

```
https://coincync.network/snapshots/<network>/<height>.tar.gz
https://coincync.network/snapshots/<network>/<height>.tar.gz.sha256
https://coincync.network/snapshots/<network>/<height>.json   # metadata
https://coincync.network/snapshots/<network>/latest -> 302 to <height>.tar.gz
```

The `latest` indirection lets a fresh-install script always grab the
most recent snapshot:

```bash
coincync-node snapshot-fetch https://coincync.network/snapshots/testnet/latest
```

## Metadata sidecar schema

```json
{
  "schema_version": "1.0",
  "network":        "testnet",
  "tip_height":     14328,
  "tip_hash":       "0x…",
  "build_commit":   "<git sha>",
  "generated":      "2026-06-11T20:14:33Z",
  "tarball_size":   194812374,
  "tarball_sha256": "abc123…"
}
```

Required for production-published snapshots. The fetcher in v1.0.14
doesn't yet parse this file (it only checks the `.sha256` sidecar
for integrity); CI follow-up adds a metadata-parse step that asserts
network + schema_version match the requesting node's expectation.
Tracked as a v1.0.15 item.

## Creating a snapshot (operator side)

The v1.0.14 release ships only the fetch subcommand. Creation is a
manual `tar` + `sha256sum` invocation:

```bash
# 1. Stop the source node.
sudo systemctl stop coincync-node

# 2. Tar the chaindata directory.
tar -czf /tmp/snapshot.tar.gz -C /var/lib/coincync testnet

# 3. Compute SHA256 (sha256sum format — first token).
sha256sum /tmp/snapshot.tar.gz > /tmp/snapshot.tar.gz.sha256

# 4. (Optional but recommended) write the metadata file.
cat > /tmp/snapshot.json <<EOF
{
  "schema_version": "1.0",
  "network":        "testnet",
  "tip_height":     $(coincync-node --network testnet status | grep Height | awk '{print $2}'),
  "tip_hash":       "$(coincync-node --network testnet status | grep 'Tip hash' | awk '{print $3}')",
  "build_commit":   "$(coincync-node --version | awk '{print $2}')",
  "generated":      "$(date -u +%FT%TZ)",
  "tarball_size":   $(stat -c %s /tmp/snapshot.tar.gz),
  "tarball_sha256": "$(cat /tmp/snapshot.tar.gz.sha256 | awk '{print $1}')"
}
EOF

# 5. Restart the source node and serve the files.
sudo systemctl start coincync-node
rsync /tmp/snapshot.* <landing-host>:/var/www/snapshots/testnet/${HEIGHT}.tar.gz
# Update the latest symlink to point at the new file.
```

`coincync-node snapshot-create` is a v1.0.15 follow-up that
automates steps 2-4 in-process (lets the node create a snapshot
without stopping; ratchets via a checkpoint).

## Using a snapshot (user side)

```bash
# Fresh data dir.
rm -rf ~/.coincync

# Fetch + extract.
coincync-node snapshot-fetch \
  https://coincync.network/snapshots/testnet/latest

# Start the node — it picks up at the snapshot's tip and resumes
# normal sync.
coincync-node --network testnet start
```

If the data dir already has chaindata, the fetcher refuses to
overwrite without `--force`:

```bash
coincync-node snapshot-fetch \
  --force \
  https://coincync.network/snapshots/testnet/latest
```

`--force` wipes the existing data dir before extraction. Use
carefully — there is no in-place "merge a partial snapshot with
existing chaindata" path.

`--no-verify` skips the SHA256 check. Use only when the sidecar
is known unavailable and you've verified the tarball out-of-band
(e.g. via gpg signature in a separate channel). The v1.0.14
fetcher does NOT yet support GPG signatures — that's a follow-up.

## Failure modes

| Symptom | Cause | Fix |
| --- | --- | --- |
| `HTTP 404 from <url>` | Snapshot rotated; `latest` not redirecting | Try the parent directory listing, or fall back to a numbered URL |
| `SHA256 mismatch` | Tampered or stale sidecar | Re-download; if persistent, alert the operator |
| `Failed to extract tarball: ...` | Disk full or permissions | Check `df` + `chmod` on the data dir |
| Node won't start after extraction | Chaindata format mismatch | The published snapshot was built against a different binary version; check build_commit in metadata and use a matching binary |

## Roadmap follow-ups

- v1.0.15: `coincync-node snapshot create` (in-process snapshot
  creation without stopping the node)
- v1.0.15: metadata-sidecar parse + network/schema-version assertion
  at fetch time
- v1.0.15: gpg signature on the sidecar, verified at fetch
- v1.0.16: HTTP Range-based resumable downloads for slow links
