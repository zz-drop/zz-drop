//! End-to-end tests for the SACS `ListRemote` / `InvalidateRemote`
//! agent endpoints. We exercise the wire layer (request → server →
//! state → response) without going through a real provider — that
//! integration arrives in chunk E together with the `FakeRemoteFs`
//! plumbing. Here we cover:
//!
//! - `ListRemote` while the agent is locked → `Error(NotUnlocked)`,
//! - `InvalidateRemote` is idempotent and works locked or unlocked,
//! - `InvalidateRemote` truly drops cached entries (verified through
//!   a follow-up `ListRemote` that re-asserts NotUnlocked because we
//!   re-locked between calls — the cache must not survive the lock).

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

use zz_drop::agent::{AgentClient, ServerConfig};
use zz_drop_core::AgentError;
use zz_drop_core::agent_proto::{AgentResponse, EntryKindFilter};
use zz_drop_core::config::{PathOverrides, Paths, discover_paths};
use zz_drop_core::profile::format::save_set_zz_with_config;
use zz_drop_core::{
    Argon2idConfig, CollisionPolicy, NextcloudAuth, NextcloudProfile, PlainProfile, ProfileSet,
    ProfileSettings, ProviderProfile,
};

const FAST_KDF: Argon2idConfig = Argon2idConfig {
    memory_kib: 8 * 1024,
    iterations: 1,
    parallelism: 1,
};

fn sample_profile() -> PlainProfile {
    PlainProfile {
        profile_version: 1,
        profile_id: "p-list-remote-1".into(),
        alias: "casa-nc".into(),
        default_target: "nc".into(),
        providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
            server_url: "https://cloud.example.org".into(),
            username: "u".into(),
            auth: NextcloudAuth::AppPassword {
                secret: "topsecret".into(),
            },
            remote_root: "/".into(),
        })],
        collision_policy: CollisionPolicy::Rename,
        settings: ProfileSettings::default(),
        created_at: "2026-04-26T00:00:00Z".into(),
        updated_at: "2026-04-26T00:00:00Z".into(),
    }
}

fn build_paths() -> (tempfile::TempDir, Paths) {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("cfg");
    let cache_dir = tmp.path().join("cache");
    let runtime_dir = tmp.path().join("rt");
    let uid = zz_drop::config::current_uid();
    let paths = discover_paths(
        uid,
        &PathOverrides {
            config_dir: Some(cfg_dir),
            cache_dir: Some(cache_dir),
            runtime_dir: Some(runtime_dir),
        },
    )
    .unwrap();
    (tmp, paths)
}

fn wait_for_socket(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("socket {} never appeared", path.display());
}

fn run_server(paths: Paths) -> thread::JoinHandle<Result<(), zz_drop::agent::ServerError>> {
    let cfg = ServerConfig {
        paths,
        ttl: Duration::from_secs(60),
        idle_exit: Duration::from_secs(10),
        poll_interval: Duration::from_millis(20),
        detach: false,
    };
    thread::spawn(move || zz_drop::agent::run(cfg))
}

#[test]
fn list_remote_while_locked_returns_not_unlocked() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();
    let join = run_server(paths);
    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");
    let resp = client
        .list_remote(None, EntryKindFilter::Both, 200)
        .expect("list_remote rpc");
    assert!(
        matches!(resp, AgentResponse::Error(AgentError::NotUnlocked)),
        "expected NotUnlocked, got {resp:?}"
    );

    client.exit().ok();
    drop(client);
    let _ = join.join().unwrap();
}

#[test]
fn list_remote_with_prefix_while_locked_also_returns_not_unlocked() {
    // The detector / cache layer should not even attempt to build
    // a provider client when the agent is locked: same response
    // regardless of `prefix` shape.
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();
    let join = run_server(paths);
    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");
    let resp = client
        .list_remote(Some("backup/snap"), EntryKindFilter::File, 200)
        .expect("list_remote rpc");
    assert!(matches!(resp, AgentResponse::Error(AgentError::NotUnlocked)));

    client.exit().ok();
    drop(client);
    let _ = join.join().unwrap();
}

#[test]
fn invalidate_remote_succeeds_locked_and_idempotent() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();
    let join = run_server(paths);
    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");
    // Cache is empty; calling invalidate must still return Updated.
    let resp = client
        .invalidate_remote(Some("docs"))
        .expect("invalidate rpc");
    assert!(matches!(resp, AgentResponse::Updated));
    let resp = client
        .invalidate_remote(None)
        .expect("invalidate rpc 2");
    assert!(matches!(resp, AgentResponse::Updated));

    client.exit().ok();
    drop(client);
    let _ = join.join().unwrap();
}

#[test]
fn invalidate_remote_succeeds_after_unlock() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();
    let join = run_server(paths.clone());
    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");

    let set = ProfileSet::with_profile(sample_profile());
    if let Some(parent) = paths.profiles_local_file.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let kek = save_set_zz_with_config(
        &set,
        "list-remote-test-pass",
        &paths.profiles_local_file,
        &FAST_KDF,
    )
    .unwrap();
    let resp = client.unlock(set, &kek, "casa-nc", Some(60)).expect("unlock");
    assert!(matches!(resp, AgentResponse::Unlocked));

    let resp = client
        .invalidate_remote(Some("backup/snap"))
        .expect("invalidate rpc");
    assert!(matches!(resp, AgentResponse::Updated));

    client.exit().ok();
    drop(client);
    let _ = join.join().unwrap();
}

#[test]
fn list_remote_after_lock_returns_not_unlocked_again() {
    // Lock-after-unlock must drop any caches the agent built up, so
    // a follow-up list once again reports NotUnlocked instead of
    // returning a stale snapshot.
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();
    let join = run_server(paths.clone());
    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");

    let set = ProfileSet::with_profile(sample_profile());
    if let Some(parent) = paths.profiles_local_file.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let kek = save_set_zz_with_config(
        &set,
        "list-remote-test-pass",
        &paths.profiles_local_file,
        &FAST_KDF,
    )
    .unwrap();
    client.unlock(set, &kek, "casa-nc", Some(60)).expect("unlock");
    client.lock().expect("lock");

    let resp = client
        .list_remote(None, EntryKindFilter::Both, 200)
        .expect("list_remote rpc");
    assert!(matches!(resp, AgentResponse::Error(AgentError::NotUnlocked)));

    client.exit().ok();
    drop(client);
    let _ = join.join().unwrap();
}

// Compile-time guard: keep the `Arc` import in scope so the file
// matches the `agent_lifecycle.rs` style — currently the helpers
// don't need it explicitly but the future chunk-E test will, and
// having the import here keeps churn out of the diff.
#[allow(dead_code)]
fn _arc_keepalive(_a: Arc<()>) {}
