# Wallet memos — encrypt + decrypt guide

Memos let a sender attach a short plaintext message to a transaction
that **only the recipient can decrypt**. They're encrypted on-chain
using the recipient's view key — bystanders see encrypted bytes,
the recipient sees plaintext.

This guide covers the user-facing flow on the v1.0.11 CLI wallet
(`coincync-wallet`).

## What memos are

A memo is **up to 256 bytes** of arbitrary plaintext (UTF-8 strings
fit cleanly). Common uses:

- Invoice IDs (`inv-2026-06-12-001`)
- Human-readable notes (`rent june`, `for coffee`)
- Test markers (the cycle 02 test used `cycle02-test-tx-from-ghostrider1092`)

The bytes are encrypted to the recipient's view-public-key using an
ECDH-derived symmetric key (ChaCha20-Poly1305). The encryption is
non-malleable — the chain can't tamper with the bytes without the
recipient detecting it. Only the holder of the matching view-secret
(i.e., the recipient's wallet) can decrypt.

## Privacy properties

- **Encrypted at rest on the chain.** Anyone running `get_block` or
  `scan` sees the encrypted bytes but cannot decrypt without the view
  key.
- **Decryption is recipient-only.** Even the sender can't decrypt
  their own sent memo from chain after submission — the sender knows
  the plaintext because they typed it, not because they could read
  the encrypted form back.
- **Memo presence is visible.** A transaction either has or doesn't
  have a memo; that bit is observable. The CONTENTS are not.
- **Memo length is bucketed.** The fixed 256-byte slot means a 4-byte
  memo and a 256-byte memo look identical on-chain — no leakage of
  message size.

## Sending — `send --memo`

```bash
coincync-wallet \
  --network testnet \
  --wallet /path/to/your.wallet \
  --node http://127.0.0.1:28081 \
  send \
  --to-spend  <recipient-spend-pubkey-64-hex> \
  --to-view   <recipient-view-pubkey-64-hex> \
  --amount    <atomic-units> \
  --memo      "your message here" \
  --password  -
```

The memo is embedded on the **first recipient output** of the
transaction. Wallets with the matching view-secret will see it on
scan; others won't.

If you omit `--memo`, no memo is attached and the output's memo
slot is filled with random bytes that look indistinguishable from
encrypted text — privacy posture is symmetric whether or not you
intended a memo.

## Receiving — `show-memo`

To decrypt a memo someone sent you, two steps:

### Step 1: Scan the chain so the wallet sees the new output

```bash
coincync-wallet \
  --network testnet \
  --wallet /path/to/your.wallet \
  --node http://127.0.0.1:28081 \
  scan \
  --password -
```

Output looks like:

```text
Scanned:        N blocks
Found outputs:  M
Tip:            height=...
Balance total:  X.YYYY CYNC
UTXO count:     K
UTXOs persisted to "/path/to/your.wallet.utxos"
```

The persisted `<wallet>.utxos` sidecar contains every owned UTXO in
the order it was discovered. **Each has an index starting at 0.**

### Step 2: Find the UTXO index of the memo-carrying output

If you know it's the most recent receive, it's the highest index. If
you're not sure which one, you can list them via the wallet's RPC
(some interfaces expose `list_utxos`), or just try indices in
descending order until you find one with a memo.

### Step 3: Decrypt + print

```bash
coincync-wallet \
  --network testnet \
  --wallet /path/to/your.wallet \
  --password - \
  show-memo \
  --utxo-index <N>
```

(Replace `<N>` with the UTXO index from step 1.)

Output if a memo is present:

```text
Memo (UTXO #N):
  cycle02-test-tx-from-ghostrider1092
```

Output if no memo (random bytes) or if you picked the wrong UTXO:

```text
Memo (UTXO #N): (no memo / random bytes)
```

## Worked example — the cycle 02 test send

ghostrider1092 sent barns 10 CYNC on testnet at block ~h=26-30 with
memo `cycle02-test-tx-from-ghostrider1092`. To verify on barns'
side:

```bash
# 1. Make sure your local node is synced past h=30
coincync-node --network testnet status
# (look at the Height value — should be h≥30 after barns' rig has mined)

# 2. Scan the wallet
coincync-wallet \
  --network testnet \
  --wallet ~/.coincync/wallets/default.wallet \
  --node http://127.0.0.1:28081 \
  scan \
  --password -
# (note: UTXO count should have grown by 1 or 2 after the receive)

# 3. Find the new UTXO index — it'll be one of the highest indices
# 4. Decrypt the memo
coincync-wallet \
  --network testnet \
  --wallet ~/.coincync/wallets/default.wallet \
  --password - \
  show-memo \
  --utxo-index <NEW-INDEX>
```

Expected output:

```text
Memo (UTXO #N):
  cycle02-test-tx-from-ghostrider1092
```

That confirms the encrypted message round-tripped through the
chain → mined → propagated → received → decrypted with the original
plaintext intact. The chain saw only encrypted bytes the entire time.

## Privacy checklist for memo use

- ✅ Safe to memo invoice IDs, transaction notes, recipient-only
  context.
- ❌ NEVER put recipient identifying info in plaintext memos. The
  view-key holder is whoever you sent to — but if the wallet file is
  later compromised, the memo plaintext leaks.
- ❌ Don't put secrets in memos (passwords, key material, mnemonic
  fragments). Same compromise reasoning.
- ⚠️ If you forward a memo to a third party (e.g., posting it as
  proof-of-payment), you've voluntarily disclosed the plaintext. That's
  fine if intended; just be aware.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `show-memo` errors "utxo-index out of range" | Scan hasn't seen the receive yet | Re-run `scan` after a few more blocks |
| Memo prints as random bytes / garbage | Wrong UTXO index (you decrypted a different output that had random fill) | Try a different index |
| `show-memo` errors "password required" | Stdin not piped | Use `--password -` and pipe the password, or `--password "pw"` directly |
| Memo appears truncated | Sender originally sent < 256 bytes — that's normal | Confirm via sender |

## See also

- The send command reference in `docs/COMMANDS.md`
- Privacy architecture overview in `docs/architecture/PRIVACY.md` — covers the ECDH derivation + ChaCha20 details
- The faucet flow (testnet) which uses `--split-output` to give
  first-time recipients two spendable UTXOs in one tx
