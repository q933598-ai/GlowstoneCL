# Accounts and Microsoft sign-in

GlowstoneCL supports two account types:

- **Offline profile** — a local Java profile with a deterministic offline UUID. It can play offline-mode worlds and servers, but it does not authenticate to online-mode servers.
- **Microsoft account** — Microsoft Device Code Flow, followed by Xbox Live, XSTS and Minecraft Services. GlowstoneCL checks the Minecraft Java entitlement (including supported PC Game Pass products) and retrieves the Minecraft profile before adding the account.

Horizon requires a Microsoft/Xbox-backed Minecraft session. Offline profiles can be used with Minecraft Java only.

## Register a Microsoft public client

A Microsoft Application (client) ID is public configuration, not a secret. Do not create, embed or distribute a Client Secret for this desktop app.

1. Open [Microsoft Entra app registrations](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade).
2. Register an application that supports personal Microsoft accounts.
3. Copy the **Application (client) ID**.
4. Open **Authentication** and enable **Allow public client flows** under Advanced settings.
5. Save the application.
6. Paste the Client ID into GlowstoneCL → Accounts → Microsoft account and save it.
7. If Minecraft Services returns HTTP 403 or rejects the app registration, submit the Client ID for Minecraft Services approval using the [Minecraft application review form](https://aka.ms/mce-reviewappid). Approval is separate from creating the Entra application.

GlowstoneCL uses the Microsoft consumers authority and requests `XboxLive.signin` and `offline_access`. Device Code Flow allows the user to approve sign-in in a browser without embedding a client secret.

## Sign in

1. Open GlowstoneCL → Accounts.
2. Enter and save the Application (client) ID.
3. Click **Войти через Microsoft**.
4. GlowstoneCL opens the Microsoft verification page in your browser and displays the device code. Enter the code and sign in.
5. Return to GlowstoneCL. Polling is already running in the background; the account is added automatically after Microsoft approves the session. The browser can be reopened from the sign-in panel if necessary.
6. GlowstoneCL exchanges the Microsoft token through Xbox Live and XSTS, checks supported Java entitlement products, and retrieves the Minecraft Java profile.
7. The account is added only after authentication and profile checks succeed, and becomes the selected account.

## Session lifecycle

- The Microsoft refresh token is stored in the platform credential store: Secret Service on Linux or Windows Credential Manager on Windows.
- The short-lived Minecraft access token is held in memory and is not written to `state.json`.
- Before starting a game, GlowstoneCL refreshes the Microsoft/Xbox/Minecraft session through CrustCore. When Microsoft rotates the refresh token, GlowstoneCL stores the new value in the credential store.
- Java receives the authenticated account through CrustCore's Java launch pipeline.
- Horizon receives the Minecraft session over the launch process's standard input (`--session-stdin`); the access token is not placed in command-line arguments.
- Deleting an account removes its stored refresh token from the credential store.

## Common issues

- **No Client ID:** create a public-client app registration and paste its Application (client) ID into GlowstoneCL.
- **invalid_client / unauthorized_client:** verify the Client ID and that public client flows are enabled.
- **HTTP 403 from Minecraft Services:** the Client ID may need Minecraft Services approval. Use the [application review form](https://aka.ms/mce-reviewappid).
- **Xbox Live / XSTS error:** verify that the account can use Xbox services and that family or age restrictions are not blocking access.
- **Minecraft profile not found:** the account may not own Minecraft Java Edition, may lack a Java profile, or may have an entitlement that the services do not report to this app.
- **Credential store unavailable:** the account profile may be visible, but the refresh token could not be saved. Restore access to Secret Service / Windows Credential Manager and sign in again.

## Security

- No Microsoft Client Secret is embedded in GlowstoneCL.
- OAuth device codes and access tokens are held in memory during authentication.
- Refresh tokens are stored through the operating system credential store, not in `state.json`.
- Horizon access tokens are sent through standard input, never command-line arguments.
