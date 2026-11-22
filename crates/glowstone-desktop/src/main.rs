#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod curseforge;
mod game;
mod horizon_mods;
mod mods;
mod modpacks;
mod store;
mod skin_preview;
mod discord;
mod updates;
mod ui;

fn main() -> iced::Result {
    apply_linux_backend_override();

    iced::application(ui::App::default, ui::update, ui::view)
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

/// Restore display variables removed to force the launcher's own Winit backend.
/// This is applied to game processes only; the launcher's process environment
/// remains unchanged after startup so it cannot leak the forced backend into games.
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
fn apply_linux_backend_override() {
    use std::env;

    let args: Vec<String> = env::args().collect();
    let backend = args
        .windows(2)
        .find(|pair| pair[0] == "--backend")
        .map(|pair| pair[1].as_str())
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix("--backend="))
        });

    let mut child_display_env = Vec::new();
    match backend {
        Some("wayland") => {
            if let Some(display) = env::var_os("DISPLAY") {
                child_display_env.push((std::ffi::OsString::from("DISPLAY"), display));
            }
            // SAFETY: this runs before Iced/Winit creates worker threads.
            // The original value is restored only on the game child process.
            unsafe { env::remove_var("DISPLAY") };
        }
        Some("x11") => {
            if let Some(display) = env::var_os("WAYLAND_DISPLAY") {
                child_display_env.push((std::ffi::OsString::from("WAYLAND_DISPLAY"), display));
            }
            // SAFETY: this runs before Iced/Winit creates worker threads.
            // The original value is restored only on the game child process.
            unsafe { env::remove_var("WAYLAND_DISPLAY") };
        }
        _ => {}
    }

    let _ = CHILD_DISPLAY_ENV.set(child_display_env);
}

#[cfg(not(target_os = "linux"))]
fn apply_linux_backend_override() {}
