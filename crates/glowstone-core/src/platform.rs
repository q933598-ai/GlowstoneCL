use serde::{Deserialize, Serialize};

/// Game runtimes supported by GlowstoneCL.
///
/// Java and Horizon deliberately have separate mod execution models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePlatform {
    Java,
    Horizon,
}

impl GamePlatform {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Java => "Minecraft Java",
            Self::Horizon => "Horizon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoaderKind {
    Vanilla,
    Forge,
    NeoForge,
    Fabric,
    Quilt,
    HorizonContent,
    HorizonNative,
}

impl LoaderKind {
    pub const fn is_valid_for(self, platform: GamePlatform) -> bool {
        matches!(
            (platform, self),
            (
                GamePlatform::Java,
                Self::Vanilla | Self::Forge | Self::NeoForge | Self::Fabric | Self::Quilt
            ) | (
                GamePlatform::Horizon,
                Self::HorizonContent | Self::HorizonNative
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_and_horizon_loaders_are_separate() {
        assert!(LoaderKind::Fabric.is_valid_for(GamePlatform::Java));
        assert!(!LoaderKind::Fabric.is_valid_for(GamePlatform::Horizon));
        assert!(LoaderKind::HorizonNative.is_valid_for(GamePlatform::Horizon));
        assert!(!LoaderKind::HorizonNative.is_valid_for(GamePlatform::Java));
    }
}
