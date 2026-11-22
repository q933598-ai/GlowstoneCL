use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use directories::BaseDirs;
use glowstone_core::{GameInstance, GamePlatform, InstanceId, InstanceState, LoaderKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
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
    /// User-selected local wallpaper. No image data is copied or uploaded.
    pub background_image: Option<String>,
    /// Opt-in local error diagnostics. Nothing is transmitted to a server.
    pub telemetry_enabled: bool,
    pub language: String,
    pub discord_rpc_enabled: bool,
    pub discord_client_id: String,
    pub show_update_notes: bool,
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

/// Per-build Java/runtime settings. Missing fields in older instance-settings.json
/// files are filled from these defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceSettings {
    pub memory_min: String,
    pub memory_max: String,
    /// Empty means use the runtime selected by Mojang/CrustCore.
    pub java_path: String,
    /// One JVM argument per line; shell parsing is deliberately not used.
    pub jvm_args: String,
    /// One game argument per line.
    pub game_args: String,
    pub resolution_width: String,
    pub resolution_height: String,
    /// KEY=VALUE per line. Do not store passwords or access tokens here.
    pub environment_variables: String,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            memory_min: "1G".to_owned(),
            memory_max: "4G".to_owned(),
            java_path: String::new(),
            jvm_args: String::new(),
            game_args: String::new(),
            resolution_width: "1280".to_owned(),
            resolution_height: "720".to_owned(),
            environment_variables: String::new(),
        }
    }
}

impl InstanceSettings {
    pub fn validate(&self) -> Result<(), String> {
        let min = parse_memory_size(&self.memory_min)
            .ok_or_else(|| "Минимальная память должна иметь формат вроде 512M или 1G.".to_owned())?;
        let max = parse_memory_size(&self.memory_max)
            .ok_or_else(|| "Максимальная память должна иметь формат вроде 4G или 6144M.".to_owned())?;
        if min > max {
            return Err("Минимальная память не может быть больше максимальной.".to_owned());
        }

        let width = parse_resolution(&self.resolution_width)?;
        let height = parse_resolution(&self.resolution_height)?;
        if width.is_some() != height.is_some() {
            return Err("Укажи одновременно ширину и высоту разрешения или оставь оба поля пустыми.".to_owned());
        }

        if !self.java_path.trim().is_empty() && !std::path::Path::new(self.java_path.trim()).is_file() {
            return Err("Указанный Java executable не найден. Проверь путь к java.".to_owned());
        }

        self.environment_entries()?;
        validate_argument_lines(&self.jvm_args, "JVM")?;
        validate_argument_lines(&self.game_args, "игровые")?;
        Ok(())
    }

    pub fn environment_entries(&self) -> Result<Vec<(String, String)>, String> {
        let mut entries = Vec::new();
        for (index, line) in self.environment_variables.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or_else(|| {
                format!("Переменная окружения в строке {} должна иметь формат KEY=VALUE.", index + 1)
            })?;
            let valid_key = !key.is_empty()
                && key.bytes().enumerate().all(|(i, byte)| {
                    byte == b'_'
                        || byte.is_ascii_alphabetic()
                        || (i > 0 && byte.is_ascii_digit())
                });
            if !valid_key || value.contains('\0') {
                return Err(format!("Некорректная переменная окружения в строке {}.", index + 1));
            }
            entries.push((key.to_owned(), value.to_owned()));
        }
        Ok(entries)
    }

    pub fn jvm_argument_list(&self) -> Result<Vec<String>, String> {
        validate_argument_lines(&self.jvm_args, "JVM")?;
        Ok(self.jvm_args.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect())
    }

    pub fn game_argument_list(&self) -> Result<Vec<String>, String> {
        validate_argument_lines(&self.game_args, "игровые")?;
        Ok(self.game_args.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect())
    }

    pub fn resolution(&self) -> Result<Option<(u32, u32)>, String> {
        let width = parse_resolution(&self.resolution_width)?;
        let height = parse_resolution(&self.resolution_height)?;
        match (width, height) {
            (Some(width), Some(height)) => Ok(Some((width, height))),
            (None, None) => Ok(None),
            _ => Err("Укажи одновременно ширину и высоту разрешения.".to_owned()),
        }
    }
}

fn parse_memory_size(value: &str) -> Option<u64> {
    let value = value.trim().to_ascii_uppercase();
    let (number, unit) = value.split_at(value.find(|ch: char| !ch.is_ascii_digit())?);
    let amount: u64 = number.parse().ok()?;
    match unit {
        "M" | "MB" => amount.checked_mul(1024),
        "G" | "GB" => amount.checked_mul(1024 * 1024),
        _ => None,
    }
}

fn parse_resolution(value: &str) -> Result<Option<u32>, String> {
    let value = value.trim();
    if value.is_empty() || value == "0" {
        return Ok(None);
    }
    let parsed: u32 = value.parse()
        .map_err(|_| "Разрешение должно состоять из целых чисел.".to_owned())?;
    if !(320..=16_384).contains(&parsed) {
        return Err("Ширина и высота должны быть от 320 до 16384 пикселей.".to_owned());
    }
    Ok(Some(parsed))
}

fn validate_argument_lines(value: &str, label: &str) -> Result<(), String> {
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.contains('\0') || line.len() > 4096 {
            return Err(format!("Некорректный {label}-аргумент в строке {}.", index + 1));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFileEntry {
    pub name: String,
    pub size: u64,
    pub modified_unix: u64,
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
            && entry.path().extension().and_then(|value| value.to_str()).is_some_and(|value| value.eq_ignore_ascii_case(extension)))
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

/// Appends a privacy-minimised local diagnostic after the user explicitly opts in.
/// No telemetry is sent over the network by this function.
pub fn append_local_diagnostic(message: &str) -> Result<(), String> {
    use std::io::Write as _;

    let directory = app_data_dir()?.join("diagnostics");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось создать каталог диагностики: {error}"))?;
    let path = directory.join("errors.log");
    let home = std::env::var("HOME").ok();
    let mut safe = message
        .replace("Bearer ", "Bearer [redacted] ")
        .replace("access_token=", "access_token=[redacted]")
        .replace("refresh_token=", "refresh_token=[redacted]")
        .replace("client_secret=", "client_secret=[redacted]");
    if let Some(home) = home.as_deref().filter(|value| !value.is_empty()) {
        safe = safe.replace(home, "~");
    }
    if safe.len() > 8_000 {
        safe.truncate(8_000);
        safe.push_str("… [truncated]");
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Не удалось открыть журнал диагностики: {error}"))?;
    writeln!(file, "[{timestamp}] {} · {safe}", std::env::consts::OS)
        .map_err(|error| format!("Не удалось записать диагностику: {error}"))
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
        loader_version: None,
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

pub fn save_instance_metadata(instance: &GameInstance) -> Result<(), String> {
    instance.validate().map_err(|error| error.to_string())?;
    let path = instance_directory(&instance.id.0)?.join("instance.json");
    let bytes = serde_json::to_vec_pretty(instance)
        .map_err(|error| format!("Не удалось сериализовать метаданные сборки: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось записать метаданные сборки: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось сохранить метаданные сборки: {error}"))?;
    Ok(())
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


/// Loads settings stored beside this instance's isolated game directory.
pub fn load_instance_settings(instance_id: &str) -> Result<InstanceSettings, String> {
    let path = instance_directory(instance_id)?.join("instance-settings.json");
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("Не удалось прочитать настройки сборки: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(InstanceSettings::default()),
        Err(error) => Err(format!("Не удалось открыть настройки сборки: {error}")),
    }
}

pub fn save_instance_settings(instance_id: &str, settings: &InstanceSettings) -> Result<(), String> {
    settings.validate()?;
    let directory = instance_directory(instance_id)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось создать папку сборки: {error}"))?;
    let path = directory.join("instance-settings.json");
    let temporary = directory.join("instance-settings.json.tmp");
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Не удалось сериализовать настройки сборки: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось записать настройки сборки: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось сохранить настройки сборки: {error}"))
}

pub fn list_instance_folder(instance_id: &str, folder: &str) -> Result<Vec<LocalFileEntry>, String> {
    if !matches!(folder, "saves" | "screenshots") {
        return Err("Недопустимый каталог содержимого сборки.".to_owned());
    }
    let directory = instance_directory(instance_id)?.join(folder);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось открыть каталог {folder}: {error}"))?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| format!("Не удалось прочитать каталог {folder}: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Не удалось прочитать запись каталога: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Не удалось прочитать свойства файла: {error}"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let is_valid = if folder == "saves" {
            metadata.is_dir()
        } else {
            metadata.is_file() && path.extension().and_then(|value| value.to_str())
                .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
        };
        if !is_valid {
            continue;
        }
        let modified_unix = metadata.modified().ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        entries.push(LocalFileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            size: metadata.len(),
            modified_unix,
        });
    }
    entries.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

pub fn remove_instance_world(instance_id: &str, world_name: &str) -> Result<(), String> {
    validate_single_component(world_name)?;
    let path = instance_directory(instance_id)?.join("saves").join(world_name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("Не удалось найти мир: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Можно удалять только обычные папки миров внутри saves.".to_owned());
    }
    fs::remove_dir_all(&path)
        .map_err(|error| format!("Не удалось удалить мир {world_name}: {error}"))
}

pub fn remove_instance_screenshot(instance_id: &str, file_name: &str) -> Result<(), String> {
    validate_single_component(file_name)?;
    let path = instance_directory(instance_id)?.join("screenshots").join(file_name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("Не удалось найти скриншот: {error}"))?;
    let valid_extension = path.extension().and_then(|value| value.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"));
    if metadata.file_type().is_symlink() || !metadata.is_file() || !valid_extension {
        return Err("Можно удалять только изображения из каталога screenshots.".to_owned());
    }
    fs::remove_file(&path)
        .map_err(|error| format!("Не удалось удалить скриншот: {error}"))
}

fn validate_single_component(value: &str) -> Result<(), String> {
    let path = std::path::Path::new(value);
    if value.is_empty()
        || value == "." || value == ".."
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(std::path::Component::Normal(_)))
        || value.contains('/') || value.contains('\\') || value.contains('\0')
    {
        return Err("Недопустимое имя файла или папки.".to_owned());
    }
    Ok(())
}

/// Opens only a folder inside the selected instance with the platform file manager.
pub fn open_instance_folder(instance_id: &str, folder: &str) -> Result<(), String> {
    if !matches!(folder, "saves" | "screenshots" | "mods" | "logs") {
        return Err("Недопустимый каталог сборки.".to_owned());
    }
    let path = instance_directory(instance_id)?.join(folder);
    fs::create_dir_all(&path)
        .map_err(|error| format!("Не удалось создать каталог: {error}"))?;

    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer");
        command.arg(&path);
        command
    };
    #[cfg(not(target_os = "windows"))]
    command.arg(&path);

    command.spawn()
        .map_err(|error| format!("Не удалось открыть папку {}: {error}", path.display()))?;
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

/// Fingerprints the decoded appearance rather than PNG file bytes. Microsoft can
/// return the same skin re-encoded with different PNG filters or colour types.
fn skin_fingerprint(bytes: &[u8]) -> Result<[u8; 32], String> {
    validate_skin_png(bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("Не удалось прочитать PNG скина: {error}"))?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("Не удалось декодировать скин: {error}"))?;
    let decoded = &buffer[..output.buffer_size()];
    let mut rgba = Vec::with_capacity((output.width * output.height * 4) as usize);
    match output.color_type {
        png::ColorType::Rgba => {
            for pixel in decoded.chunks_exact(4) {
                if pixel[3] == 0 {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                } else {
                    rgba.extend_from_slice(pixel);
                }
            }
        }
        png::ColorType::Rgb => {
            for pixel in decoded.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for value in decoded {
                rgba.extend_from_slice(&[*value, *value, *value, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in decoded.chunks_exact(2) {
                if pixel[1] == 0 {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                } else {
                    rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
                }
            }
        }
        png::ColorType::Indexed => {
            return Err("Не удалось нормализовать палитровый PNG скина.".to_owned());
        }
    }

    let mut digest = Sha256::new();
    digest.update(output.width.to_le_bytes());
    digest.update(output.height.to_le_bytes());
    digest.update(rgba);
    Ok(digest.finalize().into())
}

/// Imports a remotely managed skin once and makes that entry the active local preview.
/// Matching decoded pixels also collapse duplicates created by older byte-only sync logic.
pub fn import_skin_if_missing(
    account_uuid: &str,
    name: &str,
    bytes: &[u8],
    model: SkinModel,
) -> Result<SkinCollection, String> {
    let fingerprint = skin_fingerprint(bytes)?;
    let mut collection = load_skin_collection(account_uuid)?;
    let mut matching_ids = Vec::new();

    for skin in &collection.skins {
        let path = skin_file_path(account_uuid, &skin.id)?;
        if let Ok(stored) = fs::read(path) {
            if skin_fingerprint(&stored).is_ok_and(|existing| existing == fingerprint) {
                matching_ids.push(skin.id.clone());
            }
        }
    }

    if let Some(canonical_id) = matching_ids.first().cloned() {
        for duplicate_id in matching_ids.iter().skip(1) {
            let path = skin_file_path(account_uuid, duplicate_id)?;
            if path.is_file() {
                fs::remove_file(path)
                    .map_err(|error| format!("Не удалось удалить дубликат скина: {error}"))?;
            }
            collection.skins.retain(|skin| &skin.id != duplicate_id);
        }
        collection.active_skin_id = Some(canonical_id);
        save_skin_collection(account_uuid, &collection)?;
        return Ok(collection);
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

const CURSEFORGE_KEYRING_ID: &str = "curseforge-api-key";

pub fn store_curseforge_api_key(api_key: &str) -> Result<(), String> {
    let key = api_key.trim();
    if !(16..=512).contains(&key.len()) || key.chars().any(char::is_control) {
        return Err("CurseForge API key должен содержать от 16 до 512 символов.".to_owned());
    }
    let entry = keyring::Entry::new("GlowstoneCL", CURSEFORGE_KEYRING_ID)
        .map_err(|error| format!("Не удалось открыть системное хранилище: {error}"))?;
    entry.set_password(key)
        .map_err(|error| format!("Не удалось сохранить CurseForge API key: {error}"))
}

pub fn load_curseforge_api_key() -> Result<String, String> {
    let entry = keyring::Entry::new("GlowstoneCL", CURSEFORGE_KEYRING_ID)
        .map_err(|error| format!("Не удалось открыть системное хранилище: {error}"))?;
    entry.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => "CurseForge API key ещё не добавлен.".to_owned(),
        other => format!("Не удалось прочитать CurseForge API key: {other}"),
    })
}

pub fn remove_curseforge_api_key() {
    if let Ok(entry) = keyring::Entry::new("GlowstoneCL", CURSEFORGE_KEYRING_ID) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_instance_json_without_loader_pin_still_deserializes() {
        let instance: GameInstance = serde_json::from_str(
            r#"{
                "id": "123e4567-e89b-12d3-a456-426614174000",
                "name": "Legacy Fabric",
                "platform": "java",
                "game_version": "1.20.1",
                "loader": "fabric",
                "state": "not_installed"
            }"#
        ).unwrap();
        assert_eq!(instance.loader_version, None);
        assert_eq!(instance.loader, LoaderKind::Fabric);
    }

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

    fn make_test_rgb_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut pixels = Vec::with_capacity((width * height * 3) as usize);
            for _ in 0..width * height {
                pixels.extend_from_slice(&rgba[..3]);
            }
            writer.write_image_data(&pixels).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }

    fn make_test_rgba_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut pixels = Vec::with_capacity((width * height * 4) as usize);
            for _ in 0..width * height {
                pixels.extend_from_slice(&rgba);
            }
            writer.write_image_data(&pixels).unwrap();
            writer.finish().unwrap();
        }
        bytes
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
    fn skin_fingerprint_ignores_png_encoding_and_colour_type() {
        let rgb = make_test_rgb_png(64, 64, [90, 120, 150, 255]);
        let rgba = make_test_rgba_png(64, 64, [90, 120, 150, 255]);
        assert_ne!(rgb, rgba, "the encoded PNG files should differ");
        assert_eq!(skin_fingerprint(&rgb).unwrap(), skin_fingerprint(&rgba).unwrap());
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
