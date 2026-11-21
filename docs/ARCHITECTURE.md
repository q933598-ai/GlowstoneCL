# Architecture

## Runtime separation

GlowstoneCL uses one desktop application to manage Minecraft Java and two separate Horizon runtimes.
They share account selection, instance metadata, storage roots and launcher UI,
but they do not share a mod execution model.

## Minecraft Java Edition

The Java backend uses **CrustCore** for the official Mojang installation and
launch pipeline:

- fetches Mojang version metadata;
- downloads and verifies version JSON, client files, libraries and assets;
- installs the matching Mojang Java runtime when needed;
- installs supported loader profiles: Vanilla, Fabric, Quilt, Forge and NeoForge;
- builds and starts the Java process with the selected account and instance directory.

The Modrinth integration (`mods.rs`) is limited to Java instances with Fabric,
Forge, NeoForge or Quilt. Search facets bind results to the selected Minecraft
version and loader. Downloads are kept in that instance's `mods/` directory;
GlowstoneCL stores a manifest of files it manages and leaves unrelated user JARs
untouched. Required project/version dependencies are resolved before the root
mod is installed. Every downloaded JAR must have a SHA-1 or SHA-512 hash from
Modrinth and pass verification before it is renamed into place. The installer
uses Modrinth's HTTPS CDN and enforces filename and file-size limits.

Vanilla Java has no mod loader, so the catalogue rejects it. Modrinth Java JARs
are not installed into Horizon or Horizon Offline: those runtimes use their own
HMOD/HNMOD content formats.

The selected instance gets its own game folder. Offline accounts work only in
offline-mode play; Microsoft accounts are refreshed before launch.

## Horizon

The official Horizon backend downloads the runtime from the
[horizon-rust/horizon releases](https://github.com/horizon-rust/horizon/releases).
Before use it verifies the release's version-bound Ed25519 signature and the
archive SHA-256, then asks the installed binaries for their Launch API rather
than assuming a fixed command-line interface.

Horizon does not contain the official Minecraft game files. GlowstoneCL uses
CrustCore and the signed-in owner's account to fetch the required Mojang files
and Java runtime, then runs Horizon's resource preparer. The client receives
its Microsoft session through standard input (`--session-stdin`), not process
arguments.

The current Horizon binary assets selected by GlowstoneCL are:

- Linux x86_64;
- Windows x64.

Other architectures and operating systems are not yet enabled by this
backend. Horizon is a separate runtime: Java JAR mods are not directly
compatible with it, and its content/native mod packages must follow the
format and API of the selected Horizon build.

## Horizon Offline

Horizon Offline is a separate, unofficial fork from
[gerdaroot/Horizon-Offline-Launcher](https://github.com/gerdaroot/Horizon-Offline-Launcher).
GlowstoneCL downloads it from its own release feed, verifies it with the fork's
own Ed25519 key and checksum, and stores it under a separate runtime directory.
It is never substituted for the official Horizon binary.

The fork currently pins **Minecraft 26.3 and Java 25**. It requires an existing
local Minecraft installation: GlowstoneCL searches the standard `.minecraft`,
Prism Launcher and MultiMC directories, or the directory supplied through
`GLOWSTONE_MINECRAFT_DIR`. Prism/MultiMC data is imported through the fork's
`--prism-dir` preparer option. The offline path does not download the
Minecraft client jar for a local offline account; it prepares the fork from
the player's installed game files.

The local offline nickname is written to `horizon-player.txt` in the
instance's game directory. No Microsoft token is sent to the offline fork.
This fork is for local/offline play and does not provide official online-mode
authentication or access to online services.

## Microsoft and offline accounts

Microsoft sign-in uses Device Code Flow, Xbox Live, XSTS and Minecraft
Services. GlowstoneCL checks the Java entitlement (including supported PC
Game Pass products) and retrieves the Minecraft profile before saving it.
Refresh tokens are stored in the operating system credential store. Short-lived
access tokens stay in memory and are refreshed before launch through CrustCore.

Offline profiles use deterministic offline UUIDs. They can be used with Java
offline-mode play and with the separate Horizon Offline fork; the official
Horizon runtime still requires a Microsoft/Xbox-backed Minecraft session.

## Current implementation boundary

The desktop crate currently contains the runtime integrations:

- `auth.rs` — Microsoft browser Device Code Flow through CrustCore, profile checks and CrustCore account construction;
- `game.rs` — CrustCore Java installer/launcher, signed official Horizon release selection, and separate Horizon Offline fork integration;
- `mods.rs` — Modrinth search, compatibility selection, required-dependency resolution, hash-verified JAR installation and manifest-scoped removal;
- `store.rs` — instance metadata, per-instance workspaces, OS credential storage, and per-account skin collections;
- `skin_preview.rs` — textured 3D player preview built from the Minecraft skin atlas on Iced Canvas;
- `ui.rs` — desktop controls for authentication, launching, and the multi-account skin wardrobe.

## Skin wardrobe

Skin files are stored under `skins/<profile-uuid>/` in the GlowstoneCL data directory, with a per-profile `skins.json` collection. Import validates PNG decoding, a 2 MiB file limit, and Minecraft skin dimensions (64×64 or legacy 64×32). The desktop renders the selected texture on a rotatable 3D player model assembled from the skin atlas. For Microsoft accounts, the wardrobe can download the current active profile skin and upload a selected PNG to the authenticated Minecraft Services profile using the multipart `variant` and `file` fields. Refresh tokens remain in the OS credential store; short-lived access tokens are only held in memory. For offline Java profiles, GlowstoneCL launches the ordinary game with the standalone [OfflineSkinInjector](https://github.com/NaivG/OfflineSkinInjector) Java agent. The upstream 0.1.2 JAR is downloaded into GlowstoneCL's tools directory on demand; the launcher verifies its pinned byte size and SHA-256 before use. The agent receives the selected PNG, player UUID/name, and Steve/Alex model through JVM properties and hooks authlib at runtime. It does not install a Fabric/Forge/Quilt mod or add anything to the instance's `mods/` directory, and it works independently of the selected loader when the game's authlib version is supported. OfflineSkinInjector is GPL-3.0; GlowstoneCL uses the unmodified upstream release and the JAR contains its license. Very old versions without a supported authlib hook (1.6.2 and older) are not supported. Its skin override is client-side only; other players will not automatically receive the texture. Horizon Offline does not need a Java mod or agent: its client accepts `HORIZON_OFFLINE_SKIN` and `HORIZON_OFFLINE_SKIN_MODEL`, which GlowstoneCL sets from the active local skin when starting the offline runtime.

The following integrations remain future work:

- CurseForge catalogue and API integration;
- full modpack import/export and one-click Modrinth `.mrpack` installation;
- automatic updates for installed mods;
- detailed download queue, progress and cancellation UI;
- log and crash-report browser;
- instance import/export and packaged GlowstoneCL installers.

Potential future crates (`glowstone-java`, `glowstone-horizon`,
`glowstone-auth`, `glowstone-modrinth`, `glowstone-curseforge` and
`glowstone-downloads`) are refactoring boundaries, not currently separate
workspace crates.

## Security principles

- Keep Microsoft refresh tokens in the OS credential store, not in plaintext
  configuration files.
- Never put access tokens in process arguments.
- Verify downloaded Horizon artifacts against the version-bound release
  signature and checksums before extracting or running them.
- Validate archive paths before extracting modpacks to prevent path traversal.
- Never bypass catalogue API keys, CDN restrictions or project permissions.
