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

    let activity_result = guard.client
        .as_mut()
        .ok_or_else(|| "Discord RPC не создал соединение.".to_owned())?
        .set_activity(
            activity::Activity::new()
                .name("GlowstoneCL")
                .activity_type(activity::ActivityType::Playing)
                .details(truncate(details, 128))
                .state(truncate(state, 128)),
        );

    if let Err(error) = activity_result {
        guard.client = None;
        guard.application_id = None;
        return Err(format!("Не удалось обновить Discord Rich Presence: {error}"));
    }

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


/// Explains why local Discord IPC may not be reachable. Uses only the common
/// runtime directories and the same Flatpak/Snap paths searched by the RPC client.
pub fn diagnostics() -> String {
    #[cfg(unix)]
    {
        const ENV_KEYS: [&str; 4] = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"];
        const APP_SUBPATHS: [&str; 7] = [
            "",
            "app/com.discordapp.Discord/",
            "app/dev.vencord.Vesktop/",
            ".flatpak/com.discordapp.Discord/xdg-run/",
            ".flatpak/dev.vencord.Vesktop/xdg-run/",
            "snap.discord-canary/",
            "snap.discord/",
        ];
        let mut checked = Vec::new();
        for key in ENV_KEYS {
            let Some(value) = std::env::var_os(key) else { continue };
            let base = std::path::PathBuf::from(value);
            if !base.is_dir() { continue; }
            for subpath in APP_SUBPATHS {
                for index in 0..10 {
                    let path = base.join(subpath).join(format!("discord-ipc-{index}"));
                    if path.exists() {
                        return format!("IPC найден: {}. Проверь, что Discord и GlowstoneCL запущены от одного пользователя.", path.display());
                    }
                }
            }
            checked.push(format!("{key}={}", base.display()));
        }
        if checked.is_empty() {
            "Discord IPC не найден: XDG_RUNTIME_DIR/TMPDIR не заданы. Убедись, что Discord запущен в той же пользовательской сессии.".to_owned()
        } else {
            format!(
                "Discord IPC не найден. Проверены: {}. Запусти Discord перед GlowstoneCL; Flatpak/Snap могут изолировать сокет.",
                checked.join(" · ")
            )
        }
    }
    #[cfg(windows)]
    {
        "Windows: Discord RPC использует локальный discord-ipc named pipe. Убедись, что Discord открыт и Application ID указан без пробелов.".to_owned()
    }
    #[cfg(target_os = "macos")]
    {
        "macOS: Discord RPC использует локальный Unix socket. Убедись, что Discord открыт в том же пользовательском сеансе.".to_owned()
    }
}
