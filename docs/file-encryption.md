# File content encryption — `.zzd` blob format

This spec covers the on-the-wire and on-the-cloud format for
files encrypted client-side by zz-drop before upload to a
provider.

**Status:** design frozen for v1.1 (post-v1.0 compatibility
freeze). The format is forward-compatible with v1.0
`profile.zz` envelopes — the encryption-related fields in the
profile payload are optional and absent on a v1.0 profile.
Implementation lands in v1.1 in stages: core crypto, CLI
integration, TUI consent flow, standalone disaster-recovery
tool.

See also:

- [`security-model.md`](security-model.md) § "File content
  encryption (per-provider)" for the threat model.
- [`profile-format.md`](profile-format.md) for the `profile.zz`
  payload schema extensions.

## Scope

Applies only to providers configured with
`encryption.kind = client_side` or
`encryption.kind = native_untrusted_overlay` in the profile
payload. Providers in `native_trusted` or `native_optional`
(user said "yes") modes do not produce `.zzd` files: zz-drop
uploads plaintext and trusts the provider.

## Filename

The plaintext file name `<name>` is uploaded as `<name>.zzd`.
No filename encryption. No path mangling. Subdirectories
preserved as-is.

When `zz d <name>` is invoked, the resolver attempts both
`<name>` and `<name>.zzd` against the remote listing; the
`.zzd` entry takes precedence on tie. See
[`commands.md`](commands.md) § "Resolution rules".

## Magic header (16 bytes)

| Offset | Length | Value | Meaning |
|--------|--------|-------|---------|
| 0      | 4      | `5A 5A 44 01` (`ZZD\x01`) | magic + format version |
| 4      | 1      | `01` | algorithm: XChaCha20-Poly1305 |
| 5      | 1      | `00` | reserved (future flags) |
| 6      | 2      | big-endian `u16` | chunk size in KiB (= 64) |
| 8      | 8      | random | nonce base |

Total magic header: 16 bytes. No length prefix follows; chunk
boundaries are implicit and the final chunk is signaled by AAD
on the AEAD tag (see below).

## Chunk format

Plaintext is split into chunks of exactly 64 KiB
(= `chunk_size_kib * 1024` bytes). The final chunk may be
shorter (and may be empty for a zero-byte input).

Each chunk is encrypted with XChaCha20-Poly1305:

- **Key:** the 32-byte DMK (per-provider data master key).
- **Nonce:** 24 bytes = 8-byte `nonce_base` ‖ 16-byte
  big-endian `u128` chunk index, starting at 0.
- **Associated Data (AAD):** 1 byte — `0x00` for non-final
  chunks, `0x01` for the final chunk. This binds the
  final-chunk flag to the AEAD tag, so truncation attacks
  (dropping the last chunk to make a non-final look final, or
  vice versa) are detected.
- **Output:** ciphertext (= plaintext length) + 16-byte
  Poly1305 tag.

For the final chunk: AAD = `0x01`, plaintext length ≤ 64 KiB,
output length = plaintext length + 16-byte tag.

A zero-byte file produces a single final chunk with empty
plaintext: 16-byte tag only.

The on-disk `.zzd` blob is therefore:

```
[16 B magic header][chunk_0 ct ‖ tag][chunk_1 ct ‖ tag] ... [final chunk ct ‖ tag]
```

## Pipeline (upload, `s` family)

For each file in `s` / `sa` / `sar` / `se` / `sare` /
`sarex`, etc.:

```
plaintext source
  → [if x] tar-stream (only if a/r grouping requires it)
  → [if x] zstd-stream (level from profile, default 3)
  → [if encryption applies for this provider]
      magic header + chunked AEAD with per-provider DMK
  → upload as <name>.zzd  (or, for sarx, <archive>.tar.zst.zzd)
```

Compression always precedes encryption: ciphertext is
high-entropy, compressing it after is futile. The pipeline
order is deterministic — the modifier set is parsed but the
order of operations is not user-configurable.

"Encryption applies" means:

- provider is `ClientSide` and the profile-level toggle is on,
  OR
- provider is `ClientSide` and the user wrote `e` explicitly,
  OR
- provider is `NativeUntrusted`, regardless of toggle, OR
- provider is `NativeOptional` and the user answered "no" or
  "don't know" to the runtime question at setup.

## Pipeline (download, `d`)

```
remote blob
  → read first 4 bytes
  → if = ZZD\x01: AEAD-decrypt with DMK from per-provider
                  config (auto-detect chunk size from header)
  → if not: pass through (legacy plaintext, graceful coexistence)
  → if [decrypted output starts with zstd magic 28 B5 2F FD]:
      zstd-stream-decompress
  → if [now starts with tar ustar magic at offset 257]:
      tar-stream-extract
  → write to local target
```

The download pipeline is auto-detecting; there is no `de` /
`dx` modifier. `d` is symmetric and free of modifiers — the
operator does not need to remember whether a given remote file
was encrypted or compressed: the blob declares its own shape.

## DMK generation and storage

A DMK is 32 random bytes from the OS RNG (`getrandom` on
Linux, `SecRandomCopyBytes` on macOS — implementation detail).
Generated when the user enables encryption for a specific
provider, either:

- at profile setup (TUI live-test-upload step generates the
  DMK that is then promoted to the saved profile), or
- when the user toggles encryption on for an existing provider
  via `zz c → Account → Encryption`.

The DMK is stored exclusively inside the encrypted plaintext
payload of `profile.zz`. It is never written to disk in
plaintext, never sent to any server, never logged.

When the agent unlocks the profile, the DMK lives in
`PlainProfile` in RAM, subject to the same TTL / lock / wipe
rules as the rest of the profile (see
[`agent-protocol.md`](agent-protocol.md)).

## Recovery key

If the user generated a BIP39 24-word recovery key at setup,
the profile contains a `recovery_wrap` field:

```json
{
  "recovery_wrap": {
    "kdf": {
      "name": "argon2id",
      "memory_kib": 194560,
      "iterations": 3,
      "parallelism": 1,
      "salt": "base64..."
    },
    "cipher": {
      "name": "xchacha20poly1305",
      "nonce": "base64..."
    },
    "ciphertext": "base64..."
  }
}
```

The plaintext of `recovery_wrap.ciphertext` is the CBOR
serialization of `[(provider_id, dmk), ...]` — every DMK in
the profile, indexed by provider id.

The KEK is derived from the BIP39 mnemonic by:

1. BIP39 standard seed derivation (PBKDF2-HMAC-SHA512, 2048
   iterations, salt = `"mnemonic"` — v1.1 does not expose a
   BIP39 passphrase to the user, so the salt is the constant
   string).
2. Truncate the 64-byte seed to 32 bytes for the Argon2id
   input.
3. Argon2id with the parameters in `recovery_wrap.kdf`
   produces the 32-byte KEK.

This is more conservative than strictly necessary (Argon2id on
top of an already-strong BIP39 seed) but keeps the same KDF
model as the profile passphrase path, simplifying review.

When a new provider with `ClientSide` encryption is added to
the profile after recovery-key generation, its new DMK is
re-wrapped under the existing recovery KEK. The mnemonic does
not need to be re-shown to the user.

## Test vectors

The implementation ships test vectors under
`core/tests/vectors/` once v1.1 lands:

- `v1_passphrase_known.profile.zz` + plaintext passphrase in
  a sibling text file (`v1_passphrase.txt`).
- `v1_recovery_known.bip39` mnemonic that decrypts the same
  profile.
- Three sample files of sizes (12 B, 64 KiB exactly, 192 KiB)
  with their corresponding `.zzd` outputs and known DMK.

Any third-party implementation must produce identical
plaintext when decrypting these and identical ciphertext when
encrypting with the same DMK and `nonce_base`.

## Coexistence with plaintext files

A user may have a mix of plaintext (legacy or `e`-skipped)
and `.zzd` (encrypted) files on the same provider. The
download pipeline handles both transparently because the
magic header check is the first step.

The configuration TUI's per-profile manage screen shows the
remote names with their suffixes (`.zst`, `.tar.zst`, future
`.zzd`) so the operator can match what's on the cloud.

## Non-goals (reaffirmed)

- **Filename encryption.** The cloud sees `<name>.zzd` plain.
- **Padding.** Ciphertext length leaks plaintext length
  within 16 bytes per chunk + final-chunk slack.
- **Random-access reads inside a `.zzd` blob.** The format is
  streaming; partial reads require decrypting from the start
  to the desired offset.
- **DMK rotation as a built-in command.** Manual workaround
  documented under disaster-recovery in the CLI repo.
- **Multi-device coordinated key management.** The same
  `profile.zz` on two machines via `zz z <alias>` is
  sufficient for v1.x.

## Future extensions (not v1.x)

- Format version `\x02` for envelope v2 if rotation, multi-key,
  or random-access become required.
- Optional padding flag in the magic header reserved byte.
- Post-quantum AEAD when the standard stabilizes.
- Per-file DMK derived via HKDF from the master DMK, for
  forward secrecy on per-file granularity.
