// Live HTTP test of `api::client::ApiClient` against a real
// `zz-drop-server-minimal` process. The test boots the server bin on
// a random ephemeral port, walks register → login → list → create →
// put_blob → get_blob → delete, and shuts the server down.
//
// Skipped automatically when the server binary isn't on PATH (the
// crate doesn't depend on `zz-drop-server-minimal` at compile time;
// the test is opt-in via the workspace build).

use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use zz_drop_core::api::{ApiClient, LoginOutcome};

fn pick_free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn server_bin() -> Option<PathBuf> {
    let candidates = [
        "../zz-drop-server-minimal/target/debug/zz-drop-server-minimal",
        "../../zz-drop-server-minimal/target/debug/zz-drop-server-minimal",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

struct Server {
    child: Child,
    base: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_server() -> Option<Server> {
    let bin = server_bin()?;
    let port = pick_free_port();
    let child = Command::new(&bin)
        .env("ZZDROP_BIND", format!("127.0.0.1:{port}"))
        .env("DATABASE_URL", "sqlite::memory:")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Wait until the port accepts a TCP connection, or 5 s, whichever
    // first. Avoids race conditions with stdio piping (tracing writes
    // to stderr, our log line could deadlock the pipe).
    let deadline = Instant::now() + Duration::from_secs(5);
    let addr = format!("127.0.0.1:{port}");
    let mut ready = false;
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(200))
            .is_ok()
        {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if !ready {
        let mut child = child;
        let _ = child.kill();
        return None;
    }

    Some(Server {
        child,
        base: format!("http://127.0.0.1:{port}"),
    })
}

#[test]
fn full_flow_against_running_server() {
    let Some(srv) = start_server() else {
        eprintln!("skipping: zz-drop-server-minimal binary not found");
        return;
    };

    // register + login (no TOTP).
    let client = ApiClient::new(&srv.base);
    client
        .register("alice@example.org", "correct-horse-battery-9!")
        .expect("register");
    let outcome = client
        .login("alice@example.org", "correct-horse-battery-9!")
        .expect("login");
    let session = match outcome {
        LoginOutcome::Session(s) => s,
        LoginOutcome::TotpRequired(_) => panic!("totp not active for fresh account"),
    };
    let client = client.with_token(session.token);

    // list (empty), create, list (1).
    let list = client.list_profiles().expect("list");
    assert!(list.profiles.is_empty());

    let summary = client.create_profile("casa-nc").expect("create");
    assert_eq!(summary.alias.as_str(), "casa-nc");
    assert_eq!(summary.blob_version, 0);

    // upload blob with expected_version=0 → version 1.
    let blob = b"opaque-encrypted-blob".to_vec();
    let summary = client
        .put_blob("casa-nc", 0, blob.clone())
        .expect("put_blob");
    assert_eq!(summary.blob_version, 1);
    assert_eq!(summary.blob_size, blob.len() as u64);

    // round-trip: get returns the same bytes.
    let got = client.get_blob("casa-nc").expect("get_blob");
    assert_eq!(got, blob);

    // wrong expected_version → VersionConflict.
    let err = client
        .put_blob("casa-nc", 0, b"stale".to_vec())
        .expect_err("expected conflict");
    let s = format!("{err:?}").to_lowercase();
    assert!(
        s.contains("versionconflict") || s.contains("version_conflict") || s.contains("conflict"),
        "got `{s}`"
    );

    // delete.
    client.delete_profile("casa-nc").expect("delete");
    let list = client.list_profiles().expect("list after delete");
    assert!(list.profiles.is_empty());

    drop(srv);
}
