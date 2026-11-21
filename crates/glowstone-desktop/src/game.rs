//! Game installation and launch backends.
//!
//! Minecraft Java is managed by CrustCore (Mojang metadata, assets, libraries,
//! Java runtimes and loader installation). Horizon is installed as its own
//! signed runtime and launched through Horizon's advertised Launch API.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

use base64::Engine as _;
use crust_core::authenticator::{Account as GameAccount, AccountType};
use crust_core::foundation::events::Event;
use crust_core::launcher::{
    Launch as JavaLaunch, LaunchOptions as JavaLaunchOptions, LoaderKind as JavaLoaderKind,
};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest as _;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;

use crate::auth;
use crate::store::{self, AccountKind, AccountProfile};
use glowstone_core::{GameInstance, GamePlatform, LoaderKind};

const HORIZON_NIGHTLY_API: &str =
    "https://api.github.com/repos/horizon-rust/horizon/releases/tags/nightly";
const HORIZON_RELEASE_KEY: &str = "MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw=";
const HORIZON_SIGNATURE_DOMAIN: &str = "horizon release ";
const HORIZON_INSTALLED_MARKER: &str = ".installed";

#[derive(Debug, Clone)]
pub struct LaunchOutcome {
    pub instance_id: String,
    pub platform: GamePlatform,
    pub exit_code: Option<i32>,
}

/// Refreshes Microsoft credentials immediately before launch. Tokens are never
/// written to state.json; the rotating refresh token stays in the OS keyring.
pub async fn launch_instance(
    instance: GameInstance,
    account: AccountProfile,
    microsoft_client_id: String,
) -> Result<LaunchOutcome, String> {
    let data_root = store::app_data_dir()?;
    let instance_dir = store::instance_directory(&instance.id.0)?;
    fs::create_dir_all(&instance_dir)
        .map_err(|error| format!("Не удалось создать папку сборки: {error}"))?;

    let game_account = match account.kind {
        AccountKind::Offline => auth::offline_game_account(&account),
        AccountKind::Microsoft => {
            let refresh_token = store::load_refresh_token(&account.id)?;
            let refreshed = auth::refresh_game_account(
                microsoft_client_id,
                &account,
                refresh_token,
            )
            .await?;

            if let Some(rotated) = refreshed.refresh_token.as_deref() {
                store::store_refresh_token(&account.id, rotated)?;
            }
            refreshed
        }
    };

    match instance.platform {
        GamePlatform::Java => {
            launch_java(instance, data_root, game_account).await
        }
        GamePlatform::Horizon => {
            if account.kind != AccountKind::Microsoft
                || game_account.kind() != AccountType::Xbox
            {
                return Err(
                    "Horizon требует Microsoft-аккаунт с доступом к Minecraft: Java Edition."
                        .to_owned(),
                );
            }
            launch_horizon(instance, data_root, instance_dir, game_account).await
        }
    }
}

async fn launch_java(
    instance: GameInstance,
    data_root: PathBuf,
    account: GameAccount,
) -> Result<LaunchOutcome, String> {
    let mut options = JavaLaunchOptions::new(data_root, instance.game_version.clone());
    options.instance = Some(instance.id.0.clone());
    options.loader.path = format!("instances/{}/loader", instance.id.0);
    options.download_concurrency = 10;
    options.memory.max = "4G".to_owned();

    match instance.loader {
        LoaderKind::Vanilla => {}
        LoaderKind::Forge => {
            options.loader.kind = Some(JavaLoaderKind::Forge);
            options.loader.enable = true;
        }
        LoaderKind::NeoForge => {
            options.loader.kind = Some(JavaLoaderKind::NeoForge);
            options.loader.enable = true;
        }
        LoaderKind::Fabric => {
            options.loader.kind = Some(JavaLoaderKind::Fabric);
            options.loader.enable = true;
        }
        LoaderKind::Quilt => {
            options.loader.kind = Some(JavaLoaderKind::Quilt);
            options.loader.enable = true;
        }
        LoaderKind::HorizonContent | LoaderKind::HorizonNative => {
            return Err("Выбран Horizon-загрузчик для Minecraft Java.".to_owned());
        }
    }

    let launch = JavaLaunch::new(options, account)
        .map_err(|error| format!("Не удалось подготовить запуск Minecraft Java: {error}"))?
        .with_events(std::sync::Arc::new(|event| match event {
            Event::Patch(line) | Event::Extract(line) => {
                eprintln!("[GlowstoneCL] {line}");
            }
            Event::Error(message) => {
                eprintln!("[GlowstoneCL] {message}");
            }
            Event::Progress { downloaded, total, element } => {
                if total > 0 && downloaded >= total {
                    eprintln!("[GlowstoneCL] {element}: завершено");
                }
            }
            Event::Check { checked, total, element } => {
                if total > 0 && checked >= total {
                    eprintln!("[GlowstoneCL] {element}: проверено {total} файлов");
                }
            }
            Event::Speed(_) | Event::Estimated(_) => {}
        }));

    eprintln!(
        "[GlowstoneCL] Подготовка Minecraft Java {} ({:?})",
        instance.game_version, instance.loader
    );
    let prepared = launch
        .prepare()
        .await
        .map_err(|error| format!("Не удалось установить Minecraft Java: {error}"))?;
    let plan = launch
        .plan(&prepared)
        .map_err(|error| format!("Не удалось сформировать команду запуска: {error}"))?;
    let mut child = launch
        .spawn(&plan)
        .map_err(|error| format!("Не удалось запустить Java: {error}"))?;
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Ошибка ожидания Minecraft Java: {error}"))?;

    Ok(LaunchOutcome {
        instance_id: instance.id.0,
        platform: GamePlatform::Java,
        exit_code: status.code(),
    })
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    /// GitHub's Releases API names the human-readable release title `name`.
    #[serde(rename = "name")]
    title: String,
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
struct HorizonBuild {
    version: String,
    directory: PathBuf,
}

impl HorizonBuild {
    fn client(&self) -> PathBuf {
        self.directory.join(format!("horizon-client{}", std::env::consts::EXE_SUFFIX))
    }

    fn preparer(&self) -> PathBuf {
        self.directory.join(format!("horizon-prepare{}", std::env::consts::EXE_SUFFIX))
    }

    fn prepared(&self) -> PathBuf {
        self.directory.join("prepared")
    }

    fn pins(&self) -> Result<HorizonPins, String> {
        let path = self.directory.join("research-lock.json");
        let bytes = fs::read(&path)
            .map_err(|error| format!("Не удалось прочитать {}: {error}", path.display()))?;
        let lock: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("research-lock.json повреждён: {error}"))?;
        let minecraft = &lock["minecraft"];
        let version = minecraft["version"]
            .as_str()
            .ok_or_else(|| "В research-lock.json отсутствует minecraft.version.".to_owned())?
            .to_owned();
        let java_major = minecraft["java_major"]
            .as_u64()
            .ok_or_else(|| "В research-lock.json отсутствует minecraft.java_major.".to_owned())?
            as u32;
        Ok(HorizonPins { version, java_major })
    }
}

#[derive(Debug)]
struct HorizonPins {
    version: String,
    java_major: u32,
}

async fn launch_horizon(
    instance: GameInstance,
    data_root: PathBuf,
    instance_dir: PathBuf,
    account: GameAccount,
) -> Result<LaunchOutcome, String> {
    let client = reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)")
        .build()
        .map_err(|error| error.to_string())?;

    let build = ensure_horizon_build(&client, &data_root).await?;
    let pins = build.pins()?;

    let prepare_api = query_launch_api(&build.preparer(), &build.directory).await?;
    let advertised_version = prepare_api["minecraft"]
        .as_str()
        .ok_or_else(|| "Horizon preparer не сообщил версию Minecraft.".to_owned())?;
    let advertised_java = prepare_api["java_major"]
        .as_u64()
        .ok_or_else(|| "Horizon preparer не сообщил требуемую версию Java.".to_owned())?
        as u32;
    if advertised_version != pins.version || advertised_java != pins.java_major {
        return Err(
            "research-lock.json и Launch API Horizon расходятся по версии Minecraft/Java."
                .to_owned(),
        );
    }

    let game_root = data_root.clone();
    let horizon_ready = is_horizon_prepared(&build).await;
    if !horizon_ready {
        // Horizon does not ship Mojang's game files. CrustCore installs the
        // official, licensed files and matching Java runtime in the shared root.
        let mut java_options = JavaLaunchOptions::new(&game_root, pins.version.clone());
        java_options.download_concurrency = 10;
        java_options.memory.max = "4G".to_owned();
        let java_launch = JavaLaunch::new(java_options, account.clone())
            .map_err(|error| format!("Не удалось создать установщик Minecraft для Horizon: {error}"))?;
        let prepared_java = java_launch
            .prepare()
            .await
            .map_err(|error| format!("Не удалось загрузить файлы Minecraft для Horizon: {error}"))?;

        let version_id = prepared_java.version.id.clone();
        let version_json_path = game_root
            .join("versions")
            .join(&version_id)
            .join(format!("{version_id}.json"));
        let version_bytes = tokio::fs::read(&version_json_path)
            .await
            .map_err(|error| format!("Не найден JSON версии Minecraft: {error}"))?;
        let version_json: Value = serde_json::from_slice(&version_bytes)
            .map_err(|error| format!("JSON версии Minecraft повреждён: {error}"))?;
        let server_jar = ensure_server_jar(&client, &version_json, &game_root, &version_id).await?;

        let java_home = prepared_java
            .java
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| "Не удалось определить каталог Java Runtime.".to_owned())?
            .to_path_buf();
        prepare_horizon(&build, &game_root, &server_jar, &java_home).await?;
        if !is_horizon_prepared(&build).await {
            return Err("Horizon preparer завершился, но проверка ресурсов не прошла.".to_owned());
        }
    }

    fs::create_dir_all(&instance_dir)
        .map_err(|error| format!("Не удалось создать папку мира Horizon: {error}"))?;
    launch_horizon_client(&build, &instance_dir, &account).await
        .map(|exit_code| LaunchOutcome {
            instance_id: instance.id.0,
            platform: GamePlatform::Horizon,
            exit_code,
        })
}

async fn ensure_horizon_build(
    client: &reqwest::Client,
    data_root: &Path,
) -> Result<HorizonBuild, String> {
    let versions_dir = data_root.join("horizon").join("versions");
    fs::create_dir_all(&versions_dir)
        .map_err(|error| format!("Не удалось создать каталог Horizon: {error}"))?;

    let existing = installed_horizon_build(&versions_dir);
    let release_result = fetch_horizon_release(client).await;
    let release = match release_result {
        Ok(release) => release,
        Err(error) => {
            return existing.ok_or_else(|| {
                format!("Не удалось получить Horizon с GitHub, и локальной версии нет: {error}")
            });
        }
    };
    let asset_name = horizon_asset_name()?;
    let version = nightly_version(&release)?;
    let target = versions_dir.join(&version);
    if target.join(HORIZON_INSTALLED_MARKER).is_file() {
        return Ok(HorizonBuild { version, directory: target });
    }

    let find_asset = |name: &str| {
        release
            .assets
            .iter()
            .find(|asset| asset.name == name)
            .map(|asset| asset.browser_download_url.clone())
            .ok_or_else(|| format!("В релизе Horizon отсутствует файл {name}."))
    };
    let sums_url = find_asset("SHA256SUMS")?;
    // Nightly releases are signed for their exact date+commit identity.
    // The channel signature (SHA256SUMS.sig) is retained only for older
    // launchers and must not be used to accept a new Nightly build.
    let signature_url = find_asset("SHA256SUMS.v2.sig")?;
    let archive_url = find_asset(asset_name)?;

    let (sums_response, signature_response) = tokio::try_join!(
        client.get(sums_url).send(),
        client.get(signature_url).send()
    )
    .map_err(|error| format!("Не удалось получить подпись релиза Horizon: {error}"))?;
    let sums = sums_response
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    let signature = signature_response
        .error_for_status()
        .map_err(|error| error.to_string())?
        .text()
        .await
        .map_err(|error| error.to_string())?;
    verify_horizon_signature(&version, &sums, &signature)?;

    let expected_sha256 = expected_sha256(&sums, asset_name)?;
    let archive_response = client
        .get(archive_url)
        .send()
        .await
        .map_err(|error| format!("Не удалось скачать Horizon: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Ошибка загрузки Horizon: {error}"))?;
    let archive = archive_response
        .bytes()
        .await
        .map_err(|error| format!("Не удалось прочитать архив Horizon: {error}"))?;
    let actual_sha256 = format!("{:x}", sha2::Sha256::digest(&archive));
    if actual_sha256 != expected_sha256 {
        return Err("SHA-256 архива Horizon не совпадает с подписанным SHA256SUMS.".to_owned());
    }

    if !version
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("GitHub вернул некорректное имя версии Horizon.".to_owned());
    }

    let archive_path = versions_dir.join(format!(".{version}.archive"));
    tokio::fs::write(&archive_path, &archive)
        .await
        .map_err(|error| format!("Не удалось сохранить архив Horizon: {error}"))?;
    let stage = versions_dir.join(format!(".{version}.stage"));
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&stage).map_err(|error| error.to_string())?;
    let archive_for_extract = archive_path.clone();
    let stage_for_extract = stage.clone();
    let extract_result = tokio::task::spawn_blocking(move || {
        unpack_horizon_archive(&archive_for_extract, &stage_for_extract)
    })
    .await
    .map_err(|error| format!("Задача распаковки Horizon завершилась с ошибкой: {error}"))?;
    if let Err(error) = extract_result {
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    let _ = fs::remove_file(&archive_path);

    let client_binary = stage.join(format!("horizon-client{}", std::env::consts::EXE_SUFFIX));
    let preparer_binary = stage.join(format!("horizon-prepare{}", std::env::consts::EXE_SUFFIX));
    if !client_binary.is_file() || !preparer_binary.is_file() || !stage.join("research-lock.json").is_file() {
        let _ = fs::remove_dir_all(&stage);
        return Err("Архив Horizon не содержит client, preparer и research-lock.json.".to_owned());
    }
    fs::write(stage.join(HORIZON_INSTALLED_MARKER), b"")
        .map_err(|error| format!("Не удалось пометить установку Horizon: {error}"))?;
    HorizonBuild { version: version.clone(), directory: stage.clone() }
        .pins()?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    }
    fs::rename(&stage, &target)
        .map_err(|error| format!("Не удалось завершить установку Horizon: {error}"))?;

    Ok(HorizonBuild { version, directory: target })
}

fn installed_horizon_build(versions_dir: &Path) -> Option<HorizonBuild> {
    let mut versions: Vec<_> = fs::read_dir(versions_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join(HORIZON_INSTALLED_MARKER).is_file())
        .map(|entry| entry.path())
        .collect();
    versions.sort_by_key(|path| path.file_name().map(|name| name.to_os_string()));
    versions.reverse();
    versions.into_iter().find_map(|directory| {
        let version = directory.file_name()?.to_string_lossy().into_owned();
        let build = HorizonBuild { version, directory };
        (build.client().is_file() && build.preparer().is_file() && build.pins().is_ok())
            .then_some(build)
    })
}

async fn fetch_horizon_release(client: &reqwest::Client) -> Result<GithubRelease, String> {
    client
        .get(HORIZON_NIGHTLY_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("GitHub недоступен: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub вернул ошибку: {error}"))?
        .json::<GithubRelease>()
        .await
        .map_err(|error| format!("GitHub вернул некорректный релиз Horizon: {error}"))
}

fn horizon_asset_name() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("horizon-linux-x86_64.tar.gz"),
        ("windows", "x86_64") => Ok("horizon-windows-x64.zip"),
        _ => Err(format!(
            "Horizon пока не публикует сборку для {}-{}.",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
    }
}

fn nightly_version(release: &GithubRelease) -> Result<String, String> {
    // Horizon's v2 signature binds a Nightly to the date and commit in its
    // release title. Never infer this identity from GitHub's publication time:
    // that would let an old artifact be presented under a newer timestamp.
    let title = release
        .title
        .strip_prefix("Nightly ")
        .ok_or_else(|| "Заголовок релиза Horizon не содержит Nightly identity.".to_owned())?;
    let (date, commit) = title
        .split_once(" · ")
        .ok_or_else(|| "Заголовок Nightly Horizon не содержит commit.".to_owned())?;
    if !valid_date(date) {
        return Err("Дата в заголовке Nightly Horizon имеет неверный формат.".to_owned());
    }
    if !(7..=40).contains(&commit.len())
        || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Commit в заголовке Nightly Horizon имеет неверный формат.".to_owned());
    }
    Ok(format!("nightly-{date}-{commit.to_ascii_lowercase()}"))
}

fn valid_date(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

fn verify_horizon_signature(tag: &str, sums: &[u8], signatures: &str) -> Result<(), String> {
    let mut message = format!("{HORIZON_SIGNATURE_DOMAIN}{tag}\n").into_bytes();
    message.extend_from_slice(sums);
    let key = base64::engine::general_purpose::STANDARD
        .decode(HORIZON_RELEASE_KEY)
        .map_err(|error| format!("Встроенный ключ Horizon повреждён: {error}"))?;
    let public_key = UnparsedPublicKey::new(&ED25519, key);

    for encoded in signatures.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(signature) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
            continue;
        };
        if public_key.verify(&message, &signature).is_ok() {
            return Ok(());
        }
    }
    Err("Подпись Horizon не прошла проверку Ed25519; архив не будет установлен.".to_owned())
}

fn expected_sha256(sums: &[u8], asset_name: &str) -> Result<String, String> {
    let text = std::str::from_utf8(sums)
        .map_err(|error| format!("SHA256SUMS не является UTF-8: {error}"))?;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let Some(hash) = fields.next() else { continue };
        let Some(file) = fields.next() else { continue };
        if file.trim_start_matches('*') == asset_name
            && hash.len() == 64
            && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err(format!("SHA256SUMS не содержит контрольную сумму {asset_name}."))
}

fn unpack_horizon_archive(archive_path: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let archive_name = archive_path.to_string_lossy();
    if archive_name.ends_with(".tar.gz") {
        unpack_horizon_tar(archive_path, destination)
    } else if archive_name.ends_with(".zip") {
        unpack_horizon_zip(archive_path, destination)
    } else {
        Err(format!("Неизвестный формат архива Horizon: {archive_name}"))
    }
}

fn stripped_archive_path(entry: &Path) -> Result<Option<PathBuf>, String> {
    let mut parts = Vec::new();
    for component in entry.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_os_string()),
            Component::CurDir => {}
            _ => return Err(format!("Небезопасный путь в архиве Horizon: {}", entry.display())),
        }
    }
    if parts.len() < 2 {
        return Ok(None);
    }
    Ok(Some(parts[1..].iter().fold(PathBuf::new(), |path, part| path.join(part))))
}

fn unpack_horizon_tar(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|error| error.to_string())?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|error| format!("Архив Horizon не читается: {error}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| format!("Ошибка записи tar: {error}"))?;
        let entry_path = entry.path().map_err(|error| error.to_string())?.into_owned();
        let Some(relative) = stripped_archive_path(&entry_path)? else { continue };
        let target = destination.join(relative);
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        } else if kind.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            entry.unpack(&target).map_err(|error| format!("Не удалось распаковать {}: {error}", target.display()))?;
        } else {
            return Err(format!("В архиве Horizon найден небезопасный тип записи: {}", entry_path.display()));
        }
    }
    Ok(())
}

fn unpack_horizon_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("ZIP-архив Horizon не читается: {error}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Ошибка ZIP-записи Horizon: {error}"))?;
        let safe_name = entry
            .enclosed_name()
            .ok_or_else(|| format!("Небезопасный путь в ZIP Horizon: {}", entry.name()))?
            .to_path_buf();
        let Some(relative) = stripped_archive_path(&safe_name)? else { continue };
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = fs::File::create(&target).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Не удалось распаковать {}: {error}", target.display()))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

async fn query_launch_api(program: &Path, working_dir: &Path) -> Result<Value, String> {
    let output = TokioCommand::new(program)
        .arg("--launch-api")
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| format!("Не удалось запросить Launch API {}: {error}", program.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} не поддерживает Launch API: {}",
            program.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Launch API вернул некорректный JSON: {error}"))
}

fn advertised_options(api: &Value) -> Result<HashSet<String>, String> {
    let entries = api["options"]
        .as_array()
        .ok_or_else(|| "Launch API не содержит список options.".to_owned())?;
    Ok(entries
        .iter()
        .filter_map(|entry| entry["name"].as_str().map(str::to_owned))
        .collect())
}

async fn is_horizon_prepared(build: &HorizonBuild) -> bool {
    let Ok(api) = query_launch_api(&build.preparer(), &build.directory).await else {
        return false;
    };
    let Ok(options) = advertised_options(&api) else {
        return false;
    };
    if !options.contains("--check") {
        return false;
    }
    TokioCommand::new(build.preparer())
        .args(["--check", "--output"])
        .arg(build.prepared())
        .current_dir(&build.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

async fn prepare_horizon(
    build: &HorizonBuild,
    minecraft_dir: &Path,
    server_jar: &Path,
    java_home: &Path,
) -> Result<(), String> {
    let api = query_launch_api(&build.preparer(), &build.directory).await?;
    let options = advertised_options(&api)?;
    for required in ["--prepare", "--output", "--server-jar", "--minecraft-dir"] {
        if !options.contains(required) {
            return Err(format!("Этот Horizon preparer не поддерживает обязательную опцию {required}."));
        }
    }

    let mut command = TokioCommand::new(build.preparer());
    command
        .current_dir(&build.directory)
        .arg("--prepare")
        .arg("--output")
        .arg(build.prepared())
        .arg("--server-jar")
        .arg(server_jar)
        .arg("--minecraft-dir")
        .arg(minecraft_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if options.contains("--complete") {
        command.arg("--complete");
    }
    if options.contains("--java-home") {
        command.arg("--java-home").arg(java_home);
    }
    if options.contains("--progress") {
        command.args(["--progress", "json"]);
    }

    eprintln!("[GlowstoneCL] Подготовка ресурсов Horizon {}...", build.version);
    let output = command
        .output()
        .await
        .map_err(|error| format!("Не удалось запустить horizon-prepare: {error}"))?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            if let Some(message) = value["message"].as_str() {
                eprintln!("[Horizon] {message}");
            }
            if let Some(fraction) = value["fraction"].as_f64() {
                eprintln!("[Horizon] подготовка: {:.0}%", fraction * 100.0);
            }
        }
    }
    if !output.status.success() {
        return Err(format!(
            "Horizon не смог подготовить ресурсы: {}{}",
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .rev()
                .find(|line| line.contains("\"error\""))
                .map(|line| format!(" ({line})"))
                .unwrap_or_default()
        ));
    }
    Ok(())
}

async fn ensure_server_jar(
    client: &reqwest::Client,
    version_json: &Value,
    data_root: &Path,
    version_id: &str,
) -> Result<PathBuf, String> {
    let server = &version_json["downloads"]["server"];
    let url = server["url"]
        .as_str()
        .ok_or_else(|| format!("Mojang metadata версии {version_id} не содержит server.jar."))?;
    let expected_sha1 = server["sha1"]
        .as_str()
        .ok_or_else(|| "Mojang metadata не содержит SHA-1 server.jar.".to_owned())?
        .to_ascii_lowercase();
    let target = data_root
        .join("horizon")
        .join("server-jars")
        .join(format!("{version_id}.jar"));
    if let Ok(existing) = tokio::fs::read(&target).await {
        if format!("{:x}", sha1::Sha1::digest(&existing)) == expected_sha1 {
            return Ok(target);
        }
    }

    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Не удалось скачать Minecraft server.jar: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Ошибка загрузки Minecraft server.jar: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("Не удалось прочитать Minecraft server.jar: {error}"))?;
    if format!("{:x}", sha1::Sha1::digest(&bytes)) != expected_sha1 {
        return Err("SHA-1 Minecraft server.jar не совпадает с Mojang metadata.".to_owned());
    }
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
    }
    tokio::fs::write(&target, &bytes)
        .await
        .map_err(|error| format!("Не удалось сохранить Minecraft server.jar: {error}"))?;
    Ok(target)
}

#[derive(Serialize)]
struct HorizonSession<'a> {
    name: &'a str,
    uuid: String,
    access_token: &'a str,
    user_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    xuid: Option<&'a str>,
}

async fn launch_horizon_client(
    build: &HorizonBuild,
    game_dir: &Path,
    account: &GameAccount,
) -> Result<Option<i32>, String> {
    let api = query_launch_api(&build.client(), &build.directory).await?;
    let options = advertised_options(&api)?;
    if !options.contains("--resources") {
        return Err("Эта сборка Horizon не поддерживает --resources.".to_owned());
    }
    if !options.contains("--session-stdin") {
        return Err(
            "Эта сборка Horizon не поддерживает безопасную передачу Microsoft-сессии через stdin."
                .to_owned(),
        );
    }

    let mut args = vec![
        "--resources".to_owned(),
        build.prepared().display().to_string(),
    ];
    if options.contains("--game-dir") {
        args.push("--game-dir".to_owned());
        args.push(game_dir.display().to_string());
    }
    if options.contains("--home") {
        args.push("--home".to_owned());
        args.push(game_dir.display().to_string());
    }
    args.push("--session-stdin".to_owned());

    let xuid = account.xuid();
    let session = HorizonSession {
        name: &account.name,
        uuid: account.uuid.replace('-', ""),
        access_token: &account.access_token,
        user_type: "msa",
        xuid,
    };
    let session_json = serde_json::to_vec(&session)
        .map_err(|error| format!("Не удалось сериализовать игровую сессию: {error}"))?;

    let mut child = TokioCommand::new(build.client())
        .args(&args)
        .current_dir(game_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("Не удалось запустить horizon-client: {error}"))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Не удалось открыть stdin Horizon.".to_owned())?;
        if let Err(error) = stdin.write_all(&session_json).await {
            let _ = child.kill().await;
            return Err(format!("Не удалось передать Microsoft-сессию Horizon: {error}"));
        }
        // Dropping stdin closes the stream. Horizon reads the JSON once and continues.
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Ошибка ожидания Horizon: {error}"))?;
    Ok(status.code())
}
