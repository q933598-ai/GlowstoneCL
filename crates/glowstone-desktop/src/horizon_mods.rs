//! Horizon's reviewed content-mod registry and per-instance HMOD installation.
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use glowstone_core::{GameInstance, GamePlatform};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::store;

const INDEX_URL: &str = "https://horizon-rust.github.io/horizon-mods/index.json";
const GAME_VERSION: &str = "26.3";
const MAX_PACKAGE_SIZE: usize = 64 * 1024 * 1024;
const MAX_ICON_SIZE: usize = 5 * 1024 * 1024;
const MANIFEST_NAME: &str = ".glowstone-registry.json";

#[derive(Debug, Clone, Deserialize)]
pub struct HorizonModProject {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub stars: u64,
    #[serde(default)]
    pub versions: Vec<HorizonModVersion>,
    #[serde(skip)]
    pub icon_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HorizonModVersion {
    pub version: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub changelog: String,
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub side: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub game: String,
    pub file: HorizonFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HorizonFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RevokedFile {
    id: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryIndex {
    schema: u64,
    #[serde(default)]
    projects: Vec<HorizonModProject>,
    #[serde(default)]
    revoked: Vec<RevokedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledHorizonMod {
    pub id: String,
    pub name: String,
    pub version: String,
    pub file_name: String,
    pub sha256: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(skip)]
    pub icon_data: Option<Vec<u8>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct InstalledManifest {
    #[serde(default)]
    mods: Vec<InstalledHorizonMod>,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)")
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент Horizon Registry: {error}"))
}

fn index_cache_path() -> Result<PathBuf, String> {
    Ok(store::app_data_dir()?.join("horizon-registry").join("index.json"))
}

async fn read_index_cache(path: &Path) -> Result<RegistryIndex, String> {
    let bytes = tokio::fs::read(path).await
        .map_err(|error| format!("Не удалось прочитать кеш Horizon Registry: {error}"))?;
    let index: RegistryIndex = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Кеш Horizon Registry повреждён: {error}"))?;
    if index.schema != 1 {
        return Err(format!("Версия формата Horizon Registry {} не поддерживается.", index.schema));
    }
    Ok(index)
}

async fn registry_index() -> Result<RegistryIndex, String> {
    let cache_path = index_cache_path()?;
    let client = http_client()?;
    let response = client.get(INDEX_URL).send().await;
    let fetched = match response {
        Ok(response) => {
            let response = response.error_for_status()
                .map_err(|error| format!("Horizon Registry вернул ошибку: {error}"))?;
            let bytes = response.bytes().await
                .map_err(|error| format!("Не удалось прочитать Horizon Registry: {error}"))?;
            if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
                return Err("Horizon Registry вернул пустой или слишком большой индекс.".to_owned());
            }
            let index: RegistryIndex = serde_json::from_slice(&bytes)
                .map_err(|error| format!("Индекс Horizon Registry повреждён: {error}"))?;
            if index.schema != 1 {
                return Err(format!("Версия формата Horizon Registry {} не поддерживается.", index.schema));
            }
            Some((bytes.to_vec(), index))
        }
        Err(error) => {
            eprintln!("[GlowstoneCL] Horizon Registry недоступен, пробую кеш: {error}");
            None
        }
    };

    if let Some((bytes, index)) = fetched {
        let parent = cache_path.parent().ok_or_else(|| "Не найден каталог кеша Horizon Registry.".to_owned())?;
        tokio::fs::create_dir_all(parent).await
            .map_err(|error| format!("Не удалось создать кеш Horizon Registry: {error}"))?;
        let temporary = cache_path.with_extension("json.tmp");
        tokio::fs::write(&temporary, bytes).await
            .map_err(|error| format!("Не удалось сохранить индекс Horizon Registry: {error}"))?;
        tokio::fs::rename(&temporary, &cache_path).await
            .map_err(|error| format!("Не удалось обновить индекс Horizon Registry: {error}"))?;
        return Ok(index);
    }

    read_index_cache(&cache_path).await
        .map_err(|error| format!("Не удалось загрузить Horizon Registry и нет рабочего кеша. {error}"))
}

fn valid_mod_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn manifest_string_field<'a>(manifest: &'a str, key: &str) -> Option<&'a str> {
    manifest.lines().find_map(|line| {
        let line = line.split('#').next()?.trim();
        let (field, value) = line.split_once('=')?;
        if field.trim() != key {
            return None;
        }
        value.trim()
            .strip_prefix('"')?
            .strip_suffix('"')
    })
}

fn manifest_has_registry_capability(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((field, value)) = line.split_once('=') else {
            return false;
        };
        field.trim() == "capabilities"
            && value.split('"').skip(1).step_by(2).any(|item| item == "registries")
    })
}

fn validate_content_manifest(manifest: &str, expected_id: &str, expected_version: &str) -> Result<(), String> {
    if manifest_string_field(manifest, "id") != Some(expected_id) {
        return Err("ID в horizon-mod.toml не совпадает с ID проекта Horizon Registry.".to_owned());
    }
    if manifest_string_field(manifest, "version") != Some(expected_version) {
        return Err("Версия в horizon-mod.toml не совпадает с версией Horizon Registry.".to_owned());
    }
    if manifest_string_field(manifest, "api") != Some("0.2") {
        return Err("HMOD должен использовать content API 0.2.".to_owned());
    }
    if !manifest_has_registry_capability(manifest) {
        return Err("В horizon-mod.toml отсутствует capability registries.".to_owned());
    }
    if manifest.lines().any(|line| matches!(line.trim(), "[native]" | "[wasm]")) {
        return Err("Этот .hmod содержит нативную или WebAssembly секцию и не входит в поддерживаемый content-only режим GlowstoneCL.".to_owned());
    }
    Ok(())
}

fn safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 180
        && !name.starts_with('.')
        && name.ends_with(".hmod")
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
        })
        && name != "."
        && name != ".."
}

fn file_url_is_registry_release(value: &str, expected_name: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else { return false };
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let Some(segments) = url.path_segments() else { return false };
    let parts: Vec<_> = segments.filter(|part| !part.is_empty()).collect();
    parts.len() == 6
        && parts[0].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && parts[1].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && parts[2] == "releases"
        && parts[3] == "download"
        && parts[4].bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
        && parts[5] == expected_name
}

fn compatible_version(project: &HorizonModProject) -> Option<HorizonModVersion> {
    if project.kind != "mod" {
        return None;
    }
    project.versions.iter()
        .find(|version| {
            version.game == GAME_VERSION
                && version.file.name.ends_with(".hmod")
                && version.capabilities.iter().any(|capability| capability == "registries")
        })
        .cloned()
}

fn icon_cache_path(id: &str) -> Option<PathBuf> {
    if !valid_mod_id(id) {
        return None;
    }
    Some(store::app_data_dir().ok()?.join("mod-icons").join(format!("horizon-{id}.img")))
}

fn allowed_icon_url(value: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(value).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str() != Some("horizon-rust.github.io")
            && url.host_str() != Some("raw.githubusercontent.com")
    {
        return None;
    }
    Some(url)
}

fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF8")
        || (bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

async fn cache_icon(
    http: &reqwest::Client,
    id: &str,
    url: Option<&str>,
) -> Option<Vec<u8>> {
    let path = icon_cache_path(id)?;
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if !bytes.is_empty() && bytes.len() <= MAX_ICON_SIZE && looks_like_image(&bytes) {
            return Some(bytes);
        }
    }
    let url = allowed_icon_url(url?)?;
    let response = http.get(url).send().await.ok()?.error_for_status().ok()?;
    if response.content_length().is_some_and(|size| size > MAX_ICON_SIZE as u64) {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_ICON_SIZE || !looks_like_image(&bytes) {
        return None;
    }
    let parent = path.parent()?;
    tokio::fs::create_dir_all(parent).await.ok()?;
    let temporary = path.with_extension("img.tmp");
    tokio::fs::write(&temporary, &bytes).await.ok()?;
    tokio::fs::rename(&temporary, &path).await.ok()?;
    Some(bytes.to_vec())
}

fn cached_icon(id: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(icon_cache_path(id)?).ok()?;
    (!bytes.is_empty() && bytes.len() <= MAX_ICON_SIZE && looks_like_image(&bytes))
        .then_some(bytes)
}

fn hydrate_installed_icons(mods: &mut [InstalledHorizonMod]) {
    for item in mods {
        item.icon_data = cached_icon(&item.id);
    }
}

fn instance_mods_directory(instance_id: &str) -> Result<PathBuf, String> {
    Ok(store::instance_directory(instance_id)?.join("horizon-mods"))
}

fn manifest_path(instance_id: &str) -> Result<PathBuf, String> {
    Ok(instance_mods_directory(instance_id)?.join(MANIFEST_NAME))
}

fn read_manifest(instance_id: &str) -> Result<InstalledManifest, String> {
    let path = manifest_path(instance_id)?;
    if !path.is_file() {
        return Ok(InstalledManifest::default());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Не удалось прочитать список Horizon-модов: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Список Horizon-модов повреждён: {error}"))
}

fn save_manifest(instance_id: &str, manifest: &InstalledManifest) -> Result<(), String> {
    let path = manifest_path(instance_id)?;
    let parent = path.parent().ok_or_else(|| "Не найден каталог Horizon-модов.".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Не удалось создать каталог Horizon-модов: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Не удалось сериализовать список Horizon-модов: {error}"))?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось сохранить список Horizon-модов: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось обновить список Horizon-модов: {error}"))
}

fn set_enabled(instance_id: &str, id: &str, enabled: bool) -> Result<(), String> {
    if !valid_mod_id(id) {
        return Err("Некорректный ID Horizon-мода.".to_owned());
    }
    let game_dir = store::instance_directory(instance_id)?;
    let path = game_dir.join("horizon-mods.txt");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let key = format!("mod.{id}");
    let mut lines: Vec<String> = existing.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with(&format!("{key}:"))
        })
        .map(str::to_owned)
        .collect();
    if enabled {
        lines.push(format!("{key}:true"));
    }
    let mut output = lines.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    let temporary = path.with_extension("txt.tmp");
    std::fs::write(&temporary, output)
        .map_err(|error| format!("Не удалось сохранить настройки Horizon-модов: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось обновить настройки Horizon-модов: {error}"))
}

/// Search the reviewed Horizon registry. Only content mods with a 26.3 HMOD
/// release are listed; native code bundles are intentionally not treated as HMODs.
pub async fn search(_instance: GameInstance, query: String) -> Result<Vec<HorizonModProject>, String> {
    let index = registry_index().await?;
    if index.schema != 1 {
        return Err(format!("Версия формата Horizon Registry {} не поддерживается.", index.schema));
    }
    let query = query.trim().to_lowercase();
    let http = http_client()?;
    let mut projects: Vec<_> = index.projects.into_iter()
        .filter(|project| compatible_version(project).is_some())
        .filter(|project| {
            query.is_empty()
                || project.id.to_lowercase().contains(&query)
                || project.name.to_lowercase().contains(&query)
                || project.summary.to_lowercase().contains(&query)
                || project.description.to_lowercase().contains(&query)
                || project.authors.iter().any(|author| author.to_lowercase().contains(&query))
        })
        .collect();

    for project in &mut projects {
        project.icon_data = cache_icon(&http, &project.id, project.icon.as_deref()).await;
    }
    projects.sort_by(|a, b| b.downloads.cmp(&a.downloads).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(projects)
}

pub fn installed(instance_id: &str) -> Result<Vec<InstalledHorizonMod>, String> {
    let mut manifest = read_manifest(instance_id)?;
    manifest.mods.retain(|item| {
        valid_mod_id(&item.id)
            && safe_file_name(&item.file_name)
            && instance_mods_directory(instance_id)
                .map(|dir| dir.join(&item.file_name).is_file())
                .unwrap_or(false)
    });
    manifest.mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    hydrate_installed_icons(&mut manifest.mods);
    Ok(manifest.mods)
}

pub async fn install(instance: GameInstance, project_id: String) -> Result<Vec<InstalledHorizonMod>, String> {
    if !matches!(instance.platform, GamePlatform::Horizon | GamePlatform::HorizonOffline) {
        return Err("Horizon-моды устанавливаются только в сборки Horizon.".to_owned());
    }
    if !valid_mod_id(&project_id) {
        return Err("Некорректный ID Horizon-мода.".to_owned());
    }
    let index = registry_index().await?;
    let project = index.projects.iter()
        .find(|project| project.id == project_id && project.kind == "mod")
        .ok_or_else(|| "Мод отсутствует в актуальном реестре Horizon.".to_owned())?;
    let version = compatible_version(project)
        .ok_or_else(|| format!("У «{}» нет content-версии для Minecraft {GAME_VERSION}.", project.name))?;
    if index.revoked.iter().any(|revoked| {
        revoked.id == project.id
            && revoked.sha256.as_deref().is_none_or(|hash| hash.eq_ignore_ascii_case(&version.file.sha256))
    }) {
        let reason = index.revoked.iter()
            .find(|revoked| revoked.id == project.id)
            .map(|revoked| revoked.reason.as_str())
            .unwrap_or("Файл отозван реестром.");
        return Err(format!("Мод «{}» отозван Horizon Registry: {reason}", project.name));
    }
    if project.key.is_some() || version.file.signature.is_some() {
        return Err("Этот HMOD подписан minisign. Проверка подписи автора пока не реализована, поэтому GlowstoneCL не будет устанавливать его в обход проверки.".to_owned());
    }
    if !safe_file_name(&version.file.name)
        || version.file.size == 0
        || version.file.size > MAX_PACKAGE_SIZE as u64
        || version.file.sha256.len() != 64
        || !version.file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !file_url_is_registry_release(&version.file.url, &version.file.name)
    {
        return Err("Реестр Horizon вернул небезопасные или некорректные данные файла.".to_owned());
    }

    let directory = instance_mods_directory(&instance.id.0)?;
    tokio::fs::create_dir_all(&directory).await
        .map_err(|error| format!("Не удалось создать папку horizon-mods: {error}"))?;
    let target_name = format!("{}.hmod", project.id);
    let target = directory.join(&target_name);
    let mut manifest = read_manifest(&instance.id.0)?;
    if let Some(existing) = manifest.mods.iter().find(|item| item.id == project.id) {
        let path = directory.join(&existing.file_name);
        if path.is_file() {
            return Ok(installed(&instance.id.0)?);
        }
    }
    if target.exists() && !manifest.mods.iter().any(|item| item.id == project.id) {
        return Err(format!(
            "Файл {} уже существует и не управляется GlowstoneCL. Он не будет перезаписан.",
            target.display()
        ));
    }

    let client = http_client()?;
    let url = reqwest::Url::parse(&version.file.url)
        .map_err(|error| format!("Некорректная ссылка на HMOD: {error}"))?;
    let response = client.get(url).send().await
        .map_err(|error| format!("Не удалось скачать HMOD: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Сервер Horizon Registry не отдал HMOD: {error}"))?;
    if response.content_length().is_some_and(|size| size > MAX_PACKAGE_SIZE as u64) {
        return Err("HMOD превышает лимит 64 МиБ.".to_owned());
    }
    let bytes = response.bytes().await
        .map_err(|error| format!("Не удалось прочитать HMOD: {error}"))?;
    if bytes.len() as u64 != version.file.size {
        return Err(format!(
            "Размер HMOD не совпадает с индексом: ожидалось {} байт, получено {}.",
            version.file.size, bytes.len()
        ));
    }
    let checksum = hex(&Sha256::digest(&bytes));
    if checksum != version.file.sha256.to_ascii_lowercase() {
        return Err("SHA-256 HMOD не совпадает с проверенным индексом Horizon. Файл не установлен.".to_owned());
    }
    // Confirm that the reviewed package is a ZIP containing the matching content manifest.
    let cursor = Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|error| format!("HMOD не является корректным ZIP-пакетом: {error}"))?;
    let mut manifest_file = archive.by_name("horizon-mod.toml")
        .map_err(|_| "В HMOD отсутствует корневой horizon-mod.toml.".to_owned())?;
    if manifest_file.size() > 64 * 1024 {
        return Err("Манифест HMOD превышает 64 КиБ.".to_owned());
    }
    let mut manifest_text = String::new();
    manifest_file.read_to_string(&mut manifest_text)
        .map_err(|error| format!("Манифест HMOD повреждён: {error}"))?;
    validate_content_manifest(&manifest_text, &project.id, &version.version)?;

    let temporary = directory.join(format!(".{target_name}.part"));
    tokio::fs::write(&temporary, &bytes).await
        .map_err(|error| format!("Не удалось сохранить временный HMOD: {error}"))?;
    if target.exists() {
        tokio::fs::remove_file(&target).await
            .map_err(|error| format!("Не удалось заменить старый HMOD: {error}"))?;
    }
    tokio::fs::rename(&temporary, &target).await
        .map_err(|error| format!("Не удалось установить HMOD: {error}"))?;

    let icon_data = cache_icon(&client, &project.id, project.icon.as_deref()).await;
    manifest.mods.retain(|item| item.id != project.id);
    manifest.mods.push(InstalledHorizonMod {
        id: project.id.clone(),
        name: project.name.clone(),
        version: version.version.clone(),
        file_name: target_name,
        sha256: checksum,
        icon_url: project.icon.clone(),
        icon_data,
    });
    set_enabled(&instance.id.0, &project.id, true)?;
    save_manifest(&instance.id.0, &manifest)?;
    hydrate_installed_icons(&mut manifest.mods);
    Ok(manifest.mods)
}

/// Remove only a content mod installed and checksum-tracked by GlowstoneCL.
pub fn remove(instance_id: &str, project_id: &str) -> Result<Vec<InstalledHorizonMod>, String> {
    let mut manifest = read_manifest(instance_id)?;
    let Some(index) = manifest.mods.iter().position(|item| item.id == project_id) else {
        return Err("Этот Horizon-мод не установлен через GlowstoneCL.".to_owned());
    };
    let item = manifest.mods.remove(index);
    if !valid_mod_id(&item.id) || !safe_file_name(&item.file_name) {
        return Err("Запись установленного Horizon-мода повреждена.".to_owned());
    }
    let path = instance_mods_directory(instance_id)?.join(&item.file_name);
    if path.is_file() {
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("Не удалось проверить {}: {error}", item.file_name))?;
        if hex(&Sha256::digest(&bytes)) != item.sha256 {
            return Err("HMOD изменён вручную и не совпадает с установленной версией. GlowstoneCL не будет удалять его автоматически.".to_owned());
        }
        std::fs::remove_file(&path)
            .map_err(|error| format!("Не удалось удалить {}: {error}", item.file_name))?;
    }
    set_enabled(instance_id, &item.id, false)?;
    save_manifest(instance_id, &manifest)?;
    hydrate_installed_icons(&mut manifest.mods);
    Ok(manifest.mods)
}

fn safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 180
        && !name.starts_with('.')
        && name.ends_with(".hmod")
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
        && name != "."
        && name != ".."
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}


#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
id = "coords_hud"
name = "Coordinates HUD"
version = "0.1.0"
api = "0.2"
side = "client"
capabilities = ["registries"]
"#;

    #[test]
    fn content_manifest_must_match_registry_identity_and_version() {
        assert!(validate_content_manifest(VALID, "coords_hud", "0.1.0").is_ok());
        assert!(validate_content_manifest(VALID, "another_mod", "0.1.0").is_err());
        assert!(validate_content_manifest(VALID, "coords_hud", "0.2.0").is_err());
    }

    #[test]
    fn content_manifest_requires_registry_capability_and_api() {
        let missing = VALID.replace("capabilities = [\\"registries\\"]", "capabilities = []");
        assert!(validate_content_manifest(&missing, "coords_hud", "0.1.0").is_err());
        let wrong_api = VALID.replace("api = \\"0.2\\"", "api = \\"0.3\\"");
        assert!(validate_content_manifest(&wrong_api, "coords_hud", "0.1.0").is_err());
    }

    #[test]
    fn content_installer_refuses_native_or_wasm_manifest_tables() {
        let native = format!("{VALID}\\n[native]\\ngame = \\"26.3\\"");
        assert!(validate_content_manifest(&native, "coords_hud", "0.1.0").is_err());
        let wasm = format!("{VALID}\\n[wasm]\\nentry = \\"mod.wasm\\"");
        assert!(validate_content_manifest(&wasm, "coords_hud", "0.1.0").is_err());
    }

    #[test]
    fn github_asset_urls_must_have_expected_release_shape_and_filename() {
        assert!(file_url_is_registry_release(
            "https://github.com/horizon-rust/horizon-mods/releases/download/coords_hud-0.1.0/coords_hud-0.1.0.hmod",
            "coords_hud-0.1.0.hmod",
        ));
        assert!(!file_url_is_registry_release(
            "http://github.com/horizon-rust/horizon-mods/releases/download/coords_hud-0.1.0/coords_hud-0.1.0.hmod",
            "coords_hud-0.1.0.hmod",
        ));
        assert!(!file_url_is_registry_release(
            "https://example.com/coords_hud-0.1.0.hmod",
            "coords_hud-0.1.0.hmod",
        ));
    }
}
