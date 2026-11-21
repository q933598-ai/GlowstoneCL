# Accounts and Microsoft sign-in

GlowstoneCL supports two account types:

- **Offline profile** — a local Java profile with a deterministic offline UUID. It can play offline-mode worlds and servers, but it does not authenticate to online-mode servers. Horizon Offline is a separate fork-specific runtime.
- **Microsoft account** — Microsoft Device Code Flow through CrustCore, followed by Xbox Live, XSTS and Minecraft Services. CrustCore retrieves the Minecraft profile and resolves ownership, including PC Game Pass.

## Browser sign-in

1. Open GlowstoneCL → Accounts.
2. Click **Войти через Microsoft**.
3. GlowstoneCL opens the official Microsoft Launcher sign-in page in your browser with the device code already filled in.
4. Sign in and approve the request in the browser. CrustCore waits for approval automatically; there is no need to return and press a confirmation button.
5. After Microsoft completes Xbox Live, XSTS and Minecraft Services authentication, GlowstoneCL adds the Minecraft profile and selects it.

By default GlowstoneCL uses CrustCore's official Minecraft Launcher public client ID. You do **not** need to register your own Microsoft Entra application for the standard sign-in.

## Optional custom Microsoft Client ID

The Accounts page allows a custom Microsoft Application (client) ID as an advanced option. Leave this field empty to use the official launcher client.

A custom ID must be a public-client app that supports personal Microsoft accounts. Do not create, embed or distribute a Client Secret for this desktop app. If Minecraft Services refuses a custom registration with HTTP 403, the ID may need separate Minecraft Services approval through the [Minecraft application review form](https://aka.ms/mce-reviewappid). Changing from device-code to browser UX cannot bypass that service-side approval.

## Session lifecycle

- The Microsoft refresh token is stored in the platform credential store: Secret Service on Linux or Windows Credential Manager on Windows.
- The short-lived Minecraft access token is held in memory and is not written to `state.json`.
- Before starting a game, GlowstoneCL refreshes the Microsoft/Xbox/Minecraft session through CrustCore. When Microsoft rotates the refresh token, GlowstoneCL stores the new value in the credential store.
- Java receives the authenticated account through CrustCore's Java launch pipeline.
- Official Horizon receives the Minecraft session over the launch process's standard input (`--session-stdin`); the access token is not placed in command-line arguments.
- Horizon Offline receives no Microsoft session; the local nickname is written to `horizon-player.txt` in the instance game directory.
- Deleting an account removes its stored refresh token from the credential store.

## Common issues

- **No Client ID:** create a public-client app registration and paste its Application (client) ID into GlowstoneCL.
- **invalid_client / unauthorized_client with a custom ID:** verify the Client ID and that public client flows are enabled. Remove the custom ID to use the official Minecraft Launcher client.
- **HTTP 403 using a custom Client ID:** the ID may need Minecraft Services approval. Use the [application review form](https://aka.ms/mce-reviewappid). Try leaving the custom Client ID field empty to use the official Minecraft Launcher client.
- **Xbox Live / XSTS error:** verify that the account can use Xbox services and that family or age restrictions are not blocking access.
- **Minecraft profile not found:** the account may not own Minecraft Java Edition, may lack a Java profile, or may have an entitlement that the services do not report to this app.
- **Credential store unavailable:** the account profile may be visible, but the refresh token could not be saved. Restore access to Secret Service / Windows Credential Manager and sign in again.

## Security

- No Microsoft Client Secret is embedded in GlowstoneCL.
- OAuth device codes and access tokens are held in memory during authentication.
- Refresh tokens are stored through the operating system credential store, not in `state.json`.
- Horizon access tokens are sent through standard input, never command-line arguments.
