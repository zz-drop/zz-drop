# profile passphrase screen

The wizard's last setup step asks the user for a **profile passphrase**
— the secret used to encrypt `profile.zz`. This document covers the
UX, the validation rules, what happens on disk, and the security
properties.

## What this passphrase is (and is not)

- It encrypts the local `profile.zz` envelope (Argon2id KDF →
  XChaCha20-Poly1305 AEAD, see `core/docs/profile-format.md`).
- It is **never** sent to `zz-drop.net`.
- It is **not** the Nextcloud login password.
- It is **not** the Nextcloud "app password" (the wizard collected
  that in the auth screen; that secret lives encrypted *inside*
  `profile.zz` together with the rest of the provider configuration).
- The wizard intentionally does **not** call this a "master
  passphrase" — there is no hierarchy or master/sub structure in v1.
  It is just *the* profile passphrase.

## UX

Two masked inputs, both rendered as `*` characters:

```
> passphrase: ********
  confirm:    ********
  strength: [███ ] good
  ✓ passphrases match
```

Tab cycles between the two fields. Enter triggers save when the
confirmation matches.

## Validation

- Both fields must be non-empty.
- The two fields must match exactly. While they don't, an inline
  message says `✗ passphrases do not match` and Enter is a no-op.

## Strength meter

A four-cell bar driven by `zxcvbn`:

| Score | Label |
|---|---|
| 0 | very weak |
| 1 | weak |
| 2 | fair |
| 3 | good |
| 4 | strong |

The wizard considers a passphrase "weak" if **either**:

- zxcvbn score is `0` or `1`, **or**
- the passphrase is shorter than 12 characters.

When the user presses Enter on a weak passphrase, the screen switches
to a confirmation modal:

```
weak passphrase warning

If someone gets profile.zz, they can try an offline attack.
Recommended: 12+ characters.

Continue anyway?

    y — continue and encrypt the profile
    n — go back to edit the passphrase
```

Pressing `y` proceeds with the weak passphrase; `n` (or `Esc`) returns
to the input fields. There is no minimum-length enforcement beyond
"non-empty" — the user keeps the final decision.

## Saving `profile.zz`

On Enter (or `y` after a weak warning), the screen transitions to
`Encrypting`, the UI repaints with `encrypting profile…`, and the
main loop calls `zz_drop_core::profile::format::save_profile_zz`:

1. CBOR-encode the `PlainProfile`.
2. Argon2id-derive a 32-byte key with the **default** parameters
   (~190 MiB / 3 iterations / 1 lane). Takes 0.5–1 s on a typical
   machine; that's by design.
3. XChaCha20-Poly1305-encrypt the CBOR.
4. Wrap into the v1 JSON envelope.
5. Create the parent directory if missing.
6. Write to `<config-dir>/zz-drop/profile.zz`.
7. Set file mode `0600` on Unix.

`<config-dir>` is whatever `directories::BaseDirs::config_dir()`
returns:

| OS | Path |
|---|---|
| Linux | `~/.config/zz-drop/profile.zz` |
| macOS | `~/Library/Application Support/zz-drop/profile.zz` |

On success the screen shows `✓ profile saved` and the resolved path.
Press Enter to advance to the final `Done` screen.

On failure (encryption error, IO error, no config dir) the screen
shows `✗ save failed: <reason>`. Pressing Enter or Esc returns to
editing.

## Security

- Both fields are masked in render (`*`); their values stay in
  `App::passphrase_input` / `App::confirm_input` only.
- `WizardState::Debug` is hand-rolled and prints
  `WizardState { <redacted> }`; the test
  `debug_does_not_leak_passphrase_input` asserts a leak canary
  cannot appear via `{:?}`.
- The **Nextcloud app password** the wizard collected earlier is
  embedded into the `PlainProfile`, then encrypted into `profile.zz`
  in this step. It is never written to disk in plaintext.
- File mode is enforced to `0600` on Unix.
- No "browser opened" wording at any point.

## Recoverability

There is no recovery if the passphrase is lost. This is the same
property as the public spec in
`core/docs/profile-format.md` and is documented inline on the
screen.

## Test coverage

`tui/tests/passphrase_screen.rs` covers state machine
transitions:

- starts on the passphrase input field
- typing accumulates in the active input
- Tab cycles to the confirm field
- Enter on empty / mismatch is a no-op
- Enter on weak passphrase opens the warning
- `y` / `n` from the warning route correctly
- Enter on a strong long passphrase skips the warning and triggers
  the save request
- `apply_save_done` populates the saved path and stage
- Enter on `Saved` advances to `Done`
- Done screen quits on `q`
- `Debug` redaction holds.

The actual encryption + write round-trip is unit-tested in
`core/tests/profile_crypto.rs`
(`save_profile_zz_round_trip`, `save_profile_zz_sets_0600`,
`load_profile_zz_with_wrong_passphrase_fails_with_aead`,
`load_profile_zz_missing_file_is_io_error`).
