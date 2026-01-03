import { useState, useEffect } from 'react';
import {
  Save,
  Check,
  Loader2,
  Sparkles,
  Key,
  Shield,
  AlertTriangle,
  CheckCircle,
  XCircle,
  RefreshCw,
  Activity,
  Zap,
  Eye,
  EyeOff,
  TestTube,
  Users,
  FileText,
  ChevronLeft,
  ChevronRight,
  Wrench,
  User,
} from 'lucide-react';
import clsx from 'clsx';

interface AiSettings {
  tenant_id: string;
  enabled: boolean;
  provider: string;
  api_key_masked: string | null;
  allowed_roles: string[];
  hipaa_approved_only: boolean;
  sox_read_only: boolean;
  monthly_token_limit: number;
  daily_request_limit: number;
  tokens_used_this_month: number;
  requests_today: number;
  maintenance_mode: boolean;
  maintenance_message: string | null;
}

interface UsageStats {
  tokens_used_today: number;
  tokens_used_this_month: number;
  requests_today: number;
  monthly_token_limit: number;
  daily_request_limit: number;
  recent_actions: {
    action: string;
    tokens_used: number;
    status: string;
    created_at: string;
    user_name: string | null;
    file_name: string | null;
  }[];
  total_count: number;
  page: number;
  per_page: number;
  total_pages: number;
}

interface ProviderInfo {
  id: string;
  name: string;
  hipaa_approved: boolean;
  models: string[];
}

const AVAILABLE_ROLES = ['Employee', 'Manager', 'Admin', 'SuperAdmin'];

interface TenantAiSettingsProps {
  tenantId: string;
  authFetch: (url: string, options?: RequestInit) => Promise<Response>;
}

export function TenantAiSettings({ tenantId, authFetch }: TenantAiSettingsProps) {
  // Settings state
  const [settings, setSettings] = useState<AiSettings | null>(null);
  const [usage, setUsage] = useState<UsageStats | null>(null);
  const [providers, setProviders] = useState<ProviderInfo[]>([]);

  // Form state
  const [enabled, setEnabled] = useState(false);
  const [provider, setProvider] = useState('openai');
  const [apiKey, setApiKey] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);
  const [allowedRoles, setAllowedRoles] = useState<string[]>(['Admin', 'SuperAdmin']);
  const [hipaaApprovedOnly, setHipaaApprovedOnly] = useState(false);
  const [soxReadOnly, setSoxReadOnly] = useState(false);
  const [monthlyTokenLimit, setMonthlyTokenLimit] = useState(100000);
  const [dailyRequestLimit, setDailyRequestLimit] = useState(100);
  const [maintenanceMode, setMaintenanceMode] = useState(false);
  const [maintenanceMessage, setMaintenanceMessage] = useState('');
  
  // Pagination state
  const [currentPage, setCurrentPage] = useState(1);
  const PER_PAGE = 10;

  // UI state
  const [loading, setLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testStatus, setTestStatus] = useState<'idle' | 'testing' | 'success' | 'error'>('idle');
  const [activeTab, setActiveTab] = useState<'settings' | 'usage'>('settings');

  const hasChanges =
    settings &&
    (enabled !== settings.enabled ||
      provider !== settings.provider ||
      apiKey !== '' ||
      JSON.stringify(allowedRoles.sort()) !== JSON.stringify(settings.allowed_roles.sort()) ||
      hipaaApprovedOnly !== settings.hipaa_approved_only ||
      soxReadOnly !== settings.sox_read_only ||
      monthlyTokenLimit !== settings.monthly_token_limit ||
      dailyRequestLimit !== settings.daily_request_limit ||
      maintenanceMode !== settings.maintenance_mode ||
      maintenanceMessage !== (settings.maintenance_message || ''));

  // Fetch all data
  const fetchData = async () => {
    setLoading(true);
    setError(null);

    try {
      const [settingsRes, providersRes] = await Promise.all([
        authFetch(`/api/ai/settings?tenant_id=${tenantId}`),
        authFetch('/api/ai/providers'),
      ]);

      if (settingsRes.ok) {
        const s: AiSettings = await settingsRes.json();
        setSettings(s);
        setEnabled(s.enabled);
        setProvider(s.provider);
        setAllowedRoles(s.allowed_roles);
        setHipaaApprovedOnly(s.hipaa_approved_only);
        setSoxReadOnly(s.sox_read_only);
        setMonthlyTokenLimit(s.monthly_token_limit);
        setDailyRequestLimit(s.daily_request_limit);
        setMaintenanceMode(s.maintenance_mode);
        setMaintenanceMessage(s.maintenance_message || '');
      }

      if (providersRes.ok) {
        const data = await providersRes.json();
        setProviders(data.providers);
      }
    } catch (err) {
      setError('Failed to load AI settings');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const fetchUsage = async (page = currentPage) => {
    try {
      const res = await authFetch(`/api/ai/usage?tenant_id=${tenantId}&page=${page}&per_page=${PER_PAGE}`);
      if (res.ok) {
        setUsage(await res.json());
      }
    } catch (err) {
      console.error('Failed to fetch usage:', err);
    }
  };

  useEffect(() => {
    fetchData();
  }, [tenantId]);

  useEffect(() => {
    if (activeTab === 'usage') {
      fetchUsage();
    }
  }, [activeTab, tenantId]);

  const handleSave = async () => {
    setIsSaving(true);
    setSaveSuccess(false);
    setError(null);

    try {
      const body: Record<string, unknown> = {
        tenant_id: tenantId,
        enabled,
        provider,
        allowed_roles: allowedRoles,
        hipaa_approved_only: hipaaApprovedOnly,
        sox_read_only: soxReadOnly,
        monthly_token_limit: monthlyTokenLimit,
        daily_request_limit: dailyRequestLimit,
        maintenance_mode: maintenanceMode,
        maintenance_message: maintenanceMessage || null,
      };

      // Only include API key if it was changed
      if (apiKey) {
        body.api_key = apiKey;
      }

      const res = await authFetch('/api/ai/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });

      if (res.ok) {
        const updated: AiSettings = await res.json();
        setSettings(updated);
        setApiKey(''); // Clear API key input after save
        setSaveSuccess(true);
        setTimeout(() => setSaveSuccess(false), 3000);
      } else {
        throw new Error('Failed to save settings');
      }
    } catch (err) {
      setError('Failed to save settings');
    } finally {
      setIsSaving(false);
    }
  };

  const handleTestConnection = async () => {
    setTestStatus('testing');
    try {
      const res = await authFetch(`/api/ai/test?tenant_id=${tenantId}`, {
        method: 'POST',
      });
      if (res.ok) {
        const data = await res.json();
        setTestStatus(data.success ? 'success' : 'error');
      } else {
        setTestStatus('error');
      }
    } catch {
      setTestStatus('error');
    }
    setTimeout(() => setTestStatus('idle'), 3000);
  };

  const toggleRole = (role: string) => {
    setAllowedRoles((prev) =>
      prev.includes(role) ? prev.filter((r) => r !== role) : [...prev, role]
    );
  };

  const getSelectedProvider = () => providers.find((p) => p.id === provider);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="w-8 h-8 animate-spin text-primary-500" />
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-primary-500" />
            AI Features
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Configure AI-powered document summarization, Q&A, and search
          </p>
        </div>
        {activeTab === 'settings' && (
          <button
            onClick={handleSave}
            disabled={!hasChanges || isSaving}
            className={clsx(
              'flex items-center px-4 py-2 rounded-lg text-sm font-medium transition-all',
              hasChanges && !isSaving
                ? 'bg-primary-600 text-white hover:bg-primary-700'
                : 'bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed'
            )}
          >
            {isSaving ? (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            ) : saveSuccess ? (
              <Check className="w-4 h-4 mr-2" />
            ) : (
              <Save className="w-4 h-4 mr-2" />
            )}
            {isSaving ? 'Saving...' : saveSuccess ? 'Saved!' : 'Save'}
          </button>
        )}
      </div>

      {error && (
        <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-300 text-sm">
          {error}
        </div>
      )}

      {/* Status Card */}
      <div
        className={clsx(
          'rounded-xl border shadow-sm overflow-hidden',
          !settings?.enabled
            ? 'bg-gray-50 dark:bg-gray-800/50 border-gray-200 dark:border-gray-700'
            : settings?.api_key_masked
            ? 'bg-green-50 dark:bg-green-900/10 border-green-200 dark:border-green-800'
            : 'bg-yellow-50 dark:bg-yellow-900/10 border-yellow-200 dark:border-yellow-800'
        )}
      >
        <div className="px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div
              className={clsx(
                'p-3 rounded-xl',
                !settings?.enabled
                  ? 'bg-gray-100 dark:bg-gray-700'
                  : settings?.api_key_masked
                  ? 'bg-green-100 dark:bg-green-900/30'
                  : 'bg-yellow-100 dark:bg-yellow-900/30'
              )}
            >
              {!settings?.enabled ? (
                <Sparkles className="w-6 h-6 text-gray-400" />
              ) : settings?.api_key_masked ? (
                <CheckCircle className="w-6 h-6 text-green-600 dark:text-green-400" />
              ) : (
                <AlertTriangle className="w-6 h-6 text-yellow-600 dark:text-yellow-400" />
              )}
            </div>
            <div>
              <h3 className="font-semibold text-gray-900 dark:text-white">
                {!settings?.enabled
                  ? 'AI Features Disabled'
                  : settings?.api_key_masked
                  ? 'AI Features Active'
                  : 'API Key Required'}
              </h3>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                {!settings?.enabled
                  ? 'Enable AI features to use summarization, Q&A, and search'
                  : settings?.api_key_masked
                  ? `Using ${getSelectedProvider()?.name || provider} provider`
                  : 'Configure your API key to activate AI features'}
              </p>
            </div>
          </div>
          <button
            onClick={fetchData}
            className="p-2 rounded-lg hover:bg-white/50 dark:hover:bg-gray-700/50 transition-colors"
          >
            <RefreshCw className="w-5 h-5 text-gray-500" />
          </button>
        </div>

        {/* Usage Metrics */}
        {settings?.enabled && settings?.api_key_masked && (
          <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 grid grid-cols-2 md:grid-cols-4 gap-4">
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">Tokens This Month</p>
              <p className="text-xl font-bold text-gray-900 dark:text-white">
                {settings.tokens_used_this_month.toLocaleString()}
              </p>
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">Requests Today</p>
              <p className="text-xl font-bold text-gray-900 dark:text-white">
                {settings.requests_today}
              </p>
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">Monthly Limit</p>
              <p className="text-xl font-bold text-gray-900 dark:text-white">
                {settings.monthly_token_limit.toLocaleString()}
              </p>
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">Daily Limit</p>
              <p className="text-xl font-bold text-gray-900 dark:text-white">
                {settings.daily_request_limit}
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Tabs */}
      <div className="border-b border-gray-200 dark:border-gray-700">
        <nav className="flex gap-4">
          {[
            { id: 'settings', label: 'Settings', icon: Shield },
            { id: 'usage', label: 'Usage History', icon: Activity },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as typeof activeTab)}
              className={clsx(
                'flex items-center gap-2 px-3 py-2.5 border-b-2 text-sm font-medium transition-colors',
                activeTab === tab.id
                  ? 'border-primary-500 text-primary-600 dark:text-primary-400'
                  : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
              )}
            >
              <tab.icon className="w-4 h-4" />
              {tab.label}
            </button>
          ))}
        </nav>
      </div>

      {/* Settings Tab */}
      {activeTab === 'settings' && (
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
          <div className="p-6 space-y-6">
            {/* Enable Toggle */}
            <div className="flex items-center justify-between">
              <div>
                <label className="font-medium text-gray-900 dark:text-white">Enable AI Features</label>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Allow users to use AI-powered document features
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
            </div>

            {/* Provider Selection */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                AI Provider
              </label>
              <select
                value={provider}
                onChange={(e) => setProvider(e.target.value)}
                className="w-full px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500"
              >
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} {p.hipaa_approved && '(HIPAA Approved)'}
                  </option>
                ))}
              </select>
            </div>

            {/* API Key */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                API Key
              </label>
              <div className="relative">
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                  <Key className="w-4 h-4 text-gray-400" />
                </div>
                <input
                  type={showApiKey ? 'text' : 'password'}
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder={settings?.api_key_masked || 'Enter your API key'}
                  className="w-full pl-10 pr-20 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500"
                />
                <div className="absolute inset-y-0 right-0 flex items-center gap-1 pr-2">
                  <button
                    type="button"
                    onClick={() => setShowApiKey(!showApiKey)}
                    className="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-600 transition-colors"
                  >
                    {showApiKey ? (
                      <EyeOff className="w-4 h-4 text-gray-400" />
                    ) : (
                      <Eye className="w-4 h-4 text-gray-400" />
                    )}
                  </button>
                  {settings?.api_key_masked && (
                    <button
                      type="button"
                      onClick={handleTestConnection}
                      disabled={testStatus === 'testing'}
                      className={clsx(
                        'flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors',
                        testStatus === 'success'
                          ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                          : testStatus === 'error'
                          ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400'
                          : 'bg-gray-100 text-gray-700 dark:bg-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-500'
                      )}
                    >
                      {testStatus === 'testing' ? (
                        <Loader2 className="w-3 h-3 animate-spin" />
                      ) : testStatus === 'success' ? (
                        <CheckCircle className="w-3 h-3" />
                      ) : testStatus === 'error' ? (
                        <XCircle className="w-3 h-3" />
                      ) : (
                        <TestTube className="w-3 h-3" />
                      )}
                      {testStatus === 'testing' ? 'Testing...' : testStatus === 'success' ? 'Connected' : testStatus === 'error' ? 'Failed' : 'Test'}
                    </button>
                  )}
                </div>
              </div>
              <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                Your API key is encrypted and stored securely.
              </p>
            </div>

            {/* Role Access */}
            <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2 mb-4">
                <Users className="w-4 h-4 text-gray-500" />
                <h4 className="font-medium text-gray-900 dark:text-white">Role Access</h4>
              </div>
              <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
                Select which roles can use AI features
              </p>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                {AVAILABLE_ROLES.map((role) => (
                  <button
                    key={role}
                    onClick={() => toggleRole(role)}
                    className={clsx(
                      'p-3 rounded-lg border-2 text-sm font-medium transition-all',
                      allowedRoles.includes(role)
                        ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                        : 'border-gray-200 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:border-gray-300'
                    )}
                  >
                    {role}
                  </button>
                ))}
              </div>
            </div>

            {/* Usage Limits */}
            <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2 mb-4">
                <Zap className="w-4 h-4 text-gray-500" />
                <h4 className="font-medium text-gray-900 dark:text-white">Usage Limits</h4>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                    Monthly Token Limit
                  </label>
                  <input
                    type="number"
                    value={monthlyTokenLimit}
                    onChange={(e) => setMonthlyTokenLimit(parseInt(e.target.value) || 100000)}
                    min={1000}
                    step={1000}
                    className="w-full px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                    Daily Request Limit
                  </label>
                  <input
                    type="number"
                    value={dailyRequestLimit}
                    onChange={(e) => setDailyRequestLimit(parseInt(e.target.value) || 100)}
                    min={1}
                    className="w-full px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500"
                  />
                </div>
              </div>
            </div>

            {/* Compliance */}
            <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2 mb-4">
                <Shield className="w-4 h-4 text-gray-500" />
                <h4 className="font-medium text-gray-900 dark:text-white">Compliance</h4>
              </div>
              <div className="space-y-4">
                <label className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={hipaaApprovedOnly}
                    onChange={(e) => setHipaaApprovedOnly(e.target.checked)}
                    className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                  />
                  <div>
                    <p className="text-sm font-medium text-gray-900 dark:text-white">
                      HIPAA Mode
                    </p>
                    <p className="text-xs text-gray-500 dark:text-gray-400">
                      Only allow HIPAA-approved AI providers
                    </p>
                  </div>
                </label>
                <label className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={soxReadOnly}
                    onChange={(e) => setSoxReadOnly(e.target.checked)}
                    className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                  />
                  <div>
                    <p className="text-sm font-medium text-gray-900 dark:text-white">
                      SOX Read-Only Mode
                    </p>
                    <p className="text-xs text-gray-500 dark:text-gray-400">
                      Disable AI content generation (summarize, Q&A), allow only search
                    </p>
                  </div>
                </label>
              </div>
            </div>

            {/* Maintenance Mode */}
            <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2 mb-4">
                <Wrench className="w-4 h-4 text-gray-500" />
                <h4 className="font-medium text-gray-900 dark:text-white">Maintenance Mode</h4>
              </div>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-sm font-medium text-gray-900 dark:text-white">
                      Enable Maintenance Mode
                    </p>
                    <p className="text-xs text-gray-500 dark:text-gray-400">
                      Block new AI requests while serving cached summaries
                    </p>
                  </div>
                  <button
                    onClick={() => setMaintenanceMode(!maintenanceMode)}
                    className={clsx(
                      'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
                      maintenanceMode ? 'bg-yellow-500' : 'bg-gray-300 dark:bg-gray-600'
                    )}
                  >
                    <span
                      className={clsx(
                        'inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform',
                        maintenanceMode ? 'translate-x-6' : 'translate-x-1'
                      )}
                    />
                  </button>
                </div>
                {maintenanceMode && (
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Maintenance Message
                    </label>
                    <textarea
                      value={maintenanceMessage}
                      onChange={(e) => setMaintenanceMessage(e.target.value)}
                      placeholder="AI features are temporarily unavailable for maintenance. Please try again later."
                      rows={2}
                      className="w-full px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 resize-none"
                    />
                    <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                      This message will be shown to users when they try to use AI features.
                    </p>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Usage Tab */}
      {activeTab === 'usage' && (
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
          <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
            <div>
              <h3 className="font-medium text-gray-900 dark:text-white">Recent AI Activity</h3>
              {usage && (
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  {usage.total_count.toLocaleString()} total actions
                </p>
              )}
            </div>
            <button
              onClick={() => fetchUsage()}
              className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            >
              <RefreshCw className="w-4 h-4 text-gray-500" />
            </button>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-gray-50 dark:bg-gray-700/50">
                <tr>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    User
                  </th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    Action
                  </th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    File
                  </th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    Status
                  </th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    Tokens
                  </th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                    Time
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {usage?.recent_actions && usage.recent_actions.length > 0 ? (
                  usage.recent_actions.map((action, idx) => (
                    <tr key={idx} className="hover:bg-gray-50 dark:hover:bg-gray-700/30">
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <User className="w-4 h-4 text-gray-400" />
                          <span className="text-sm text-gray-900 dark:text-white">
                            {action.user_name || 'Unknown'}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-sm text-gray-900 dark:text-white capitalize">
                          {action.action}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <FileText className="w-4 h-4 text-gray-400" />
                          <span className="text-sm text-gray-600 dark:text-gray-300 truncate max-w-[150px]" title={action.file_name || undefined}>
                            {action.file_name || '-'}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <span
                          className={clsx(
                            'inline-flex items-center gap-1 px-2 py-1 rounded text-xs font-medium',
                            action.status === 'success'
                              ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                              : 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400'
                          )}
                        >
                          {action.status === 'success' ? (
                            <CheckCircle className="w-3 h-3" />
                          ) : (
                            <XCircle className="w-3 h-3" />
                          )}
                          {action.status}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-gray-500 dark:text-gray-400">
                        {action.tokens_used.toLocaleString()}
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-gray-500 dark:text-gray-400">
                        {new Date(action.created_at).toLocaleString(undefined, {
                          month: 'short',
                          day: 'numeric',
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={6} className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
                      <Sparkles className="w-8 h-8 mx-auto mb-2 opacity-50" />
                      No AI activity yet
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          
          {/* Pagination */}
          {usage && usage.total_pages > 1 && (
            <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Page {usage.page} of {usage.total_pages}
              </p>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => {
                    const newPage = currentPage - 1;
                    setCurrentPage(newPage);
                    fetchUsage(newPage);
                  }}
                  disabled={currentPage <= 1}
                  className={clsx(
                    'p-2 rounded-lg transition-colors',
                    currentPage <= 1
                      ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                      : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700'
                  )}
                >
                  <ChevronLeft className="w-5 h-5" />
                </button>
                <button
                  onClick={() => {
                    const newPage = currentPage + 1;
                    setCurrentPage(newPage);
                    fetchUsage(newPage);
                  }}
                  disabled={currentPage >= usage.total_pages}
                  className={clsx(
                    'p-2 rounded-lg transition-colors',
                    currentPage >= usage.total_pages
                      ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                      : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700'
                  )}
                >
                  <ChevronRight className="w-5 h-5" />
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

