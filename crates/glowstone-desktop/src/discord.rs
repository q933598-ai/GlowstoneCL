use std::sync::{Mutex, OnceLock};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

#[derive(Default)]
struct RpcState {
    application_id: Option<String>,
    client: Option<DiscordIpcClient>,
}

static CLIENT: OnceLock<Mutex<RpcState>> = OnceLock::new();

/// Shared Discord Application ID used by GlowstoneCL Rich Presence.
pub fn application_id() -> &'static str {
    "1574029556392923247"
}

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
        connected.connect().map_err(|error| {
            format!(
                "Не удалось подключиться к Discord IPC: {error}. {}",
                diagnostics()
            )
        })?;
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
        use std::os::unix::fs::FileTypeExt;

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
        let mut checked_dirs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for key in ENV_KEYS {
            let Some(value) = std::env::var_os(key) else { continue };
            let base = std::path::PathBuf::from(value);
            if !base.is_dir() { continue; }
            checked_dirs.push(format!("{key}={}", base.display()));
            for subpath in APP_SUBPATHS {
                for index in 0..10 {
                    let path = base.join(subpath).join(format!("discord-ipc-{index}"));
                    if !seen.insert(path.clone()) { continue; }
                    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                        if metadata.file_type().is_socket() {
                            return format!(
                                "IPC-сокет найден: {}. Если соединение всё равно не устанавливается, проверь Application ID и запуск desktop Discord в той же сессии пользователя.",
                                path.display()
                            );
                        }
                    }
                }
            }
        }

        // The official Discord client can also use /tmp when no runtime directory is
        // configured. Report it even though the library prefers environment paths.
        for index in 0..10 {
            let path = std::path::PathBuf::from(format!("/tmp/discord-ipc-{index}"));
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if metadata.file_type().is_socket() {
                    return format!(
                        "IPC-сокет найден в /tmp: {}. Убедись, что один из TMPDIR/TMP/TEMP указывает на /tmp, иначе используемая RPC-библиотека может не найти этот путь.",
                        path.display()
                    );
                }
            }
        }

        if checked_dirs.is_empty() {
            "Discord IPC не найден. Переменные XDG_RUNTIME_DIR/TMPDIR/TMP/TEMP не заданы или каталоги недоступны. Запусти desktop Discord в той же пользовательской сессии.".to_owned()
        } else {
            format!(
                "Discord IPC-сокет не найден. Проверены: {}. Убедись, что открыт именно desktop Discord (не веб-версия) и что он запущен от того же пользователя.",
                checked_dirs.join(" · ")
            )
        }
    }
    #[cfg(windows)]
    {
        "Windows: Discord RPC использует локальный discord-ipc named pipe. Убедись, что открыт desktop Discord, Application ID числовой, а лаунчер и Discord работают от одного пользователя.".to_owned()
    }
    #[cfg(target_os = "macos")]
    {
        "macOS: Discord RPC использует локальный Unix socket. Убедись, что открыт desktop Discord в том же пользовательском сеансе, а Application ID скопирован из Discord Developer Portal.".to_owned()
    }
}
