//! CurseForge Java mod search and per-instance installation.
//! API keys are read from the operating-system keyring by the UI and never persisted
//! into GlowstoneCL's plain-text launcher state.
use std::path::{Path, PathBuf};

use glowstone_core::{GameInstance, GamePlatform, LoaderKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::store;

const API: &str = "https://api.curseforge.com/v1";
const GAME_ID: &str = "432";
const CLASS_MODS: &str = "6";
const MAX_DOWNLOAD: usize = 512 * 1024 * 1024;
const MANIFEST_NAME: &str = ".glowstone-curseforge.json";

#[derive(Debug, Clone)]
pub struct CurseForgeProject {
    pub mod_id: u32,
    pub name: String,
    pub summary: String,
    pub downloads: u64,
    pub logo_url: Option<String>,
    pub file_id: Option<u32>,
    pub file_name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledCurseForgeMod {
    pub mod_id: u32,
    pub name: String,
    pub file_id: u32,
    pub file_name: String,
    pub version: String,
    pub sha1: String,
    #[serde(default)]
    pub logo_url: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct InstalledManifest {
    #[serde(default)]
    mods: Vec<InstalledCurseForgeMod>,
}

fn loader_type(instance: &GameInstance) -> Result<u8, String> {
    if instance.platform != GamePlatform::Java {
        return Err("CurseForge JAR-моды устанавливаются только в Minecraft Java.".to_owned());
    }
    match instance.loader {
        LoaderKind::Forge => Ok(1),
        LoaderKind::Fabric => Ok(4),
        LoaderKind::Quilt => Ok(5),
        LoaderKind::NeoForge => Ok(6),
        LoaderKind::Vanilla => Err("Для CurseForge-модов нужна сборка с Forge, Fabric, NeoForge или Quilt.".to_owned()),
        LoaderKind::HorizonContent | LoaderKind::HorizonNative => {
            Err("Для Horizon используются HMOD/HNMOD, а не CurseForge JAR.".to_owned())
        }
    }
}

fn client(api_key: &str) -> Result<reqwest::Client, String> {
    if !(16..=512).contains(&api_key.trim().len()) {
        return Err("Укажи действительный CurseForge API key в Настройках.".to_owned());
    }
    reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            let value = reqwest::header::HeaderValue::from_str(api_key.trim())
                .map_err(|_| "CurseForge API key содержит недопустимые символы.".to_owned())?;
            headers.insert("x-api-key", value);
            headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("application/json"));
            headers
        })
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент CurseForge: {error}"))
}

async fn resolve_game_version(http: &reqwest::Client, requested: &str) -> Result<String, String> {
    match requested.trim().to_ascii_lowercase().as_str() {
        "stable" | "latest" | "release" | "latest_release" | "snapshot" | "latest_snapshot" => {}
        _ => return Ok(requested.trim().to_owned()),
    }
    let manifest = http
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send().await
        .map_err(|error| format!("Не удалось определить версию Minecraft: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Mojang не вернул список версий: {error}"))?
        .json::<Value>().await
        .map_err(|error| format!("Манифест Mojang повреждён: {error}"))?;
    let key = if requested.to_ascii_lowercase().contains("snapshot") { "snapshot" } else { "release" };
    manifest["latest"][key].as_str().map(str::to_owned)
        .ok_or_else(|| "В манифесте Mojang не найдена версия Minecraft.".to_owned())
}

/// Search for mod projects and select a latest file that matches the selected game version
/// and loader. CurseForge does not expose unauthenticated downloads; the API key is mandatory.
pub async fn search(
    instance: GameInstance,
    query: String,
    api_key: String,
) -> Result<Vec<CurseForgeProject>, String> {
    let loader = loader_type(&instance)?;
    let http = client(&api_key)?;
    let version = resolve_game_version(&http, &instance.game_version).await?;
    let mut url = reqwest::Url::parse(&format!("{API}/mods/search"))
        .map_err(|error| format!("Некорректный URL CurseForge: {error}"))?;
    url.query_pairs_mut()
        .append_pair("gameId", GAME_ID)
        .append_pair("classId", CLASS_MODS)
        .append_pair("gameVersion", &version)
        .append_pair("modLoaderType", &loader.to_string())
        .append_pair("searchFilter", query.trim())
        .append_pair("pageSize", "18")
        .append_pair("index", "0")
        .append_pair("sortField", "6")
        .append_pair("sortOrder", "desc");

    let response = http.get(url).send().await
        .map_err(|error| format!("Не удалось обратиться к CurseForge: {error}"))?;
    let status = response.status();
    let response = response.error_for_status()
        .map_err(|error| format!("CurseForge API вернул HTTP {status}: {error}"))?
        .json::<Value>().await
        .map_err(|error| format!("CurseForge вернул некорректный JSON: {error}"))?;
    let data = response["data"].as_array()
        .ok_or_else(|| "Ответ CurseForge не содержит списка модов.".to_owned())?;

    let mut results = Vec::new();
    for item in data {
        let Some(mod_id) = item["id"].as_u64().and_then(|value| u32::try_from(value).ok()) else {
            continue;
        };
        let name = item["name"].as_str().unwrap_or("CurseForge mod").to_owned();
        let indexes = item["latestFilesIndexes"].as_array();
        let matching = indexes.and_then(|indexes| indexes.iter().find(|file| {
            file["gameVersion"].as_str() == Some(version.as_str())
                && file["modLoader"].as_u64() == Some(loader as u64)
        }));
        let file_id = matching.and_then(|file| file["fileId"].as_u64())
            .and_then(|value| u32::try_from(value).ok());
        let file_name = matching.and_then(|file| file["filename"].as_str()).map(str::to_owned);
        let version_number = matching.and_then(|file| file["displayName"].as_str())
            .or_else(|| matching.and_then(|file| file["gameVersion"].as_str()))
            .map(str::to_owned);
        results.push(CurseForgeProject {
            mod_id,
            name,
            summary: item["summary"].as_str().unwrap_or_default().to_owned(),
            downloads: item["downloadCount"].as_u64().unwrap_or_default(),
            logo_url: item["logo"]["url"].as_str().map(str::to_owned),
            file_id,
            file_name,
            version: version_number,
        });
    }
    Ok(results)
}

fn manifest_path(instance_id: &str) -> Result<PathBuf, String> {
    Ok(store::instance_directory(instance_id)?.join("mods").join(MANIFEST_NAME))
}

fn read_manifest(instance_id: &str) -> Result<InstalledManifest, String> {
    let path = manifest_path(instance_id)?;
    if !path.is_file() {
        return Ok(InstalledManifest::default());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Не удалось прочитать CurseForge manifest: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("CurseForge manifest повреждён: {error}"))
}

fn save_manifest(instance_id: &str, manifest: &InstalledManifest) -> Result<(), String> {
    let path = manifest_path(instance_id)?;
    let parent = path.parent().ok_or_else(|| "Не найден каталог модов.".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Не удалось создать каталог модов: {error}"))?;
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Не удалось сериализовать CurseForge manifest: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Не удалось записать CurseForge manifest: {error}"))?;
    std::fs::rename(temporary, path)
        .map_err(|error| format!("Не удалось сохранить CurseForge manifest: {error}"))
}

fn valid_jar_name(value: &str) -> Option<&str> {
    let path = Path::new(value);
    let name = path.file_name()?.to_str()?;
    if name != value
        || name.is_empty()
        || name == "." || name == ".."
        || name.contains('/') || name.contains('\\') || name.contains('\0')
        || !name.to_ascii_lowercase().ends_with(".jar")
    {
        None
    } else {
        Some(name)
    }
}

fn digest(bytes: &[u8]) -> String {
    let value = Sha1::digest(bytes);
    let mut result = String::with_capacity(value.len() * 2);
    for byte in value.iter() {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

pub fn installed(instance_id: &str) -> Result<Vec<InstalledCurseForgeMod>, String> {
    let mut manifest = read_manifest(instance_id)?;
    let mods_dir = store::instance_directory(instance_id)?.join("mods");
    manifest.mods.retain(|item| {
        valid_jar_name(&item.file_name).is_some()
            && mods_dir.join(&item.file_name).is_file()
    });
    manifest.mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(manifest.mods)
}

/// Download a file through CurseForge's own download-url API endpoint and verify its SHA-1.
pub async fn install(
    instance: GameInstance,
    mod_id: u32,
    file_id: u32,
    api_key: String,
) -> Result<Vec<InstalledCurseForgeMod>, String> {
    loader_type(&instance)?;
    let http = client(&api_key)?;
    let file_endpoint = format!("{API}/mods/{mod_id}/files/{file_id}");
    let file_response = http.get(&file_endpoint).send().await
        .map_err(|error| format!("Не удалось получить метаданные файла CurseForge: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge отклонил запрос файла: {error}"))?
        .json::<Value>().await
        .map_err(|error| format!("Метаданные файла CurseForge повреждены: {error}"))?;
    let file = &file_response["data"];
    let file_name = file["fileName"].as_str()
        .ok_or_else(|| "CurseForge не указал имя файла.".to_owned())?;
    let file_name = valid_jar_name(file_name)
        .ok_or_else(|| "CurseForge вернул небезопасное имя JAR-файла.".to_owned())?
        .to_owned();
    let file_size = file["fileLength"].as_u64()
        .ok_or_else(|| "CurseForge не указал размер файла.".to_owned())?;
    if file_size == 0 || file_size > MAX_DOWNLOAD as u64 {
        return Err("Размер CurseForge-мода превышает допустимый предел 512 MiB.".to_owned());
    }
    let expected_hash = file["hashes"].as_array()
        .and_then(|hashes| hashes.iter().find(|hash| hash["algo"].as_u64() == Some(1)))
        .and_then(|hash| hash["value"].as_str())
        .ok_or_else(|| "CurseForge не предоставил SHA-1 для проверки файла.".to_owned())?
        .to_ascii_lowercase();

    let download_response = http.get(format!("{API}/mods/{mod_id}/files/{file_id}/download-url"))
        .send().await
        .map_err(|error| format!("Не удалось запросить URL загрузки CurseForge: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CurseForge не выдал URL загрузки: {error}"))?
        .json::<Value>().await
        .map_err(|error| format!("Ответ URL загрузки повреждён: {error}"))?;
    let download_url = download_response["data"].as_str()
        .ok_or_else(|| "CurseForge не вернул прямую ссылку на файл.".to_owned())?;
    let parsed_url = reqwest::Url::parse(download_url)
        .map_err(|error| format!("CurseForge вернул некорректную ссылку: {error}"))?;
    let download_host = parsed_url.host_str().unwrap_or_default().to_ascii_lowercase();
    if parsed_url.scheme() != "https" || !parsed_url.username().is_empty()
        || parsed_url.password().is_some()
        || !(download_host == "forgecdn.net" || download_host.ends_with(".forgecdn.net"))
    {
        return Err("CurseForge вернул ссылку не на доверенный HTTPS CDN.".to_owned());
    }

    // The API key is only forwarded to CurseForge's own CDN after its host was checked.
    let response = http.get(parsed_url).send().await
        .map_err(|error| format!("Не удалось скачать CurseForge-мод: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Ошибка загрузки CurseForge-мода: {error}"))?;
    if response.content_length().is_some_and(|length| length > MAX_DOWNLOAD as u64) {
        return Err("Скачиваемый CurseForge-мод слишком большой.".to_owned());
    }
    let bytes = response.bytes().await
        .map_err(|error| format!("Не удалось прочитать JAR: {error}"))?;
    if bytes.len() != file_size as usize || bytes.len() > MAX_DOWNLOAD {
        return Err("Размер скачанного файла не совпадает с метаданными CurseForge.".to_owned());
    }
    if digest(&bytes) != expected_hash {
        return Err("SHA-1 CurseForge-мода не совпадает с официальными метаданными.".to_owned());
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err("Файл CurseForge не является корректным ZIP/JAR-архивом.".to_owned());
    }

    let instance_id = instance.id.0.clone();
    let mods_dir = store::instance_directory(&instance_id)?.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await
        .map_err(|error| format!("Не удалось создать папку модов: {error}"))?;
    let target = mods_dir.join(&file_name);
    if target.exists() {
        return Err(format!("Файл {file_name} уже существует в этой сборке; не перезаписываю пользовательский мод."));
    }
    let temporary = target.with_extension("jar.download");
    tokio::fs::write(&temporary, &bytes).await
        .map_err(|error| format!("Не удалось записать JAR: {error}"))?;
    tokio::fs::rename(&temporary, &target).await
        .map_err(|error| format!("Не удалось завершить установку мода: {error}"))?;

    let name = file["displayName"].as_str()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&file_name)
        .to_owned();
    let version = file["displayName"].as_str()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("CurseForge file")
        .to_owned();
    let icon_url = Value::Null;
    let mut manifest = read_manifest(&instance_id)?;
    manifest.mods.retain(|item| !(item.mod_id == mod_id && item.file_id == file_id));
    manifest.mods.push(InstalledCurseForgeMod {
        mod_id, name, file_id, file_name, version, sha1: expected_hash,
        logo_url: icon_url.as_str().map(str::to_owned),
    });
    if let Err(error) = save_manifest(&instance_id, &manifest) {
        let _ = tokio::fs::remove_file(target).await;
        return Err(error);
    }
    installed(&instance_id)
}

pub fn remove(instance_id: &str, mod_id: u32, file_id: u32) -> Result<Vec<InstalledCurseForgeMod>, String> {
    let mut manifest = read_manifest(instance_id)?;
    let Some(index) = manifest.mods.iter().position(|item| item.mod_id == mod_id && item.file_id == file_id) else {
        return Err("Этот CurseForge-мод не управляется GlowstoneCL.".to_owned());
    };
    let item = &manifest.mods[index];
    let file_name = valid_jar_name(&item.file_name)
        .ok_or_else(|| "Некорректное имя файла в CurseForge manifest.".to_owned())?;
    let path = store::instance_directory(instance_id)?.join("mods").join(file_name);
    if path.is_file() {
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("Не удалось прочитать файл перед удалением: {error}"))?;
        if digest(&bytes) != item.sha1 {
            return Err("Файл изменён после установки; не удаляю его автоматически.".to_owned());
        }
        std::fs::remove_file(&path)
            .map_err(|error| format!("Не удалось удалить CurseForge-мод: {error}"))?;
    }
    manifest.mods.remove(index);
    save_manifest(instance_id, &manifest)?;
    installed(instance_id)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_plain_jar_file_names() {
        assert_eq!(valid_jar_name("some-mod.jar"), Some("some-mod.jar"));
        assert!(valid_jar_name("../outside.jar").is_none());
        assert!(valid_jar_name("nested/mod.jar").is_none());
        assert!(valid_jar_name("nested\\mod.jar").is_none());
        assert!(valid_jar_name("not-a-jar.zip").is_none());
    }

    #[test]
    fn sha1_digest_uses_lowercase_hex() {
        assert_eq!(digest(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn curseforge_loader_mapping_is_platform_specific() {
        let mut instance = GameInstance {
            id: glowstone_core::InstanceId("123e4567-e89b-12d3-a456-426614174000".to_owned()),
            name: "Test".to_owned(),
            description: String::new(),
            platform: GamePlatform::Java,
            game_version: "1.20.1".to_owned(),
            loader: LoaderKind::Fabric,
            loader_version: None,
            state: glowstone_core::InstanceState::NotInstalled,
        };
        assert_eq!(loader_type(&instance).unwrap(), 4);
        instance.platform = GamePlatform::Horizon;
        instance.loader = LoaderKind::HorizonContent;
        assert!(loader_type(&instance).is_err());
    }
}
