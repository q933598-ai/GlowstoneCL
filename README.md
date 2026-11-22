# GlowstoneCL

**Universal Minecraft Launcher built with Rust.**

GlowstoneCL is a desktop launcher for two separate game platforms:

- **Minecraft Java Edition** — Vanilla, Forge, NeoForge, Fabric and Quilt.
- **Horizon** — the official Rust-based Minecraft runtime, launched through Horizon's Launch API.
- **Horizon Offline** — a separately signed, unofficial offline fork for local/offline profiles, currently pinned to Minecraft 26.3 and Java 25.

The project targets **Windows and Linux**. On Linux, the desktop app supports native Wayland and X11 backends; XWayland is not required by the Wayland backend.

## Current implementation

The `test/glowstonecl` branch includes:

- a native Iced desktop interface with a dark graphite and warm amber liquid-glass visual system, a three-column launcher dashboard, a custom glowstone-block SVG brand mark, and SVG icons;
- the bundled **Monocraft** pixel font (including Cyrillic), loaded directly by Iced on startup; its SIL Open Font License is included in `crates/glowstone-desktop/assets/fonts/Monocraft-OFL.txt`;
- local instance creation, selection, isolated workspaces and safe deletion to a local trash directory;
- **official Minecraft Java installation and launch** through CrustCore: Mojang version metadata, client files, libraries, assets and matching Java runtimes are downloaded as needed;
- first-launch installation and launch of Vanilla, Fabric, Quilt, Forge and NeoForge profiles;
- **Horizon installation and launch**: the selected official game release (Nightly or tagged version) is downloaded, its Ed25519 signature and archive SHA-256 are checked, and the binary's Launch API is queried before use;
- **Horizon Offline** installation and launch from a separate signed fork, without Microsoft session injection; it imports existing local Minecraft 26.3 files from .minecraft or Prism/MultiMC and does not download the client jar for an offline profile;
- Java version catalogue from Mojang, including `release`, `snapshot`, `old_beta` and `old_alpha`; Horizon catalogues from the official and offline-fork GitHub game releases, with pagination and install-by-selected-release ID;
- **Modrinth mod catalogue and installer** for Java profiles with Fabric, Forge, NeoForge and Quilt: search is filtered by the instance's Minecraft version and loader, compatible JARs and required Modrinth dependencies are installed into that instance's `mods/` directory, SHA-1/SHA-512 hashes are checked, project icons are cached, and the launcher tracks/removes only the files it installed;
- **Horizon Registry content-mod catalogue** for Horizon and Horizon Offline profiles: reviewed `kind = mod` HMODs for Minecraft 26.3 are searched and installed into that instance's `horizon-mods/` directory, with SHA-256, revoked-file, package manifest, ID, version and API checks. GlowstoneCL enables installed mods through `horizon-mods.txt`. Signed packages are shown but not installed yet because minisign verification is not implemented; native-mod and WebAssembly packages are not installed by this content-only path;
- a per-instance **Моды** action on every non-Vanilla Java or Horizon build card; Vanilla Java profiles correctly have no mod-install action;
- automatic download of the Mojang game files and Java version required by official Horizon, followed by resource preparation and launch through `--session-stdin`;
- Microsoft browser sign-in through CrustCore using the official Minecraft Launcher public client by default (optional custom Client ID), followed by Xbox Live, XSTS and Minecraft Services, including ownership / Game Pass checks;
- Microsoft refresh-token storage in the operating system credential store, with session refresh before launching a game;
- offline profiles for Java offline-mode play and the separate Horizon Offline fork;
- a skin wardrobe for offline and Microsoft profiles, with per-profile PNG collections, current-skin sync for Microsoft accounts, official Minecraft Services upload, a mouse-rotatable 3D player, compact 3D previews for the other saved skins, and decoded-pixel deduplication so PNG re-encoding does not create duplicate Microsoft skins. The selected skin preview rotates by dragging with the mouse; miniature 3D previews are shown for saved skins. Offline skins are applied directly by Horizon Offline. For ordinary Minecraft Java, GlowstoneCL uses the standalone [OfflineSkinInjector](https://github.com/NaivG/OfflineSkinInjector) Java agent: no Fabric/Forge skin mod is installed and no `mods/` folder entry is needed. Its upstream 0.1.2 JAR is downloaded on demand and checked against a pinned SHA-256 before launch. The injector works at runtime with supported authlib generations across Vanilla and mod-loader clients; the skin is client-side only;
- a Prism-style build library with a compact create action, a separate create form, context-click creation, and saved favourite profiles;
- a centered live launch diagnostics window for Java and Horizon: CrustCore preparation events, Minecraft `logs/latest.log` tail, Horizon stdout/stderr, final exit status, and copyable error/full-log actions;
- user-selectable wallpaper images, plus bundled preset discovery from `crates/glowstone-desktop/assets/backgrounds/` (the folder currently contains instructions and awaits artwork);
- opt-in **local-only diagnostics**: selected launch errors can be stored in `diagnostics/errors.log` after basic token/path redaction; this build does not upload telemetry to a remote server;
- optional Discord Rich Presence using a user-supplied Discord Application ID;
- a GitHub commit changelog in Settings and News, with a setting to hide it; binary self-updating is not available until signed release assets are published;
- a language selector for Russian and English; primary navigation/profile labels are translated, while full-screen translation remains in progress;
- persistent Recent Play history for Java and Horizon launches, with quick relaunch actions;
- a **CurseForge Java mod catalogue** for Forge, Fabric, NeoForge and Quilt. The official API key is saved only in the operating-system credential store; installed JARs are SHA-1 checked and tracked per instance;
- a **Modrinth modpack catalogue and .mrpack installer**. Each import creates a new Java instance, validates pack paths, file sizes and hashes, extracts client overrides, and pins the loader build specified by the pack index.

The Horizon game binary is currently provided for **Linux x86_64** and **Windows x64**. The Java backend supports the platform/runtime combinations provided by CrustCore.

## Not implemented yet

- CurseForge modpack ZIP import and Horizon modpack conversion/import;
- full modpack export and automatic modpack updates;
- automatic updates for installed mods;
- a detailed download queue/progress view and log/crash-report browser;
- import/export of complete instances and packaged installers for GlowstoneCL;

Java, official Horizon and Horizon Offline remain separate runtimes: Java JAR mods are not automatically Horizon mods, and the offline fork is not interchangeable with the official client.

## Requirements

- Rust stable, minimum 1.88 (Rust 2024 edition).
- Linux: Wayland or X11 development libraries; see [docs/LINUX.md](docs/LINUX.md).
- Windows: MSVC Build Tools and Windows SDK for the MSVC target.
- No external UI font installation is required; Monocraft is bundled with GlowstoneCL.

### CachyOS / Arch Linux

    sudo pacman -S --needed rustup git wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi vulkan-icd-loader

    rustup default stable

## Run

Force native Wayland for the GlowstoneCL window:

    cargo run -p glowstonecl -- --backend wayland

Force X11 for the GlowstoneCL window:

    cargo run -p glowstonecl -- --backend x11

Run with automatic backend selection:

    cargo run -p glowstonecl

The backend switch applies to GlowstoneCL only. The original `DISPLAY` / `WAYLAND_DISPLAY` value is restored on Java and Horizon game processes, so the game receives the desktop session environment and can select its own supported backend.

## Local data

On Linux, application state and game files are stored under:

    ~/.local/share/GlowstoneCL/

The `state.json` file contains instance metadata, local account profiles and the Microsoft public Client ID. Microsoft refresh tokens are stored separately through the platform credential store (Secret Service on Linux, Windows Credential Manager on Windows). Short-lived Minecraft access tokens are held in memory and are not written to `state.json`.

Each instance has a separate game directory under `instances/<uuid>/`. Imported Modrinth modpacks retain their exact loader build pin in instance metadata. Recent launch history is stored separately in `recent-plays.json`; favourites remain part of `state.json`. Java, official Horizon and Horizon Offline use separate launch paths, caches and loader models.

## Microsoft sign-in setup

A custom Microsoft Application (client) ID is optional. GlowstoneCL uses CrustCore's official Minecraft Launcher public client ID by default; custom Entra registrations may require Minecraft Services approval. See [docs/ACCOUNTS.md](docs/ACCOUNTS.md). GlowstoneCL does not use or embed a Client Secret.

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
| Official Horizon runtime and Launch API | Implemented for Linux x86_64 and Windows x64 |
| Horizon Offline fork | Implemented as a separate runtime; requires local Minecraft 26.3 + Java 25 |
| Microsoft browser sign-in (official client by default) and token refresh | Implemented |
| Modrinth Java mod catalogue and installation | Implemented for Fabric, Forge, NeoForge and Quilt; version/loader filtered |
| CurseForge Java mod catalogue and installation | Implemented; API key is kept in the OS credential store |
| Required Modrinth mod dependencies, cached icons and file hash verification | Implemented |
| Modrinth .mrpack discovery and isolated Java-profile installation | Implemented; loader build and file hashes are pinned/verified |
| Horizon Registry content-HMOD catalogue, SHA-256 and manifest checks | Implemented for unsigned Minecraft 26.3 content mods |
| Per-instance mod action on non-Vanilla Java and Horizon builds | Implemented |
| Native Horizon mod builds, signed HMOD verification and WebAssembly HMOD support | Planned |
| CurseForge modpack ZIP import, Horizon modpack conversion and modpack updates | Planned |
| Automatic mod updates and detailed download queue/progress UI | Planned |
| Download queue, detailed progress and logs UI | Planned |
| Installers and distribution packages | Planned |

## Third-party components

Offline Java skin rendering uses the separate [OfflineSkinInjector](https://github.com/NaivG/OfflineSkinInjector) project (GPL-3.0). GlowstoneCL downloads its unmodified upstream release JAR at runtime and verifies its SHA-256; the JAR includes the GPL-3.0 license. See the upstream repository for its source and compatibility table. Horizon Offline uses its own built-in local-skin interface and does not need this agent.

## Licensing

GlowstoneCL source code is licensed under MIT. Third-party libraries, game files, mod files, APIs and trademarks remain subject to their respective owners' terms and licenses.
