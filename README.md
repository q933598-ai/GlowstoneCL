# GlowstoneCL

**Universal Minecraft Launcher built with Rust.**

GlowstoneCL is a desktop launcher for two separate game platforms:

- **Minecraft Java Edition** — Vanilla, Forge, NeoForge, Fabric and Quilt.
- **Horizon** — the Rust-based Minecraft runtime, launched through Horizon's official Launch API.

The project targets **Windows and Linux**. On Linux, the desktop app supports native Wayland and X11 backends; XWayland is not required by the Wayland backend.

## Current implementation

The `test/glowstonecl` branch includes:

- a native Iced desktop interface with a three-column launcher layout and SVG icons;
- local instance creation, selection, isolated workspaces and safe deletion to a local trash directory;
- **official Minecraft Java installation and launch** through CrustCore: Mojang version metadata, client files, libraries, assets and matching Java runtimes are downloaded as needed;
- first-launch installation and launch of Vanilla, Fabric, Quilt, Forge and NeoForge profiles;
- **Horizon installation and launch**: the latest supported official Nightly is downloaded, its version-bound Ed25519 signature and archive SHA-256 are checked, and the binary's Launch API is queried before use;
- automatic download of the Mojang game files and Java version required by Horizon, followed by resource preparation and launch through `--session-stdin`;
- Microsoft browser sign-in using Device Code Flow, with automatic browser opening and background polling, followed by Xbox Live, XSTS and Minecraft Services, including Java ownership / Game Pass product checks;
- Microsoft refresh-token storage in the operating system credential store, with session refresh before launching a game;
- offline profiles for Java offline-mode play.

The Horizon game binary is currently provided for **Linux x86_64** and **Windows x64**. The Java backend supports the platform/runtime combinations provided by CrustCore.

## Not implemented yet

- Modrinth and CurseForge catalogues;
- in-app mod and modpack installation, updates and dependency resolution;
- a detailed download queue/progress view and log/crash-report browser;
- import/export of complete instances and packaged installers for GlowstoneCL.

Java and Horizon remain separate runtimes: Java JAR mods are not automatically Horizon mods.

## Requirements

- Rust stable, minimum 1.88 (Rust 2024 edition).
- Linux: Wayland or X11 development libraries; see [docs/LINUX.md](docs/LINUX.md).
- Windows: MSVC Build Tools and Windows SDK for the MSVC target.
- Recommended UI font: Inter on Linux and Segoe UI on Windows.

### CachyOS / Arch Linux

    sudo pacman -S --needed rustup git wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi vulkan-icd-loader inter-font

    rustup default stable

## Run

    cargo run -p glowstonecl -- --backend wayland

Force X11:

    cargo run -p glowstonecl -- --backend x11

Run with automatic backend selection:

    cargo run -p glowstonecl

## Local data

On Linux, application state and game files are stored under:

    ~/.local/share/GlowstoneCL/

The `state.json` file contains instance metadata, local account profiles and the Microsoft public Client ID. Microsoft refresh tokens are stored separately through the platform credential store (Secret Service on Linux, Windows Credential Manager on Windows). Short-lived Minecraft access tokens are held in memory and are not written to `state.json`.

Each instance has a separate game directory under `instances/<uuid>/`. Java and Horizon use separate launch paths and loader models.

## Microsoft sign-in setup

A Microsoft Application (client) ID for a public-client app is required. Minecraft Services may require the app to be approved before its APIs accept the Client ID. See [docs/ACCOUNTS.md](docs/ACCOUNTS.md) for registration and access-review steps. GlowstoneCL does not use or embed a Client Secret.

## Workspace

- `crates/glowstone-core` — shared game and loader domain types.
- `crates/glowstone-desktop` — native Iced launcher, local storage, authentication and game backends.
- `docs/ARCHITECTURE.md` — runtime and security boundaries.
- `docs/LINUX.md` — Wayland/X11 support and build dependencies.
- `docs/ACCOUNTS.md` — Microsoft app registration and account notes.

## Planned integrations

| Area | Status |
|---|---|
| Minecraft Java download and launch | Implemented through CrustCore |
| Vanilla, Fabric, Quilt, Forge and NeoForge | Implemented in the launch backend |
| Horizon runtime and Launch API | Implemented for Linux x86_64 and Windows x64 |
| Microsoft Device Code sign-in and token refresh | Implemented |
| Modrinth / CurseForge catalogues | Planned |
| Mod and modpack installation / updates | Planned |
| Download queue, detailed progress and logs UI | Planned |
| Installers and distribution packages | Planned |

## Licensing

GlowstoneCL source code is licensed under MIT. Third-party libraries, game files, mod files, APIs and trademarks remain subject to their respective owners' terms and licenses.
