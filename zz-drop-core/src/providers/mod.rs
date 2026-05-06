pub mod google_drive;
pub mod nextcloud;
pub mod oauth;
pub mod onedrive;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use google_drive::{GoogleDriveAuth, GoogleDriveProfile};
pub use nextcloud::{NextcloudAuth, NextcloudProfile};
pub use onedrive::{OneDriveAuth, OneDriveProfile};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProfile {
    Nextcloud(NextcloudProfile),
    GoogleDrive(GoogleDriveProfile),
    OneDrive(OneDriveProfile),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    Rename,
    Overwrite,
    Fail,
}

impl Default for CollisionPolicy {
    fn default() -> Self {
        Self::Rename
    }
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("nextcloud: {0}")]
    Nextcloud(#[from] nextcloud::NextcloudError),

    #[error("google_drive: {0}")]
    GoogleDrive(#[from] google_drive::GoogleDriveError),

    #[error("onedrive: {0}")]
    OneDrive(#[from] onedrive::OneDriveError),
}
