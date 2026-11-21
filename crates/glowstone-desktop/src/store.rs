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
    /// IDs of user-pinned profiles. Missing in older state.json files is supported.
    pub favorite_instance_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentPlay {
    pub instance_id: String,
    pub launched_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct RecentPlayState {
    entries: Vec<RecentPlay>,
}

const MAX_RECENT_PLAYS: usize = 12;

/// Stores recent launches separately from state.json. The launch process can update
/// history while the UI is waiting for the game without overwriting account/settings data.
pub fn load_recent_plays() -> Vec<RecentPlay> {
    let Some(path) = app_data_dir().ok().map(|dir| dir.join("recent-plays.json")) else {
        return Vec::new();
    };
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RecentPlayState>(&bytes).ok())
        .map(|history| history.entries)
        .unwrap_or_default()
}

pub fn record_recent_play(instance_id: &str) -> Result<(), String> {
    Uuid::parse_str(instance_id)
        .map_err(|_| "Некорректный идентификатор сборки в истории запусков.".to_owned())?;
    let path = app_data_dir()?.join("recent-plays.json");
    let parent = path.parent()
        .ok_or_else(|| "Некорректный каталог истории запусков.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Не удалось создать каталог истории запусков: {error}"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut history = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RecentPlayState>(&bytes).ok())
        .unwrap_or_default();
    history.entries.retain(|entry| entry.instance_id != instance_id);
    history.entries.insert(0, RecentPlay {
        instance_id: instance_id.to_owned(),
        launched_at: now,
    });
    history.entries.truncate(MAX_RECENT_PLAYS);

    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&history)
        .map_err(|error| format!("Не удалось подготовить историю запусков: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось записать историю запусков: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось сохранить историю запусков: {error}"))?;
    Ok(())
}

/// Counts only package files belonging to the selected platform's mod directory.
pub fn installed_mod_count(instance_id: &str, platform: GamePlatform) -> usize {
    let Ok(directory) = instance_directory(instance_id) else {
        return 0;
    };
    let (folder, extension) = match platform {
        GamePlatform::Java => ("mods", "jar"),
        GamePlatform::Horizon | GamePlatform::HorizonOffline => ("horizon-mods", "hmod"),
    };
    fs::read_dir(directory.join(folder))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file()
            && entry.path().extension().is_some_and(|value| value.eq_ignore_ascii_case(extension)))
        .count()
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
        .map_err(|_| "Некорректный UUID игрового аккаунта.".to_owned())?;
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

/// Imports a remotely managed skin once and makes that entry the active local preview.
pub fn import_skin_if_missing(
    account_uuid: &str,
    name: &str,
    bytes: &[u8],
    model: SkinModel,
) -> Result<SkinCollection, String> {
    let collection = load_skin_collection(account_uuid)?;
    for skin in &collection.skins {
        let path = skin_file_path(account_uuid, &skin.id)?;
        if fs::read(path).is_ok_and(|stored| stored == bytes) {
            return set_active_skin(account_uuid, &skin.id);
        }
    }
    import_skin(account_uuid, name, bytes, model)
}

pub fn set_active_skin(account_uuid: &str, skin_id: &str) -> Result<SkinCollection, String> {
    let mut collection = load_skin_collection(account_uuid)?;
    if !collection.skins.iter().any(|skin| skin.id == skin_id) {
        return Err("Скин не найден в этом аккаунте.".to_owned());
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
        .ok_or_else(|| "Скин не найден в коллекции этого аккаунта.".to_owned())?;
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
        .ok_or_else(|| "Скин не найден в коллекции этого аккаунта.".to_owned())?;
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


/// Decodes a stored skin into RGBA pixels for the 3D wardrobe preview.
/// The decoded texture is cached by the UI, so this is not repeated on every redraw.
pub fn load_skin_pixels(account_uuid: &str, skin_id: &str) -> Result<std::sync::Arc<Vec<[u8; 4]>>, String> {
    let path = skin_file_path(account_uuid, skin_id)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("Не удалось прочитать PNG скина: {error}"))?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("Не удалось декодировать PNG скина: {error}"))?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("Не удалось декодировать PNG скина: {error}"))?;
    if output.width != 64 || !matches!(output.height, 32 | 64) {
        return Err("Для 3D-предпросмотра нужен скин 64×64 или 64×32.".to_owned());
    }

    let mut pixels = Vec::with_capacity((output.width * output.height) as usize);
    let decoded = &buffer[..output.buffer_size()];
    match output.color_type {
        png::ColorType::Rgba => {
            for px in decoded.chunks_exact(4) {
                pixels.push([px[0], px[1], px[2], px[3]]);
            }
        }
        png::ColorType::Rgb => {
            for px in decoded.chunks_exact(3) {
                pixels.push([px[0], px[1], px[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for value in decoded {
                pixels.push([*value, *value, *value, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for px in decoded.chunks_exact(2) {
                pixels.push([px[0], px[0], px[0], px[1]]);
            }
        }
        png::ColorType::Indexed => {
            return Err("Не удалось преобразовать палитровый PNG скина.".to_owned());
        }
    }
    Ok(std::sync::Arc::new(pixels))
}

/// Returns the active skin file and its model for game launch integration.
pub fn active_skin_file(account_uuid: &str) -> Result<Option<(PathBuf, SkinModel)>, String> {
    let collection = load_skin_collection(account_uuid)?;
    let Some(active_id) = collection.active_skin_id.as_deref() else {
        return Ok(None);
    };
    let skin = collection.skins.iter()
        .find(|skin| skin.id == active_id)
        .ok_or_else(|| "Активный скин отсутствует в коллекции аккаунта.".to_owned())?;
    let path = skin_file_path(account_uuid, &skin.id)?;
    if !path.is_file() {
        return Err("Файл активного скина отсутствует. Импортируй его заново.".to_owned());
    }
    Ok(Some((path, skin.model)))
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

    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![0; (width * height * 4) as usize])
                .unwrap();
            writer.finish().unwrap();
        }
        bytes
    }

    #[test]
    fn skin_validation_accepts_standard_and_legacy_dimensions() {
        assert!(!validate_skin_png(&make_test_png(64, 64)).unwrap());
        assert!(validate_skin_png(&make_test_png(64, 32)).unwrap());
    }

    #[test]
    fn skin_validation_rejects_wrong_dimensions_and_corrupt_data() {
        assert!(validate_skin_png(&make_test_png(128, 128)).is_err());
        assert!(validate_skin_png(b"not a png").is_err());
    }

    #[test]
    fn skin_names_are_trimmed_and_bounded() {
        assert_eq!(clean_skin_name("  Explorer  ").unwrap(), "Explorer");
        assert!(clean_skin_name("").is_err());
        assert!(clean_skin_name("x".repeat(33).as_str()).is_err());
    }
}
