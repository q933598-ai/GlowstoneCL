use serde::{Deserialize, Serialize};

use crate::{GamePlatform, LoaderKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    NotInstalled,
    Ready,
    Broken,
    Updating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInstance {
    pub id: InstanceId,
    pub name: String,
    pub platform: GamePlatform,
    pub game_version: String,
    pub loader: LoaderKind,
    /// Exact loader build pinned by imported modpacks; absent for ordinary profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    pub state: InstanceState,
}

impl GameInstance {
    pub fn validate(&self) -> Result<(), InstanceValidationError> {
        if self.name.trim().is_empty() {
            return Err(InstanceValidationError::EmptyName);
        }

        if !self.loader.is_valid_for(self.platform) {
            return Err(InstanceValidationError::LoaderPlatformMismatch {
                platform: self.platform,
                loader: self.loader,
            });
        }

        if let Some(version) = self.loader_version.as_deref() {
            if version.is_empty()
                || version.len() > 128
                || !version.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
                })
            {
                return Err(InstanceValidationError::InvalidLoaderVersion);
            }
            if self.platform != GamePlatform::Java || self.loader == LoaderKind::Vanilla {
                return Err(InstanceValidationError::LoaderVersionRequiresModdedJava);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InstanceValidationError {
    #[error("Instance name must not be empty")]
    EmptyName,
    #[error("Loader {loader:?} is incompatible with platform {platform:?}")]
    LoaderPlatformMismatch {
        platform: GamePlatform,
        loader: LoaderKind,
    },
    #[error("Loader build identifier contains unsupported characters or is too long")]
    InvalidLoaderVersion,
    #[error("An exact loader build can only be pinned to a modded Java profile")]
    LoaderVersionRequiresModdedJava,
}
