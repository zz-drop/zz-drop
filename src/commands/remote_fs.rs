use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use thiserror::Error;
use zz_drop_core::providers::ProviderProfile;
use zz_drop_core::CollisionPolicy;

use zz_drop_core::providers::google_drive::{
    GoogleDriveClient, GoogleDriveError, diagnose as gdrive_diagnose,
};
use zz_drop_core::providers::nextcloud::{
    NextcloudClient, NextcloudError, RemoteEntry, UploadOutcome, diagnose,
};
use zz_drop_core::providers::onedrive::{
    OneDriveClient, OneDriveError, diagnose as onedrive_diagnose,
};

/// Abstraction over a remote filesystem so the command layer can be
/// tested with an in-memory fake instead of a real WebDAV server.
pub trait RemoteFs {
    fn ensure_dir(&self, segments: &[&str]) -> Result<(), RemoteError>;
    fn upload(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, RemoteError>;
    fn download(&self, segments: &[&str], dest: &Path) -> Result<u64, RemoteError>;
    fn list(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, RemoteError>;
}

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("{0}")]
    Diagnostic(&'static str),
}

impl From<NextcloudError> for RemoteError {
    fn from(value: NextcloudError) -> Self {
        Self::Diagnostic(diagnose(&value))
    }
}

impl From<GoogleDriveError> for RemoteError {
    fn from(value: GoogleDriveError) -> Self {
        Self::Diagnostic(gdrive_diagnose(&value))
    }
}

impl From<OneDriveError> for RemoteError {
    fn from(value: OneDriveError) -> Self {
        Self::Diagnostic(onedrive_diagnose(&value))
    }
}

/// `RemoteFs` impl backed by a real Nextcloud server.
pub struct NextcloudRemoteFs {
    inner: NextcloudClient,
}

impl NextcloudRemoteFs {
    pub fn new(client: NextcloudClient) -> Self {
        Self { inner: client }
    }
}

impl RemoteFs for NextcloudRemoteFs {
    fn ensure_dir(&self, segments: &[&str]) -> Result<(), RemoteError> {
        Ok(self.inner.ensure_dir_segments(segments)?)
    }

    fn upload(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, RemoteError> {
        Ok(self.inner.upload_to(local, segments, policy)?)
    }

    fn download(&self, segments: &[&str], dest: &Path) -> Result<u64, RemoteError> {
        Ok(self.inner.download_from(segments, dest)?)
    }

    fn list(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, RemoteError> {
        Ok(self.inner.list_at(segments)?)
    }
}

/// `RemoteFs` impl backed by a real Google Drive client.
pub struct GoogleDriveRemoteFs {
    inner: GoogleDriveClient,
}

impl GoogleDriveRemoteFs {
    pub fn new(client: GoogleDriveClient) -> Self {
        Self { inner: client }
    }

    /// True when the underlying client refreshed an OAuth token or
    /// resolved a folder id during this run, i.e. the persisted
    /// profile is now stale and the agent should be told.
    pub fn dirty(&self) -> bool {
        self.inner.dirty()
    }

    /// Snapshot of the up-to-date `GoogleDriveProfile` carried by
    /// the underlying client.
    pub fn current_provider(&self) -> ProviderProfile {
        ProviderProfile::GoogleDrive(self.inner.current_profile())
    }
}

impl RemoteFs for GoogleDriveRemoteFs {
    fn ensure_dir(&self, segments: &[&str]) -> Result<(), RemoteError> {
        self.inner.ensure_dir_segments(segments)?;
        Ok(())
    }

    fn upload(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, RemoteError> {
        Ok(self.inner.upload_to(local, segments, policy)?)
    }

    fn download(&self, segments: &[&str], dest: &Path) -> Result<u64, RemoteError> {
        Ok(self.inner.download_from(segments, dest)?)
    }

    fn list(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, RemoteError> {
        Ok(self.inner.list_at(segments)?)
    }
}

/// `RemoteFs` impl backed by a real OneDrive client.
pub struct OneDriveRemoteFs {
    inner: OneDriveClient,
}

impl OneDriveRemoteFs {
    pub fn new(client: OneDriveClient) -> Self {
        Self { inner: client }
    }

    pub fn dirty(&self) -> bool {
        self.inner.dirty()
    }

    pub fn current_provider(&self) -> ProviderProfile {
        ProviderProfile::OneDrive(self.inner.current_profile())
    }
}

impl RemoteFs for OneDriveRemoteFs {
    fn ensure_dir(&self, segments: &[&str]) -> Result<(), RemoteError> {
        self.inner.ensure_dir_segments(segments)?;
        Ok(())
    }

    fn upload(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, RemoteError> {
        Ok(self.inner.upload_to(local, segments, policy)?)
    }

    fn download(&self, segments: &[&str], dest: &Path) -> Result<u64, RemoteError> {
        Ok(self.inner.download_from(segments, dest)?)
    }

    fn list(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, RemoteError> {
        Ok(self.inner.list_at(segments)?)
    }
}

/// Provider-agnostic wrapper used by the CLI dispatcher. The
/// [`RemoteFs`] impl forwards to whichever concrete client matches
/// the active profile, so all upload/download/list/wipe code stays
/// generic over a single type.
pub enum AnyRemote {
    Nextcloud(NextcloudRemoteFs),
    GoogleDrive(GoogleDriveRemoteFs),
    OneDrive(OneDriveRemoteFs),
}

impl AnyRemote {
    /// Returns the freshest provider state if the active client
    /// mutated something during this run (OAuth token refresh,
    /// `root_folder_id` resolution). The dispatcher pushes this back
    /// to the agent so subsequent CLI invocations skip the redundant
    /// round-trips.
    pub fn pending_provider_update(&self) -> Option<ProviderProfile> {
        match self {
            // Nextcloud uses an app password / login-flow token: the
            // CLI never mutates it.
            Self::Nextcloud(_) => None,
            Self::GoogleDrive(g) => {
                if g.dirty() {
                    Some(g.current_provider())
                } else {
                    None
                }
            }
            Self::OneDrive(o) => {
                if o.dirty() {
                    Some(o.current_provider())
                } else {
                    None
                }
            }
        }
    }
}

impl RemoteFs for AnyRemote {
    fn ensure_dir(&self, segments: &[&str]) -> Result<(), RemoteError> {
        match self {
            Self::Nextcloud(r) => r.ensure_dir(segments),
            Self::GoogleDrive(r) => r.ensure_dir(segments),
            Self::OneDrive(r) => r.ensure_dir(segments),
        }
    }

    fn upload(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, RemoteError> {
        match self {
            Self::Nextcloud(r) => r.upload(local, segments, policy),
            Self::GoogleDrive(r) => r.upload(local, segments, policy),
            Self::OneDrive(r) => r.upload(local, segments, policy),
        }
    }

    fn download(&self, segments: &[&str], dest: &Path) -> Result<u64, RemoteError> {
        match self {
            Self::Nextcloud(r) => r.download(segments, dest),
            Self::GoogleDrive(r) => r.download(segments, dest),
            Self::OneDrive(r) => r.download(segments, dest),
        }
    }

    fn list(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, RemoteError> {
        match self {
            Self::Nextcloud(r) => r.list(segments),
            Self::GoogleDrive(r) => r.list(segments),
            Self::OneDrive(r) => r.list(segments),
        }
    }
}

/// In-memory fake `RemoteFs` for tests.
pub struct FakeRemoteFs {
    state: RefCell<FakeState>,
}

#[derive(Default)]
struct FakeState {
    files: BTreeMap<Vec<String>, Vec<u8>>,
    dirs: BTreeSet<Vec<String>>,
    upload_count: u32,
    download_count: u32,
    list_count: u32,
}

impl Default for FakeRemoteFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeRemoteFs {
    pub fn new() -> Self {
        let mut s = FakeState::default();
        s.dirs.insert(Vec::new()); // root exists
        Self {
            state: RefCell::new(s),
        }
    }

    pub fn put_file(&self, segments: &[&str], body: Vec<u8>) {
        let key: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        // ensure parent dirs
        for i in 0..key.len() {
            self.state.borrow_mut().dirs.insert(key[..i].to_vec());
        }
        self.state.borrow_mut().files.insert(key, body);
    }

    pub fn put_dir(&self, segments: &[&str]) {
        let key: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        self.state.borrow_mut().dirs.insert(key);
    }

    pub fn has_file(&self, segments: &[&str]) -> bool {
        let key: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        self.state.borrow().files.contains_key(&key)
    }

    pub fn upload_count(&self) -> u32 {
        self.state.borrow().upload_count
    }

    pub fn download_count(&self) -> u32 {
        self.state.borrow().download_count
    }
}

impl RemoteFs for FakeRemoteFs {
    fn ensure_dir(&self, segments: &[&str]) -> Result<(), RemoteError> {
        let key: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        for i in 0..=key.len() {
            self.state.borrow_mut().dirs.insert(key[..i].to_vec());
        }
        Ok(())
    }

    fn upload(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, RemoteError> {
        if segments.is_empty() {
            return Err(RemoteError::Diagnostic("invalid remote path"));
        }
        let body = std::fs::read(local).map_err(|_| RemoteError::Diagnostic("local file error"))?;
        let size = body.len() as u64;

        let parent: Vec<String> = segments[..segments.len() - 1]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let leaf = segments[segments.len() - 1].to_string();

        // ensure parents
        if !parent.is_empty() {
            for i in 1..=parent.len() {
                self.state.borrow_mut().dirs.insert(parent[..i].to_vec());
            }
        }

        let mut final_name = leaf.clone();
        let mut renamed = false;
        let mut full_key: Vec<String> = parent.clone();
        full_key.push(leaf.clone());

        match policy {
            CollisionPolicy::Overwrite => {
                self.state.borrow_mut().files.insert(full_key, body);
            }
            CollisionPolicy::Fail => {
                if self.state.borrow().files.contains_key(&full_key) {
                    return Err(RemoteError::Diagnostic("file already exists"));
                }
                self.state.borrow_mut().files.insert(full_key, body);
            }
            CollisionPolicy::Rename => {
                use zz_drop_core::providers::nextcloud::collision::rename_with_suffix;
                let mut n = 0u32;
                loop {
                    let candidate = rename_with_suffix(&leaf, n);
                    let mut k = parent.clone();
                    k.push(candidate.clone());
                    if !self.state.borrow().files.contains_key(&k) {
                        final_name = candidate.clone();
                        renamed = n > 0;
                        self.state.borrow_mut().files.insert(k, body);
                        break;
                    }
                    n += 1;
                    if n > 100 {
                        return Err(RemoteError::Diagnostic("too many name conflicts"));
                    }
                }
            }
        }

        self.state.borrow_mut().upload_count += 1;
        Ok(UploadOutcome {
            final_name,
            size,
            renamed,
        })
    }

    fn download(&self, segments: &[&str], dest: &Path) -> Result<u64, RemoteError> {
        let key: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        let state = self.state.borrow();
        let body = state
            .files
            .get(&key)
            .ok_or(RemoteError::Diagnostic("not found"))?
            .clone();
        drop(state);
        let size = body.len() as u64;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|_| RemoteError::Diagnostic("local file error"))?;
        }
        std::fs::write(dest, body).map_err(|_| RemoteError::Diagnostic("local file error"))?;
        self.state.borrow_mut().download_count += 1;
        Ok(size)
    }

    fn list(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, RemoteError> {
        let prefix: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        let state = self.state.borrow();
        if !state.dirs.contains(&prefix) {
            return Err(RemoteError::Diagnostic("not found"));
        }

        let mut out = Vec::new();
        let depth = prefix.len();

        for (k, v) in &state.files {
            if k.len() == depth + 1 && k.starts_with(&prefix) {
                out.push(RemoteEntry {
                    name: k[depth].clone(),
                    size: Some(v.len() as u64),
                    is_directory: false,
                });
            }
        }
        for k in &state.dirs {
            if k.len() == depth + 1 && k.starts_with(&prefix) {
                out.push(RemoteEntry {
                    name: k[depth].clone(),
                    size: None,
                    is_directory: true,
                });
            }
        }

        drop(state);
        self.state.borrow_mut().list_count += 1;
        Ok(out)
    }
}
