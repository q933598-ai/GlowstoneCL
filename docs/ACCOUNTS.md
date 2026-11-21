# Accounts and Microsoft sign-in

GlowstoneCL supports two account types:

- **Offline profile** — a local Java profile with a deterministic offline UUID. It is not a Microsoft login and cannot authenticate to online-mode servers.
- **Microsoft account** — Device Code Flow followed by Xbox Live, XSTS and Minecraft Services authentication. GlowstoneCL verifies that a Minecraft profile is returned before adding the account.

## Register a Microsoft public client

A Microsoft application Client ID is required. The Client ID is public configuration and is not a secret. Do not create or distribute a Client Secret for this desktop app.

1. Open the [Microsoft Entra app registrations](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade).
2. Register a new application.
3. For account types, choose personal Microsoft accounts (or the equivalent option that includes personal Microsoft accounts).
4. Copy the **Application (client) ID**.
5. Open **Authentication** and enable **Allow public client flows** under Advanced settings.
6. Save the application.
7. Paste the Client ID into GlowstoneCL → Accounts → Microsoft account, then save it.

GlowstoneCL uses the Microsoft consumers authority and requests the XboxLive.signin and offline_access scopes. The Device Code Flow is designed for public-client apps and allows the user to approve the login in a browser.

## Sign in

1. Open GlowstoneCL → Accounts.
2. Enter and save the Application (client) ID.
3. Click **Войти через Microsoft**.
4. Open the displayed verification page and enter the device code.
5. After approving the login, return to GlowstoneCL and click **Я подтвердил вход**.
6. GlowstoneCL exchanges the Microsoft token through Xbox Live and XSTS, then requests the Minecraft Java profile.
7. The account is added only after Minecraft Services returns a profile.

The Microsoft refresh token is written to the platform credential store (Secret Service on Linux or Windows Credential Manager). Account display data is stored in the GlowstoneCL state file. If the platform credential store is unavailable, the profile can still appear in the account list, but the UI warns that a future sign-in may be needed.

## Common issues

- **No Client ID:** create a public-client app registration and paste its Application (client) ID into GlowstoneCL.
- **invalid_client / unauthorized_client:** verify the ID and that public client flows are enabled.
- **Xbox Live / XSTS error:** verify that the Microsoft account can use Xbox services and that any Xbox family or age restrictions are satisfied.
- **Minecraft profile not found:** the account may not own Minecraft Java Edition, or may not have a Java profile.
- **HTTP 403 from Minecraft Services:** Minecraft Services may reject an application registration that has not been granted the required access. Check the relevant Minecraft Services access requirements for your registration.

## Security

- No Microsoft Client Secret is embedded in GlowstoneCL.
- OAuth device codes and access tokens are held in memory during the sign-in operation.
- Refresh tokens are not written into state.json; they use the operating system credential store.
- Deleting a Microsoft account from GlowstoneCL also requests removal of its refresh token from the credential store.
