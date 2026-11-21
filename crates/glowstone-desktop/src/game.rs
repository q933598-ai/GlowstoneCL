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
use crust_core::authenticator::{Account as GameAccount, AccountType, Ownership};
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
use crate::store::{self, AccountKind, AccountProfile, SkinModel};
use glowstone_core::{GameInstance, GamePlatform, LoaderKind};

const HORIZON_NIGHTLY_API: &str =
    "https://api.github.com/repos/horizon-rust/horizon/releases/tags/nightly";
const HORIZON_OFFLINE_NIGHTLY_API: &str =
    "https://api.github.com/repos/gerdaroot/Horizon-Offline-Launcher/releases/tags/nightly";
const HORIZON_RELEASE_KEY: &str = "MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw=";
const HORIZON_OFFLINE_RELEASE_KEY: &str = "uzGcg2Sc1fAl26IpR2jEnhbmRhXTaiTyO9eWc2cpK1E=";
const HORIZON_SIGNATURE_DOMAIN: &str = "horizon release ";
const HORIZON_INSTALLED_MARKER: &str = ".installed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HorizonFlavor {
    Official,
    Offline,
}

impl HorizonFlavor {
    fn storage_name(self) -> &'static str {
        match self {
            Self::Official => "horizon",
            Self::Offline => "horizon-offline",
        }
    }

    fn release_api(self) -> &'static str {
        match self {
            Self::Official => "https://api.github.com/repos/horizon-rust/horizon/releases",
            Self::Offline => "https://api.github.com/repos/gerdaroot/Horizon-Offline-Launcher/releases",
        }
    }

    fn catalog_cache_name(self) -> &'static str {
        match self {
            Self::Official => "horizon-official.json",
            Self::Offline => "horizon-offline.json",
        }
    }

    fn release_key(self) -> &'static str {
        match self {
            Self::Official => HORIZON_RELEASE_KEY,
            Self::Offline => HORIZON_OFFLINE_RELEASE_KEY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionChoice {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
struct MojangVersionManifest {
    latest: MojangLatestVersions,
    versions: Vec<MojangVersionEntry>,
}

#[derive(Debug, Deserialize)]
struct MojangLatestVersions {
    release: String,
    snapshot: String,
}

#[derive(Debug, Deserialize)]
struct MojangVersionEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, rename = "releaseTime")]
    release_time: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthSkinVersion {
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<ModrinthSkinFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthSkinFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
    hashes: ModrinthSkinHashes,
}

#[derive(Debug, Deserialize)]
struct ModrinthSkinHashes {
    sha512: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LaunchOutcome {
    pub instance_id: String,
    pub platform: GamePlatform,
    pub exit_code: Option<i32>,
    pub notice: Option<String>,
}

/// Fetch the selectable game versions for the requested runtime. Lists are cached so
/// already-seen versions remain available when Mojang or GitHub is temporarily offline.
pub async fn available_versions(platform: GamePlatform) -> Result<Vec<VersionChoice>, String> {
    let root = store::app_data_dir()?.join("version-catalog");
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| format!("Не удалось создать кеш версий: {error}"))?;

    let client = reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)")
        .build()
        .map_err(|error| error.to_string())?;

    match platform {
        GamePlatform::Java => {
            let path = root.join("minecraft-java.json");
            let value = fetch_json_cached(
                &client,
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
                &path,
            )
            .await?;
            let manifest: MojangVersionManifest = serde_json::from_value(value)
                .map_err(|error| format!("Список Minecraft от Mojang повреждён: {error}"))?;

            let mut versions = vec![
                VersionChoice {
                    id: "stable".to_owned(),
                    label: format!("Последний релиз · {}", manifest.latest.release),
                    kind: "Последний релиз".to_owned(),
                },
                VersionChoice {
                    id: "snapshot".to_owned(),
                    label: format!("Последний snapshot · {}", manifest.latest.snapshot),
                    kind: "Последний snapshot".to_owned(),
                },
            ];
            for entry in manifest.versions {
                let kind = match entry.kind.as_str() {
                    "release" => "Релиз",
                    "snapshot" => "Snapshot",
                    "old_beta" => "Старая Beta",
                    "old_alpha" => "Старая Alpha / Classic",
                    _ => continue,
                };
                versions.push(VersionChoice {
                    id: entry.id.clone(),
                    label: if entry.release_time.is_empty() {
                        format!("{} · {}", entry.id, kind)
                    } else {
                        format!("{} · {} · {}", entry.id, kind, &entry.release_time[..10.min(entry.release_time.len())])
                    },
                    kind: kind.to_owned(),
                });
            }
            Ok(versions)
        }
        GamePlatform::Horizon => fetch_horizon_version_choices(&client, &root.join("horizon-official.json"), HorizonFlavor::Official).await,
        GamePlatform::HorizonOffline => fetch_horizon_version_choices(&client, &root.join("horizon-offline.json"), HorizonFlavor::Offline).await,
    }
}

async fn fetch_json_cached(
    client: &reqwest::Client,
    url: &str,
    cache_path: &Path,
) -> Result<Value, String> {
    let network_result = async {
        client
            .get(url)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())
    }
    .await;

    match network_result {
        Ok(value) => {
            if let Some(parent) = cache_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Ok(bytes) = serde_json::to_vec(&value) {
                let _ = tokio::fs::write(cache_path, bytes).await;
            }
            Ok(value)
        }
        Err(network_error) => {
            let bytes = tokio::fs::read(cache_path).await.map_err(|_| {
                format!("Не удалось получить список версий ({network_error}), локального кеша пока нет.")
            })?;
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("Кеш списка версий повреждён: {error}"))
        }
    }
}

async fn fetch_horizon_version_choices(
    client: &reqwest::Client,
    cache_path: &Path,
    flavor: HorizonFlavor,
) -> Result<Vec<VersionChoice>, String> {
    let releases = fetch_horizon_releases(client, cache_path, flavor).await?;
    let asset_name = horizon_asset_name()?;
    let mut choices = vec![VersionChoice {
        id: "stable".to_owned(),
        label: "Последняя сборка".to_owned(),
        kind: "Последняя сборка".to_owned(),
    }];
    for release in releases {
        if !release.assets.iter().any(|asset| asset.name == asset_name) {
            continue;
        }
        let Ok(id) = horizon_release_version(&release) else {
            continue;
        };
        let kind = if release.title.starts_with("Nightly ") {
            "Nightly"
        } else {
            "Стабильный релиз"
        };
        choices.push(VersionChoice {
            label: format!("{id} · {kind}"),
            id,
            kind: kind.to_owned(),
        });
    }
    Ok(choices)
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

    let active_offline_skin = if account.kind == AccountKind::Offline {
        store::active_skin_file(&account.uuid)?
    } else {
        None
    };

    if account.kind == AccountKind::Microsoft
        && !matches!(
            game_account.meta.ownership,
            Ownership::Owned | Ownership::GamePass
        )
    {
        return Err(
            "Не удалось подтвердить право запуска Minecraft Java Edition для этого Microsoft-аккаунта. Проверь лицензию или активную подписку Game Pass."
                .to_owned(),
        );
    }

    match instance.platform {
        GamePlatform::Java => {
            launch_java(instance, data_root, game_account, active_offline_skin).await
        }
        GamePlatform::Horizon => {
            if account.kind != AccountKind::Microsoft
                || game_account.kind() != AccountType::Xbox
            {
                return Err(
                    "Официальный Horizon требует Microsoft-аккаунт с доступом к Minecraft: Java Edition. Для локальной игры создай сборку Horizon Offline."
                        .to_owned(),
                );
            }
            launch_horizon(instance, data_root, instance_dir, game_account, HorizonFlavor::Official, None).await
        }
        GamePlatform::HorizonOffline => {
            if account.kind != AccountKind::Offline {
                return Err(
                    "Horizon Offline использует локальный offline-профиль. Выбери или создай offline-аккаунт."
                        .to_owned(),
                );
            }
            launch_horizon(instance, data_root, instance_dir, game_account, HorizonFlavor::Offline, active_offline_skin).await
        }
    }
}

/// Uploads a locally selected skin to the signed-in Microsoft Minecraft profile.
/// This changes the account's official skin, so the new texture is used by vanilla
/// clients and other devices after Minecraft's profile cache refreshes.
pub async fn apply_microsoft_skin(
    account: AccountProfile,
    microsoft_client_id: String,
    skin_path: PathBuf,
    model: SkinModel,
) -> Result<(), String> {
    if account.kind != AccountKind::Microsoft {
        return Err("Загрузка в Minecraft Services доступна только для Microsoft-аккаунта.".to_owned());
    }

    let bytes = tokio::fs::read(&skin_path)
        .await
        .map_err(|error| format!("Не удалось прочитать PNG скина: {error}"))?;
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err("PNG скина пустой или превышает 2 МБ.".to_owned());
    }

    let refresh_token = store::load_refresh_token(&account.id)?;
    let refreshed = auth::refresh_game_account(
        microsoft_client_id,
        &account,
        refresh_token,
    ).await?;

    if refreshed.kind() != AccountType::Xbox {
        return Err("Microsoft-сессия не содержит действующий Minecraft access token.".to_owned());
    }
    if let Some(rotated) = refreshed.refresh_token.as_deref() {
        store::store_refresh_token(&account.id, rotated)?;
    }

    let variant = match model {
        SkinModel::Classic => "classic",
        SkinModel::Slim => "slim",
    };
    let file = reqwest::multipart::Part::bytes(bytes)
        .file_name("glowstone-skin.png")
        .mime_str("image/png")
        .map_err(|error| format!("Не удалось подготовить PNG для загрузки: {error}"))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.to_owned())
        .part("file", file);

    let response = reqwest::Client::new()
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(&refreshed.access_token)
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("Не удалось отправить скин в Minecraft Services: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let detail: String = body.chars().take(260).collect();
        return Err(format!(
            "Minecraft Services отклонил скин (HTTP {status}). {}",
            if detail.trim().is_empty() { "Проверь лицензию, формат PNG и доступ к профилю." } else { detail.trim() }
        ));
    }

    Ok(())
}

/// Reads the currently active skin from an authenticated Microsoft profile.
pub async fn fetch_microsoft_skin(
    account: AccountProfile,
    microsoft_client_id: String,
) -> Result<Option<(Vec<u8>, SkinModel)>, String> {
    if account.kind != AccountKind::Microsoft {
        return Ok(None);
    }

    let refresh_token = store::load_refresh_token(&account.id)?;
    let refreshed = auth::refresh_game_account(
        microsoft_client_id,
        &account,
        refresh_token,
    ).await?;

    if refreshed.kind() != AccountType::Xbox {
        return Err("Microsoft-сессия не содержит действующий Minecraft access token.".to_owned());
    }
    if let Some(rotated) = refreshed.refresh_token.as_deref() {
        store::store_refresh_token(&account.id, rotated)?;
    }

    let client = reqwest::Client::new();
    let profile_response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&refreshed.access_token)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить скин Minecraft-профиля: {error}"))?;
    let status = profile_response.status();
    let profile: Value = profile_response
        .json()
        .await
        .map_err(|error| format!("Minecraft Services вернул некорректный профиль: {error}"))?;
    if !status.is_success() {
        return Err(format!("Не удалось прочитать Minecraft-профиль (HTTP {status})."));
    }

    let skins = profile["skins"].as_array().ok_or_else(|| {
        "Minecraft Services не вернул список скинов профиля.".to_owned()
    })?;
    let active = skins.iter()
        .find(|skin| skin["state"].as_str() == Some("ACTIVE"))
        .or_else(|| skins.first());
    let Some(active) = active else {
        return Ok(None);
    };
    let Some(url) = active["url"].as_str() else {
        return Ok(None);
    };
    let model = match active["variant"].as_str().unwrap_or("CLASSIC").to_ascii_uppercase().as_str() {
        "SLIM" => SkinModel::Slim,
        _ => SkinModel::Classic,
    };

    let skin_response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Не удалось скачать текстуру из Minecraft-профиля: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Сервер текстуры Minecraft вернул ошибку: {error}"))?;
    let bytes = skin_response
        .bytes()
        .await
        .map_err(|error| format!("Не удалось прочитать текстуру Minecraft: {error}"))?
        .to_vec();
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err("Minecraft Services вернул пустую или слишком большую текстуру.".to_owned());
    }

    Ok(Some((bytes, model)))
}

async fn resolve_java_skin_version(input: &str) -> Result<String, String> {
    let selector = java_version_selector(input);
    let alias = match selector.as_str() {
        "latest_release" => Some("stable"),
        "latest_snapshot" => Some("snapshot"),
        _ => None,
    };

    let Some(alias) = alias else {
        return Ok(input.trim().to_owned());
    };

    let choices = available_versions(GamePlatform::Java).await?;
    let choice = choices.iter()
        .find(|choice| choice.id == alias)
        .ok_or_else(|| "Не удалось определить последнюю версию Minecraft для установки OfflineSkins.".to_owned())?;
    choice.label
        .split('·')
        .nth(1)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Mojang не вернул точный номер версии Minecraft.".to_owned())
}

/// Installs the client-only OfflineSkins Fabric mod and the selected texture into
/// this isolated instance. Modrinth's version metadata is used for compatibility
/// matching and its SHA-512 is verified before the JAR is placed in the mods folder.
async fn install_offline_skins_mod(
    game_dir: &Path,
    game_version: &str,
    player_name: &str,
    skin_path: &Path,
    model: SkinModel,
) -> Result<(), String> {
    if !(3..=16).contains(&player_name.len())
        || !player_name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err("OfflineSkins требует игровой ник из 3–16 латинских букв, цифр или _.".to_owned());
    }

    let mut versions_url = reqwest::Url::parse(
        "https://api.modrinth.com/v2/project/dUHrybXD/version",
    ).map_err(|error| format!("Некорректный адрес каталога OfflineSkins: {error}"))?;
    versions_url.query_pairs_mut()
        .append_pair("loaders", r#"["fabric"]"#)
        .append_pair("game_versions", &serde_json::to_string(&[game_version])
            .map_err(|error| error.to_string())?);

    let client = reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)")
        .build()
        .map_err(|error| error.to_string())?;
    let versions = client
        .get(versions_url)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить совместимые версии OfflineSkins: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Modrinth не вернул список OfflineSkins: {error}"))?
        .json::<Vec<ModrinthSkinVersion>>()
        .await
        .map_err(|error| format!("Modrinth вернул некорректный список OfflineSkins: {error}"))?;

    let compatible = versions.into_iter().find(|version| {
        version.game_versions.iter().any(|supported| supported == game_version)
            && version.loaders.iter().any(|loader| loader == "fabric")
            && !version.files.is_empty()
    }).ok_or_else(|| format!(
        "Для Minecraft {game_version} пока нет подтверждённой версии OfflineSkins для Fabric. Поддерживаются версии, перечисленные в каталоге OfflineSkins на Modrinth."
    ))?;

    let file = compatible.files.iter()
        .find(|file| file.primary && file.filename.to_ascii_lowercase().ends_with(".jar"))
        .or_else(|| compatible.files.iter().find(|file| file.filename.to_ascii_lowercase().ends_with(".jar")))
        .ok_or_else(|| format!("В релизе OfflineSkins {} не найден JAR-файл.", compatible.version_number))?;

    let file_url = reqwest::Url::parse(&file.url)
        .map_err(|error| format!("Modrinth вернул некорректный URL OfflineSkins: {error}"))?;
    if file_url.scheme() != "https" || file_url.host_str() != Some("cdn.modrinth.com") {
        return Err("Загрузка OfflineSkins остановлена: Modrinth вернул недоверенный адрес файла.".to_owned());
    }
    if file.size == 0 || file.size > 20 * 1024 * 1024 {
        return Err("Размер JAR OfflineSkins выходит за допустимый предел 20 МБ.".to_owned());
    }

    let mods_dir = game_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|error| format!("Не удалось создать папку mods: {error}"))?;
    let safe_version: String = game_version.chars()
        .map(|character| if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
            character
        } else {
            '_'
        })
        .collect();
    let mod_path = mods_dir.join(format!("glowstone-offlineskins-{safe_version}.jar"));

    let need_download = match tokio::fs::read(&mod_path).await {
        Ok(existing) => {
            let expected = file.hashes.sha512.as_deref()
                .ok_or_else(|| "Modrinth не прислал SHA-512 для OfflineSkins.".to_owned())?;
            digest_hex(sha2::Sha512::digest(&existing).as_slice()) != expected
        }
        Err(_) => true,
    };

    if need_download {
        let bytes = client
            .get(file_url)
            .send()
            .await
            .map_err(|error| format!("Не удалось скачать OfflineSkins: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Ошибка загрузки OfflineSkins: {error}"))?
            .bytes()
            .await
            .map_err(|error| format!("Не удалось прочитать OfflineSkins: {error}"))?;
        if bytes.len() as u64 != file.size {
            return Err("Размер скачанного OfflineSkins не совпадает с метаданными Modrinth.".to_owned());
        }
        let expected = file.hashes.sha512.as_deref()
            .ok_or_else(|| "Modrinth не прислал SHA-512 для OfflineSkins.".to_owned())?;
        let actual = digest_hex(sha2::Sha512::digest(&bytes).as_slice());
        if actual != expected {
            return Err("SHA-512 OfflineSkins не совпадает с контрольной суммой Modrinth.".to_owned());
        }

        let temporary = mod_path.with_extension("jar.tmp");
        tokio::fs::write(&temporary, &bytes)
            .await
            .map_err(|error| format!("Не удалось сохранить OfflineSkins: {error}"))?;
        if mod_path.exists() {
            tokio::fs::remove_file(&mod_path)
                .await
                .map_err(|error| format!("Не удалось заменить OfflineSkins: {error}"))?;
        }
        tokio::fs::rename(&temporary, &mod_path)
            .await
            .map_err(|error| format!("Не удалось завершить установку OfflineSkins: {error}"))?;
    }

    let skins_dir = game_dir.join("config").join("offlineskins");
    tokio::fs::create_dir_all(&skins_dir)
        .await
        .map_err(|error| format!("Не удалось создать каталог config/offlineskins: {error}"))?;
    let skin_bytes = tokio::fs::read(skin_path)
        .await
        .map_err(|error| format!("Не удалось прочитать выбранный offline-скин: {error}"))?;
    // OfflineSkins resolves the file by the offline GameProfile name.
    // Preserve its exact casing instead of normalizing it to lowercase.
    tokio::fs::write(skins_dir.join(format!("{player_name}.png")), &skin_bytes)
        .await
        .map_err(|error| format!("Не удалось записать PNG в config/offlineskins: {error}"))?;
    let config = serde_json::json!({
        "selectedSkinName": player_name,
        "defaultModel": match model {
            SkinModel::Classic => "steve",
            SkinModel::Slim => "alex",
        }
    });
    tokio::fs::write(
        skins_dir.join("config.json"),
        serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(|error| format!("Не удалось сохранить настройки OfflineSkins: {error}"))?;

    Ok(())
}

async fn launch_java(
    instance: GameInstance,
    data_root: PathBuf,
    account: GameAccount,
    offline_skin: Option<(PathBuf, SkinModel)>,
) -> Result<LaunchOutcome, String> {
    let mut version = java_version_selector(&instance.game_version);
    let mut install_fabric_for_skin = false;
    let mut skin_notice = None;

    if let Some((skin_path, model)) = offline_skin.as_ref() {
        if matches!(instance.loader, LoaderKind::Vanilla | LoaderKind::Fabric) {
            match resolve_java_skin_version(&instance.game_version).await {
                Ok(resolved) => {
                    let game_dir = store::instance_directory(&instance.id.0)?;
                    match install_offline_skins_mod(&game_dir, &resolved, &account.name, skin_path, *model).await {
                        Ok(()) => {
                            version = resolved;
                            install_fabric_for_skin = instance.loader == LoaderKind::Vanilla;
                        }
                        Err(error) => {
                            skin_notice = Some(format!(
                                "Offline-скин не применён к Minecraft {resolved}: {error} Игра запущена без локального скина."
                            ));
                            eprintln!("[GlowstoneCL] {}", skin_notice.as_deref().unwrap_or_default());
                        }
                    }
                }
                Err(error) => {
                    skin_notice = Some(format!(
                        "Не удалось определить совместимую версию для OfflineSkins: {error} Игра запущена без локального скина."
                    ));
                    eprintln!("[GlowstoneCL] {}", skin_notice.as_deref().unwrap_or_default());
                }
            }
        } else {
            skin_notice = Some(format!(
                "Локальные скины Java сейчас подключаются через Fabric. Выбран загрузчик {:?}; игра запущена с ним без автоматического применения скина.",
                instance.loader
            ));
            eprintln!("[GlowstoneCL] {}", skin_notice.as_deref().unwrap_or_default());
        }
    }
    let mut options = JavaLaunchOptions::new(data_root, version);
    options.instance = Some(instance.id.0.clone());
    options.loader.path = format!("instances/{}/loader", instance.id.0);
    options.download_concurrency = 10;
    options.memory.max = "4G".to_owned();

    match instance.loader {
        LoaderKind::Vanilla => {
            if install_fabric_for_skin {
                options.loader.kind = Some(JavaLoaderKind::Fabric);
                options.loader.enable = true;
            }
        }
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
        notice: skin_notice,
    })
}

fn validate_horizon_build_pin(
    flavor: HorizonFlavor,
    minecraft_version: &str,
    java_major: u32,
) -> Result<(), String> {
    if flavor == HorizonFlavor::Offline
        && (minecraft_version != "26.3" || java_major != 25)
    {
        return Err(format!(
            "Horizon Offline поддерживает Minecraft 26.3 и Java 25. Эта сборка закрепляет Minecraft {minecraft_version} / Java {java_major}."
        ));
    }
    Ok(())
}

fn java_version_selector(input: &str) -> String {
    match input.trim().to_ascii_lowercase().as_str() {
        "stable" | "latest" | "release" | "latest release" => "latest_release".to_owned(),
        "snapshot" | "latest snapshot" => "latest_snapshot".to_owned(),
        _ => input.trim().to_owned(),
    }
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
    flavor: HorizonFlavor,
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
    flavor: HorizonFlavor,
    offline_skin: Option<(PathBuf, SkinModel)>,
) -> Result<LaunchOutcome, String> {
    let platform = instance.platform;
    let client = reqwest::Client::builder()
        .user_agent("GlowstoneCL/0.1.0 (https://github.com/q933598-ai/GlowstoneCL)")
        .build()
        .map_err(|error| error.to_string())?;

    let build = ensure_horizon_build(
        &client,
        &data_root,
        flavor,
        &instance.game_version,
    )
    .await?;
    let pins = build.pins()?;
    validate_horizon_build_pin(flavor, &pins.version, pins.java_major)?;

    let client_api = query_launch_api(&build.client(), &build.directory).await?;
    let client_version = client_api["minecraft"]
        .as_str()
        .ok_or_else(|| "Horizon client не сообщил версию Minecraft.".to_owned())?;
    if client_version != pins.version {
        return Err(format!(
            "Horizon client собран для Minecraft {client_version}, но research-lock.json закрепляет {}.",
            pins.version
        ));
    }

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

    let horizon_ready = is_horizon_prepared(&build).await;
    if !horizon_ready {
        match flavor {
            HorizonFlavor::Official => {
                // Official Horizon uses CrustCore to install Mojang files for the signed-in owner.
                let mut java_options = JavaLaunchOptions::new(data_root.clone(), pins.version.clone());
                java_options.download_concurrency = 10;
                java_options.memory.max = "4G".to_owned();
                let java_launch = JavaLaunch::new(java_options, account.clone())
                    .map_err(|error| format!("Не удалось создать установщик Minecraft для Horizon: {error}"))?;
                let prepared_java = java_launch
                    .prepare()
                    .await
                    .map_err(|error| format!("Не удалось загрузить файлы Minecraft для Horizon: {error}"))?;

                let version_id = prepared_java.version.id.clone();
                let version_json_path = data_root
                    .join("versions")
                    .join(&version_id)
                    .join(format!("{version_id}.json"));
                let version_bytes = tokio::fs::read(&version_json_path)
                    .await
                    .map_err(|error| format!("Не найден JSON версии Minecraft: {error}"))?;
                let version_json: Value = serde_json::from_slice(&version_bytes)
                    .map_err(|error| format!("JSON версии Minecraft повреждён: {error}"))?;
                let server_jar = ensure_server_jar(&client, &version_json, &data_root, &version_id).await?;

                let java_home = prepared_java
                    .java
                    .parent()
                    .and_then(Path::parent)
                    .ok_or_else(|| "Не удалось определить каталог Java Runtime.".to_owned())?
                    .to_path_buf();
                prepare_horizon(&build, Some(&data_root), None, Some(&server_jar), Some(&java_home)).await?;
            }
            HorizonFlavor::Offline => {
                let source = find_offline_minecraft_source(&pins.version)?;
                let (server_jar, minecraft_dir, prism_dir) = if source.prism {
                    (None, None, Some(source.directory.as_path()))
                } else {
                    let version_json_path = source.directory
                        .join("versions")
                        .join(&pins.version)
                        .join(format!("{}.json", pins.version));
                    let version_bytes = tokio::fs::read(&version_json_path)
                        .await
                        .map_err(|error| format!("Не найден локальный JSON Minecraft {}: {error}", pins.version))?;
                    let version_json: Value = serde_json::from_slice(&version_bytes)
                        .map_err(|error| format!("Локальный JSON Minecraft повреждён: {error}"))?;
                    let local_client = source.directory
                        .join("versions")
                        .join(&pins.version)
                        .join(format!("{}.jar", pins.version));
                    if !local_client.is_file() {
                        return Err(format!("Не найден локальный client jar Minecraft {}. Запусти эту версию в своём Java-лаунчере один раз.", pins.version));
                    }
                    let server_jar = ensure_server_jar(
                        &client,
                        &version_json,
                        &data_root.join("horizon-offline"),
                        &pins.version,
                    ).await?;
                    (Some(server_jar), Some(source.directory.as_path()), None)
                };
                prepare_horizon(&build, minecraft_dir, prism_dir, server_jar.as_deref(), None).await?;
            }
        }
        if !is_horizon_prepared(&build).await {
            return Err("Horizon preparer завершился, но проверка ресурсов не прошла.".to_owned());
        }
    }

    fs::create_dir_all(&instance_dir)
        .map_err(|error| format!("Не удалось создать папку мира Horizon: {error}"))?;
    if flavor == HorizonFlavor::Offline {
        fs::write(instance_dir.join("horizon-player.txt"), format!("name:{}\n", account.name))
            .map_err(|error| format!("Не удалось сохранить offline-ник Horizon: {error}"))?;
    }
    launch_horizon_client(&build, &instance_dir, &account, offline_skin).await
        .map(|exit_code| LaunchOutcome {
            instance_id: instance.id.0,
            platform,
            exit_code,
            notice: None,
        })
}

async fn ensure_horizon_build(
    client: &reqwest::Client,
    data_root: &Path,
    flavor: HorizonFlavor,
    requested_version: &str,
) -> Result<HorizonBuild, String> {
    let versions_dir = data_root.join(flavor.storage_name()).join("versions");
    fs::create_dir_all(&versions_dir)
        .map_err(|error| format!("Не удалось создать каталог Horizon: {error}"))?;

    let asset_name = horizon_asset_name()?;
    let cache_path = data_root.join("version-catalog").join(flavor.catalog_cache_name());
    let releases = match fetch_horizon_releases(client, &cache_path, flavor).await {
        Ok(releases) => releases,
        Err(error) => {
            if let Some(existing) = installed_horizon_build(&versions_dir, flavor, requested_version) {
                return Ok(existing);
            }
            return Err(format!("Не удалось получить список релизов Horizon: {error}"));
        }
    };
    let release = select_horizon_release(&releases, requested_version, asset_name)?;
    let version = horizon_release_version(release)?;
    let target = versions_dir.join(&version);
    if target.join(HORIZON_INSTALLED_MARKER).is_file() {
        let candidate = HorizonBuild {
            version: version.clone(),
            directory: target.clone(),
            flavor,
        };
        if candidate.client().is_file()
            && candidate.preparer().is_file()
            && candidate.pins().is_ok()
        {
            return Ok(candidate);
        }
        // A marker alone is not proof of a complete install. Re-download and
        // replace a damaged folder instead of failing later during launch.
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
    let (signature_asset, signature_context) = if release.title.starts_with("Nightly ") {
        ("SHA256SUMS.v2.sig", version.as_str())
    } else {
        ("SHA256SUMS.sig", release.tag_name.as_str())
    };
    let signature_url = find_asset(signature_asset)?;
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
    verify_horizon_signature(signature_context, &sums, &signature, flavor)?;

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
    let actual_sha256 = digest_hex(sha2::Sha256::digest(&archive));
    if actual_sha256 != expected_sha256 {
        return Err("SHA-256 архива Horizon не совпадает с подписанным SHA256SUMS.".to_owned());
    }

    if !valid_horizon_version_id(&version) {
        return Err("GitHub вернул некорректное имя версии Horizon.".to_owned());
    }

    let archive_path = horizon_archive_cache_path(&versions_dir, &version, asset_name)?;
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
    HorizonBuild { version: version.clone(), directory: stage.clone(), flavor }
        .pins()?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    }
    fs::rename(&stage, &target)
        .map_err(|error| format!("Не удалось завершить установку Horizon: {error}"))?;

    Ok(HorizonBuild { version, directory: target, flavor })
}

fn installed_horizon_build(
    versions_dir: &Path,
    flavor: HorizonFlavor,
    requested_version: &str,
) -> Option<HorizonBuild> {
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
        if !is_latest_alias(requested_version) && version != requested_version {
            return None;
        }
        let build = HorizonBuild { version, directory, flavor };
        (build.client().is_file() && build.preparer().is_file() && build.pins().is_ok())
            .then_some(build)
    })
}

fn is_latest_alias(version: &str) -> bool {
    matches!(version.trim().to_ascii_lowercase().as_str(), "" | "stable" | "latest" | "horizon")
}

async fn fetch_horizon_releases(
    client: &reqwest::Client,
    cache_path: &Path,
    flavor: HorizonFlavor,
) -> Result<Vec<GithubRelease>, String> {
    let mut releases = Vec::new();
    let cache_stem = cache_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(flavor.storage_name());

    // GitHub paginates release history. Fetch pages until the last page so the
    // version picker can include the earliest game tags, not just recent Nightlies.
    for page in 1..=50_u32 {
        let page_url = format!("{}?per_page=100&page={page}", flavor.release_api());
        let page_cache = cache_path.with_file_name(format!("{cache_stem}-page-{page}.json"));
        let value = match fetch_json_cached(client, &page_url, &page_cache).await {
            Ok(value) => value,
            Err(error) if page > 1 && !releases.is_empty() => {
                eprintln!("[GlowstoneCL] Horizon release history stopped at page {}: {error}", page - 1);
                break;
            }
            Err(error) => return Err(error),
        };
        let batch: Vec<GithubRelease> = serde_json::from_value(value)
            .map_err(|error| format!("Список релизов Horizon повреждён: {error}"))?;
        let count = batch.len();
        releases.extend(batch);
        if count < 100 {
            break;
        }
    }

    if releases.is_empty() {
        return Err("GitHub не вернул ни одного релиза Horizon.".to_owned());
    }
    Ok(releases)
}

fn select_horizon_release<'a>(
    releases: &'a [GithubRelease],
    requested_version: &str,
    asset_name: &str,
) -> Result<&'a GithubRelease, String> {
    let eligible = releases.iter().filter(|release| {
        release.assets.iter().any(|asset| asset.name == asset_name)
            && horizon_release_version(release).is_ok()
    });
    if is_latest_alias(requested_version) {
        return eligible.into_iter().next().ok_or_else(|| {
            format!("GitHub не опубликовал сборку Horizon для этой платформы ({asset_name}).")
        });
    }
    eligible
        .into_iter()
        .find(|release| horizon_release_version(release).is_ok_and(|id| id == requested_version))
        .ok_or_else(|| format!("В доступных релизах Horizon нет версии {requested_version}."))
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
    let commit = commit.to_ascii_lowercase();
    Ok(format!("nightly-{date}-{commit}"))
}

fn horizon_release_version(release: &GithubRelease) -> Result<String, String> {
    if release.title.starts_with("Nightly ") {
        return nightly_version(release);
    }
    let tag = release.tag_name.trim();
    if !valid_horizon_version_id(tag) {
        return Err("GitHub вернул некорректный tag Horizon.".to_owned());
    }
    Ok(tag.to_owned())
}

fn valid_horizon_version_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'@')
        })
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

fn verify_horizon_signature(
    tag: &str,
    sums: &[u8],
    signatures: &str,
    flavor: HorizonFlavor,
) -> Result<(), String> {
    let mut message = format!("{HORIZON_SIGNATURE_DOMAIN}{tag}\n").into_bytes();
    message.extend_from_slice(sums);
    let key = base64::engine::general_purpose::STANDARD
        .decode(flavor.release_key())
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

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut result = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        result.push_str(&format!("{byte:02x}"));
    }
    result
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

fn horizon_archive_cache_path(
    versions_dir: &Path,
    version: &str,
    asset_name: &str,
) -> Result<PathBuf, String> {
    let suffix = if asset_name.ends_with(".tar.gz") {
        ".tar.gz"
    } else if asset_name.ends_with(".zip") {
        ".zip"
    } else {
        return Err(format!("Неизвестный формат архива Horizon: {asset_name}"));
    };
    Ok(versions_dir.join(format!(".{version}{suffix}")))
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

#[derive(Debug, Clone)]
struct OfflineMinecraftSource {
    directory: PathBuf,
    prism: bool,
}

/// Find an existing copy of the pinned Minecraft version. Horizon Offline never downloads the
/// client jar or libraries for an offline account; it imports files from the player's launcher.
fn find_offline_minecraft_source(version: &str) -> Result<OfflineMinecraftSource, String> {
    let base = directories::BaseDirs::new()
        .ok_or_else(|| "Не удалось определить домашний каталог пользователя.".to_owned())?;
    let home = base.home_dir();
    let mut candidates: Vec<(PathBuf, bool)> = Vec::new();

    if let Some(path) = std::env::var_os("GLOWSTONE_MINECRAFT_DIR") {
        let path = PathBuf::from(path);
        let prism = path.join("instances").is_dir()
            || path.join("mmc-pack.json").is_file()
            || path.join("prismlauncher.cfg").is_file();
        candidates.push((path, prism));
    }

    #[cfg(target_os = "linux")]
    {
        candidates.extend([
            (home.join(".minecraft"), false),
            (base.data_local_dir().join("PrismLauncher"), true),
            (home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"), true),
            (base.data_local_dir().join("multimc"), true),
        ]);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let appdata = PathBuf::from(appdata);
            candidates.push((appdata.join(".minecraft"), false));
            candidates.push((appdata.join("PrismLauncher"), true));
            candidates.push((appdata.join("MultiMC"), true));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let support = home.join("Library/Application Support");
        candidates.push((support.join("minecraft"), false));
        candidates.push((support.join("PrismLauncher"), true));
        candidates.push((support.join("MultiMC"), true));
    }

    for (directory, prism) in candidates {
        if !directory.is_dir() {
            continue;
        }
        if prism {
            // Prism/MultiMC share assets and libraries in the launcher root. The preparer
            // resolves the instance whose mmc-pack.json selects the pinned Minecraft version.
            if directory.join("assets").is_dir()
                && directory.join("libraries").is_dir()
                && (directory.join("instances").is_dir()
                    || directory.join("mmc-pack.json").is_file()
                    || directory.join("meta/net.minecraft").is_dir())
            {
                return Ok(OfflineMinecraftSource { directory, prism: true });
            }
        } else {
            let version_dir = directory.join("versions").join(version);
            if version_dir.join(format!("{}.jar", version)).is_file()
                && version_dir.join(format!("{}.json", version)).is_file()
                && directory.join("assets").is_dir()
                && directory.join("libraries").is_dir()
            {
                return Ok(OfflineMinecraftSource { directory, prism: false });
            }
        }
    }

    Err(format!(
        "Horizon Offline не нашёл локальные файлы Minecraft {}. Установи и запусти эту версию в официальном лаунчере или Prism Launcher. GlowstoneCL не скачивает клиент Minecraft для offline-профиля. Можно указать путь через GLOWSTONE_MINECRAFT_DIR.",
        version
    ))
}

async fn prepare_horizon(
    build: &HorizonBuild,
    minecraft_dir: Option<&Path>,
    prism_dir: Option<&Path>,
    server_jar: Option<&Path>,
    java_home: Option<&Path>,
) -> Result<(), String> {
    let api = query_launch_api(&build.preparer(), &build.directory).await?;
    let options = advertised_options(&api)?;
    for required in ["--prepare", "--output"] {
        if !options.contains(required) {
            return Err(format!("Этот Horizon preparer не поддерживает обязательную опцию {required}."));
        }
    }
    if minecraft_dir.is_none() && prism_dir.is_none() {
        return Err("Не указан источник локальных файлов Minecraft для подготовки Horizon.".to_owned());
    }
    if minecraft_dir.is_some() && !options.contains("--minecraft-dir") {
        return Err("Этот Horizon preparer не поддерживает --minecraft-dir.".to_owned());
    }
    if prism_dir.is_some() && !options.contains("--prism-dir") {
        return Err("Этот Horizon preparer не поддерживает --prism-dir. Обнови offline runtime.".to_owned());
    }
    if server_jar.is_some() && !options.contains("--server-jar") {
        return Err("Этот Horizon preparer не поддерживает --server-jar.".to_owned());
    }

    let mut command = TokioCommand::new(build.preparer());
    command
        .current_dir(&build.directory)
        .arg("--prepare")
        .arg("--output")
        .arg(build.prepared())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(minecraft_dir) = minecraft_dir {
        command.arg("--minecraft-dir").arg(minecraft_dir);
    }
    if let Some(prism_dir) = prism_dir {
        command.arg("--prism-dir").arg(prism_dir);
    }
    if let Some(server_jar) = server_jar {
        command.arg("--server-jar").arg(server_jar);
    }

    if options.contains("--complete") {
        command.arg("--complete");
    }
    if let Some(java_home) = java_home {
        if options.contains("--java-home") {
            command.arg("--java-home").arg(java_home);
        }
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
        if digest_hex(sha1::Sha1::digest(&existing)) == expected_sha1 {
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
    if digest_hex(sha1::Sha1::digest(&bytes)) != expected_sha1 {
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
    offline_skin: Option<(PathBuf, SkinModel)>,
) -> Result<Option<i32>, String> {
    let api = query_launch_api(&build.client(), &build.directory).await?;
    let options = advertised_options(&api)?;
    if !options.contains("--resources") {
        return Err("Эта сборка Horizon не поддерживает --resources.".to_owned());
    }
    if build.flavor == HorizonFlavor::Official && !options.contains("--session-stdin") {
        return Err(
            "Официальная сборка Horizon не поддерживает безопасную передачу Microsoft-сессии через stdin."
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

    let session_json = if build.flavor == HorizonFlavor::Official {
        args.push("--session-stdin".to_owned());
        let xuid = account.xuid();
        let session = HorizonSession {
            name: &account.name,
            uuid: account.uuid.replace('-', ""),
            access_token: &account.access_token,
            user_type: "msa",
            xuid,
        };
        Some(serde_json::to_vec(&session)
            .map_err(|error| format!("Не удалось сериализовать игровую сессию: {error}"))?)
    } else {
        None
    };

    let mut command = TokioCommand::new(build.client());
    command
        .args(&args)
        .current_dir(game_dir)
        .stdin(if session_json.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if build.flavor == HorizonFlavor::Offline {
        if let Some((skin_path, model)) = offline_skin {
            command
                .env("HORIZON_OFFLINE_SKIN", skin_path)
                .env(
                    "HORIZON_OFFLINE_SKIN_MODEL",
                    match model {
                        SkinModel::Classic => "classic",
                        SkinModel::Slim => "slim",
                    },
                );
        }
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Не удалось запустить horizon-client: {error}"))?;
    if let Some(session_json) = session_json {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_hex_encodes_sha_bytes_as_lowercase_hex() {
        assert_eq!(digest_hex([0x00_u8, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn maps_java_release_aliases_to_crust_core_selectors() {
        assert_eq!(java_version_selector("stable"), "latest_release");
        assert_eq!(java_version_selector("latest"), "latest_release");
        assert_eq!(java_version_selector("snapshot"), "latest_snapshot");
        assert_eq!(java_version_selector("1.21.1"), "1.21.1");
    }

    #[test]
    fn offline_fork_is_pinned_to_minecraft_26_3_and_java_25() {
        assert!(validate_horizon_build_pin(HorizonFlavor::Offline, "26.3", 25).is_ok());
        assert!(validate_horizon_build_pin(HorizonFlavor::Offline, "1.21.1", 25).is_err());
        assert!(validate_horizon_build_pin(HorizonFlavor::Offline, "26.3", 21).is_err());
        assert!(validate_horizon_build_pin(HorizonFlavor::Official, "26.3", 25).is_ok());
    }

    #[test]
    fn horizon_version_paths_accept_semver_tags_with_dots() {
        assert!(valid_horizon_version_id("v0.2.97.1"));
        assert!(valid_horizon_version_id("nightly-2026-11-21-8f83e5c"));
        assert!(!valid_horizon_version_id("../v0.2.97.1"));
        assert!(!valid_horizon_version_id("v0.2.97.1/../../outside"));
    }

    #[test]
    fn horizon_release_ids_distinguish_nightly_and_tagged_builds() {
        let nightly: GithubRelease = serde_json::from_str(
            r#"{"name":"Nightly 2026-11-21 · 8f83e5c","tag_name":"nightly","assets":[]}"#,
        ).unwrap();
        assert_eq!(
            horizon_release_version(&nightly).unwrap(),
            "nightly-2026-11-21-8f83e5c"
        );

        let tagged: GithubRelease = serde_json::from_str(
            r#"{"name":"Horizon v0.2.97.1","tag_name":"v0.2.97.1","assets":[]}"#,
        ).unwrap();
        assert_eq!(horizon_release_version(&tagged).unwrap(), "v0.2.97.1");
    }

    #[test]
    fn horizon_release_selection_skips_launcher_and_mod_sdk_releases() {
        let releases: Vec<GithubRelease> = serde_json::from_str(
            r#"[
                {"name":"Horizon launcher 0.2.96","tag_name":"launcher@0.2.96","assets":[{"name":"horizon-launcher-linux-x86_64.tar.gz","browser_download_url":"https://example.invalid/launcher"}]},
                {"name":"Mod SDK 8f83e5c85a","tag_name":"sdk-8f83e5c","assets":[{"name":"sdk.json","browser_download_url":"https://example.invalid/sdk"}]},
                {"name":"Nightly 2026-11-21 · 8f83e5c","tag_name":"nightly","assets":[{"name":"horizon-linux-x86_64.tar.gz","browser_download_url":"https://example.invalid/game"}]}
            ]"#,
        ).unwrap();

        assert_eq!(
            horizon_release_version(select_horizon_release(
                &releases,
                "stable",
                "horizon-linux-x86_64.tar.gz"
            ).unwrap()).unwrap(),
            "nightly-2026-11-21-8f83e5c"
        );
        assert!(select_horizon_release(
            &releases,
            "launcher@0.2.96",
            "horizon-linux-x86_64.tar.gz"
        ).is_err());
    }

    #[test]
    fn parses_the_github_release_name_as_the_nightly_title() {
        let release: GithubRelease = serde_json::from_str(
            r#"{
                "name": "Nightly 2026-11-21 · 8f83e5c",
                "tag_name": "nightly",
                "assets": []
            }"#,
        )
        .unwrap();

        assert_eq!(
            nightly_version(&release).unwrap(),
            "nightly-2026-11-21-8f83e5c"
        );
    }

    #[test]
    fn nightly_identity_rejects_untrusted_title_parts() {
        let release: GithubRelease = serde_json::from_str(
            r#"{"name":"Nightly 2026-11-21 · ../../x","tag_name":"nightly","assets":[]}"#,
        )
        .unwrap();

        assert!(nightly_version(&release).is_err());
    }

    #[test]
    fn horizon_archive_cache_path_keeps_the_archive_format() {
        let root = Path::new("/tmp/glowstone-horizon");

        assert_eq!(
            horizon_archive_cache_path(root, "nightly-2026-11-21-8f83e5c", "horizon-linux-x86_64.tar.gz").unwrap(),
            root.join(".nightly-2026-11-21-8f83e5c.tar.gz")
        );
        assert_eq!(
            horizon_archive_cache_path(root, "nightly-2026-11-21-8f83e5c", "horizon-windows-x64.zip").unwrap(),
            root.join(".nightly-2026-11-21-8f83e5c.zip")
        );
        assert!(horizon_archive_cache_path(root, "x", "unknown.bin").is_err());
    }
}
