# Single Sign-On (SSO) Authentication

This guide covers configuring and managing OIDC and SAML 2.0 Single Sign-On for ClovaLink tenants. SSO enables enterprise users to authenticate using their organization's identity provider (IdP), with support for auto-provisioning, attribute mapping, account linking, and MFA trust policies.

> **Added in v0.1.4** | **Required role:** SuperAdmin (provider management), Admin (enable/disable toggles)

---

## Table of Contents

- [Overview](#overview)
- [Supported Providers](#supported-providers)
- [Setting Up OIDC Providers](#setting-up-oidc-providers)
- [Setting Up SAML Providers](#setting-up-saml-providers)
- [Attribute and Claim Mapping](#attribute-and-claim-mapping)
- [Auto-Provisioning](#auto-provisioning)
- [Account Linking](#account-linking)
- [MFA Trust Settings](#mfa-trust-settings)
- [API Endpoints Reference](#api-endpoints-reference)
- [Security Considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)

---

## Overview

ClovaLink supports two industry-standard SSO protocols:

| Protocol | Standard | Use Case |
|----------|----------|----------|
| **OIDC** | OpenID Connect 1.0 | Modern cloud IdPs (Google, Microsoft, Okta) |
| **SAML** | SAML 2.0 | Enterprise IdPs (ADFS, Azure AD, Okta, Shibboleth) |

Both protocols share the same provisioning, attribute mapping, and account linking infrastructure. A tenant can have multiple providers of either type configured simultaneously.

### How SSO Login Works

1. The user enters their email on the ClovaLink login page.
2. ClovaLink checks the email domain against configured providers (`email_domains` field).
3. Matching SSO provider buttons appear automatically on the login page.
4. The user clicks an SSO button and is redirected to their IdP.
5. After authenticating with the IdP, the user is redirected back to ClovaLink.
6. ClovaLink resolves the user by identity lookup, email match, or auto-provisioning.
7. A session is created and the user is logged in.

### User Identity Types

The `identity_provider` column on the user record tracks how a user authenticates:

| Value | Description |
|-------|-------------|
| `local` | Password-only authentication (default for all existing users) |
| `oidc` | OIDC SSO-only, no password set |
| `saml` | SAML SSO-only, no password set |
| `hybrid` | Both password and SSO enabled |

### Tenant Auth Methods

The `auth_methods` array on the tenant record controls which authentication methods are available:

```
auth_methods = ['local']          -- Password only (default)
auth_methods = ['local', 'oidc']  -- Password + OIDC
auth_methods = ['local', 'saml']  -- Password + SAML
auth_methods = ['local', 'oidc', 'saml']  -- All methods
```

Auth methods are updated automatically when providers are created or deleted.

---

## Supported Providers

### OIDC Providers

| Provider | `provider_type` | Issuer URL |
|----------|----------------|------------|
| Google Workspace | `google` | `https://accounts.google.com` |
| Microsoft Entra ID | `microsoft` | `https://login.microsoftonline.com/{tenant-id}/v2.0` |
| Okta | `okta` | `https://{your-domain}.okta.com` |
| Auth0 | `auth0` | `https://{your-domain}.auth0.com` |
| Generic OIDC | `generic` | Any compliant OIDC issuer URL |

### SAML Providers

| Provider | `provider_type` | Notes |
|----------|----------------|-------|
| Microsoft ADFS | `adfs` | On-premises Active Directory Federation Services |
| Microsoft Entra ID (Azure AD) | `azure_ad` | Cloud-based Azure AD SAML apps |
| Okta | `okta` | Okta SAML 2.0 application |
| OneLogin | `onelogin` | OneLogin SAML connector |
| Generic SAML 2.0 | `generic` | Any SAML 2.0 compliant IdP |

---

## Setting Up OIDC Providers

### Prerequisites

- SuperAdmin role in ClovaLink
- Admin access to your identity provider
- The `SECRETS_ENCRYPTION_KEY` environment variable set on the backend (used to encrypt client secrets at rest)

### Step 1: Register ClovaLink in Your IdP

Create an OAuth 2.0 / OpenID Connect application in your IdP with the following settings:

| Setting | Value |
|---------|-------|
| Application type | Web application |
| Redirect URI | `{BASE_URL}/api/auth/oidc/callback` |
| Scopes | `openid email profile` (minimum) |

Record the **Client ID** and **Client Secret** from your IdP.

#### Google Workspace

1. Go to [Google Cloud Console](https://console.cloud.google.com/) > APIs & Services > Credentials.
2. Create an OAuth 2.0 Client ID (Web application type).
3. Add the redirect URI: `https://your-clovalink-domain.com/api/auth/oidc/callback`.
4. Note the Client ID and Client Secret.

#### Microsoft Entra ID

1. Go to [Azure Portal](https://portal.azure.com/) > Microsoft Entra ID > App registrations.
2. Register a new application (Web platform).
3. Add the redirect URI: `https://your-clovalink-domain.com/api/auth/oidc/callback`.
4. Create a Client Secret under Certificates & secrets.
5. Note the Application (client) ID, Directory (tenant) ID, and client secret value.

#### Okta

1. Go to Okta Admin Console > Applications > Create App Integration.
2. Select OIDC - OpenID Connect, then Web Application.
3. Set the sign-in redirect URI: `https://your-clovalink-domain.com/api/auth/oidc/callback`.
4. Note the Client ID and Client Secret.

### Step 2: Configure the Provider in ClovaLink

Navigate to **Settings > SSO > OIDC Providers** and click **Add Provider**.

| Field | Description | Required |
|-------|-------------|----------|
| Name | Display name (e.g., "Google Workspace") | Yes |
| Slug | URL-safe identifier (e.g., `google`) | Yes |
| Provider Type | `google`, `microsoft`, `okta`, or `generic` | No (defaults to `generic`) |
| Issuer URL | The OIDC issuer URL from your IdP | Yes |
| Client ID | OAuth client ID from Step 1 | Yes |
| Client Secret | OAuth client secret from Step 1 | Yes |
| Scopes | Space-separated scopes | No (defaults to `openid email profile`) |
| Email Domains | Domains for login discovery (e.g., `["example.com"]`) | No |
| Auto-Provision | Create new users on first SSO login | No (defaults to `false`) |
| Default Role | Role assigned to auto-provisioned users | No (defaults to `Employee`) |
| Default Department | Department assigned to auto-provisioned users | No |
| Trust IdP MFA | Skip ClovaLink 2FA if IdP performed MFA | No (defaults to `true`) |

### Step 3: Test and Enable

1. Click **Test Connection** on the provider card to verify OIDC discovery and connectivity.
2. Toggle the provider to **Enabled** once the test passes.
3. Open an incognito window and verify the SSO button appears on the login page for an email address matching your configured domain.

---

## Setting Up SAML Providers

### Prerequisites

- SuperAdmin role in ClovaLink
- Admin access to your SAML identity provider
- The IdP's SSO URL, Entity ID, and signing certificate (X.509, PEM format)

### Step 1: Get ClovaLink SP Metadata

ClovaLink exposes a Service Provider (SP) metadata endpoint for each SAML provider. After creating the provider (Step 2), you can retrieve the metadata at:

```
GET /api/auth/saml/metadata/{provider_id}
```

This returns a SAML metadata XML document that you can import directly into most IdPs. The metadata includes:

- SP Entity ID
- Assertion Consumer Service (ACS) URL: `{BASE_URL}/api/auth/saml/acs`
- NameID format
- SP signing certificate (if request signing is enabled)

### Step 2: Configure the Provider in ClovaLink

Navigate to **Settings > SSO > SAML Providers** and click **Add Provider**.

| Field | Description | Required |
|-------|-------------|----------|
| Name | Display name (e.g., "Corporate ADFS") | Yes |
| Slug | URL-safe identifier (e.g., `adfs`) | Yes |
| Provider Type | `adfs`, `azure_ad`, `okta`, or `generic` | No (defaults to `generic`) |
| IdP Entity ID | The entity ID from your IdP metadata | Yes |
| IdP SSO URL | The IdP's single sign-on URL | Yes |
| IdP SLO URL | Single logout URL (optional) | No |
| IdP Signing Certificate | X.509 certificate (PEM) for verifying assertions | Yes |
| IdP Metadata URL | URL to fetch IdP metadata (for reference) | No |
| NameID Format | SAML NameID format | No (defaults to `emailAddress`) |
| SSO Binding | `HTTP-POST` or `HTTP-Redirect` | No (defaults to `HTTP-POST`) |
| Attribute: Email | SAML attribute name for email | No (defaults to `email`) |
| Attribute: Name | SAML attribute name for display name | No (defaults to `displayName`) |
| Request Signing | Sign SAML AuthnRequests | No (defaults to `false`) |
| Want Assertions Signed | Require signed assertions | No (defaults to `true`) |
| Want Response Signed | Require signed SAML response | No (defaults to `true`) |
| Email Domains | Domains for login discovery | No |
| Auto-Provision | Create new users on first SSO login | No (defaults to `false`) |
| Default Role | Role for auto-provisioned users | No (defaults to `Employee`) |
| Default Custom Role | Custom role for auto-provisioned users | No |
| Default Department | Department for auto-provisioned users | No |
| Trust IdP MFA | Skip ClovaLink 2FA if IdP performed MFA | No (defaults to `true`) |

### Step 3: Configure Your IdP

Import the ClovaLink SP metadata (from Step 1) into your IdP, or manually configure:

| IdP Setting | Value |
|-------------|-------|
| ACS URL | `{BASE_URL}/api/auth/saml/acs` |
| SP Entity ID | As shown in provider details (auto-generated) |
| NameID Format | `urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress` |
| Attributes | Map `email` and `displayName` (or configure custom attribute names) |

#### Microsoft ADFS

1. Open ADFS Management > Relying Party Trusts > Add Relying Party Trust.
2. Import the ClovaLink SP metadata URL or XML.
3. Add claim rules to map LDAP attributes to outgoing claims (`E-Mail Address`, `Display-Name`).

#### Microsoft Entra ID (Azure AD)

1. Go to Azure Portal > Enterprise Applications > New Application > Create your own.
2. Set up SAML-based sign-on.
3. Enter the ACS URL and Entity ID from ClovaLink SP metadata.
4. Configure attribute mappings (email, displayName).
5. Download the SAML Signing Certificate (Base64) and Federation Metadata XML.

#### Okta

1. Go to Okta Admin > Applications > Create App Integration > SAML 2.0.
2. Set the Single sign-on URL to the ACS URL.
3. Set the Audience URI to the SP Entity ID.
4. Configure attribute statements for `email` and `displayName`.

### Step 4: Test and Enable

1. Click **Test Connection** on the provider card.
2. Toggle the provider to **Enabled**.
3. Verify the login flow in an incognito window with a matching email domain.

---

## Attribute and Claim Mapping

Attribute mappings let you automatically assign ClovaLink roles and departments based on attributes (claims) returned by the IdP. This works identically for both OIDC claims and SAML attribute statements.

### How It Works

1. When a user authenticates via SSO, ClovaLink receives attributes from the IdP.
2. Attribute mappings are evaluated in **priority order** (highest priority first).
3. The **first matching rule** determines the user's role and department.
4. If no rules match, the provider's default role and department are used.

### Creating Mappings

Navigate to **Settings > SSO > Attribute Mappings** tab. Click **Add Mapping** for the desired provider.

| Field | Description | Required |
|-------|-------------|----------|
| Attribute Name | The IdP attribute/claim name to match (e.g., `groups`, `role`, `department`) | Yes |
| Attribute Value | The value to match against | Yes |
| Match Type | How to compare the value | No (defaults to `exact`) |
| Target Role | ClovaLink base role to assign | Yes |
| Target Custom Role | ClovaLink custom role to assign | No |
| Target Department | ClovaLink department to assign | No |
| Priority | Evaluation order (higher = evaluated first) | No (defaults to `0`) |

### Match Types

| Match Type | Behavior | Example |
|------------|----------|---------|
| `exact` | Value must exactly equal the attribute value | `groups` = `Engineering` |
| `contains` | Attribute value must contain the specified string | `groups` contains `Admin` |
| `regex` | Attribute value must match the regular expression | `email` matches `.*@engineering\.example\.com` |

### Example Configuration

Map IdP groups to ClovaLink roles:

| Priority | Attribute | Value | Match | Target Role | Target Department |
|----------|-----------|-------|-------|-------------|-------------------|
| 100 | `groups` | `IT-Admins` | exact | Admin | IT |
| 50 | `groups` | `Engineering` | exact | Manager | Engineering |
| 10 | `department` | `HR` | exact | Employee | Human Resources |
| 0 | *(default)* | | | Employee | *(none)* |

### Common IdP Attribute Names

| IdP | Groups Attribute | Department Attribute |
|-----|-----------------|---------------------|
| Google Workspace | `groups` (requires Directory API) | `department` |
| Microsoft Entra ID | `groups` (Object IDs) or `roles` | `department` |
| Okta | `groups` | `department` |
| ADFS | `http://schemas.xmlsoap.org/claims/Group` | Custom claim |

---

## Auto-Provisioning

When **Auto-Provision** is enabled on a provider, ClovaLink automatically creates a new user account when someone authenticates via SSO for the first time and no matching account exists.

### Provisioning Flow

1. User authenticates with the IdP and is redirected back to ClovaLink.
2. ClovaLink checks for an existing linked identity (by subject/NameID).
3. If not found, ClovaLink checks for an existing user with the same email address.
4. If no existing user is found:
   - **Auto-provision enabled:** A new user is created with the IdP-provided email and name.
   - **Auto-provision disabled:** The login is rejected with an error message.

### Default Assignments

Auto-provisioned users receive:

| Setting | Source |
|---------|--------|
| Base Role | Provider's `default_role` (default: `Employee`) |
| Custom Role | Provider's `default_custom_role_id` (optional) |
| Department | Provider's `default_department_id` (optional) |
| Identity Provider | `oidc` or `saml` (SSO-only, no password) |

These defaults can be overridden by [attribute mappings](#attribute-and-claim-mapping) if configured.

### Enabling Auto-Provisioning

1. Navigate to **Settings > SSO** and select the provider.
2. Toggle **Auto-Provision** to enabled.
3. Set the **Default Role** and optionally a **Default Department**.
4. Consider adding attribute mappings for more granular role assignment.

> **Security note:** Auto-provisioning creates accounts for anyone who can authenticate with the IdP. Ensure your IdP application assignment restricts access to authorized users only.

---

## Account Linking

Account linking allows existing ClovaLink users to connect their SSO identity to their local account, enabling them to sign in with either password or SSO.

### Linking an SSO Identity

1. The user navigates to their **Profile > Linked Accounts** section.
2. Available SSO providers for the tenant are listed with a **Link** button.
3. Clicking Link redirects to the IdP for authentication.
4. After authenticating, the SSO identity is linked to the user's ClovaLink account.
5. The user's `identity_provider` is updated to `hybrid`.

### Unlinking an SSO Identity

1. Navigate to **Profile > Linked Accounts**.
2. Click **Unlink** next to the identity to remove.

> **Safety check:** ClovaLink prevents unlinking if it would lock the user out of their account. If the user has no password set and only one linked SSO identity, the unlink operation is blocked.

### Linked Accounts Display

The Profile page shows all linked identities with:
- Protocol badge (OIDC or SAML)
- Provider name
- Email from the IdP
- Last login timestamp
- Total login count

---

## MFA Trust Settings

The **Trust IdP MFA** setting controls whether ClovaLink requires its own two-factor authentication (2FA) when the user has already authenticated through an IdP that may have performed MFA.

| Setting | Behavior |
|---------|----------|
| `trust_idp_mfa = true` (default) | If the user has ClovaLink 2FA enabled, it is **skipped** for SSO logins. The IdP is trusted to handle MFA. |
| `trust_idp_mfa = false` | ClovaLink 2FA is **always required** for SSO logins if the user has it enabled, regardless of IdP MFA. |

### When to Disable IdP MFA Trust

- Your organization requires defense-in-depth with MFA at both IdP and application layers.
- The IdP does not enforce MFA and you want ClovaLink to fill that gap.
- Compliance requirements mandate application-level MFA regardless of upstream authentication.

### Configuration

Set per provider in **Settings > SSO** under the provider's configuration form.

---

## API Endpoints Reference

### Public Endpoints (Unauthenticated)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/auth/oidc/providers?email={email}` | Discover OIDC providers by email domain |
| `GET` | `/api/auth/oidc/authorize/{provider_id}` | Start OIDC login flow (redirects to IdP) |
| `GET` | `/api/auth/oidc/callback` | OIDC callback (handles code exchange) |
| `GET` | `/api/auth/saml/metadata/{provider_id}` | Download SP metadata XML |
| `GET` | `/api/auth/saml/authorize/{provider_id}` | Start SAML login flow (redirects to IdP) |
| `POST` | `/api/auth/saml/acs` | SAML Assertion Consumer Service endpoint |

### Provider Management (SuperAdmin)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/oidc/providers` | List all OIDC providers for the tenant |
| `POST` | `/api/oidc/providers` | Create a new OIDC provider |
| `PUT` | `/api/oidc/providers/{id}` | Update an OIDC provider |
| `DELETE` | `/api/oidc/providers/{id}` | Delete an OIDC provider |
| `POST` | `/api/oidc/providers/{id}/test` | Test OIDC provider connectivity |
| `GET` | `/api/saml/providers` | List all SAML providers for the tenant |
| `POST` | `/api/saml/providers` | Create a new SAML provider |
| `PUT` | `/api/saml/providers/{id}` | Update a SAML provider |
| `DELETE` | `/api/saml/providers/{id}` | Delete a SAML provider |
| `POST` | `/api/saml/providers/{id}/test` | Test SAML provider connectivity |

### Account Linking (Authenticated User)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/auth/oidc/link/{provider_id}` | Start OIDC account linking (redirects to IdP) |
| `DELETE` | `/api/auth/oidc/unlink/{identity_id}` | Unlink an OIDC identity |
| `GET` | `/api/auth/oidc/identities` | List the current user's OIDC identities |
| `GET` | `/api/auth/saml/link/{provider_id}` | Start SAML account linking (redirects to IdP) |
| `DELETE` | `/api/auth/saml/unlink/{identity_id}` | Unlink a SAML identity |
| `GET` | `/api/auth/saml/identities` | List the current user's SAML identities |

### Attribute Mappings (SuperAdmin)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/sso/mappings/{protocol}/{provider_id}` | List mappings for a provider |
| `POST` | `/api/sso/mappings/{protocol}/{provider_id}` | Create a mapping |
| `PUT` | `/api/sso/mappings/{mapping_id}` | Update a mapping |
| `DELETE` | `/api/sso/mappings/{mapping_id}` | Delete a mapping |

> The `{protocol}` path parameter is either `oidc` or `saml`.

---

## Security Considerations

### Secrets at Rest

- OIDC client secrets and SAML SP signing keys are **encrypted at rest** in the database using the `SECRETS_ENCRYPTION_KEY` environment variable.
- Client secrets are never returned in API responses (fields are marked `#[serde(skip_serializing)]`).

### CSRF Protection

- **OIDC:** A random `state` parameter and `nonce` are generated for each login attempt, stored in the `oidc_oauth_states` table with a **10-minute expiry**. The callback validates both values and deletes the state record (one-time use).
- **SAML:** A random `RelayState` parameter and `AuthnRequest ID` are generated for each login, stored in the `saml_auth_states` table with a **10-minute expiry**. The ACS endpoint validates `InResponseTo` against the stored request ID.

### SAML Signature Verification

ClovaLink uses a **pure Rust** SAML implementation with no C dependencies:

- XML signatures are verified using **Exclusive Canonicalization (C14N)**.
- Supported algorithms: **RSA-SHA256** and **RSA-SHA1**.
- Both response-level and assertion-level signatures are verified based on provider configuration (`want_response_signed`, `want_assertions_signed`).

### SAML Replay Protection

- Assertion IDs are recorded in the `saml_consumed_assertions` table after processing.
- Duplicate assertion IDs are rejected.
- Expired assertion records are cleaned up by the backend cron system.

### SAML Time Validation

- Assertion `NotBefore` and `NotOnOrAfter` conditions are validated against server time.
- Audience restriction is validated against the configured SP Entity ID.

### Account Safety

- **Unlink protection:** Users cannot unlink their last SSO identity if they have no password set, preventing account lockout.
- **Provider deletion warning:** Deleting a provider warns about SSO-only users who would lose access.
- **SSO-only login enforcement:** Users with `identity_provider` set to `oidc` or `saml` cannot use the password login endpoint (receives `sso_required` error).
- **Password reset blocking:** SSO-only users are directed to their IdP instead of the password reset flow.

### Access Control

| Operation | Required Role |
|-----------|--------------|
| Create / update / delete providers | SuperAdmin |
| Enable / disable provider toggles | Admin |
| Create / update / delete attribute mappings | SuperAdmin |
| Link / unlink own SSO identity | Any authenticated user |

---

## Troubleshooting

### OIDC Issues

#### "Provider discovery failed" on test

- Verify the **Issuer URL** is correct and accessible from the ClovaLink backend.
- Ensure the IdP's `.well-known/openid-configuration` endpoint is reachable.
- Check for network/firewall rules blocking outbound HTTPS from the backend container.

#### "Invalid redirect URI" error from IdP

- Confirm the redirect URI registered in your IdP exactly matches `{BASE_URL}/api/auth/oidc/callback`.
- Ensure `BASE_URL` in your ClovaLink environment does not have a trailing slash.

#### "State mismatch" or "Invalid state" on callback

- The OIDC state token expires after **10 minutes**. If the user took too long to authenticate, they need to restart the flow.
- Ensure the user is not using a cached or bookmarked callback URL.

#### User not auto-provisioned

- Verify **Auto-Provision** is enabled on the provider.
- Ensure the IdP returns an `email` claim in the ID token.
- Check that the provider is **Enabled**.

### SAML Issues

#### "Signature verification failed"

- Ensure the **IdP Signing Certificate** is the correct X.509 certificate in PEM format.
- If the IdP rotated its signing certificate, update the certificate in ClovaLink.
- Verify the certificate is the signing certificate, not the encryption certificate.

#### "Assertion expired" or "NotBefore condition not met"

- Check for **clock skew** between the ClovaLink server and the IdP. Ensure NTP is configured on both systems.
- SAML assertions typically have a validity window of a few minutes.

#### "Assertion replay detected"

- The same SAML assertion was submitted more than once. This is expected if the user clicks the browser back button after login. They should start a new login flow.

#### "Audience mismatch"

- The SP Entity ID configured in ClovaLink must match the Audience Restriction in the SAML assertion. Verify both sides use the same Entity ID value.

#### IdP returns attributes with unexpected names

- Different IdPs use different attribute names. Configure the **Attribute: Email** and **Attribute: Name** fields on the SAML provider to match your IdP's attribute names.
- Common email attributes: `email`, `Email`, `emailAddress`, `http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress`, `urn:oid:0.9.2342.19200300.100.1.3`.
- Common name attributes: `displayName`, `name`, `cn`, `http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name`.

### General Issues

#### SSO button not appearing on login page

- Verify the provider is **Enabled**.
- Ensure the **Email Domains** field includes the domain portion of the user's email address.
- Check that the tenant's `auth_methods` array includes `oidc` or `saml`.

#### "SSO required" error on password login

- The user's account is set to SSO-only (`identity_provider` = `oidc` or `saml`). They must use the SSO login flow.
- If the user needs password access, a SuperAdmin can update their `identity_provider` to `hybrid` and set a password.

#### Cannot unlink SSO identity

- The user has no password and this is their only linked identity. Set a password first via admin action, then unlink.

#### `SECRETS_ENCRYPTION_KEY` not set

- SSO provider creation will fail without this environment variable.
- Generate a key: `openssl rand -base64 32` and add it to your `.env` file.
- This key is used to encrypt client secrets and SAML signing keys at rest in the database.
