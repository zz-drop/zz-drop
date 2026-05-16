//! KDF policy — the Argon2id parameters that any newly-saved or
//! freshly-rotated profile container should use today.
//!
//! For v1 the policy is a single in-tree constant equal to
//! `Argon2idConfig::DEFAULT`. The shape (a plain `Argon2idConfig`)
//! is deliberately the same as what a future server-advertised
//! policy would carry, so swapping to a fetched value is a single
//! call-site change.
//!
//! Rotation rule: a container is "at policy" iff every parameter is
//! >= the policy baseline. Any parameter below the baseline triggers
//! re-encryption on the next successful unlock. Parameters above the
//! baseline are never downgraded — a stronger envelope is always
//! preferable to a weaker one.

use crate::crypto::kdf::Argon2idConfig;

/// Current Argon2id baseline for v1. Equal to `Argon2idConfig::DEFAULT`
/// today; bumping this on a future v1.x raises the floor for every
/// container at the next unlock without any operator action.
pub const POLICY_V1: Argon2idConfig = Argon2idConfig::DEFAULT;

/// Returns `true` when at least one parameter of `current` is below
/// `policy`. Stronger-than-policy containers return `false` (never
/// downgrade).
pub fn needs_rotation(current: &Argon2idConfig, policy: &Argon2idConfig) -> bool {
    current.memory_kib < policy.memory_kib
        || current.iterations < policy.iterations
        || current.parallelism < policy.parallelism
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(m: u32, t: u32, p: u32) -> Argon2idConfig {
        Argon2idConfig {
            memory_kib: m,
            iterations: t,
            parallelism: p,
        }
    }

    #[test]
    fn no_rotation_when_all_equal() {
        let c = Argon2idConfig::DEFAULT;
        assert!(!needs_rotation(&c, &POLICY_V1));
    }

    #[test]
    fn rotation_when_memory_lower() {
        let weak = cfg(65_536, 3, 1);
        assert!(needs_rotation(&weak, &POLICY_V1));
    }

    #[test]
    fn rotation_when_iterations_lower() {
        let weak = cfg(POLICY_V1.memory_kib, 1, 1);
        assert!(needs_rotation(&weak, &POLICY_V1));
    }

    #[test]
    fn rotation_when_parallelism_lower() {
        // Constructing this case requires policy.parallelism > 1.
        // POLICY_V1 has parallelism = 1, so we test against a
        // synthetic policy to exercise the branch.
        let synthetic_policy = cfg(POLICY_V1.memory_kib, POLICY_V1.iterations, 2);
        let weak = cfg(POLICY_V1.memory_kib, POLICY_V1.iterations, 1);
        assert!(needs_rotation(&weak, &synthetic_policy));
    }

    #[test]
    fn no_rotation_when_stronger_than_policy() {
        // Never downgrade an envelope that already exceeds the floor.
        let stronger = cfg(
            POLICY_V1.memory_kib * 2,
            POLICY_V1.iterations + 2,
            POLICY_V1.parallelism,
        );
        assert!(!needs_rotation(&stronger, &POLICY_V1));
    }

    #[test]
    fn no_rotation_when_mixed_above_only() {
        // Memory above floor, iterations equal, parallelism equal.
        let mixed = cfg(
            POLICY_V1.memory_kib + 1024,
            POLICY_V1.iterations,
            POLICY_V1.parallelism,
        );
        assert!(!needs_rotation(&mixed, &POLICY_V1));
    }
}
