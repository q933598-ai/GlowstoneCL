//! Domain types shared by the GlowstoneCL desktop app and services.

mod instance;
mod platform;

pub use instance::{GameInstance, InstanceId, InstanceState, InstanceValidationError};
pub use platform::{GamePlatform, LoaderKind};
