# GlowstoneCL

**Universal Minecraft Launcher built with Rust.**

GlowstoneCL is planned as one desktop launcher for two separate game platforms:

- **Minecraft Java Edition** — Vanilla, Forge, NeoForge, Fabric and Quilt.
- **Horizon** — the Rust-based Minecraft runtime, with its own .hmod content mods and .hnmod native mods.

The project targets **Windows and Linux**. On Linux, the desktop app supports native Wayland and X11 backends; XWayland is not required by the Wayland backend.

## Current development status

The current test branch includes:

- a native Iced desktop interface with a three-column launcher layout;
- SVG-based Material-style icons and Noto Sans system typography;
- local game-instance creation, listing and deletion;
- Java loader selection (Vanilla, Forge, NeoForge, Fabric, Quilt);
- Horizon profile selection (Horizon Content and Horizon Native);
- local offline account creation with deterministic offline UUIDs;
- Microsoft Device Code login, followed by Xbox Live, XSTS and Minecraft Services authentication;
- account metadata stored in the app data directory;
- Microsoft refresh tokens stored through the operating system credential store;
- an embedded Minecraft hero image.

**Not implemented yet:** downloading Minecraft versions/libraries/assets, installing Forge/Fabric/NeoForge, launching a game process, real Modrinth/CurseForge modpack installation, and launching Horizon profiles. A created instance is currently a saved profile, not an installed game.

## Requirements

- Rust stable, minimum 1.88 (Rust 2024 edition).
- Linux: Wayland or X11 development libraries; see docs/LINUX.md.
- Windows: MSVC Build Tools and Windows SDK for the MSVC target.
- Recommended UI font: Noto Sans (Cyrillic coverage).

### CachyOS / Arch Linux

    sudo pacman -S --needed rustup git wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi vulkan-icd-loader noto-fonts

    rustup default stable

## Run

    cargo run -p glowstonecl -- --backend wayland

Force X11:

    cargo run -p glowstonecl -- --backend x11

Run default backend selection:

    cargo run -p glowstonecl

## Local data

On Linux, application state is stored at:

    ~/.local/share/GlowstoneCL/state.json

The state file contains instance metadata, local account profiles and the Microsoft public Client ID. Microsoft refresh tokens are stored separately via the platform credential store (Secret Service on Linux, Windows Credential Manager on Windows). Never put a Microsoft Client Secret into this desktop application.

## Microsoft sign-in setup

A Microsoft application Client ID is required. See docs/ACCOUNTS.md for registration steps. GlowstoneCL uses the public-client Device Code Flow and does not use a client secret.

## Workspace

- crates/glowstone-core — shared game and loader domain types.
- crates/glowstone-desktop — native Iced launcher application, local storage and authentication.
- docs/ARCHITECTURE.md — planned system boundaries.
- docs/LINUX.md — Wayland/X11 support and build dependencies.
- docs/ACCOUNTS.md — Microsoft app registration and account notes.

## Planned integrations

| Area | Planned implementation |
|---|---|
| Java runtime | Mojang version manifest, game files, libraries/assets and JVM launch |
| Mod loaders | Forge, NeoForge, Fabric and Quilt installers |
| Mod catalog | Modrinth API and CurseForge API |
| Horizon | Horizon runtime and native mod management |
| Authentication | Microsoft token refresh, profile management and secure session lifecycle |
| Storage | Per-instance directories, cache, settings and OS credential storage |
| Distribution | Windows installer, Linux packages and portable archive |

## Licensing

GlowstoneCL source code is licensed under MIT. Third-party game files, mod files, APIs and trademarks remain subject to their respective owners' terms and licenses.
