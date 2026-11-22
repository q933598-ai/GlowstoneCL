use std::sync::{Mutex, OnceLock};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

#[derive(Default)]
struct RpcState {
    application_id: Option<String>,
    client: Option<DiscordIpcClient>,
}

static CLIENT: OnceLock<Mutex<RpcState>> = OnceLock::new();

/// Updates Discord Rich Presence, reconnecting automatically if the Application ID changes.
/// The Discord client is local IPC; this never sends account tokens or server addresses.
pub fn set_presence(
    enabled: bool,
    application_id: &str,
    details: &str,
    state: &str,
) -> Result<String, String> {
    let client = CLIENT.get_or_init(|| Mutex::new(RpcState::default()));
    let mut guard = client
        .lock()
        .map_err(|_| "Не удалось получить доступ к Discord RPC.".to_owned())?;

    if !enabled {
        if let Some(mut connected) = guard.client.take() {
            let _ = connected.clear_activity();
            let _ = connected.close();
        }
        guard.application_id = None;
        return Ok("Discord Rich Presence выключен.".to_owned());
    }

    let application_id = application_id.trim();
    if application_id.is_empty() || !application_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Укажи числовой Application ID из Discord Developer Portal.".to_owned());
    }

    if guard.application_id.as_deref() != Some(application_id) {
        if let Some(mut old) = guard.client.take() {
            let _ = old.clear_activity();
            let _ = old.close();
        }
        guard.application_id = None;

        let mut connected = DiscordIpcClient::new(application_id);
        connected
            .connect()
            .map_err(|error| format!("Не удалось подключиться к Discord. Проверь, что Discord запущен: {error}"))?;
        guard.client = Some(connected);
        guard.application_id = Some(application_id.to_owned());
    }

    let connected = guard.client
        .as_mut()
        .ok_or_else(|| "Discord RPC не создал соединение.".to_owned())?;

    connected
        .set_activity(
            activity::Activity::new()
                .details(truncate(details, 128))
                .state(truncate(state, 128)),
        )
        .map_err(|error| {
            guard.client = None;
            guard.application_id = None;
            format!("Не удалось обновить Discord Rich Presence: {error}")
        })?;

    Ok("Discord Rich Presence подключён.".to_owned())
}

/// Compatibility wrapper used by the settings toggle.
pub fn set_enabled(enabled: bool, application_id: &str) -> Result<String, String> {
    set_presence(
        enabled,
        application_id,
        "GlowstoneCL",
        "Выбирает сборку Minecraft",
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}
