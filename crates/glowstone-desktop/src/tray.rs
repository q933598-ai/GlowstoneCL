//! Small cross-platform tray integration used only when the user enables
//! "minimize to tray on game launch". The icon is kept on the UI thread because
//! tray-icon is intentionally !Send/!Sync on some platforms.
use std::cell::RefCell;

use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

thread_local! {
    static TRAY: RefCell<Option<tray_icon::TrayIcon>> = const { RefCell::new(None) };
}

pub fn ensure() -> Result<(), String> {
    TRAY.with(|slot| {
        if slot.borrow().is_some() {
            return Ok(());
        }
        let icon = build_icon()?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("GlowstoneCL · Minecraft")
            .with_icon(icon)
            .build()
            .map_err(|error| format!("Не удалось создать значок в системном трее: {error}"))?;
        *slot.borrow_mut() = Some(tray);
        Ok(())
    })
}

pub fn poll_restore_request() -> bool {
    let mut restore = false;
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        match event {
            TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. } => {
                restore = true;
            }
            _ => {}
        }
    }
    restore
}

fn build_icon() -> Result<Icon, String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../assets/icons/glowstone.png"
    )));
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("Не удалось прочитать иконку трея: {error}"))?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut rgba)
        .map_err(|error| format!("Не удалось декодировать иконку трея: {error}"))?;

    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("Иконка трея должна быть PNG RGBA 8-bit".to_owned());
    }

    rgba.truncate(info.buffer_size());
    Icon::from_rgba(rgba, info.width, info.height)
        .map_err(|error| format!("Не удалось создать иконку трея: {error}"))
}
