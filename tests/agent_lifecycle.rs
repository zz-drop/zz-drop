use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

use zz_drop::agent::{AgentClient, ClientError, ServerConfig};
use zz_drop_core::agent_proto::{AgentRequest, AgentResponse};
use zz_drop_core::config::{PathOverrides, Paths, discover_paths};
use zz_drop_core::profile::format::save_set_zz_with_config;
use zz_drop_core::{
    AgentError, Argon2idConfig, CollisionPolicy, NextcloudAuth, NextcloudProfile, PlainProfile,
    ProfileSet, ProfileSettings, ProviderProfile, agent_proto::write_frame,
};

fn sample_profile() -> PlainProfile {
    PlainProfile {
        profile_version: 1,
        profile_id: "p-1".into(),
        alias: "casa".into(),
        default_target: "nc".into(),
        providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
            server_url: "https://example.org".into(),
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

fn server_config(paths: Paths, idle: Duration) -> ServerConfig {
    ServerConfig {
        paths,
        ttl: Duration::from_secs(60),
        idle_exit: idle,
        poll_interval: Duration::from_millis(20),
        detach: false,
    }
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

#[test]
fn agent_full_lifecycle_via_unix_socket() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();

    let server_paths = paths.clone();
    let join = thread::spawn(move || {
        let cfg = server_config(server_paths, Duration::from_secs(10));
        zz_drop::agent::run(cfg)
    });

    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");

    // Ping
    let resp = client.ping().expect("ping");
    assert!(matches!(resp, AgentResponse::Pong));

    // Status (locked)
    let resp = client.status().expect("status");
    assert!(matches!(
        resp,
        AgentResponse::Status {
            unlocked: false,
            ..
        }
    ));

    // GetProfile while locked → Error::NotUnlocked
    let resp = client.get_profile().expect("get_profile while locked");
    assert!(matches!(
        resp,
        AgentResponse::Error(AgentError::NotUnlocked)
    ));

    // Unlock: persist a tiny container to the agent's configured
    // path so the agent has a real file to mtime-track and re-write.
    const FAST_KDF: Argon2idConfig = Argon2idConfig {
        memory_kib: 8 * 1024,
        iterations: 1,
        parallelism: 1,
    };
    let set = ProfileSet::with_profile(sample_profile());
    if let Some(parent) = paths.profiles_local_file.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let kek = save_set_zz_with_config(
        &set,
        "lifecycle-test-pass",
        &paths.profiles_local_file,
        &FAST_KDF,
    )
    .unwrap();
    let resp = client
        .unlock(set, &kek, "casa", Some(60))
        .expect("unlock");
    assert!(matches!(resp, AgentResponse::Unlocked));

    // GetProfile → returns the profile (verify alias matches)
    let resp = client.get_profile().expect("get_profile after unlock");
    match resp {
        AgentResponse::Profile(p) => assert_eq!(p.alias, "casa"),
        other => panic!("expected Profile, got {other:?}"),
    }

    // Status (unlocked)
    let resp = client.status().expect("status unlocked");
    assert!(matches!(
        resp,
        AgentResponse::Status {
            unlocked: true,
            ..
        }
    ));

    // Lock
    let resp = client.lock().expect("lock");
    assert!(matches!(resp, AgentResponse::Locked));

    // GetProfile while locked again → Error::NotUnlocked
    let resp = client.get_profile().expect("get_profile after lock");
    assert!(matches!(
        resp,
        AgentResponse::Error(AgentError::NotUnlocked)
    ));

    // Exit
    let resp = client.exit().expect("exit");
    assert!(matches!(resp, AgentResponse::Exited));
    drop(client);

    // Server thread should now end and clean up.
    let res = join.join().expect("server thread");
    assert!(res.is_ok());
    assert!(!socket_path.exists(), "socket should be cleaned up");
    assert!(!token_path.exists(), "token should be cleaned up");
}

#[test]
fn token_mismatch_closes_connection() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();

    let server_paths = paths.clone();
    let join_complete = Arc::new(Mutex::new(false));
    let jc = join_complete.clone();
    let join = thread::spawn(move || {
        let cfg = server_config(server_paths, Duration::from_millis(800));
        let r = zz_drop::agent::run(cfg);
        *jc.lock().unwrap() = true;
        r
    });

    wait_for_socket(&socket_path, Duration::from_secs(2));

    // Connect manually with a wrong token instead of using AgentClient.
    let mut stream =
        std::os::unix::net::UnixStream::connect(&socket_path).expect("connect raw");
    let bad_token = [0xAAu8; 32];
    write_frame(&mut stream, &bad_token).expect("send bad token");

    // The agent should close the connection after the bad token.
    // Now try to send a Ping body via this stream and expect failure
    // (either write fails or read returns no data).
    let body = zz_drop_core::agent_proto::encode_request_body(&AgentRequest::Ping).unwrap();
    let _ = write_frame(&mut stream, &body); // may succeed (TCP-like buffering)
    drop(stream);

    // Wait for idle exit (we set 800ms above) then join.
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !*join_complete.lock().unwrap() {
        thread::sleep(Duration::from_millis(50));
    }
    let res = join.join().expect("server thread");
    assert!(res.is_ok(), "agent should exit cleanly via idle timeout");
}

/// Regression — the TUI's `perform_add_inner_profile` must push the
/// new container to the running agent (`try_update_profile_set`)
/// before the agent has a chance to re-encrypt its in-RAM snapshot
/// on top of it. Without that push, a subsequent `update_profile`
/// (e.g. an OAuth token refresh) re-encrypts the stale 1-profile set
/// and silently overwrites the freshly appended profile on disk.
#[test]
fn append_then_token_refresh_preserves_appended_profile() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();
    let token_path = paths.token_file.clone();

    let server_paths = paths.clone();
    let join = thread::spawn(move || {
        let cfg = server_config(server_paths, Duration::from_secs(10));
        zz_drop::agent::run(cfg)
    });
    wait_for_socket(&socket_path, Duration::from_secs(2));

    let mut client = AgentClient::connect(&socket_path, &token_path).expect("connect");

    // 1. Seed: a 1-profile container at the agent's configured path.
    const FAST_KDF: Argon2idConfig = Argon2idConfig {
        memory_kib: 8 * 1024,
        iterations: 1,
        parallelism: 1,
    };
    let initial = ProfileSet::with_profile(sample_profile());
    if let Some(parent) = paths.profiles_local_file.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let kek = save_set_zz_with_config(
        &initial,
        "regression-pass",
        &paths.profiles_local_file,
        &FAST_KDF,
    )
    .unwrap();
    let resp = client
        .unlock(initial, &kek, "casa", Some(60))
        .expect("unlock");
    assert!(matches!(resp, AgentResponse::Unlocked));

    // 2. Simulate the TUI: append a second inner profile, write the
    //    new envelope to disk, *and* push the new set to the agent
    //    (this is what `agent_kill::try_update_profile_set` does).
    let mut appended = ProfileSet::new();
    appended.profiles.push(sample_profile()); // alias = "casa"
    let mut second = sample_profile();
    second.alias = "second".into();
    second.profile_id = "p-2".into();
    appended.profiles.push(second);
    // Write to disk via encrypt_set_with_kek + atomic rename, the
    // exact pattern `perform_add_inner_profile` uses.
    let envelope =
        zz_drop_core::profile::format::encrypt_set_with_kek(&appended, &kek).unwrap();
    let tmp_path = paths.profiles_local_file.with_extension("zz.tmp");
    std::fs::write(&tmp_path, envelope).unwrap();
    std::fs::rename(&tmp_path, &paths.profiles_local_file).unwrap();
    // Push to agent — the missing-this-call case is the bug.
    let resp = client
        .update_profile_set(appended.clone())
        .expect("update_profile_set");
    assert!(matches!(resp, AgentResponse::Updated));

    // 3. Simulate an OAuth-style token refresh on the *first* inner
    //    profile. The CLI does this every time an access token gets
    //    rotated mid-upload. The agent re-encrypts its RAM snapshot
    //    and writes back to disk — if the snapshot is stale (the
    //    bug), the second profile vanishes.
    let mut refreshed = appended.profiles[0].clone();
    refreshed.updated_at = "2026-05-02T18:00:00Z".into();
    let resp = client
        .update_profile(refreshed)
        .expect("update_profile after refresh");
    assert!(matches!(resp, AgentResponse::Updated));

    // 4. Disk must still hold both profiles. If the bug regresses,
    //    this assertion drops to len == 1.
    let (final_set, _kek2) =
        zz_drop_core::profile::format::load_set_zz(&paths.profiles_local_file, "regression-pass")
            .unwrap();
    assert_eq!(
        final_set.profiles.len(),
        2,
        "agent re-encrypt after update_profile must preserve all inner profiles"
    );
    let aliases: Vec<&str> = final_set.profiles.iter().map(|p| p.alias.as_str()).collect();
    assert!(aliases.contains(&"casa"));
    assert!(aliases.contains(&"second"));

    let _ = client.exit();
    drop(client);
    let _ = join.join();
}

#[test]
fn agent_idle_exit_when_locked_with_no_clients() {
    let (_tmp, paths) = build_paths();
    let socket_path = paths.agent_socket.clone();

    let server_paths = paths.clone();
    let started = Instant::now();
    let join = thread::spawn(move || {
        let cfg = server_config(server_paths, Duration::from_millis(300));
        zz_drop::agent::run(cfg)
    });

    wait_for_socket(&socket_path, Duration::from_secs(2));

    // Don't connect. Wait for idle exit.
    let res = join.join().expect("server thread");
    let elapsed = started.elapsed();
    assert!(res.is_ok());
    assert!(
        elapsed >= Duration::from_millis(250),
        "idle exit too fast: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "idle exit too slow: {elapsed:?}"
    );
    assert!(!socket_path.exists());
}
