//! Modrinth modpack discovery and safe .mrpack installation.
use std::io::{Cursor, Read};
use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};

use glowstone_core::{GamePlatform, GameInstance, LoaderKind};
use serde::Deserialize;
use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};

use crate::store;

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)";
const MAX_PACK_ARCHIVE: usize = 256 * 1024 * 1024;
const MAX_PACK_FILE: usize = 512 * 1024 * 1024;
const MAX_TOTAL_FILES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_INDEX_FILES: usize = 1_000;
const MAX_OVERRIDE_FILE: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ModpackProject {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub latest_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectVersion {
    id: String,
    name: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    files: Vec<VersionFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct VersionFile {
    filename: String,
    url: String,
    #[serde(default)]
    primary: bool,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент Modrinth: {error}"))
}

/// Search actual Modrinth projects with project_type:modpack.
pub async fn search(query: String) -> Result<Vec<ModpackProject>, String> {
    let http = http_client()?;
    let facets = serde_json::json!([["project_type:modpack"]]).to_string();
    let mut url = reqwest::Url::parse(&format!("{API}/search"))
        .map_err(|error| format!("Некорректный URL поиска Modrinth: {error}"))?;
    url.query_pairs_mut()
        .append_pair("query", query.trim())
        .append_pair("facets", &facets)
        .append_pair("limit", "24")
        .append_pair("index", "relevance");

    let response = http.get(url).send().await
        .map_err(|error| format!("Не удалось обратиться к Modrinth: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth вернул ошибку поиска: {error}"))?
        .json::<Value>().await
        .map_err(|error| format!("Ответ Modrinth повреждён: {error}"))?;
    let hits = response["hits"].as_array()
        .ok_or_else(|| "Modrinth не вернул список модпаков.".to_owned())?;

    Ok(hits.iter().filter_map(|item| {
        let project_id = item["project_id"].as_str()?.to_owned();
        Some(ModpackProject {
            project_id,
            slug: item["slug"].as_str().unwrap_or_default().to_owned(),
            title: item["title"].as_str().unwrap_or("Modpack").to_owned(),
            description: item["description"].as_str().unwrap_or_default().to_owned(),
            author: item["author"].as_str().unwrap_or("Modrinth").to_owned(),
            downloads: item["downloads"].as_u64().unwrap_or_default(),
            icon_url: item["icon_url"].as_str().map(str::to_owned),
            latest_version: item["latest_version"].as_str().map(str::to_owned),
        })
    }).collect())
}

fn safe_relative_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.contains('\\') || raw.contains('\0') {
        return Err("Модпак содержит небезопасный путь файла.".to_owned());
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err("Модпак содержит абсолютный путь файла.".to_owned());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => return Err("Модпак пытается выйти за пределы папки сборки.".to_owned()),
        }
    }
    if path.components().next().is_none() {
        return Err("Модпак содержит пустой путь.".to_owned());
    }
    Ok(path.to_path_buf())
}

fn public_https_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw)
        .map_err(|error| format!("Модпак содержит некорректный URL: {error}"))?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return Err("Модпак содержит ссылку без безопасного HTTPS.".to_owned());
    }
    if let Some(host) = url.host_str() {
        let host_lower = host.to_ascii_lowercase();
        if host_lower == "localhost" || host_lower.ends_with(".localhost")
            || host_lower.ends_with(".local") || host_lower == "metadata.google.internal"
        {
            return Err("Модпак содержит ссылку на локальный адрес.".to_owned());
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            let forbidden = match ip {
                IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local()
                    || ip.is_broadcast() || ip.is_unspecified(),
                IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local()
                    || ip.is_unicast_link_local() || ip.is_unspecified(),
            };
            if forbidden {
                return Err("Модпак содержит ссылку на частный IP-адрес.".to_owned());
            }
        }
    } else {
        return Err("У URL модпака отсутствует hostname.".to_owned());
    }
    Ok(url)
}

fn selected_version_dependency(version: &ProjectVersion) -> Result<(String, LoaderKind), String> {
    let minecraft = version.game_versions.first().cloned()
        .ok_or_else(|| "В версии модпака отсутствует список версий Minecraft.".to_owned())?;
    let loader = if version.loaders.iter().any(|loader| loader == "fabric") {
        LoaderKind::Fabric
    } else if version.loaders.iter().any(|loader| loader == "neoforge") {
        LoaderKind::NeoForge
    } else if version.loaders.iter().any(|loader| loader == "forge") {
        LoaderKind::Forge
    } else if version.loaders.iter().any(|loader| loader == "quilt") {
        LoaderKind::Quilt
    } else {
        return Err("Этот модпак не содержит поддерживаемый загрузчик Fabric, Forge, NeoForge или Quilt.".to_owned());
    };
    Ok((minecraft, loader))
}

fn loader_dependency_key(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Fabric => "fabric-loader",
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        LoaderKind::Quilt => "quilt-loader",
        _ => "",
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn verify_hashes(bytes: &[u8], hashes: &Value) -> Result<(), String> {
    if let Some(expected) = hashes["sha512"].as_str() {
        let actual = hex_digest(&Sha512::digest(bytes));
        if actual.eq_ignore_ascii_case(expected) {
            return Ok(());
        }
        return Err("SHA-512 файла из модпака не совпадает.".to_owned());
    }
    if let Some(expected) = hashes["sha256"].as_str() {
        let actual = hex_digest(&Sha256::digest(bytes));
        if actual.eq_ignore_ascii_case(expected) {
            return Ok(());
        }
        return Err("SHA-256 файла из модпака не совпадает.".to_owned());
    }
    if let Some(expected) = hashes["sha1"].as_str() {
        let actual = hex_digest(&Sha1::digest(bytes));
        if actual.eq_ignore_ascii_case(expected) {
            return Ok(());
        }
        return Err("SHA-1 файла из модпака не совпадает.".to_owned());
    }
    Err("Для одного из файлов модпака отсутствует поддерживаемый хеш.".to_owned())
}

async fn download_mrpack(
    http: &reqwest::Client,
    project_id: &str,
 ) -> Result<(Vec<u8>, String, String, LoaderKind), String> {
    let versions_url = format!("{API}/project/{project_id}/version");
    let response = http.get(versions_url).send().await
        .map_err(|error| format!("Не удалось получить версии модпака: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth не вернул версии модпака: {error}"))?
        .json::<Vec<ProjectVersion>>().await
        .map_err(|error| format!("Список версий модпака повреждён: {error}"))?;
    let mut candidates: Vec<_> = response.into_iter()
        .filter(|version| version.files.iter().any(|file| file.filename.to_ascii_lowercase().ends_with(".mrpack")))
        .collect();
    candidates.sort_by(|a, b| {
        let a_release = a.version_type == "release";
        let b_release = b.version_type == "release";
        b_release.cmp(&a_release)
    });
    let version = candidates.into_iter().next()
        .ok_or_else(|| "Для этого проекта Modrinth не нашёл файл .mrpack.".to_owned())?;
    let (minecraft, loader) = selected_version_dependency(&version)?;
    let file = version.files.iter()
        .find(|file| file.primary && file.filename.to_ascii_lowercase().ends_with(".mrpack"))
        .or_else(|| version.files.iter().find(|file| file.filename.to_ascii_lowercase().ends_with(".mrpack")))
        .ok_or_else(|| "У версии модпака отсутствует .mrpack.".to_owned())?;
    let url = public_https_url(&file.url)?;
    let bytes = http.get(url).send().await
        .map_err(|error| format!("Не удалось скачать .mrpack: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Ошибка скачивания .mrpack: {error}"))?
        .bytes().await
        .map_err(|error| format!("Не удалось прочитать .mrpack: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_PACK_ARCHIVE {
        return Err("Архив модпака пустой или больше 256 MiB.".to_owned());
    }
    Ok((
        bytes.to_vec(),
        minecraft,
        version.name,
        loader,
    ))
}

fn write_overrides(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, destination: &Path) -> Result<(), String> {
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)
            .map_err(|error| format!("Не удалось прочитать содержимое .mrpack: {error}"))?;
        let name = file.name().to_owned();
        let prefix = if name.starts_with("overrides/") {
            "overrides/"
        } else if name.starts_with("client-overrides/") {
            "client-overrides/"
        } else {
            continue;
        };
        if file.is_dir() {
            continue;
        }
        if file.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err("Модпак содержит символическую ссылку в overrides.".to_owned());
        }
        let relative = safe_relative_path(name.strip_prefix(prefix).unwrap_or_default())?;
        let size = file.size();
        if size > MAX_OVERRIDE_FILE {
            return Err("Файл overrides превышает 64 MiB.".to_owned());
        }
        total = total.saturating_add(size);
        if total > MAX_TOTAL_FILES {
            return Err("Общий размер overrides превышает 2 GiB.".to_owned());
        }
        let target = destination.join(relative);
        if !target.starts_with(destination) {
            return Err("Путь overrides выходит за пределы сборки.".to_owned());
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Не удалось создать каталог overrides: {error}"))?;
        }
        let mut data = Vec::with_capacity(size as usize);
        file.take(MAX_OVERRIDE_FILE + 1).read_to_end(&mut data)
            .map_err(|error| format!("Не удалось извлечь overrides: {error}"))?;
        if data.len() as u64 != size {
            return Err("Размер извлечённого overrides не совпадает с архивом.".to_owned());
        }
        std::fs::write(target, data)
            .map_err(|error| format!("Не удалось записать overrides: {error}"))?;
    }
    Ok(())
}

async fn install_index_files(
    http: &reqwest::Client,
    archive_bytes: &[u8],
    game_directory: &Path,
    expected_minecraft: &str,
    expected_loader: LoaderKind,
) -> Result<(usize, String), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(archive_bytes.to_vec()))
        .map_err(|error| format!("Архив .mrpack не является корректным ZIP: {error}"))?;
    let index_value: Value = {
        let mut index_file = archive.by_name("modrinth.index.json")
            .map_err(|_| "В .mrpack отсутствует modrinth.index.json.".to_owned())?;
        let mut bytes = Vec::new();
        index_file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)
            .map_err(|error| format!("Не удалось прочитать index.json: {error}"))?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("modrinth.index.json больше 8 MiB.".to_owned());
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("modrinth.index.json повреждён: {error}"))?
    };
    if index_value["formatVersion"].as_u64() != Some(1)
        || index_value["game"].as_str() != Some("minecraft")
    {
        return Err("Версия формата или целевая игра .mrpack не поддерживается.".to_owned());
    }
    let index_dependencies = index_value["dependencies"].as_object()
        .ok_or_else(|| "В index.json отсутствует объект dependencies.".to_owned())?;
    let index_minecraft = index_dependencies.get("minecraft").and_then(Value::as_str)
        .ok_or_else(|| "В index.json не указана версия Minecraft.".to_owned())?;
    if index_minecraft != expected_minecraft {
        return Err(format!(
            "Версии Minecraft в Modrinth API ({expected_minecraft}) и modrinth.index.json ({index_minecraft}) не совпадают."
        ));
    }
    let dependency_key = loader_dependency_key(expected_loader);
    let loader_version = index_dependencies.get(dependency_key).and_then(Value::as_str)
        .ok_or_else(|| format!("В index.json отсутствует версия загрузчика {dependency_key}."))?
        .to_owned();
    let files = index_value["files"].as_array()
        .ok_or_else(|| "В index.json отсутствует массив files.".to_owned())?;
    if files.len() > MAX_INDEX_FILES {
        return Err("Модпак содержит более 1000 файлов.".to_owned());
    }

    let mut total = 0u64;
    let mut installed = 0usize;
    for item in files {
        if item["env"]["client"].as_str() == Some("unsupported") {
            continue;
        }
        let raw_path = item["path"].as_str()
            .ok_or_else(|| "В index.json есть файл без пути.".to_owned())?;
        let relative = safe_relative_path(raw_path)?;
        let file_size = item["fileSize"].as_u64()
            .ok_or_else(|| format!("Не указан размер для {raw_path}."))?;
        if file_size == 0 || file_size > MAX_PACK_FILE as u64 {
            return Err(format!("Недопустимый размер файла {raw_path}."));
        }
        total = total.saturating_add(file_size);
        if total > MAX_TOTAL_FILES {
            return Err("Общий размер клиентских файлов модпака превышает 2 GiB.".to_owned());
        }
        let downloads = item["downloads"].as_array()
            .ok_or_else(|| format!("Для {raw_path} отсутствуют ссылки скачивания."))?;
        let url = downloads.iter().filter_map(Value::as_str).find_map(|raw| public_https_url(raw).ok())
            .ok_or_else(|| format!("Для {raw_path} нет безопасной HTTPS-ссылки."))?;

        let response = http.get(url).send().await
            .map_err(|error| format!("Не удалось скачать {raw_path}: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Ошибка загрузки {raw_path}: {error}"))?;
        if response.content_length().is_some_and(|length| length > MAX_PACK_FILE as u64) {
            return Err(format!("Загружаемый файл {raw_path} превышает 512 MiB."));
        }
        let bytes = response.bytes().await
            .map_err(|error| format!("Не удалось прочитать {raw_path}: {error}"))?;
        if bytes.len() as u64 != file_size {
            return Err(format!("Размер {raw_path} не совпадает с index.json."));
        }
        verify_hashes(&bytes, &item["hashes"])
            .map_err(|error| format!("{raw_path}: {error}"))?;

        let target = game_directory.join(&relative);
        if !target.starts_with(game_directory) {
            return Err(format!("Путь {raw_path} выходит за пределы сборки."));
        }
        if target.exists() {
            return Err(format!("Файл {raw_path} уже существует; модпак не будет его перезаписывать."));
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|error| format!("Не удалось создать папку для {raw_path}: {error}"))?;
        }
        tokio::fs::write(target, &bytes).await
            .map_err(|error| format!("Не удалось сохранить {raw_path}: {error}"))?;
        installed += 1;
    }

    write_overrides(&mut archive, game_directory)?;
    Ok((installed, loader_version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_and_absolute_pack_paths() {
        assert!(safe_relative_path("mods/example.jar").is_ok());
        assert!(safe_relative_path("../outside.jar").is_err());
        assert!(safe_relative_path("/tmp/outside.jar").is_err());
        assert!(safe_relative_path("mods\\\\outside.jar").is_err());
    }

    #[test]
    fn selects_java_loader_from_modrinth_version_metadata() {
        let version: ProjectVersion = serde_json::from_str(
            r#"{"id":"version","name":"Pack","version_number":"1.0","game_versions":["1.20.1"],"loaders":["fabric"],"files":[]}"#
        ).unwrap();
        assert_eq!(
            selected_version_dependency(&version).unwrap(),
            ("1.20.1".to_owned(), LoaderKind::Fabric),
        );
    }

    #[test]
    fn hash_verification_rejects_modified_pack_files() {
        assert!(verify_hashes(b"expected", &serde_json::json!({
            "sha256": hex_digest(&Sha256::digest(b"expected"))
        })).is_ok());
        assert!(verify_hashes(b"tampered", &serde_json::json!({
            "sha256": format!("{:x}", Sha256::digest(b"expected"))
        })).is_err());
    }
}

/// Installs the latest compatible .mrpack as a new Java instance. Any error moves the
/// partially-created instance to the existing local trash mechanism.
pub async fn install_project(project_id: String, project_name: String) -> Result<GameInstance, String> {
    let http = http_client()?;
    let (bytes, minecraft, version_name, loader) =
        download_mrpack(&http, &project_id).await?;
    let instance_name = {
        let base = if project_name.trim().is_empty() { "Modpack" } else { project_name.trim() };
        let title = format!("{base} · {version_name}");
        title.chars().take(64).collect::<String>()
    };
    let mut instance = store::create_instance(
        instance_name,
        minecraft,
        GamePlatform::Java,
        loader,
    )?;
    let game_directory = match store::instance_directory(&instance.id.0) {
        Ok(path) => path,
        Err(error) => {
            let _ = store::remove_instance(&instance.id.0);
            return Err(error);
        }
    };

    let result = async {
        let (installed, loader_version) = install_index_files(
            &http, &bytes, &game_directory, &instance.game_version, loader,
        ).await?;
        if installed == 0 {
            return Err("В этом модпаке нет клиентских файлов для установки.".to_owned());
        }

        instance.loader_version = Some(loader_version.clone());
        store::save_instance_metadata(&instance)?;

        let pack_manifest = serde_json::json!({
            "source": "modrinth",
            "project_id": project_id,
            "version": version_name,
            "minecraft": instance.game_version,
            "loader": format!("{loader:?}"),
            "loader_version": loader_version,
            "installed_files": installed,
        });
        tokio::fs::write(
            game_directory.join("modpack.json"),
            serde_json::to_vec_pretty(&pack_manifest).map_err(|error| error.to_string())?,
        ).await.map_err(|error| format!("Не удалось сохранить сведения о модпаке: {error}"))?;
        Ok::<(), String>(())
    }.await;

    if let Err(error) = result {
        let _ = store::remove_instance(&instance.id.0);
        return Err(error);
    }
    Ok(instance)
}
