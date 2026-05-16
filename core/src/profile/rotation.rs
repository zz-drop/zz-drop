//! KDF rotation for `ProfileSet` containers.
//!
//! After a successful unlock, if the envelope's stored Argon2id
//! parameters are weaker than [`crate::profile::policy::POLICY_V1`],
//! re-encrypt the container with policy params and a fresh salt,
//! then atomically replace the on-disk file. The agent receives the
//! new `ProfileKek` so subsequent inner mutations encrypt under the
//! new floor.
//!
//! Atomicity: write to `<path>.tmp` with mode `0600`, then `rename`
//! to `<path>`. POSIX rename within the same filesystem is atomic,
//! so a crash before rename leaves the original file intact and the
//! `.tmp` orphan is overwritten on the next attempt.

use std::path::Path;

use crate::crypto::kdf::Argon2idConfig;
use crate::profile::format::{ProfileCryptoError, encrypt_set_with_config};
use crate::profile::policy::needs_rotation;
use crate::profile::set::{ProfileKek, ProfileSet};

/// Outcome of [`rotate_set_if_needed`].
///
/// `Ok(None)`           — envelope already at policy, nothing written.
/// `Ok(Some(new_kek))`  — re-encrypted and replaced on disk; caller
///                        must use `new_kek` for any subsequent
///                        `encrypt_set_with_kek` calls.
/// `Err(_)`             — rotation failed; the original file is
///                        untouched and the existing `ProfileKek`
///                        remains valid. Caller decides whether to
///                        warn and proceed or abort.
pub fn rotate_set_if_needed(
    set: &ProfileSet,
    current_kek: &ProfileKek,
    passphrase: &str,
    path: &Path,
    policy: &Argon2idConfig,
) -> Result<Option<ProfileKek>, ProfileCryptoError> {
    if !needs_rotation(current_kek.kdf_config(), policy) {
        return Ok(None);
    }

    // Re-derive + re-encrypt with policy params and a fresh salt.
    // `encrypt_set_with_config` already generates a new random salt
    // and nonce internally.
    let (envelope, new_kek) = encrypt_set_with_config(set, passphrase, policy)?;

    atomic_write_0600(path, envelope.as_bytes())?;

    Ok(Some(new_kek))
}

/// Write `bytes` to `path` atomically with mode `0600`. Writes to
/// `<path>.tmp` first, then renames over the target — so a partial
/// write never corrupts the existing file.
fn atomic_write_0600(path: &Path, bytes: &[u8]) -> Result<(), ProfileCryptoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| ProfileCryptoError::Io)?;
    }

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp_path = std::path::PathBuf::from(tmp);

    std::fs::write(&tmp_path, bytes).map_err(|_| ProfileCryptoError::Io)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&tmp_path, perms).map_err(|_| ProfileCryptoError::Io)?;
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        // Best-effort cleanup; ignore the result because the rename
        // failure is already the meaningful error to report.
        let _ = std::fs::remove_file(&tmp_path);
        let _ = e;
        ProfileCryptoError::Io
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::profile::format::{decrypt_set, save_set_zz_with_config};
    use crate::profile::set::ProfileSet;

    /// Cheap Argon2id config — keeps the tests fast. The exact
    /// numbers don't matter, only the relation to the synthetic
    /// policy used in each test.
    fn cheap() -> Argon2idConfig {
        Argon2idConfig {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        }
    }

    /// Synthetic policy stronger than `cheap()` so rotation triggers
    /// without paying the cost of `POLICY_V1` (190 MiB / 3 iters)
    /// in every test.
    fn synthetic_policy() -> Argon2idConfig {
        Argon2idConfig {
            memory_kib: 2048,
            iterations: 2,
            parallelism: 1,
        }
    }

    fn empty_set() -> ProfileSet {
        ProfileSet::new()
    }

    #[test]
    fn no_op_when_at_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.zz");

        let cfg = cheap();
        save_set_zz_with_config(&empty_set(), "pw", &path, &cfg).unwrap();
        let (set, kek) = decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();

        // Policy = same as on-disk → no rotation, no write.
        let before = std::fs::read(&path).unwrap();
        let outcome = rotate_set_if_needed(&set, &kek, "pw", &path, &cfg).unwrap();
        let after = std::fs::read(&path).unwrap();

        assert!(outcome.is_none(), "should be no-op");
        assert_eq!(before, after, "on-disk envelope must be unchanged");
    }

    #[test]
    fn no_op_when_stronger_than_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.zz");

        let stronger = synthetic_policy();
        let weaker_policy = cheap();
        save_set_zz_with_config(&empty_set(), "pw", &path, &stronger).unwrap();
        let (set, kek) = decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();

        let before = std::fs::read(&path).unwrap();
        let outcome = rotate_set_if_needed(&set, &kek, "pw", &path, &weaker_policy).unwrap();
        let after = std::fs::read(&path).unwrap();

        assert!(outcome.is_none(), "must never downgrade");
        assert_eq!(before, after);
    }

    #[test]
    fn rotates_and_writes_new_envelope() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.zz");

        let weak = cheap();
        let policy = synthetic_policy();
        save_set_zz_with_config(&empty_set(), "pw", &path, &weak).unwrap();
        let (set, kek_old) =
            decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();
        let old_envelope = std::fs::read(&path).unwrap();

        let outcome = rotate_set_if_needed(&set, &kek_old, "pw", &path, &policy)
            .unwrap()
            .expect("rotation should fire");

        // The new KEK reports the policy params.
        assert_eq!(outcome.kdf_config(), &policy);

        // The envelope on disk has actually been replaced.
        let new_envelope = std::fs::read(&path).unwrap();
        assert_ne!(old_envelope, new_envelope);

        // The new envelope decrypts cleanly with the same passphrase,
        // and reports the new params.
        let (set2, kek2) = decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();
        assert_eq!(kek2.kdf_config(), &policy);
        assert_eq!(set2.profiles.len(), set.profiles.len());
    }

    #[test]
    fn rotation_uses_fresh_salt() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.zz");

        save_set_zz_with_config(&empty_set(), "pw", &path, &cheap()).unwrap();
        let (set, kek_old) =
            decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();
        let old_salt = kek_old.salt().to_vec();

        let new_kek = rotate_set_if_needed(&set, &kek_old, "pw", &path, &synthetic_policy())
            .unwrap()
            .expect("rotation should fire");

        assert_ne!(new_kek.salt().to_vec(), old_salt, "salt must be refreshed");
    }

    #[test]
    fn idempotent_on_second_call() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.zz");

        save_set_zz_with_config(&empty_set(), "pw", &path, &cheap()).unwrap();
        let (set, kek) = decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();
        let policy = synthetic_policy();

        let new_kek = rotate_set_if_needed(&set, &kek, "pw", &path, &policy)
            .unwrap()
            .expect("first call rotates");

        // Re-load with the rotated envelope and rotate again — must
        // be a no-op now that on-disk params == policy.
        let (set2, kek2) = decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();
        assert_eq!(kek2.kdf_config(), &policy);
        let _ = new_kek;
        let second = rotate_set_if_needed(&set2, &kek2, "pw", &path, &policy).unwrap();
        assert!(second.is_none(), "second call must be a no-op");
    }

    #[test]
    fn tmp_file_left_behind_does_not_break_rotation() {
        // Simulate a prior crashed rotation by pre-creating .tmp with
        // garbage. The next rotation must overwrite it cleanly.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.zz");

        save_set_zz_with_config(&empty_set(), "pw", &path, &cheap()).unwrap();
        let tmp_path = {
            let mut s = path.as_os_str().to_owned();
            s.push(".tmp");
            std::path::PathBuf::from(s)
        };
        std::fs::write(&tmp_path, b"garbage from prior crash").unwrap();

        let (set, kek) = decrypt_set(&std::fs::read_to_string(&path).unwrap(), "pw").unwrap();
        let new_kek = rotate_set_if_needed(&set, &kek, "pw", &path, &synthetic_policy())
            .unwrap()
            .expect("rotation should still succeed");
        assert_eq!(new_kek.kdf_config(), &synthetic_policy());

        // After rotation .tmp must not exist (rename consumed it).
        assert!(
            !tmp_path.exists(),
            ".tmp must be gone after successful rotation"
        );
    }

    #[test]
    fn original_file_intact_on_io_failure() {
        // Point `path` at a directory that exists but where the
        // parent is read-only — write to .tmp will fail. The
        // pre-existing file at a separate path must remain intact.
        // We simulate by: write a valid envelope to a path, then
        // attempt rotation to a path whose parent doesn't exist
        // AND can't be created.
        //
        // Actually, atomic_write_0600 creates parents via
        // create_dir_all, so we need a path under a file (not a
        // dir) to force the failure.
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"i am a file, not a dir").unwrap();
        // path is "blocker/p.zz" — create_dir_all on parent
        // ("blocker") fails because it's a file.
        let bad_path = blocker.join("p.zz");
        let good_path = tmp.path().join("p.zz");
        save_set_zz_with_config(&empty_set(), "pw", &good_path, &cheap()).unwrap();
        let before = std::fs::read(&good_path).unwrap();

        let (set, kek) =
            decrypt_set(&std::fs::read_to_string(&good_path).unwrap(), "pw").unwrap();
        let res =
            rotate_set_if_needed(&set, &kek, "pw", &bad_path, &synthetic_policy());
        assert!(res.is_err(), "rotation must fail on unwritable path");

        // The unrelated good file is intact.
        let after = std::fs::read(&good_path).unwrap();
        assert_eq!(before, after);
    }
}
