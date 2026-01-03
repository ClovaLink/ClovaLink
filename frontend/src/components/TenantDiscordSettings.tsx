import { useState, useEffect, useCallback } from 'react';
import { Save, Loader2, Check, AlertCircle } from 'lucide-react';
import clsx from 'clsx';

// Discord brand color
const DISCORD_COLOR = '#5865F2';

interface TenantDiscordSettingsProps {
  tenantId: string;
  authFetch: (url: string, options?: RequestInit) => Promise<Response>;
}

interface DiscordSettings {
  enabled: boolean;
}

export function TenantDiscordSettings({ tenantId, authFetch }: TenantDiscordSettingsProps) {
  const [settings, setSettings] = useState<DiscordSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [enabled, setEnabled] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchSettings = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const res = await authFetch(`/api/discord/settings?tenant_id=${tenantId}`);
      if (res.ok) {
        const data: DiscordSettings = await res.json();
        setSettings(data);
        setEnabled(data.enabled);
      }
    } catch (err) {
      setError('Failed to load Discord settings');
    } finally {
      setLoading(false);
    }
  }, [authFetch, tenantId]);

  useEffect(() => {
    fetchSettings();
  }, [fetchSettings]);

  const handleSave = async () => {
    setSaving(true);
    setSaveSuccess(false);
    setError(null);

    try {
      const res = await authFetch('/api/discord/settings/update', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      });

      if (res.ok) {
        const updated: DiscordSettings = await res.json();
        setSettings(updated);
        setSaveSuccess(true);
        setTimeout(() => setSaveSuccess(false), 3000);
      } else {
        const data = await res.json();
        throw new Error(data.error || 'Failed to save');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to save settings');
    } finally {
      setSaving(false);
    }
  };

  const hasChanges = settings && enabled !== settings.enabled;

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="w-6 h-6 animate-spin text-gray-400" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
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
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white">Discord Notifications</h3>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Allow users to receive DM notifications via Discord
          </p>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <AlertCircle className="w-4 h-4 text-red-500" />
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}

      {/* Settings Card */}
      <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <label className="flex items-center justify-between">
          <div>
            <p className="font-medium text-gray-900 dark:text-white">Enable Discord Integration</p>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              When enabled, users can connect their Discord accounts to receive DM notifications
            </p>
          </div>
          <button
            onClick={() => setEnabled(!enabled)}
            className={clsx(
              'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
              enabled ? 'bg-primary-600' : 'bg-gray-300 dark:bg-gray-600'
            )}
          >
            <span
              className={clsx(
                'inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform',
                enabled ? 'translate-x-6' : 'translate-x-1'
              )}
            />
          </button>
        </label>

        {enabled && (
          <div className="mt-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
            <p className="text-sm text-blue-800 dark:text-blue-300">
              <strong>Note:</strong> Users will be able to connect their Discord accounts from their Profile page. 
              They'll receive DMs when files are shared with them, uploaded to their requests, and more.
            </p>
          </div>
        )}
      </div>

      {/* Save Button */}
      <div className="flex justify-end">
        <button
          onClick={handleSave}
          disabled={saving || !hasChanges}
          className={clsx(
            'flex items-center gap-2 px-6 py-2.5 rounded-lg font-medium transition-all',
            hasChanges
              ? 'bg-primary-600 text-white hover:bg-primary-700'
              : 'bg-gray-100 text-gray-400 dark:bg-gray-700 dark:text-gray-500 cursor-not-allowed'
          )}
        >
          {saving ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : saveSuccess ? (
            <Check className="w-4 h-4" />
          ) : (
            <Save className="w-4 h-4" />
          )}
          {saveSuccess ? 'Saved!' : 'Save Changes'}
        </button>
      </div>
    </div>
  );
}

