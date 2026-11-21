#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod curseforge;
mod game;
mod horizon_mods;
mod mods;
mod store;
mod skin_preview;
mod ui;

fn main() -> iced::Result {
    apply_linux_backend_override();

    iced::application(ui::App::default, ui::update, ui::view)
        .subscription(ui::subscription)
        .title("GlowstoneCL")
        .theme(iced::Theme::Light)
        .default_font(iced::Font::MONOSPACE)
        .window_size(iced::Size::new(1600.0, 900.0))
        .centered()
        .run()
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

    match backend {
        Some("wayland") => {
            // SAFETY: this runs before Iced/Winit creates worker threads.
            unsafe { env::remove_var("DISPLAY") };
        }
        Some("x11") => {
            // SAFETY: this runs before Iced/Winit creates worker threads.
            unsafe { env::remove_var("WAYLAND_DISPLAY") };
        }
        _ => {}
    }
}

#[cfg(not(target_os = "linux"))]
fn apply_linux_backend_override() {}
