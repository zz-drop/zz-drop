pub mod collision;
pub mod login_flow;
pub mod path;
pub mod types;
pub mod webdav;

use std::path::Path;

use thiserror::Error;
use url::Url;

pub use types::{NextcloudAuth, NextcloudProfile};

use crate::CollisionPolicy;

use collision::rename_with_suffix;
use path::{PathError, encode_path, encode_remote_root, validate_filename};
use webdav::{BasicAuth, WebDavClient, WebDavError};

#[derive(Debug, Error)]
pub enum NextcloudError {
    #[error("invalid server url")]
    BadUrl,

    #[error("path: {0}")]
    Path(#[from] PathError),

    #[error("webdav: {0}")]
    WebDav(#[from] WebDavError),

    #[error("local io error")]
    LocalIo,

    #[error("collision: file already exists")]
    CollisionExists,

    #[error("collision rename gave up after {tries} attempts")]
    RenameExhausted { tries: u32 },

    #[error("auth method not supported in this milestone: {0}")]
    AuthNotSupported(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadOutcome {
    pub final_name: String,
    pub size: u64,
    pub renamed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub size: Option<u64>,
    pub is_directory: bool,
}

pub const RENAME_MAX_TRIES: u32 = 100;

pub struct NextcloudClient {
    server_url: Url,
    username: String,
    /// Already encoded remote root, with leading slash, no trailing
    /// slash. `""` if root is `/`.
    remote_root: String,
    client: WebDavClient,
}

impl NextcloudClient {
    pub fn from_profile(profile: &NextcloudProfile) -> Result<Self, NextcloudError> {
        let server_url = Url::parse(&profile.server_url).map_err(|_| NextcloudError::BadUrl)?;

        let secret = match &profile.auth {
            NextcloudAuth::AppPassword { secret } => secret.clone(),
            NextcloudAuth::LoginFlowToken { .. } => {
                return Err(NextcloudError::AuthNotSupported("login_flow_token (TASK 13)"));
            }
        };

        let basic = BasicAuth {
            username: profile.username.clone(),
            password: secret,
        };

        let webdav = WebDavClient::new(basic);
        let root = encode_remote_root(&profile.remote_root)?;
        let normalized_root = if root == "/" {
            String::new()
        } else {
            root.trim_end_matches('/').to_string()
        };

        Ok(Self {
            server_url,
            username: profile.username.clone(),
            remote_root: normalized_root,
            client: webdav,
        })
    }

    fn dav_root_url(&self) -> Url {
        let encoded_user = percent_encoding::utf8_percent_encode(
            &self.username,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        let path = format!(
            "/remote.php/dav/files/{encoded_user}{}",
            self.remote_root
        );
        let mut u = self.server_url.clone();
        u.set_path(&path);
        u
    }

    fn url_for_segments(&self, segments: &[&str]) -> Result<Url, NextcloudError> {
        for s in segments {
            validate_filename(s)?;
        }
        let mut u = self.dav_root_url();
        if segments.is_empty() {
            return Ok(u);
        }
        let encoded = encode_path(segments)?;
        let new_path = format!("{}/{}", u.path().trim_end_matches('/'), encoded);
        u.set_path(&new_path);
        Ok(u)
    }

    fn url_for_segments_with_collection_suffix(
        &self,
        segments: &[&str],
    ) -> Result<Url, NextcloudError> {
        let mut u = self.url_for_segments(segments)?;
        let p = u.path().to_string();
        if !p.ends_with('/') {
            u.set_path(&format!("{p}/"));
        }
        Ok(u)
    }

    pub fn ensure_remote_root(&self) -> Result<(), NextcloudError> {
        let url = self.dav_root_url();
        match self.client.mkcol(url.as_str()) {
            Ok(()) => Ok(()),
            Err(WebDavError::Conflict) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Idempotently `MKCOL` each prefix of `segments` so the full
    /// directory chain exists.
    pub fn ensure_dir_segments(&self, segments: &[&str]) -> Result<(), NextcloudError> {
        for s in segments {
            validate_filename(s)?;
        }
        for i in 1..=segments.len() {
            let prefix: Vec<&str> = segments[..i].to_vec();
            let url = self.url_for_segments(&prefix)?;
            match self.client.mkcol(url.as_str()) {
                Ok(()) => {}
                Err(WebDavError::Conflict) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    /// Upload to a single-segment destination (back-compat).
    pub fn upload(
        &self,
        local: &Path,
        remote_name: &str,
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, NextcloudError> {
        self.upload_to(local, &[remote_name], policy)
    }

    /// Upload to a multi-segment destination. Intermediate directories
    /// are created idempotently. Collision policy applies to the leaf
    /// segment only.
    pub fn upload_to(
        &self,
        local: &Path,
        segments: &[&str],
        policy: CollisionPolicy,
    ) -> Result<UploadOutcome, NextcloudError> {
        if segments.is_empty() {
            return Err(NextcloudError::Path(PathError::Empty));
        }
        for s in segments {
            validate_filename(s)?;
        }

        if segments.len() > 1 {
            self.ensure_dir_segments(&segments[..segments.len() - 1])?;
        }

        let body = std::fs::read(local).map_err(|_| NextcloudError::LocalIo)?;
        let size = body.len() as u64;
        let last = segments[segments.len() - 1];
        let parent = &segments[..segments.len() - 1];

        match policy {
            CollisionPolicy::Overwrite => {
                let mut full = parent.to_vec();
                full.push(last);
                let url = self.url_for_segments(&full)?;
                self.client.put(url.as_str(), body)?;
                Ok(UploadOutcome {
                    final_name: last.to_string(),
                    size,
                    renamed: false,
                })
            }
            CollisionPolicy::Fail => {
                let mut full = parent.to_vec();
                full.push(last);
                let url = self.url_for_segments(&full)?;
                if self.client.head(url.as_str())? {
                    return Err(NextcloudError::CollisionExists);
                }
                self.client.put(url.as_str(), body)?;
                Ok(UploadOutcome {
                    final_name: last.to_string(),
                    size,
                    renamed: false,
                })
            }
            CollisionPolicy::Rename => {
                for n in 0..=RENAME_MAX_TRIES {
                    let candidate = rename_with_suffix(last, n);
                    let mut full = parent.to_vec();
                    full.push(&candidate);
                    let url = self.url_for_segments(&full)?;
                    if !self.client.head(url.as_str())? {
                        self.client.put(url.as_str(), body)?;
                        return Ok(UploadOutcome {
                            final_name: candidate,
                            size,
                            renamed: n > 0,
                        });
                    }
                }
                Err(NextcloudError::RenameExhausted {
                    tries: RENAME_MAX_TRIES,
                })
            }
        }
    }

    pub fn download(&self, remote_name: &str, dest: &Path) -> Result<u64, NextcloudError> {
        self.download_from(&[remote_name], dest)
    }

    pub fn download_from(
        &self,
        segments: &[&str],
        dest: &Path,
    ) -> Result<u64, NextcloudError> {
        if segments.is_empty() {
            return Err(NextcloudError::Path(PathError::Empty));
        }
        let url = self.url_for_segments(segments)?;
        let body = self.client.get(url.as_str())?;
        let size = body.len() as u64;
        std::fs::write(dest, body).map_err(|_| NextcloudError::LocalIo)?;
        Ok(size)
    }

    pub fn list(&self) -> Result<Vec<RemoteEntry>, NextcloudError> {
        self.list_at(&[])
    }

    /// DELETE on a multi-segment path. Used by the TUI test-upload
    /// probe to clean up its temporary file; the high-level CLI does
    /// not expose this in v1.
    pub fn delete_at(&self, segments: &[&str]) -> Result<(), NextcloudError> {
        if segments.is_empty() {
            return Err(NextcloudError::Path(PathError::Empty));
        }
        for s in segments {
            validate_filename(s)?;
        }
        let url = self.url_for_segments(segments)?;
        self.client.delete(url.as_str())?;
        Ok(())
    }

    pub fn list_at(&self, segments: &[&str]) -> Result<Vec<RemoteEntry>, NextcloudError> {
        let url = self.url_for_segments_with_collection_suffix(segments)?;
        let xml = self.client.propfind(url.as_str(), "1")?;
        parse_propfind_multistatus(&xml).ok_or(NextcloudError::WebDav(WebDavError::XmlParse))
    }
}

/// Parse a WebDAV multistatus response into entries. The first response
/// element is the queried collection itself and is skipped.
pub fn parse_propfind_multistatus(xml: &str) -> Option<Vec<RemoteEntry>> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    enum State {
        None,
        Href,
        ContentLength,
    }

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut state = State::None;
    let mut saw_multistatus = false;
    let mut in_response = false;
    let mut current_href: Option<String> = None;
    let mut current_size: Option<u64> = None;
    let mut current_is_dir = false;
    let mut entries: Vec<RemoteEntry> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name_owned = e.name();
                let local_bytes = name_owned.local_name();
                let local = std::str::from_utf8(local_bytes.as_ref()).ok()?;
                match local {
                    "multistatus" => saw_multistatus = true,
                    "response" => {
                        in_response = true;
                        current_href = None;
                        current_size = None;
                        current_is_dir = false;
                    }
                    "href" if in_response => state = State::Href,
                    "getcontentlength" if in_response => state = State::ContentLength,
                    "collection" if in_response => current_is_dir = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                // Self-closing tags (e.g. `<d:collection/>` inside resourcetype).
                let name_owned = e.name();
                let local_bytes = name_owned.local_name();
                let local = std::str::from_utf8(local_bytes.as_ref()).ok()?;
                if local == "collection" && in_response {
                    current_is_dir = true;
                }
            }
            Ok(Event::End(e)) => {
                let name_owned = e.name();
                let local_bytes = name_owned.local_name();
                let local = std::str::from_utf8(local_bytes.as_ref()).ok()?;
                if local == "response" {
                    if let Some(href) = current_href.take() {
                        let trimmed = href.trim_end_matches('/');
                        if let Some(last) = trimmed.rsplit('/').next() {
                            let decoded = percent_encoding::percent_decode_str(last)
                                .decode_utf8_lossy()
                                .to_string();
                            if !decoded.is_empty() {
                                entries.push(RemoteEntry {
                                    name: decoded,
                                    size: current_size.take(),
                                    is_directory: current_is_dir,
                                });
                            }
                        }
                    }
                    in_response = false;
                    current_is_dir = false;
                }
                state = State::None;
            }
            Ok(Event::Text(t)) => match state {
                State::Href => {
                    // quick-xml 0.39 splits decode and unescape; XML entities
                    // in `<href>` are uncommon, decode-only is good enough.
                    current_href = Some(t.decode().ok()?.to_string());
                }
                State::ContentLength => {
                    let s = t.decode().ok()?.to_string();
                    current_size = s.trim().parse().ok();
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    if !saw_multistatus {
        return None;
    }

    // The first response is the queried collection itself; skip it.
    if !entries.is_empty() {
        entries.remove(0);
    }

    Some(entries)
}

/// Map a [`NextcloudError`] into a single short, sanitized stderr line
/// suitable for `output::err_line` and the `9 = provider error` exit.
pub fn diagnose(err: &NextcloudError) -> &'static str {
    match err {
        NextcloudError::BadUrl => "invalid server url",
        NextcloudError::Path(_) => "invalid remote path",
        NextcloudError::WebDav(WebDavError::Unauthorized) => "auth failed",
        NextcloudError::WebDav(WebDavError::NotFound) => "not found",
        NextcloudError::WebDav(WebDavError::Conflict) => "conflict",
        NextcloudError::WebDav(WebDavError::ServerError { .. }) => "server error",
        NextcloudError::WebDav(WebDavError::Transport(_)) => "network error",
        NextcloudError::WebDav(WebDavError::UnexpectedStatus { .. }) => "unexpected response",
        NextcloudError::WebDav(WebDavError::Io) => "io error",
        NextcloudError::WebDav(WebDavError::XmlParse) => "bad server response",
        NextcloudError::LocalIo => "local file error",
        NextcloudError::CollisionExists => "file already exists",
        NextcloudError::RenameExhausted { .. } => "too many name conflicts",
        NextcloudError::AuthNotSupported(_) => "auth method not supported yet",
    }
}

/// Like [`diagnose`] but includes the underlying transport / status
/// message when it carries one. Useful for the interactive TUI where
/// the user can see and act on the detail. The CLI stays on the
/// short, static `diagnose` to keep stderr predictable.
pub fn diagnose_full(err: &NextcloudError) -> String {
    match err {
        NextcloudError::WebDav(WebDavError::Transport(reason)) => {
            format!("network error: {reason}")
        }
        NextcloudError::WebDav(WebDavError::ServerError { status }) => {
            format!("server error: HTTP {status}")
        }
        NextcloudError::WebDav(WebDavError::UnexpectedStatus { status }) => {
            format!("unexpected response: HTTP {status}")
        }
        _ => diagnose(err).to_string(),
    }
}
