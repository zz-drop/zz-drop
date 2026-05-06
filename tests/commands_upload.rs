use std::os::unix::fs::symlink;
use std::path::PathBuf;

use tempfile::tempdir;
use zz_drop_core::{
    CollisionPolicy, NextcloudAuth, NextcloudProfile, PlainProfile, ProfileSettings,
    ProviderProfile,
};

use zz_drop::color::{ColorPolicy, MockEnv};
use zz_drop::commands::EXIT_OK;
use zz_drop::commands::remote_fs::FakeRemoteFs;
use zz_drop::commands::upload::{run_save_all, run_upload};

fn sample_profile() -> PlainProfile {
    PlainProfile {
        profile_version: 1,
        profile_id: "p".into(),
        alias: "casa".into(),
        default_target: "nc".into(),
        providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
            server_url: "https://example.org".into(),
            username: "u".into(),
            auth: NextcloudAuth::AppPassword {
                secret: "secret".into(),
            },
            remote_root: "/".into(),
        })],
        collision_policy: CollisionPolicy::Rename,
        settings: ProfileSettings::default(),
        created_at: "2026-04-26T00:00:00Z".into(),
        updated_at: "2026-04-26T00:00:00Z".into(),
    }
}

fn no_color() -> ColorPolicy {
    ColorPolicy::from_parts(&MockEnv::empty(), false)
}

#[test]
fn run_upload_single_file_succeeds() {
    let tmp = tempdir().unwrap();
    let f = tmp.path().join("readme.md");
    std::fs::write(&f, b"hello").unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_upload(&remote, &[f], &profile, &no_color(), false, None);
    assert_eq!(code, EXIT_OK);
    assert_eq!(remote.upload_count(), 1);
    assert!(remote.has_file(&["readme.md"]));
}

#[test]
fn run_upload_continues_after_failure() {
    let tmp = tempdir().unwrap();
    let real = tmp.path().join("a.md");
    std::fs::write(&real, b"x").unwrap();
    let missing = tmp.path().join("does-not-exist.md");
    let real2 = tmp.path().join("b.md");
    std::fs::write(&real2, b"y").unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_upload(&remote, &[real, missing, real2], &profile, &no_color(), false, None);
    // one missing → exit 9, but the other two should still be uploaded
    assert_eq!(code, zz_drop::commands::EXIT_PROVIDER_ERROR);
    assert!(remote.has_file(&["a.md"]));
    assert!(remote.has_file(&["b.md"]));
    assert_eq!(remote.upload_count(), 2);
}

#[test]
fn run_upload_skips_directory() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path().join("subdir");
    std::fs::create_dir(&dir).unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_upload(&remote, &[dir], &profile, &no_color(), false, None);
    // skip: non-failure
    assert_eq!(code, EXIT_OK);
    assert_eq!(remote.upload_count(), 0);
}

#[test]
fn run_upload_skips_symlink() {
    let tmp = tempdir().unwrap();
    let real = tmp.path().join("real.md");
    std::fs::write(&real, b"x").unwrap();
    let link = tmp.path().join("link.md");
    symlink(&real, &link).unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_upload(&remote, &[link], &profile, &no_color(), false, None);
    assert_eq!(code, EXIT_OK);
    assert_eq!(remote.upload_count(), 0);
}

#[test]
fn run_save_all_top_level_only_when_non_recursive() {
    let tmp = tempdir().unwrap();
    std::fs::write(tmp.path().join("top.md"), b"x").unwrap();
    std::fs::write(tmp.path().join(".hidden"), b"x").unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub").join("inner.md"), b"x").unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_save_all(&remote, tmp.path(), false, &profile, &no_color(), false, None);
    assert_eq!(code, EXIT_OK);
    assert!(remote.has_file(&["top.md"]));
    assert!(!remote.has_file(&[".hidden"]));
    assert!(!remote.has_file(&["sub", "inner.md"]));
    assert_eq!(remote.upload_count(), 1);
}

#[test]
fn run_save_all_descends_when_recursive() {
    let tmp = tempdir().unwrap();
    std::fs::write(tmp.path().join("top.md"), b"x").unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub").join("inner.md"), b"x").unwrap();
    std::fs::create_dir(tmp.path().join(".git")).unwrap();
    std::fs::write(tmp.path().join(".git").join("HEAD"), b"x").unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_save_all(&remote, tmp.path(), true, &profile, &no_color(), false, None);
    assert_eq!(code, EXIT_OK);
    assert!(remote.has_file(&["top.md"]));
    assert!(remote.has_file(&["sub", "inner.md"]));
    assert!(!remote.has_file(&[".git", "HEAD"]));
    assert_eq!(remote.upload_count(), 2);
}

#[test]
fn run_upload_rejects_dotfile_argument() {
    let tmp = tempdir().unwrap();
    let f = tmp.path().join(".bashrc");
    std::fs::write(&f, b"x").unwrap();

    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    // The skip path is taken before opening the file.
    // We pass the dotfile as the user argument string.
    let code = run_upload(&remote, &[PathBuf::from(".bashrc")], &profile, &no_color(), false, None);
    assert_eq!(code, EXIT_OK); // skip is not a failure
    assert_eq!(remote.upload_count(), 0);
}

#[test]
fn empty_batch_succeeds() {
    let remote = FakeRemoteFs::new();
    let profile = sample_profile();

    let code = run_upload(&remote, &[], &profile, &no_color(), false, None);
    assert_eq!(code, EXIT_OK);
    assert_eq!(remote.upload_count(), 0);
}
