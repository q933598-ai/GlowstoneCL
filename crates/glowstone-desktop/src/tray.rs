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
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let edge = x < 3 || x > 28 || y < 3 || y > 28;
            let face = if y < 12 {
                [255, 221, 143, 255]
            } else if x < 16 {
                [224, 161, 82, 255]
            } else {
                [180, 112, 53, 255]
            };
            let highlight = x >= 8 && x <= 11 && y >= 7 && y <= 10;
            let index = ((y * SIZE + x) * 4) as usize;
            let color = if edge {
                [83, 58, 38, 255]
            } else if highlight {
                [255, 245, 201, 255]
            } else {
                face
            };
            rgba[index..index + 4].copy_from_slice(&color);
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE)
        .map_err(|error| format!("Не удалось создать иконку трея: {error}"))
}
