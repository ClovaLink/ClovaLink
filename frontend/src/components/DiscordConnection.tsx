import { useState, useEffect, useCallback } from 'react';
import { 
  Link2, Unlink, Check, AlertCircle, Loader2, Bell, BellOff, Send, 
  FileText, MessageSquare, FolderInput, RefreshCw
} from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch } from '../context/AuthContext';

// Discord brand color
const DISCORD_COLOR = '#5865F2';

interface DiscordStatus {
  connected: boolean;
  discord_username: string | null;
  discord_avatar_url: string | null;
  dm_notifications_enabled: boolean;
  notify_file_shared: boolean;
  notify_file_uploaded: boolean;
  notify_comments: boolean;
  notify_file_requests: boolean;
}

interface TenantDiscordSettings {
  enabled: boolean;
}

interface NotificationPreference {
  key: 'notify_file_shared' | 'notify_file_uploaded' | 'notify_comments' | 'notify_file_requests';
  label: string;
  description: string;
  icon: typeof FileText;
}

const NOTIFICATION_PREFS: NotificationPreference[] = [
  {
    key: 'notify_file_shared',
    label: 'File Shared',
    description: 'When someone shares a file with you',
    icon: Link2,
  },
  {
    key: 'notify_file_uploaded',
    label: 'File Uploaded',
    description: 'When someone uploads to your file request',
    icon: FileText,
  },
  {
    key: 'notify_comments',
    label: 'Comments',
    description: 'When someone comments on your files',
    icon: MessageSquare,
  },
  {
    key: 'notify_file_requests',
    label: 'File Requests',
    description: 'When someone sends you a file request',
    icon: FolderInput,
  },
];

export function DiscordConnection() {
  const authFetch = useAuthFetch();
  
  const [tenantSettings, setTenantSettings] = useState<TenantDiscordSettings | null>(null);
  const [status, setStatus] = useState<DiscordStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [connecting, setConnecting] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testSuccess, setTestSuccess] = useState<boolean | null>(null);
  const [savingPrefs, setSavingPrefs] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    
    try {
      const [settingsRes, statusRes] = await Promise.all([
        authFetch('/api/discord/settings'),
        authFetch('/api/discord/status'),
      ]);
      
      if (settingsRes.ok) {
        setTenantSettings(await settingsRes.json());
      }
      
      if (statusRes.ok) {
        setStatus(await statusRes.json());
      }
    } catch (err) {
      setError('Failed to load Discord settings');
    } finally {
      setLoading(false);
    }
  }, [authFetch]);

  useEffect(() => {
    fetchData();
    
    // Check for OAuth callback result in URL
    const params = new URLSearchParams(window.location.search);
    const discordResult = params.get('discord');
    
    if (discordResult === 'connected') {
      setTestSuccess(true);
      // Clean up URL
      window.history.replaceState({}, '', window.location.pathname);
      // Refresh status
      fetchData();
    } else if (discordResult === 'error') {
      setError('Failed to connect Discord. Please try again.');
      window.history.replaceState({}, '', window.location.pathname);
    }
  }, [fetchData]);

  const handleConnect = () => {
    setConnecting(true);
    // Redirect to OAuth start endpoint
    window.location.href = '/api/discord/connect';
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    setError(null);
    
    try {
      const res = await authFetch('/api/discord/disconnect', { method: 'POST' });
      
      if (res.ok) {
        setStatus((prev) => prev ? { ...prev, connected: false, discord_username: null } : null);
      } else {
        const data = await res.json();
        setError(data.error || 'Failed to disconnect');
      }
    } catch (err) {
      setError('Failed to disconnect');
    } finally {
      setDisconnecting(false);
    }
  };

  const handleTest = async () => {
    setTesting(true);
    setTestSuccess(null);
    setError(null);
    
    try {
      const res = await authFetch('/api/discord/test', { method: 'POST' });
      
      if (res.ok) {
        setTestSuccess(true);
        setTimeout(() => setTestSuccess(null), 5000);
      } else {
        const data = await res.json();
        setError(data.error || 'Test failed');
        setTestSuccess(false);
      }
    } catch (err) {
      setError('Test failed');
      setTestSuccess(false);
    } finally {
      setTesting(false);
    }
  };

  const toggleMasterNotifications = async () => {
    if (!status) return;
    
    setSavingPrefs(true);
    try {
      const newValue = !status.dm_notifications_enabled;
      const res = await authFetch('/api/discord/preferences', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ dm_notifications_enabled: newValue }),
      });
      
      if (res.ok) {
        setStatus((prev) => prev ? { ...prev, dm_notifications_enabled: newValue } : null);
      }
    } finally {
      setSavingPrefs(false);
    }
  };

  const togglePreference = async (key: NotificationPreference['key']) => {
    if (!status) return;
    
    setSavingPrefs(true);
    try {
      const newValue = !status[key];
      const res = await authFetch('/api/discord/preferences', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ [key]: newValue }),
      });
      
      if (res.ok) {
        setStatus((prev) => prev ? { ...prev, [key]: newValue } : null);
      }
    } finally {
      setSavingPrefs(false);
    }
  };

  if (loading) {
    return (
      <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center justify-center py-8">
          <Loader2 className="w-6 h-6 animate-spin text-gray-400" />
        </div>
      </div>
    );
  }

  // Discord not enabled for this tenant
  if (!tenantSettings?.enabled) {
    return (
      <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div 
            className="p-2 rounded-xl"
            style={{ backgroundColor: `${DISCORD_COLOR}20` }}
          >
            <svg className="w-6 h-6" style={{ fill: DISCORD_COLOR }} viewBox="0 0 24 24">
              <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
            </svg>
          </div>
          <div>
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white">Discord</h3>
            <p className="text-sm text-gray-500 dark:text-gray-400">Receive DM notifications</p>
          </div>
        </div>
        
        <div className="flex items-center gap-2 p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
          <AlertCircle className="w-5 h-5 text-gray-400" />
          <p className="text-sm text-gray-600 dark:text-gray-400">
            Discord notifications are not enabled for your organization. Contact your administrator.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between p-6 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-3">
          <div 
            className="p-2 rounded-xl"
            style={{ backgroundColor: `${DISCORD_COLOR}20` }}
          >
            <svg className="w-6 h-6" style={{ fill: DISCORD_COLOR }} viewBox="0 0 24 24">
              <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
            </svg>
          </div>
          <div>
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white">Discord</h3>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {status?.connected 
                ? `Connected as ${status.discord_username}` 
                : 'Receive DM notifications'
              }
            </p>
          </div>
        </div>
        
        {/* Connection status badge */}
        <div className={clsx(
          'flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium',
          status?.connected 
            ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
            : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400'
        )}>
          {status?.connected ? (
            <>
              <Check className="w-4 h-4" />
              Connected
            </>
          ) : (
            <>
              <Unlink className="w-4 h-4" />
              Not Connected
            </>
          )}
        </div>
      </div>

      {/* Error message */}
      {error && (
        <div className="mx-6 mt-4 flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <AlertCircle className="w-4 h-4 text-red-500" />
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}

      {/* Content */}
      <div className="p-6 space-y-6">
        {!status?.connected ? (
          /* Not connected - Show connect button */
          <div className="text-center py-4">
            <p className="text-gray-600 dark:text-gray-400 mb-4">
              Connect your Discord account to receive direct message notifications when files are shared with you, uploaded to your requests, and more.
            </p>
            <button
              onClick={handleConnect}
              disabled={connecting}
              className="inline-flex items-center gap-2 px-6 py-3 rounded-xl text-white font-medium transition-all hover:opacity-90 disabled:opacity-50"
              style={{ backgroundColor: DISCORD_COLOR }}
            >
              {connecting ? (
                <Loader2 className="w-5 h-5 animate-spin" />
              ) : (
                <Link2 className="w-5 h-5" />
              )}
              Connect Discord
            </button>
          </div>
        ) : (
          /* Connected - Show preferences */
          <>
            {/* Master toggle */}
            <div className="flex items-center justify-between p-4 bg-gray-50 dark:bg-gray-700/30 rounded-xl">
              <div className="flex items-center gap-3">
                {status.dm_notifications_enabled ? (
                  <Bell className="w-5 h-5 text-primary-500" />
                ) : (
                  <BellOff className="w-5 h-5 text-gray-400" />
                )}
                <div>
                  <p className="font-medium text-gray-900 dark:text-white">DM Notifications</p>
                  <p className="text-sm text-gray-500 dark:text-gray-400">
                    {status.dm_notifications_enabled ? 'Enabled' : 'Disabled'} for all events
                  </p>
                </div>
              </div>
              <button
                onClick={toggleMasterNotifications}
                disabled={savingPrefs}
                className={clsx(
                  'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
                  status.dm_notifications_enabled 
                    ? 'bg-primary-600' 
                    : 'bg-gray-300 dark:bg-gray-600'
                )}
              >
                <span
                  className={clsx(
                    'inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform',
                    status.dm_notifications_enabled ? 'translate-x-6' : 'translate-x-1'
                  )}
                />
              </button>
            </div>

            {/* Individual preferences */}
            {status.dm_notifications_enabled && (
              <div className="space-y-3">
                <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">Notification Types</h4>
                {NOTIFICATION_PREFS.map((pref) => (
                  <label
                    key={pref.key}
                    className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/30 rounded-lg cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700/50 transition-colors"
                  >
                    <div className="flex items-center gap-3">
                      <pref.icon className="w-4 h-4 text-gray-400" />
                      <div>
                        <p className="text-sm font-medium text-gray-900 dark:text-white">{pref.label}</p>
                        <p className="text-xs text-gray-500 dark:text-gray-400">{pref.description}</p>
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      checked={status[pref.key]}
                      onChange={() => togglePreference(pref.key)}
                      disabled={savingPrefs}
                      className="w-4 h-4 text-primary-600 bg-gray-100 border-gray-300 rounded focus:ring-primary-500 dark:bg-gray-700 dark:border-gray-600"
                    />
                  </label>
                ))}
              </div>
            )}

            {/* Actions */}
            <div className="flex items-center gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
              <button
                onClick={handleTest}
                disabled={testing}
                className="flex items-center gap-2 px-4 py-2 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors disabled:opacity-50"
              >
                {testing ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : testSuccess ? (
                  <Check className="w-4 h-4 text-green-500" />
                ) : (
                  <Send className="w-4 h-4" />
                )}
                {testSuccess ? 'Sent!' : 'Send Test DM'}
              </button>
              
              <button
                onClick={() => fetchData()}
                className="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
              >
                <RefreshCw className="w-4 h-4" />
              </button>
              
              <div className="flex-1" />
              
              <button
                onClick={handleDisconnect}
                disabled={disconnecting}
                className="flex items-center gap-2 px-4 py-2 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors disabled:opacity-50"
              >
                {disconnecting ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Unlink className="w-4 h-4" />
                )}
                Disconnect
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

