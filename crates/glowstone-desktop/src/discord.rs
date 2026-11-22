use std::sync::{Mutex, OnceLock};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

static CLIENT: OnceLock<Mutex<Option<DiscordIpcClient>>> = OnceLock::new();

/// Starts or stops the process-wide Discord Rich Presence connection.
/// The application ID is user-configured; no hard-coded Discord identity is used.
pub fn set_enabled(enabled: bool, client_id: &str) -> Result<String, String> {
    let client = CLIENT.get_or_init(|| Mutex::new(None));
    let mut guard = client
        .lock()
        .map_err(|_| "Не удалось получить доступ к Discord RPC.".to_owned())?;

    if !enabled {
        if let Some(mut connected) = guard.take() {
            let _ = connected.clear_activity();
            let _ = connected.close();
        }
        return Ok("Discord Rich Presence выключен.".to_owned());
    }

    let client_id = client_id.trim();
    if client_id.is_empty() || !client_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Укажи числовой Application ID из Discord Developer Portal.".to_owned());
    }

    if guard.is_none() {
        let mut connected = DiscordIpcClient::new(client_id);
        connected
            .connect()
            .map_err(|error| format!("Не удалось подключиться к Discord: {error}"))?;
        *guard = Some(connected);
    }

    let connected = guard
        .as_mut()
        .ok_or_else(|| "Discord RPC не создал соединение.".to_owned())?;

    connected
        .set_activity(
            activity::Activity::new()
                .details("GlowstoneCL")
                .state("Настраивает Minecraft"),
        )
        .map_err(|error| format!("Не удалось обновить Discord Rich Presence: {error}"))?;

    Ok("Discord Rich Presence включён.".to_owned())
}
