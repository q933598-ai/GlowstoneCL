use std::fs;
use std::io::Cursor;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinModel {
    Classic,
    Slim,
}

impl SkinModel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "Steve · Classic",
            Self::Slim => "Alex · Slim",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkinEntry {
    pub id: String,
    pub name: String,
    pub model: SkinModel,
    /// Legacy 64x32 skins are accepted for compatibility.
    pub legacy: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkinCollection {
    pub skins: Vec<SkinEntry>,
    pub active_skin_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub microsoft_client_id: String,
    pub selected_account_id: Option<String>,
    pub selected_instance_id: Option<String>,
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

pub fn app_data_dir() -> Result<PathBuf, String> {
    let dirs = BaseDirs::new()
        .ok_or_else(|| "Не удалось определить каталог данных GlowstoneCL.".to_owned())?;
    Ok(dirs.data_local_dir().join("GlowstoneCL"))
}

fn state_path() -> Option<PathBuf> {
    app_data_dir().ok().map(|dir| dir.join("state.json"))
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

    let directory = instance_directory(&instance.id.0)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось создать папку сборки: {error}"))?;

    // Each profile gets its own workspace, so Java/Horizon files never overlap.
    for child in [
        "mods",
        "config",
        "resourcepacks",
        "shaderpacks",
        "saves",
        "logs",
        "horizon-mods",
        "horizon-native-mods",
    ] {
        fs::create_dir_all(directory.join(child))
            .map_err(|error| format!("Не удалось создать каталог {child}: {error}"))?;
    }

    let metadata = serde_json::to_vec_pretty(&instance)
        .map_err(|error| format!("Не удалось сериализовать сборку: {error}"))?;
    fs::write(directory.join("instance.json"), metadata)
        .map_err(|error| format!("Не удалось записать instance.json: {error}"))?;

    Ok(instance)
}

pub fn remove_instance(instance_id: &str) -> Result<(), String> {
    let directory = instance_directory(instance_id)?;
    if !directory.exists() {
        return Ok(());
    }

    // Keep worlds and configs recoverable: deleting a profile from the library
    // moves its workspace into local trash instead of permanently purging it.
    let base = BaseDirs::new()
        .ok_or_else(|| "Не удалось определить каталог данных GlowstoneCL.".to_owned())?;
    let instances_root = base.data_local_dir().join("GlowstoneCL").join("instances");
    let trash = instances_root.join(".trash");
    fs::create_dir_all(&trash)
        .map_err(|error| format!("Не удалось создать корзину сборок: {error}"))?;

    let id = Uuid::parse_str(instance_id)
        .map_err(|_| "Некорректный идентификатор сборки.".to_owned())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let destination = trash.join(format!("{}-{timestamp}", id));

    fs::rename(&directory, &destination)
        .map_err(|error| format!("Не удалось переместить сборку в корзину: {error}"))?;

    Ok(())
}

pub fn instance_directory(instance_id: &str) -> Result<PathBuf, String> {
    // Instance identifiers are UUIDs generated by GlowstoneCL. Parse them before
    // joining a path to prevent path traversal from a modified state.json.
    let id = Uuid::parse_str(instance_id)
        .map_err(|_| "Некорректный идентификатор сборки.".to_owned())?;
    let base = BaseDirs::new()
        .ok_or_else(|| "Не удалось определить каталог данных GlowstoneCL.".to_owned())?;

    Ok(base
        .data_local_dir()
        .join("GlowstoneCL")
        .join("instances")
        .join(id.to_string()))
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

const MAX_SKIN_BYTES: usize = 2 * 1024 * 1024;

fn skin_account_directory(account_uuid: &str) -> Result<PathBuf, String> {
    let uuid = Uuid::parse_str(account_uuid)
        .map_err(|_| "Некорректный UUID offline-аккаунта.".to_owned())?;
    Ok(app_data_dir()?.join("skins").join(uuid.to_string()))
}

fn skin_collection_path(account_uuid: &str) -> Result<PathBuf, String> {
    Ok(skin_account_directory(account_uuid)?.join("skins.json"))
}

fn save_skin_collection(account_uuid: &str, collection: &SkinCollection) -> Result<(), String> {
    let directory = skin_account_directory(account_uuid)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось создать каталог скинов: {error}"))?;
    let path = directory.join("skins.json");
    let temporary = directory.join("skins.json.tmp");
    let bytes = serde_json::to_vec_pretty(collection)
        .map_err(|error| format!("Не удалось подготовить список скинов: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось сохранить список скинов: {error}"))?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Не удалось заменить список скинов: {error}"))?;
    }
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось завершить сохранение скинов: {error}"))
}

pub fn load_skin_collection(account_uuid: &str) -> Result<SkinCollection, String> {
    let path = skin_collection_path(account_uuid)?;
    if !path.exists() {
        return Ok(SkinCollection::default());
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Не удалось прочитать список скинов: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Список скинов повреждён: {error}"))
}

fn validate_skin_png(bytes: &[u8]) -> Result<bool, String> {
    if bytes.is_empty() || bytes.len() > MAX_SKIN_BYTES {
        return Err("PNG должен быть размером не более 2 МБ.".to_owned());
    }

    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("Не удалось прочитать PNG: {error}"))?;
    let width = reader.info().width;
    let height = reader.info().height;
    if width != 64 || !matches!(height, 64 | 32) {
        return Err(format!(
            "Неверный размер скина: {}×{}. Поддерживаются PNG 64×64 и 64×32.",
            width, height
        ));
    }

    let mut decoded = vec![0; reader.output_buffer_size()];
    reader
        .next_frame(&mut decoded)
        .map_err(|error| format!("PNG повреждён или не может быть декодирован: {error}"))?;

    Ok(height == 32)
}

fn skin_file_path(account_uuid: &str, skin_id: &str) -> Result<PathBuf, String> {
    let account_directory = skin_account_directory(account_uuid)?;
    let skin_id = Uuid::parse_str(skin_id)
        .map_err(|_| "Некорректный идентификатор скина.".to_owned())?;
    Ok(account_directory.join(format!("{skin_id}.png")))
}

fn clean_skin_name(input: &str) -> Result<String, String> {
    let name = input.trim();
    if name.is_empty() || name.chars().count() > 32 || name.chars().any(char::is_control) {
        return Err("Название скина должно содержать от 1 до 32 обычных символов.".to_owned());
    }
    Ok(name.to_owned())
}

pub fn import_skin(
    account_uuid: &str,
    name: &str,
    bytes: &[u8],
    model: SkinModel,
) -> Result<SkinCollection, String> {
    let name = clean_skin_name(name)?;
    let legacy = validate_skin_png(bytes)?;
    let mut collection = load_skin_collection(account_uuid)?;
    let id = Uuid::new_v4().to_string();
    let directory = skin_account_directory(account_uuid)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось создать каталог скинов: {error}"))?;

    let target = skin_file_path(account_uuid, &id)?;
    let temporary = target.with_extension("png.tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось сохранить PNG скина: {error}"))?;
    fs::rename(&temporary, &target)
        .map_err(|error| format!("Не удалось завершить импорт скина: {error}"))?;

    collection.skins.push(SkinEntry {
        id: id.clone(),
        name,
        model,
        legacy,
    });
    if collection.active_skin_id.is_none() {
        collection.active_skin_id = Some(id);
    }
    if let Err(error) = save_skin_collection(account_uuid, &collection) {
        let _ = fs::remove_file(target);
        return Err(error);
    }
    Ok(collection)
}

pub fn set_active_skin(account_uuid: &str, skin_id: &str) -> Result<SkinCollection, String> {
    let mut collection = load_skin_collection(account_uuid)?;
    if !collection.skins.iter().any(|skin| skin.id == skin_id) {
        return Err("Скин не найден в этом offline-профиле.".to_owned());
    }
    let path = skin_file_path(account_uuid, skin_id)?;
    if !path.is_file() {
        return Err("Файл выбранного скина отсутствует. Импортируй его заново.".to_owned());
    }
    collection.active_skin_id = Some(skin_id.to_owned());
    save_skin_collection(account_uuid, &collection)?;
    Ok(collection)
}

pub fn rename_skin(
    account_uuid: &str,
    skin_id: &str,
    new_name: &str,
) -> Result<SkinCollection, String> {
    let new_name = clean_skin_name(new_name)?;
    let mut collection = load_skin_collection(account_uuid)?;
    let skin = collection
        .skins
        .iter_mut()
        .find(|skin| skin.id == skin_id)
        .ok_or_else(|| "Скин не найден в этом offline-профиле.".to_owned())?;
    skin.name = new_name;
    save_skin_collection(account_uuid, &collection)?;
    Ok(collection)
}

pub fn set_skin_model(
    account_uuid: &str,
    skin_id: &str,
    model: SkinModel,
) -> Result<SkinCollection, String> {
    let mut collection = load_skin_collection(account_uuid)?;
    let skin = collection
        .skins
        .iter_mut()
        .find(|skin| skin.id == skin_id)
        .ok_or_else(|| "Скин не найден в этом offline-профиле.".to_owned())?;
    skin.model = model;
    save_skin_collection(account_uuid, &collection)?;
    Ok(collection)
}

pub fn delete_skin(account_uuid: &str, skin_id: &str) -> Result<SkinCollection, String> {
    let mut collection = load_skin_collection(account_uuid)?;
    let Some(index) = collection.skins.iter().position(|skin| skin.id == skin_id) else {
        return Err("Скин уже удалён или не найден.".to_owned());
    };
    let path = skin_file_path(account_uuid, skin_id)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Не удалось удалить PNG скина: {error}"))?;
    }
    collection.skins.remove(index);
    if collection.active_skin_id.as_deref() == Some(skin_id) {
        collection.active_skin_id = collection.skins.first().map(|skin| skin.id.clone());
    }
    save_skin_collection(account_uuid, &collection)?;
    Ok(collection)
}

pub fn skin_image_path(account_uuid: &str, skin_id: &str) -> Result<PathBuf, String> {
    skin_file_path(account_uuid, skin_id)
}

pub fn store_refresh_token(account_id: &str, refresh_token: &str) -> Result<(), String> {
    let entry = keyring::Entry::new("GlowstoneCL", account_id)
        .map_err(|error| format!("Не удалось открыть системное хранилище ключей: {error}"))?;

    entry
        .set_password(refresh_token)
        .map_err(|error| format!("Не удалось сохранить токен в системном хранилище: {error}"))
}

pub fn load_refresh_token(account_id: &str) -> Result<String, String> {
    let entry = keyring::Entry::new("GlowstoneCL", account_id)
        .map_err(|error| format!("Не удалось открыть системное хранилище ключей: {error}"))?;

    entry.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => {
            "Для этого Microsoft-аккаунта не найден refresh token. Войди заново.".to_owned()
        }
        other => format!("Не удалось прочитать Microsoft refresh token: {other}"),
    })
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
