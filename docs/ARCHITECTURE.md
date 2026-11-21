# Architecture

## Runtime separation

GlowstoneCL treats Minecraft Java and Horizon as different game platforms.
They may share the launcher UI, instance metadata, storage primitives and
catalog presentation, but they do not share a mod execution model.

### Java Edition

- Vanilla
- Forge
- NeoForge
- Fabric
- Quilt

Java loader installation and JVM launch belong to the Java runtime module.

### Horizon

- Horizon content mods (.hmod)
- Horizon native Rust mods (.hnmod)
- Horizon-specific registries, resources and runtime versions

A Java JAR mod must never be shown as directly compatible with Horizon.
Cross-platform import must use explicit compatibility metadata:

- compatible — installs as-is;
- partial — only supported assets/data can be imported;
- unsupported — cannot be used by this runtime.

## Planned crates

The current workspace contains the desktop shell and shared domain models.
As features are implemented, add focused crates:

- glowstone-java — Mojang manifests, game files, libraries/assets and JVM launch;
- glowstone-horizon — Horizon runtime and native mod management;
- glowstone-auth — Microsoft and offline profiles;
- glowstone-modrinth — Modrinth API client;
- glowstone-curseforge — CurseForge API client;
- glowstone-downloads — concurrent downloads, cache and integrity checks;
- glowstone-storage — configuration, SQLite metadata and instance paths.

## Security principles

- Keep Microsoft refresh/access tokens in the OS credential store, not in
  plaintext configuration files.
- Never bypass catalog API keys, CDN restrictions or project permissions.
- Validate archive paths before extracting modpacks to prevent path traversal.
- Verify downloaded artifacts using trusted hashes where provided.
