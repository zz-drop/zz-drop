//! Standalone decrypt probe — read a `profiles-*.zz` envelope and
//! attempt decryption with a passphrase fed via the `ZZ_DROP_PASS`
//! env var. Stays in `examples/` (not the binary tree) so it
//! can't end up shipped accidentally.
//!
//! Usage:
//!   ZZ_DROP_PASS='!' cargo run --release --example decrypt_check \
//!     -- "/Users/me/Library/Application Support/zz-drop/profiles-local.zz"

use zz_drop_core::profile::format::decrypt_set;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: decrypt_check <profiles-local.zz path>");
    let pass = std::env::var("ZZ_DROP_PASS").expect("set ZZ_DROP_PASS");
    let envelope = std::fs::read_to_string(&path).expect("read envelope");
    println!("envelope len: {}", envelope.len());
    println!("pass len: {}", pass.len());
    println!("pass bytes: {:?}", pass.as_bytes());
    match decrypt_set(&envelope, &pass) {
        Ok((set, kek)) => {
            println!(
                "✓ decrypted: {} inner profile(s), kdf m={} t={} p={}",
                set.profiles.len(),
                kek.kdf_config().memory_kib,
                kek.kdf_config().iterations,
                kek.kdf_config().parallelism,
            );
            for p in &set.profiles {
                println!("  - alias={} target={}", p.alias, p.default_target);
            }
        }
        Err(e) => {
            println!("✗ decrypt failed: {e:?}");
            std::process::exit(2);
        }
    }
}
