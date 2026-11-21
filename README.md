# GlowstoneCL

**Universal Minecraft Launcher built with Rust.**

GlowstoneCL is planned as one desktop launcher for two separate game platforms:

- **Minecraft Java Edition** — Vanilla, Forge, NeoForge, Fabric and Quilt.
- **Horizon** — the Rust-based Minecraft runtime, with its own .hmod content mods and .hnmod native mods.

The project targets **Windows and Linux**. On Linux, the desktop app is built with native Wayland and X11 backends. XWayland is not required when using the Wayland backend.

## Current status

This is the initial project foundation, not a finished launcher. The first commit contains:

- a native Iced desktop shell with Russian UI;
- navigation for the main launcher sections;
- separate Java and Horizon platform selection;
- initial domain types for game instances and loader compatibility;
- explicit Linux Wayland/X11 build features and backend overrides;
- GitHub Actions checks for Linux and Windows.

Game downloading, Microsoft authentication, Java launch, mod-loader installation, Modrinth/CurseForge catalog access and Horizon runtime integration are not connected yet.

## Requirements

- Rust stable, minimum 1.88 (Rust 2024 edition).
- Linux: Wayland or X11 development libraries; see docs/LINUX.md.
- Windows: MSVC Build Tools and Windows SDK for the MSVC target.

## Run from source

    cargo run --package glowstonecl

The default Linux build enables both native display backends. Winit selects the backend exposed by the desktop session. To force a backend:

    cargo run --package glowstonecl -- --backend wayland
    cargo run --package glowstonecl -- --backend x11

The override applies only to the GlowstoneCL process and does not modify the parent shell environment.

## Workspace

- crates/glowstone-core — shared domain types and validation.
- crates/glowstone-desktop — native Iced desktop application.
- docs/ARCHITECTURE.md — planned system boundaries.
- docs/LINUX.md — Wayland/X11 support and build dependencies.

## Planned integrations

| Area | Planned implementation |
|---|---|
| Java runtime | Mojang version manifest, libraries/assets and JVM launch |
| Mod loaders | Forge, NeoForge, Fabric and Quilt installers |
| Mod catalog | Modrinth API and CurseForge API |
| Horizon | Horizon runtime integration and native mod management |
| Authentication | Microsoft OAuth and local offline profiles |
| Storage | Per-instance directories, cache, settings and secure token storage |
| Distribution | Windows installer, Linux packages and portable archive |

## Licensing

GlowstoneCL source code is licensed under MIT. Third-party game files, mod files, APIs and trademarks remain subject to their respective owners' terms and licenses.
