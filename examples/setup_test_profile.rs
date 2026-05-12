//! Build a single-Nextcloud `ProfileSet` and write it as
//! `profiles-local.zz` under the user's config dir. Used by the
//! decrypt-bug investigation — pairs with `decrypt_check`.
//!
//! Usage:
//!   ZZ_DROP_PASS='!' cargo run --release --example setup_test_profile -- \
//!     <server_url> <username> <app_password> <remote_root> <alias>

use std::time::SystemTime;

use zz_drop_core::config::{PathOverrides, discover_paths};
use zz_drop_core::profile::format::save_set_zz;
use zz_drop_core::providers::nextcloud::types::{NextcloudAuth, NextcloudProfile};
use zz_drop_core::{
    CollisionPolicy, PlainProfile, ProfileSet, ProfileSettings, ProviderProfile,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let server_url = args.next().expect("server_url");
    let username = args.next().expect("username");
    let app_pass = args.next().expect("app_password");
    let remote_root = args.next().expect("remote_root");
    let alias = args.next().expect("alias");
    let pass = std::env::var("ZZ_DROP_PASS").expect("set ZZ_DROP_PASS");

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let timestamp = format!("epoch:{now}");

    let nc = NextcloudProfile {
        server_url,
        username,
        auth: NextcloudAuth::AppPassword { secret: app_pass },
        remote_root,
    };
    let profile = PlainProfile {
        profile_version: 1,
        profile_id: format!("local-{now}"),
        alias: alias.clone(),
        default_target: "nextcloud".into(),
        providers: vec![ProviderProfile::Nextcloud(nc)],
        collision_policy: CollisionPolicy::Rename,
        settings: ProfileSettings::default(),
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    let set = ProfileSet::with_profile(profile);

    let paths = discover_paths(0, &PathOverrides::default()).expect("discover_paths");
    let path = paths.profiles_local_file;

    save_set_zz(&set, &pass, &path).expect("save_set_zz");
    println!("wrote {} (alias={})", path.display(), alias);
}
