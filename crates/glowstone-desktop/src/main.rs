#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod curseforge;
mod game;
mod horizon_mods;
mod instance_tools;
mod mods;
mod modpacks;
mod store;
mod skin_preview;
mod discord;
mod updates;
mod ui;

fn main() -> iced::Result {
    apply_linux_backend_override();

    iced::application(
        || {
            let app = ui::App::default();
            let startup = ui::initial_task(&app);
            (app, startup)
        },
        ui::update,
        ui::view,
    )
    .subscription(ui::subscription)
    .title("GlowstoneCL")
    .theme(iced::Theme::Dark)
    .font(include_bytes!("../assets/fonts/Monocraft.ttf").as_slice())
    .default_font(iced::Font::with_name("Monocraft"))
    .window_size(iced::Size::new(1600.0, 900.0))
    .centered()
    .run()
}

#[cfg(target_os = "linux")]
static CHILD_DISPLAY_ENV: std::sync::OnceLock<
    Vec<(std::ffi::OsString, std::ffi::OsString)>,
> = std::sync::OnceLock::new();

/// Restore display variables saved before forcing the launcher's Winit backend.
/// Called for game processes so they receive the user's original session environment.
#[cfg(target_os = "linux")]
pub(crate) fn restore_child_display_environment(
    command: &mut tokio::process::Command,
) {
    if let Some(variables) = CHILD_DISPLAY_ENV.get() {
        for (name, value) in variables {
            command.env(name, value);
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxBackend {
    Wayland,
    X11,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendPreference {
    Auto,
    Wayland,
    X11,
}

#[cfg(target_os = "linux")]
fn parse_backend_preference(args: &[String]) -> BackendPreference {
    let value = args
        .windows(2)
        .find(|pair| pair[0] == "--backend")
        .map(|pair| pair[1].as_str())
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix("--backend="))
        });

    match value {
        Some("wayland") => BackendPreference::Wayland,
        Some("x11") => BackendPreference::X11,
        Some("auto") | None => BackendPreference::Auto,
        Some(other) => {
            eprintln!(
                "[GlowstoneCL] Unknown backend '{other}'. Falling back to automatic detection."
            );
            BackendPreference::Auto
        }
    }
}

#[cfg(target_os = "linux")]
fn choose_linux_backend(
    preference: BackendPreference,
    wayland_available: bool,
    x11_available: bool,
) -> Option<LinuxBackend> {
    match preference {
        BackendPreference::Wayland => Some(LinuxBackend::Wayland),
        BackendPreference::X11 => Some(LinuxBackend::X11),
        BackendPreference::Auto if wayland_available => Some(LinuxBackend::Wayland),
        BackendPreference::Auto if x11_available => Some(LinuxBackend::X11),
        BackendPreference::Auto => None,
    }
}

#[cfg(target_os = "linux")]
fn wayland_socket_path() -> Option<std::path::PathBuf> {
    use std::env;
    use std::path::{Path, PathBuf};

    let display = env::var_os("WAYLAND_DISPLAY")?;
    let display_path = Path::new(&display);
    if display_path.is_absolute() {
        return Some(display_path.to_path_buf());
    }

    Some(PathBuf::from(env::var_os("XDG_RUNTIME_DIR")?).join(display_path))
}

#[cfg(target_os = "linux")]
fn wayland_socket_is_available() -> bool {
    use std::os::unix::net::UnixStream;

    let Some(path) = wayland_socket_path() else {
        return false;
    };

    UnixStream::connect(&path).is_ok()
}

#[cfg(target_os = "linux")]
fn apply_linux_backend_override() {
    use std::env;

    let args: Vec<String> = env::args().collect();
    let preference = parse_backend_preference(&args);
    let wayland_available = wayland_socket_is_available();
    let x11_available = env::var_os("DISPLAY").is_some();

    let Some(backend) = choose_linux_backend(
        preference,
        wayland_available,
        x11_available,
    ) else {
        eprintln!(
            "[GlowstoneCL] No Wayland socket or DISPLAY was detected. Letting Winit choose its default backend."
        );
        let _ = CHILD_DISPLAY_ENV.set(Vec::new());
        return;
    };

    let mut child_display_env = Vec::new();
    match backend {
        LinuxBackend::Wayland => {
            eprintln!("[GlowstoneCL] Using native Wayland backend.");
            if let Some(display) = env::var_os("DISPLAY") {
                child_display_env.push((std::ffi::OsString::from("DISPLAY"), display));
            }
            // SAFETY: this runs before Iced/Winit creates worker threads.
            // The original value is restored only on game child processes.
            unsafe { env::remove_var("DISPLAY") };
        }
        LinuxBackend::X11 => {
            eprintln!("[GlowstoneCL] Using X11 backend.");
            if let Some(display) = env::var_os("WAYLAND_DISPLAY") {
                child_display_env.push((std::ffi::OsString::from("WAYLAND_DISPLAY"), display));
            }
            // SAFETY: this runs before Iced/Winit creates worker threads.
            // The original value is restored only on game child processes.
            unsafe { env::remove_var("WAYLAND_DISPLAY") };
        }
    }

    let _ = CHILD_DISPLAY_ENV.set(child_display_env);
}

#[cfg(all(test, target_os = "linux"))]
mod backend_tests {
    use super::*;

    #[test]
    fn automatic_backend_prefers_available_wayland() {
        assert_eq!(
            choose_linux_backend(BackendPreference::Auto, true, true),
            Some(LinuxBackend::Wayland)
        );
    }

    #[test]
    fn automatic_backend_falls_back_to_x11_when_wayland_is_unavailable() {
        assert_eq!(
            choose_linux_backend(BackendPreference::Auto, false, true),
            Some(LinuxBackend::X11)
        );
    }

    #[test]
    fn automatic_backend_returns_none_when_no_display_is_available() {
        assert_eq!(
            choose_linux_backend(BackendPreference::Auto, false, false),
            None
        );
    }

    #[test]
    fn explicit_backend_selection_is_respected() {
        assert_eq!(
            choose_linux_backend(BackendPreference::Wayland, false, true),
            Some(LinuxBackend::Wayland)
        );
        assert_eq!(
            choose_linux_backend(BackendPreference::X11, true, false),
            Some(LinuxBackend::X11)
        );
    }

    #[test]
    fn backend_argument_supports_separate_and_equals_forms() {
        assert_eq!(
            parse_backend_preference(&["glowstonecl".into(), "--backend".into(), "wayland".into()]),
            BackendPreference::Wayland
        );
        assert_eq!(
            parse_backend_preference(&["glowstonecl".into(), "--backend=x11".into()]),
            BackendPreference::X11
        );
        assert_eq!(
            parse_backend_preference(&["glowstonecl".into(), "--backend=auto".into()]),
            BackendPreference::Auto
        );
    }
}
#[cfg(not(target_os = "linux"))]
fn apply_linux_backend_override() {}
