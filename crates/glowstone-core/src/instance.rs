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
    #[serde(default)]
    pub description: String,
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


#[cfg(test)]
mod tests {
    use super::*;

    fn instance(
        platform: GamePlatform,
        loader: LoaderKind,
        loader_version: Option<&str>,
    ) -> GameInstance {
        GameInstance {
            id: InstanceId("123e4567-e89b-12d3-a456-426614174000".to_owned()),
            name: "Test".to_owned(),
            description: String::new(),
            platform,
            game_version: "1.20.1".to_owned(),
            loader,
            loader_version: loader_version.map(str::to_owned),
            state: InstanceState::NotInstalled,
        }
    }

    #[test]
    fn accepts_an_exact_build_pin_for_a_modded_java_profile() {
        assert!(instance(GamePlatform::Java, LoaderKind::Fabric, Some("0.15.7")).validate().is_ok());
        assert!(instance(GamePlatform::Java, LoaderKind::Forge, Some("47.2.0-1.20.1")).validate().is_ok());
    }

    #[test]
    fn rejects_unsafe_or_incompatible_loader_build_pins() {
        assert_eq!(
            instance(GamePlatform::Java, LoaderKind::Fabric, Some("../outside"))
                .validate(),
            Err(InstanceValidationError::InvalidLoaderVersion),
        );
        assert_eq!(
            instance(GamePlatform::Horizon, LoaderKind::HorizonContent, Some("1.0"))
                .validate(),
            Err(InstanceValidationError::LoaderVersionRequiresModdedJava),
        );
        assert_eq!(
            instance(GamePlatform::Java, LoaderKind::Vanilla, Some("1.0"))
                .validate(),
            Err(InstanceValidationError::LoaderVersionRequiresModdedJava),
        );
    }
}
