use std::fs;
use std::path::PathBuf;

use directories::BaseDirs;
use glowstone_core::{GameInstance, GamePlatform, InstanceId, InstanceState, LoaderKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Offline,
    Microsoft,
}

impl AccountKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "Offline",
            Self::Microsoft => "Microsoft",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountProfile {
    pub id: String,
    pub name: String,
    pub uuid: String,
    pub kind: AccountKind,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub microsoft_client_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedState {
    pub instances: Vec<GameInstance>,
    pub accounts: Vec<AccountProfile>,
    pub settings: AppSettings,
}

pub fn load() -> PersistedState {
    let Some(path) = state_path() else {
        return PersistedState::default();
    };

    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => PersistedState::default(),
    }
}

pub fn save(state: &PersistedState) -> Result<(), String> {
    let path = state_path().ok_or_else(|| {
        "Не удалось определить каталог данных GlowstoneCL.".to_owned()
    })?;

    let parent = path
        .parent()
        .ok_or_else(|| "Некорректный путь файла настроек.".to_owned())?;

    fs::create_dir_all(parent)
        .map_err(|error| format!("Не удалось создать каталог данных: {error}"))?;

    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("Не удалось сериализовать настройки: {error}"))?;

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось записать настройки: {error}"))?;

    fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось сохранить настройки: {error}"))?;

    Ok(())
}

fn state_path() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| {
        dirs.data_local_dir()
            .join("GlowstoneCL")
            .join("state.json")
    })
}

pub fn create_instance(
    name: String,
    version: String,
    platform: GamePlatform,
    loader: LoaderKind,
) -> Result<GameInstance, String> {
    let instance = GameInstance {
        id: InstanceId(Uuid::new_v4().to_string()),
        name,
        platform,
        game_version: version,
        loader,
        state: InstanceState::NotInstalled,
    };

    instance
        .validate()
        .map_err(|error| error.to_string())?;

    Ok(instance)
}

pub fn create_offline_account(name: &str) -> Result<AccountProfile, String> {
    let name = name.trim();

    if !(3..=16).contains(&name.len())
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(
            "Ник должен содержать 3–16 символов: латинские буквы, цифры или _.".to_owned(),
        );
    }

    let digest = md5::compute(format!("OfflinePlayer:{name}"));
    let mut uuid_bytes = digest.0;
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x30;
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
    let player_uuid = Uuid::from_bytes(uuid_bytes).to_string();

    Ok(AccountProfile {
        id: format!("offline:{player_uuid}"),
        name: name.to_owned(),
        uuid: player_uuid,
        kind: AccountKind::Offline,
    })
}

pub fn store_refresh_token(account_id: &str, refresh_token: &str) -> Result<(), String> {
    let entry = keyring::Entry::new("GlowstoneCL", account_id)
        .map_err(|error| format!("Не удалось открыть системное хранилище ключей: {error}"))?;

    entry
        .set_password(refresh_token)
        .map_err(|error| format!("Не удалось сохранить токен в системном хранилище: {error}"))
}

pub fn remove_refresh_token(account_id: &str) {
    if let Ok(entry) = keyring::Entry::new("GlowstoneCL", account_id) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_account_uses_validated_name_and_deterministic_uuid() {
        let first = create_offline_account("GlowPlayer").unwrap();
        let second = create_offline_account("GlowPlayer").unwrap();
        assert_eq!(first.uuid, second.uuid);
        assert_eq!(first.kind, AccountKind::Offline);
    }

    #[test]
    fn offline_account_rejects_invalid_names() {
        assert!(create_offline_account("ab").is_err());
        assert!(create_offline_account("bad name").is_err());
        assert!(create_offline_account("Glow_Player").is_ok());
    }

    #[test]
    fn loader_validation_is_platform_specific() {
        assert!(create_instance(
            "Fabric test".into(),
            "1.21.1".into(),
            GamePlatform::Java,
            LoaderKind::Fabric,
        ).is_ok());

        assert!(create_instance(
            "Invalid Horizon".into(),
            "stable".into(),
            GamePlatform::Horizon,
            LoaderKind::Fabric,
        ).is_err());
    }
}
