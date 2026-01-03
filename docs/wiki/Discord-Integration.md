# Discord Integration Setup

ClovaLink can send DM notifications to users via Discord when files are shared, uploaded, or when other important events occur.

## Overview

- **Cost**: Free (Discord API has no charges)
- **Setup Time**: ~2 minutes
- **Requirements**: A Discord account

## Step 1: Create a Discord Application

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications)
2. Click **"New Application"**
3. Name it something like "ClovaLink" or "YourCompany File Notifications"
4. Click **Create**

## Step 2: Get Your Credentials

Once your application is created:

1. You're now on the **General Information** page
2. Copy the **Application ID** (also called Client ID)
3. Go to **OAuth2** in the left sidebar
4. Copy the **Client Secret** (click "Reset Secret" if you don't see one)

> ⚠️ **Keep your Client Secret private!** Never commit it to git or share it publicly.

## Step 3: Configure Redirect URL

Still in the OAuth2 settings:

1. Scroll down to **Redirects**
2. Click **Add Redirect**
3. Enter your ClovaLink callback URL:
   - For local development: `http://localhost:3000/api/discord/callback`
   - For production: `https://your-domain.com/api/discord/callback`
4. Click **Save Changes**

## Step 4: Add Environment Variables

Add these to your `.env` file (or your deployment's environment configuration):

```env
DISCORD_CLIENT_ID=your_application_id_here
DISCORD_CLIENT_SECRET=your_client_secret_here
DISCORD_REDIRECT_URI=https://your-domain.com/api/discord/callback
```

## Step 5: Enable Discord for Your Tenant

1. Log in to ClovaLink as an Admin
2. Go to **Company Details** → **Discord** tab
3. Toggle **Enable Discord Integration** to ON
4. Save

## Step 6: Users Connect Their Accounts

Once enabled, users can connect their Discord accounts:

1. Go to **Profile** (click your avatar → Profile)
2. Scroll to **Connected Accounts**
3. Click **Connect Discord**
4. Authorize the application in Discord
5. Configure notification preferences

## What Notifications Are Sent?

Users can choose to receive DMs for:

| Event | Description | Status |
|-------|-------------|--------|
| 📁 **File Uploads** | When someone uploads to their file request | ✅ Available |
| 📨 **File Requests** | When they receive a new file request | ✅ Available |
| 🔗 **File Shared** | When someone shares a file with them | 🚧 Coming soon |
| 💬 **Comments** | When someone comments on their files | 🚧 Coming soon |

## Troubleshooting

### "Discord integration is not configured"
Make sure all three environment variables are set:
- `DISCORD_CLIENT_ID`
- `DISCORD_CLIENT_SECRET`
- `DISCORD_REDIRECT_URI`

### "Invalid redirect URI"
The redirect URI in your `.env` must **exactly match** what you configured in the Discord Developer Portal (including http vs https).

### Users can't connect
1. Check that Discord is enabled for your tenant (Company Details → Discord)
2. Verify the redirect URI is correct
3. Check backend logs for specific errors

## Security Notes

- **Client Secret**: Keep this private. It should only exist in your server environment, never in frontend code or public repositories.
- **Tokens**: User Discord tokens are stored encrypted in the database.
- **Permissions**: The integration only requests the `identify` scope - it can only read basic profile info and send DMs, not access servers or other data.

## Disabling Discord

To disable Discord integration:

1. **Per-tenant**: Go to Company Details → Discord → Toggle OFF
2. **System-wide**: Remove the `DISCORD_CLIENT_ID` environment variable

Users who have connected their accounts will no longer receive notifications, but their connection data is preserved in case you re-enable it.

