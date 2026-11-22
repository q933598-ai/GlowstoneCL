//! Modrinth search and per-instance Java mod installation.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::task::JoinSet;

use glowstone_core::{GameInstance, GamePlatform, LoaderKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest as _, Sha512};

use crate::store;

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)";
const MANIFEST_NAME: &str = ".glowstone-modrinth.json";
const MAX_MOD_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct ModProject {
    pub project_id: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(skip)]
    pub icon_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledMod {
    pub project_id: String,
    pub title: String,
    pub file_name: String,
    pub version_number: String,
    pub version_id: String,
    #[serde(default)]
    pub dependency: bool,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(skip)]
    pub icon_data: Option<Vec<u8>>,
}
/// Any JAR found in this profile's mods folder, including files installed
/// manually or by a modpack rather than through GlowstoneCL.
#[derive(Debug, Clone)]
pub struct InstalledModFile {
    pub file_name: String,
    pub title: String,
    pub version: String,
    pub size: u64,
    pub managed: bool,
    pub icon_data: Option<Vec<u8>>,
}



#[derive(Debug, Default, Serialize, Deserialize)]
struct InstalledManifest {
    #[serde(default)]
    mods: Vec<InstalledMod>,
}

fn loader_tag(instance: &GameInstance) -> Result<&'static str, String> {
    if instance.platform != GamePlatform::Java {
        return Err("Моды Modrinth устанавливаются в сборки Minecraft Java, не в Horizon.".to_owned());
    }
    match instance.loader {
        LoaderKind::Fabric => Ok("fabric"),
        LoaderKind::Forge => Ok("forge"),
        LoaderKind::NeoForge => Ok("neoforge"),
        LoaderKind::Quilt => Ok("quilt"),
        LoaderKind::Vanilla => Err("Для установки модов создай сборку с Fabric, Forge, NeoForge или Quilt.".to_owned()),
        LoaderKind::HorizonContent | LoaderKind::HorizonNative => {
            Err("Выбран Horizon-загрузчик. Для него используются HMOD/HNMOD, а не Java-моды.".to_owned())
        }
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент Modrinth: {error}"))
}

async fn resolve_game_version(client: &reqwest::Client, requested: &str) -> Result<String, String> {
    match requested.trim().to_ascii_lowercase().as_str() {
        "stable" | "latest" | "release" | "latest_release" => {}
        "snapshot" | "latest snapshot" | "latest_snapshot" => {}
        _ => return Ok(requested.trim().to_owned()),
    }
    let manifest = client
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await
        .map_err(|error| format!("Не удалось определить версию Minecraft для поиска модов: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Mojang не вернул список версий: {error}"))?
        .json::<Value>()
        .await
        .map_err(|error| format!("Список версий Mojang повреждён: {error}"))?;
    let key = if matches!(requested.trim().to_ascii_lowercase().as_str(), "snapshot" | "latest snapshot" | "latest_snapshot") {
        "snapshot"
    } else {
        "release"
    };
    manifest["latest"][key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "В манифесте Mojang не найдена актуальная версия Minecraft.".to_owned())
}

/// Search only projects that have a version for the selected profile's game version and loader.
pub async fn search(instance: GameInstance, query: String) -> Result<Vec<ModProject>, String> {
    let loader = loader_tag(&instance)?;
    let http = client()?;
    let version = resolve_game_version(&http, &instance.game_version).await?;
    let facets = serde_json::json!([
        ["project_type:mod"],
        [format!("versions:{version}")],
        [format!("categories:{loader}")]
    ]).to_string();

    let mut search_url = reqwest::Url::parse(&format!("{API}/search"))
        .map_err(|error| format!("Некорректный адрес поиска Modrinth: {error}"))?;
    search_url.query_pairs_mut()
        .append_pair("query", query.trim())
        .append_pair("facets", &facets)
        .append_pair("index", "downloads")
        .append_pair("limit", "30");
    let response = http
        .get(search_url)
        .send()
        .await
        .map_err(|error| format!("Не удалось обратиться к Modrinth: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth отклонил поиск: {error}"))?
        .json::<Value>()
        .await
        .map_err(|error| format!("Modrinth вернул некорректный результат поиска: {error}"))?;

    let projects: Vec<ModProject> = serde_json::from_value(response["hits"].clone())
        .map_err(|error| format!("Список модов Modrinth не удалось прочитать: {error}"))?;
    let mut tasks = JoinSet::new();
    for (index, mut project) in projects.into_iter().enumerate() {
        let http = http.clone();
        let project_id = project.project_id.clone();
        let icon_url = project.icon_url.clone();
        tasks.spawn(async move {
            project.icon_data = cache_remote_icon(
                &http,
                "modrinth",
                &project_id,
                icon_url.as_deref(),
            ).await;
            (index, project)
        });
    }
    let mut enriched = Vec::new();
    while let Some(result) = tasks.join_next().await {
        if let Ok(project) = result {
            enriched.push(project);
        }
    }
    enriched.sort_by_key(|(index, _)| *index);
    Ok(enriched.into_iter().map(|(_, project)| project).collect())
}

const MAX_ICON_BYTES: usize = 5 * 1024 * 1024;

fn icon_cache_path(source: &str, project_id: &str) -> Option<PathBuf> {
    if project_id.is_empty()
        || !project_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return None;
    }
    Some(store::app_data_dir().ok()?.join("mod-icons").join(format!("{source}-{project_id}.img")))
}

fn allowed_icon_url(source: &str, value: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host_str()?;
    let allowed = match source {
        "modrinth" => host == "cdn.modrinth.com",
        "horizon" => host == "horizon-rust.github.io" || host == "raw.githubusercontent.com",
        _ => false,
    };
    allowed.then_some(url)
}

fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF8")
        || (bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

async fn cache_remote_icon(
    http: &reqwest::Client,
    source: &str,
    project_id: &str,
    icon_url: Option<&str>,
) -> Option<Vec<u8>> {
    let path = icon_cache_path(source, project_id)?;
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if !bytes.is_empty() && bytes.len() <= MAX_ICON_BYTES && looks_like_image(&bytes) {
            return Some(bytes);
        }
    }
    let url = allowed_icon_url(source, icon_url?)?;
    let response = http.get(url).send().await.ok()?.error_for_status().ok()?;
    if response.content_length().is_some_and(|size| size > MAX_ICON_BYTES as u64) {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_ICON_BYTES || !looks_like_image(&bytes) {
        return None;
    }
    let parent = path.parent()?;
    tokio::fs::create_dir_all(parent).await.ok()?;
    let temporary = path.with_extension("img.tmp");
    tokio::fs::write(&temporary, &bytes).await.ok()?;
    tokio::fs::rename(&temporary, &path).await.ok()?;
    Some(bytes.to_vec())
}

fn cached_icon(source: &str, project_id: &str) -> Option<Vec<u8>> {
    let path = icon_cache_path(source, project_id)?;
    let bytes = std::fs::read(path).ok()?;
    (!bytes.is_empty() && bytes.len() <= MAX_ICON_BYTES && looks_like_image(&bytes))
        .then_some(bytes)
}

fn hydrate_installed_icons(mods: &mut [InstalledMod]) {
    for item in mods {
        item.icon_data = cached_icon("modrinth", &item.project_id);
    }
}

fn manifest_path(instance_id: &str) -> Result<PathBuf, String> {
    Ok(store::instance_directory(instance_id)?.join("mods").join(MANIFEST_NAME))
}

fn read_manifest(instance_id: &str) -> Result<InstalledManifest, String> {
    let path = manifest_path(instance_id)?;
    if !path.is_file() {
        return Ok(InstalledManifest::default());
    }
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("Не удалось прочитать список установленных модов: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Список установленных модов повреждён: {error}"))
}

fn save_manifest(instance_id: &str, manifest: &InstalledManifest) -> Result<(), String> {
    let path = manifest_path(instance_id)?;
    let parent = path.parent().ok_or_else(|| "Не найден каталог модов.".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Не удалось создать каталог модов: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Не удалось сериализовать список модов: {error}"))?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось сохранить список модов: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось обновить список модов: {error}"))
}

/// Resolve icon URLs for mods installed by older GlowstoneCL builds, whose
/// manifest predates icon_url. This never changes the installed JAR itself.
pub async fn refresh_installed_icons(instance_id: String) -> Result<Vec<InstalledMod>, String> {
    let mut manifest = read_manifest(&instance_id)?;
    let http = client()?;
    let mut changed = false;
    for item in &mut manifest.mods {
        if cached_icon("modrinth", &item.project_id).is_some() {
            continue;
        }
        let Ok(project) = get_project(&http, &item.project_id).await else {
            continue;
        };
        let Some(icon_url) = project["icon_url"].as_str().map(str::to_owned) else {
            continue;
        };
        if let Some(icon_data) = cache_remote_icon(
            &http,
            "modrinth",
            &item.project_id,
            Some(&icon_url),
        ).await {
            item.icon_url = Some(icon_url);
            item.icon_data = Some(icon_data);
            changed = true;
        }
    }
    if changed {
        save_manifest(&instance_id, &manifest)?;
    }
    installed(&instance_id)
}

pub fn installed(instance_id: &str) -> Result<Vec<InstalledMod>, String> {
    let mut mods = read_manifest(instance_id)?.mods;
    mods.retain(|item| {
        safe_filename(&item.file_name).is_some()
            && store::instance_directory(instance_id)
                .map(|dir| dir.join("mods").join(&item.file_name).is_file())
                .unwrap_or(false)
    });
    mods.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    hydrate_installed_icons(&mut mods);
    Ok(mods)
}

/// Scans the actual mods directory instead of relying only on GlowstoneCL's
/// Modrinth/CurseForge manifests, so manually added and modpack JARs are visible.
pub fn local_files(instance_id: &str) -> Result<Vec<InstalledModFile>, String> {
    let directory = store::instance_directory(instance_id)?.join("mods");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Не удалось открыть каталог модов: {error}"))?;
    let mut managed_icons: HashMap<String, Vec<u8>> = HashMap::new();
    for item in installed(instance_id)? {
        if let Some(bytes) = item.icon_data {
            managed_icons.insert(item.file_name, bytes);
        }
    }
    let managed_names: HashSet<String> = read_manifest(instance_id)?.mods
        .into_iter()
        .filter_map(|item| safe_filename(&item.file_name).map(str::to_owned))
        .chain(
            crate::curseforge::installed(instance_id).unwrap_or_default()
                .into_iter()
                .filter_map(|item| safe_filename(&item.file_name).map(str::to_owned))
        )
        .collect();

    let mut result = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("Не удалось прочитать каталог модов: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Не удалось прочитать файл мода: {error}"))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("Не удалось прочитать свойства мода: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file()
            || !path.extension().and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("jar"))
        {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let (title, version, embedded_icon) = read_mod_jar_metadata(&path);
        let icon_data = embedded_icon.or_else(|| managed_icons.get(&file_name).cloned());
        result.push(InstalledModFile {
            managed: managed_names.contains(&file_name),
            title,
            version,
            file_name,
            size: metadata.len(),
            icon_data,
        });
    }
    result.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(result)
}

/// Scans a profile's mod directory off the Iced UI thread.
pub async fn local_files_async(instance_id: String) -> Result<Vec<InstalledModFile>, String> {
    tokio::task::spawn_blocking(move || local_files(&instance_id))
        .await
        .map_err(|error| format!("Не удалось просканировать моды в фоновой задаче: {error}"))?
}

fn read_mod_jar_metadata(path: &Path) -> (String, String, Option<Vec<u8>>) {
    use std::io::Read as _;
    let fallback = path.file_stem().and_then(|value| value.to_str())
        .unwrap_or("Minecraft mod").to_owned();
    let Ok(file) = std::fs::File::open(path) else {
        return (fallback, "unknown".to_owned(), None);
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return (fallback, "unknown".to_owned(), None);
    };
    for metadata_path in ["fabric.mod.json", "quilt.mod.json"] {
        let Ok(mut metadata_file) = archive.by_name(metadata_path) else { continue };
        let mut content = String::new();
        if metadata_file.read_to_string(&mut content).is_err() { continue; }
        drop(metadata_file);
        if let Ok(value) = serde_json::from_str::<Value>(&content) {
            let name = value["name"].as_str().or_else(|| value["id"].as_str())
                .unwrap_or(&fallback).to_owned();
            let version = value["version"].as_str().unwrap_or("unknown").to_owned();
            let icon_path = value["icon"].as_str().or_else(|| {
                value["icon"].as_object().and_then(|icons| {
                    icons.iter().next().and_then(|(_, path)| path.as_str())
                })
            });
            let icon_data = icon_path.and_then(|icon_path| {
                let mut icon_file = archive.by_name(icon_path).ok()?;
                let mut bytes = Vec::new();
                icon_file.take((MAX_ICON_BYTES + 1) as u64).read_to_end(&mut bytes).ok()?;
                if bytes.len() <= MAX_ICON_BYTES && looks_like_image(&bytes) {
                    Some(bytes)
                } else {
                    None
                }
            });
            return (name, version, icon_data);
        }
    }
    (fallback, "metadata unavailable".to_owned(), None)
}

/// Safely removes one JAR by its file name. Manifest entries are pruned by the
/// existing installed() filters on the next refresh, preserving all other mods.
pub fn remove_file(instance_id: &str, file_name: &str) -> Result<(), String> {
    let file_name = safe_filename(file_name)
        .ok_or_else(|| "Недопустимое имя JAR-файла.".to_owned())?;

    // Preserve manifest integrity when the file was installed through one of the
    // supported catalogues; otherwise it is treated as a manually copied mod.
    if let Some(item) = installed(instance_id)?.into_iter()
        .find(|item| item.file_name == file_name)
    {
        remove(instance_id, &item.project_id)?;
        return Ok(());
    }
    if let Some(item) = crate::curseforge::installed(instance_id)?.into_iter()
        .find(|item| item.file_name == file_name)
    {
        crate::curseforge::remove(instance_id, item.mod_id, item.file_id)?;
        return Ok(());
    }

    let path = store::instance_directory(instance_id)?.join("mods").join(file_name);
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("Не удалось найти файл мода: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Удалять можно только обычные JAR-файлы из каталога mods.".to_owned());
    }
    std::fs::remove_file(&path)
        .map_err(|error| format!("Не удалось удалить мод: {error}"))
}

fn safe_filename(value: &str) -> Option<&str> {
    let path = Path::new(value);
    let name = path.file_name()?.to_str()?;
    if name != value || !name.to_ascii_lowercase().ends_with(".jar")
        || name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/') || name.contains('\\') || name.contains('\0')
    {
        return None;
    }
    Some(name)
}

fn valid_project_id(value: &str) -> bool {
    (4..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

async fn get_project(http: &reqwest::Client, project_id: &str) -> Result<Value, String> {
    http.get(format!("{API}/project/{project_id}"))
        .send()
        .await
        .map_err(|error| format!("Не удалось получить карточку мода: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth не нашёл проект {project_id}: {error}"))?
        .json::<Value>()
        .await
        .map_err(|error| format!("Карточка мода {project_id} повреждена: {error}"))
}

async fn compatible_versions(
    http: &reqwest::Client,
    instance: &GameInstance,
    project_id: &str,
) -> Result<Vec<Value>, String> {
    let loader = loader_tag(instance)?;
    let game_version = resolve_game_version(http, &instance.game_version).await?;
    let game_versions = serde_json::json!([game_version]).to_string();
    let loaders = serde_json::json!([loader]).to_string();
    let mut versions_url = reqwest::Url::parse(&format!("{API}/project/{project_id}/version"))
        .map_err(|error| format!("Некорректный адрес списка версий {project_id}: {error}"))?;
    versions_url.query_pairs_mut()
        .append_pair("game_versions", &game_versions)
        .append_pair("loaders", &loaders);
    http.get(versions_url)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить версии мода {project_id}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth не вернул совместимые версии {project_id}: {error}"))?
        .json::<Vec<Value>>()
        .await
        .map_err(|error| format!("Список версий мода {project_id} повреждён: {error}"))
}

async fn get_version(http: &reqwest::Client, version_id: &str) -> Result<Value, String> {
    http.get(format!("{API}/version/{version_id}"))
        .send()
        .await
        .map_err(|error| format!("Не удалось получить зависимость {version_id}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth не нашёл версию зависимости {version_id}: {error}"))?
        .json::<Value>()
        .await
        .map_err(|error| format!("Данные зависимости {version_id} повреждены: {error}"))
}

fn selected_jar(version: &Value) -> Result<Value, String> {
    let files = version["files"].as_array()
        .ok_or_else(|| "Modrinth не приложил файлы к выбранной версии.".to_owned())?;
    files.iter()
        .find(|file| file["primary"].as_bool() == Some(true)
            && file["filename"].as_str().is_some_and(|name| name.to_ascii_lowercase().ends_with(".jar")))
        .or_else(|| files.iter().find(|file| {
            file["filename"].as_str().is_some_and(|name| name.to_ascii_lowercase().ends_with(".jar"))
        }))
        .cloned()
        .ok_or_else(|| "В выбранной версии нет JAR-файла для Minecraft Java.".to_owned())
}

async fn download_jar(
    http: &reqwest::Client,
    instance_id: &str,
    project_id: &str,
    file: &Value,
) -> Result<String, String> {
    let original_name = file["filename"].as_str()
        .ok_or_else(|| "Modrinth не указал имя файла мода.".to_owned())?;
    let original_name = safe_filename(original_name)
        .ok_or_else(|| "Modrinth вернул небезопасное имя JAR-файла.".to_owned())?;
    let url_text = file["url"].as_str()
        .ok_or_else(|| "Modrinth не указал ссылку на JAR-файл.".to_owned())?;
    let url = reqwest::Url::parse(url_text)
        .map_err(|error| format!("Modrinth вернул некорректную ссылку на файл: {error}"))?;
    if url.scheme() != "https" || url.host_str() != Some("cdn.modrinth.com") {
        return Err("Загрузка остановлена: Modrinth вернул ссылку не на свой HTTPS CDN.".to_owned());
    }

    let mods_dir = store::instance_directory(instance_id)?.join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|error| format!("Не удалось создать папку mods: {error}"))?;

    // Prefixing the project ID avoids overwriting an unrelated JAR with the same name.
    let mut file_name = format!("{project_id}-{original_name}");
    let mut target = mods_dir.join(&file_name);
    let mut suffix = 1_u32;
    while target.exists() {
        file_name = format!("{project_id}-{suffix}-{original_name}");
        target = mods_dir.join(&file_name);
        suffix += 1;
    }
    let temporary = mods_dir.join(format!(".{file_name}.part"));
    let response = http.get(url).send().await
        .map_err(|error| format!("Не удалось скачать {original_name}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CDN Modrinth не отдал {original_name}: {error}"))?;
    if response.content_length().is_some_and(|size| size > MAX_MOD_BYTES as u64) {
        return Err("Файл мода превышает лимит 512 МиБ.".to_owned());
    }
    let bytes = response.bytes().await
        .map_err(|error| format!("Не удалось прочитать {original_name}: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_MOD_BYTES {
        return Err("Modrinth вернул пустой или слишком большой файл мода.".to_owned());
    }

    let hashes = &file["hashes"];
    let valid_hash = if let Some(expected) = hashes["sha512"].as_str() {
        hex(&Sha512::digest(&bytes)) == expected
    } else if let Some(expected) = hashes["sha1"].as_str() {
        hex(&Sha1::digest(&bytes)) == expected
    } else {
        return Err("Modrinth не предоставил SHA-1 или SHA-512 для проверки файла.".to_owned());
    };
    if !valid_hash {
        return Err(format!("Контрольная сумма {original_name} не совпала. Файл удалён."));
    }

    tokio::fs::write(&temporary, &bytes).await
        .map_err(|error| format!("Не удалось сохранить временный файл мода: {error}"))?;
    if target.exists() {
        tokio::fs::remove_file(&target).await
            .map_err(|error| format!("Не удалось заменить старую версию {file_name}: {error}"))?;
    }
    tokio::fs::rename(&temporary, &target).await
        .map_err(|error| format!("Не удалось установить {file_name}: {error}"))?;
    Ok(file_name)
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

async fn compatible_version(
    http: &reqwest::Client,
    instance: &GameInstance,
    project_id: &str,
) -> Result<Value, String> {
    let versions = compatible_versions(http, instance, project_id).await?;
    let latest_release = versions.iter()
        .find(|item| item["version_type"].as_str() == Some("release"))
        .cloned();
    latest_release
        .or_else(|| versions.first().cloned())
        .ok_or_else(|| format!(
            "У мода {project_id} нет версии для выбранной Minecraft и загрузчика."
        ))
}

fn version_matches_instance(version: &Value, game_version: &str, loader: &str) -> bool {
    version["game_versions"].as_array().is_some_and(|items| {
        items.iter().any(|item| item.as_str() == Some(game_version))
    }) && version["loaders"].as_array().is_some_and(|items| {
        items.iter().any(|item| item.as_str() == Some(loader))
    })
}

enum InstallStep {
    Visit {
        project_id: String,
        version_id: Option<String>,
        dependency: bool,
    },
    Install {
        project_id: String,
        project: Value,
        version: Value,
        icon_url: Option<String>,
        dependency: bool,
    },
}

/// Resolve the complete dependency graph first, then install dependencies before
/// the requested root. If a later file download fails, retrying can finish the graph
/// without marking the root as installed prematurely.
pub async fn install(instance: GameInstance, project_id: String) -> Result<Vec<InstalledMod>, String> {
    let loader = loader_tag(&instance)?;
    if !valid_project_id(&project_id) {
        return Err("Некорректный ID мода Modrinth.".to_owned());
    }
    let http = client()?;
    let game_version = resolve_game_version(&http, &instance.game_version).await?;
    let mut manifest = read_manifest(&instance.id.0)?;
    let mods_dir = store::instance_directory(&instance.id.0)?.join("mods");
    // The manifest is only bookkeeping. If a user manually removes a managed JAR,
    // drop its stale record so a later install click can download it again.
    manifest.mods.retain(|item| {
        safe_filename(&item.file_name).is_some_and(|name| mods_dir.join(name).is_file())
    });

    let root_version = manifest.mods.iter()
        .find(|item| item.project_id == project_id)
        .map(|item| item.version_id.clone())
        .filter(|value| !value.is_empty());

    let mut pending = vec![InstallStep::Visit {
        project_id,
        version_id: root_version,
        dependency: false,
    }];
    let mut visited = HashSet::new();
    let mut resolved_count = 0usize;

    while let Some(step) = pending.pop() {
        match step {
            InstallStep::Visit { project_id, mut version_id, dependency } => {
                if !visited.insert(project_id.clone()) {
                    continue;
                }
                resolved_count += 1;
                if resolved_count > 128 {
                    return Err("У мода обнаружено слишком много зависимостей (лимит 128).".to_owned());
                }
                if !valid_project_id(&project_id) {
                    return Err("Modrinth вернул некорректный ID зависимости.".to_owned());
                }

                // Reuse the exact version already installed by GlowstoneCL so that
                // a dependency retry does not silently upgrade part of the modpack.
                if version_id.is_none() {
                    version_id = manifest.mods.iter()
                        .find(|item| item.project_id == project_id)
                        .map(|item| item.version_id.clone())
                        .filter(|value| !value.is_empty());
                }

                let project = get_project(&http, &project_id).await?;
                if project["project_type"].as_str() != Some("mod") {
                    return Err(format!(
                        "Зависимость «{}» не является модом Minecraft.",
                        project["title"].as_str().unwrap_or(&project_id)
                    ));
                }
                let version = if let Some(version_id) = version_id.as_deref() {
                    get_version(&http, version_id).await?
                } else {
                    compatible_version(&http, &instance, &project_id).await?
                };
                if !version_matches_instance(&version, &game_version, loader) {
                    return Err(format!(
                        "Версия «{}» не совместима с Minecraft {game_version} и загрузчиком {loader}.",
                        project["title"].as_str().unwrap_or(&project_id)
                    ));
                }

                let project_title = project["title"].as_str().unwrap_or(&project_id).to_owned();
                let icon_url = project["icon_url"].as_str().map(str::to_owned);
                // The install step is pushed first. Dependencies go on top of it,
                // so the requested root is written only after its required graph.
                pending.push(InstallStep::Install {
                    project_id: project_id.clone(),
                    project,
                    version: version.clone(),
                    icon_url,
                    dependency,
                });

                if let Some(dependencies) = version["dependencies"].as_array() {
                    for dep in dependencies {
                        if dep["dependency_type"].as_str() != Some("required") {
                            continue;
                        }
                        let dependency_version_id = dep["version_id"].as_str().map(str::to_owned);
                        if let Some(dependency_project_id) = dep["project_id"].as_str() {
                            pending.push(InstallStep::Visit {
                                project_id: dependency_project_id.to_owned(),
                                version_id: dependency_version_id,
                                dependency: true,
                            });
                        } else if let Some(version_id) = dependency_version_id {
                            let fixed = get_version(&http, &version_id).await?;
                            let dependency_project_id = fixed["project_id"].as_str()
                                .ok_or_else(|| format!(
                                    "Не удалось определить проект обязательной зависимости {version_id}."
                                ))?;
                            pending.push(InstallStep::Visit {
                                project_id: dependency_project_id.to_owned(),
                                version_id: Some(version_id),
                                dependency: true,
                            });
                        } else if let Some(file_name) = dep["file_name"].as_str() {
                            return Err(format!(
                                "У обязательной зависимости {file_name} нет ID проекта или версии на Modrinth; её нужно добавить вручную."
                            ));
                        } else {
                            return Err(format!(
                                "Modrinth вернул обязательную зависимость без проекта, версии или имени файла для мода {}.",
                                &project_title
                            ));
                        }
                    }
                }
            }
            InstallStep::Install { project_id, project, version, icon_url, dependency } => {
                if manifest.mods.iter().any(|item| item.project_id == project_id) {
                    continue;
                }
                let file = selected_jar(&version)?;
                let file_name = download_jar(&http, &instance.id.0, &project_id, &file).await?;
                let icon_data = cache_remote_icon(
                    &http,
                    "modrinth",
                    &project_id,
                    icon_url.as_deref(),
                ).await;
                manifest.mods.push(InstalledMod {
                    project_id: project_id.clone(),
                    title: project["title"].as_str().unwrap_or(&project_id).to_owned(),
                    file_name,
                    version_number: version["version_number"].as_str().unwrap_or("unknown").to_owned(),
                    version_id: version["id"].as_str().unwrap_or_default().to_owned(),
                    dependency,
                    icon_url,
                    icon_data,
                });
                save_manifest(&instance.id.0, &manifest)?;
            }
        }
    }

    hydrate_installed_icons(&mut manifest.mods);
    Ok(manifest.mods)
}

/// Remove only JAR files tracked by GlowstoneCL's Modrinth manifest.
pub fn remove(instance_id: &str, project_id: &str) -> Result<Vec<InstalledMod>, String> {
    let mut manifest = read_manifest(instance_id)?;
    let Some(index) = manifest.mods.iter().position(|item| item.project_id == project_id) else {
        return Err("Этот мод не установлен через каталог GlowstoneCL.".to_owned());
    };
    let item = manifest.mods.remove(index);
    if let Some(name) = safe_filename(&item.file_name) {
        let path = store::instance_directory(instance_id)?.join("mods").join(name);
        if path.is_file() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("Не удалось удалить {}: {error}", item.file_name))?;
        }
    }
    save_manifest(instance_id, &manifest)?;
    hydrate_installed_icons(&mut manifest.mods);
    Ok(manifest.mods)
}
