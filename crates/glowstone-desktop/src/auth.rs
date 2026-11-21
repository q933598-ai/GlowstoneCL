use serde::Deserialize;
use serde_json::{Value, json};

use crust_core::authenticator::{
    Account as GameAccount, AccountMeta, AccountProfile as GameAccountProfile,
    AccountType, Authenticator, Ownership,
};

use crate::store::{AccountKind, AccountProfile};

const DEVICE_CODE_ENDPOINT: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_ENDPOINT: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Clone)]
pub struct DeviceCodePrompt {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Clone)]
pub enum PollResult {
    SignedIn {
        account: AccountProfile,
        refresh_token: String,
    },
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    message: String,
    #[serde(default = "default_interval")]
    interval: u64,
    #[serde(default = "default_expires_in")]
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn default_interval() -> u64 {
    5
}

fn default_expires_in() -> u64 {
    900
}

pub async fn request_device_code(client_id: String) -> Result<DeviceCodePrompt, String> {
    if client_id.trim().is_empty() {
        return Err("Сначала укажи Microsoft Client ID в разделе «Аккаунты».".to_owned());
    }

    let response = reqwest::Client::new()
        .post(DEVICE_CODE_ENDPOINT)
        .form(&[
            ("client_id", client_id.trim()),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await
        .map_err(|error| format!("Не удалось связаться с Microsoft: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Microsoft вернул HTTP {status}: {body}"));
    }

    let device: DeviceCodeResponse = response
        .json()
        .await
        .map_err(|error| format!("Microsoft вернул некорректный ответ: {error}"))?;

    Ok(DeviceCodePrompt {
        device_code: device.device_code,
        user_code: device.user_code,
        verification_uri: device.verification_uri,
        message: if device.message.is_empty() {
            "Открой страницу входа Microsoft и введи показанный код.".to_owned()
        } else {
            device.message
        },
        interval: device.interval.max(5),
        expires_in: device.expires_in.max(60),
    })
}

pub async fn poll_device_code(
    client_id: String,
    device_code: String,
    interval: u64,
    expires_in: u64,
) -> Result<PollResult, String> {
    // Keep polling while the user completes browser sign-in. Microsoft explicitly
    // requires retrying authorization_pending after the supplied interval.
    let client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(expires_in.max(60));
    let mut polling_interval = interval.max(5);

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err("Код Microsoft истёк. Начни вход заново.".to_owned());
        }

        tokio::time::sleep(std::time::Duration::from_secs(polling_interval)).await;

        if tokio::time::Instant::now() >= deadline {
            return Err("Код Microsoft истёк. Начни вход заново.".to_owned());
        }

        let response = client
            .post(TOKEN_ENDPOINT)
            .form(&[
                ("client_id", client_id.trim()),
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code",
                ),
                ("device_code", device_code.as_str()),
            ])
            .send()
            .await
            .map_err(|error| format!("Не удалось проверить вход Microsoft: {error}"))?;

        let status = response.status();
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("Не удалось прочитать ответ Microsoft: {error}"))?;

        if !status.is_success() {
            let code = body["error"].as_str().unwrap_or_default();
            let description = body["error_description"]
                .as_str()
                .unwrap_or("Microsoft не завершил авторизацию.");

            match code {
                "authorization_pending" => continue,
                "slow_down" => {
                    polling_interval = polling_interval.saturating_add(5);
                    continue;
                }
                "authorization_declined" => {
                    return Err("Вход Microsoft был отклонён.".to_owned());
                }
                "expired_token" => {
                    return Err("Код Microsoft истёк. Начни вход заново.".to_owned());
                }
                _ => return Err(format!("Microsoft OAuth: {description}")),
            }
        }

        let tokens: TokenResponse = serde_json::from_value(body)
            .map_err(|error| format!("Не удалось разобрать токены Microsoft: {error}"))?;

        if let Some(error) = tokens.error {
            return Err(tokens
                .error_description
                .unwrap_or_else(|| format!("Microsoft OAuth: {error}")));
        }

        let microsoft_access_token = tokens
            .access_token
            .ok_or_else(|| "Microsoft не выдал access token.".to_owned())?;
        let refresh_token = tokens
            .refresh_token
            .ok_or_else(|| "Microsoft не выдал refresh token. Проверь scope offline_access.".to_owned())?;

        let account = authenticate_minecraft(&microsoft_access_token).await?;

        return Ok(PollResult::SignedIn {
            account,
            refresh_token,
        });
    }
}

async fn authenticate_minecraft(microsoft_access_token: &str) -> Result<AccountProfile, String> {
    let client = reqwest::Client::new();

    let xbox_response = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={microsoft_access_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|error| format!("Не удалось выполнить вход Xbox Live: {error}"))?;

    let xbox_status = xbox_response.status();
    let xbox_body = xbox_response
        .json::<Value>()
        .await
        .map_err(|error| format!("Некорректный ответ Xbox Live: {error}"))?;

    if !xbox_status.is_success() {
        return Err(format!(
            "Xbox Live не принял аккаунт (HTTP {xbox_status}): {}",
            xbox_body["Message"].as_str().unwrap_or("проверь регион и настройки Xbox")
        ));
    }

    let xbox_token = xbox_body["Token"]
        .as_str()
        .ok_or_else(|| "Xbox Live не вернул токен.".to_owned())?;
    let user_hash = xbox_body["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| "Xbox Live не вернул user hash.".to_owned())?;

    let xsts_response = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|error| format!("Не удалось выполнить XSTS-проверку: {error}"))?;

    let xsts_status = xsts_response.status();
    let xsts_body = xsts_response
        .json::<Value>()
        .await
        .map_err(|error| format!("Некорректный ответ XSTS: {error}"))?;

    if !xsts_status.is_success() {
        let xsts_error = xsts_body["XErr"]
            .as_u64()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "неизвестная ошибка".to_owned());
        return Err(format!(
            "Xbox XSTS отклонил аккаунт (код {xsts_error}). Проверь возрастные ограничения и настройки Xbox."
        ));
    }

    let xsts_token = xsts_body["Token"]
        .as_str()
        .ok_or_else(|| "Xbox XSTS не вернул токен.".to_owned())?;

    let minecraft_response = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({
            "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}")
        }))
        .send()
        .await
        .map_err(|error| format!("Не удалось войти в Minecraft Services: {error}"))?;

    let minecraft_status = minecraft_response.status();
    let minecraft_body = minecraft_response
        .json::<Value>()
        .await
        .map_err(|error| format!("Некорректный ответ Minecraft Services: {error}"))?;

    if !minecraft_status.is_success() {
        return Err(format!(
            "Minecraft Services не принял аккаунт (HTTP {minecraft_status}): {}",
            minecraft_body["errorMessage"]
                .as_str()
                .unwrap_or("проверь доступ к Minecraft Java Edition")
        ));
    }

    let minecraft_access_token = minecraft_body["access_token"]
        .as_str()
        .ok_or_else(|| "Minecraft Services не вернул access token.".to_owned())?;

    let entitlement_response = client
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .bearer_auth(minecraft_access_token)
        .send()
        .await
        .map_err(|error| format!("Не удалось проверить лицензию Minecraft: {error}"))?;

    let entitlement_status = entitlement_response.status();
    let entitlement_body = entitlement_response
        .json::<Value>()
        .await
        .map_err(|error| format!("Некорректный ответ о лицензии Minecraft: {error}"))?;

    if !entitlement_status.is_success() {
        return Err(format!(
            "Minecraft Services не позволил проверить лицензию (HTTP {entitlement_status}). Проверь настройки доступа приложения Microsoft."
        ));
    }

    let owns_java_edition = entitlement_body["items"]
        .as_array()
        .is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item["name"].as_str(),
                    Some("game_minecraft" | "product_minecraft")
                )
            })
        });

    if !owns_java_edition {
        return Err(
            "Microsoft-аккаунт вошёл успешно, но Minecraft Java Edition не найдена среди лицензий."
                .to_owned(),
        );
    }

    let profile_response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(minecraft_access_token)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить Minecraft-профиль: {error}"))?;

    let profile_status = profile_response.status();
    let profile_body = profile_response
        .json::<Value>()
        .await
        .map_err(|error| format!("Некорректный Minecraft-профиль: {error}"))?;

    if !profile_status.is_success() {
        return Err(
            "Minecraft-профиль не найден. У этого Microsoft-аккаунта может не быть купленной Java Edition или созданного профиля."
                .to_owned(),
        );
    }

    let uuid = profile_body["id"]
        .as_str()
        .ok_or_else(|| "Minecraft не вернул UUID профиля.".to_owned())?;
    let name = profile_body["name"]
        .as_str()
        .ok_or_else(|| "Minecraft не вернул имя профиля.".to_owned())?;

    let canonical_uuid = uuid::Uuid::parse_str(uuid)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| uuid.to_owned());

    Ok(AccountProfile {
        id: format!("microsoft:{canonical_uuid}"),
        name: name.to_owned(),
        uuid: canonical_uuid,
        kind: AccountKind::Microsoft,
    })
}


/// Creates a CrustCore account from an existing offline profile.
pub fn offline_game_account(profile: &AccountProfile) -> GameAccount {
    GameAccount {
        access_token: profile.uuid.replace('-', ""),
        client_token: profile.uuid.replace('-', ""),
        uuid: profile.uuid.replace('-', ""),
        name: profile.name.clone(),
        refresh_token: None,
        user_properties: "{}".to_owned(),
        meta: AccountMeta::offline(AccountType::Mojang, false),
        xbox_account: None,
        profile: GameAccountProfile::default(),
        client_id: None,
        user_info: None,
    }
}

/// Refreshes a Microsoft session for launching a game. Only the refresh token is loaded
/// from the OS credential store; the short-lived Minecraft access token stays in memory.
pub async fn refresh_game_account(
    client_id: String,
    profile: &AccountProfile,
    refresh_token: String,
) -> Result<GameAccount, String> {
    let client_id = client_id.trim().to_owned();
    if client_id.is_empty() {
        return Err("Сначала укажи Microsoft Client ID в разделе «Аккаунты».".to_owned());
    }

    let authenticator = Authenticator::with_client_id(client_id)
        .map_err(|error| format!("Не удалось инициализировать Microsoft: {error}"))?;

    // An empty, expired access token forces CrustCore to exchange the stored
    // Microsoft refresh token for a fresh Xbox/XSTS/Minecraft session.
    let stored = GameAccount {
        access_token: String::new(),
        client_token: uuid::Uuid::new_v4().to_string(),
        uuid: profile.uuid.replace('-', ""),
        name: profile.name.clone(),
        refresh_token: Some(refresh_token),
        user_properties: "{}".to_owned(),
        meta: AccountMeta::xbox(0, Ownership::Unknown, Vec::new()),
        xbox_account: None,
        profile: GameAccountProfile::default(),
        client_id: Some(client_id),
        user_info: None,
    };

    authenticator
        .refresh(&stored)
        .await
        .map_err(|error| format!("Не удалось обновить Microsoft-сессию: {error}"))
}
