import { useState, useEffect } from 'react';
import {
    Save,
    Plus,
    Trash2,
    Check,
    X,
    Loader2,
    Shield,
    Globe,
    Eye,
    EyeOff,
    TestTube,
    AlertTriangle,
    CheckCircle,
    XCircle,
    ToggleLeft,
    ToggleRight,
    Copy,
} from 'lucide-react';
import { useAuthFetch, useAuth } from '../../context/AuthContext';
import { SsoAttributeMappings } from '../../components/SsoAttributeMappings';
import clsx from 'clsx';

const API_URL = import.meta.env.VITE_API_URL || '';

// ==================== Types ====================

interface OidcProvider {
    id: string;
    tenant_id: string;
    name: string;
    slug: string;
    provider_type: string;
    issuer_url: string;
    client_id: string;
    scopes: string;
    auto_provision: boolean;
    default_role: string;
    default_department_id: string | null;
    email_domains: string[];
    trust_idp_mfa: boolean;
    enabled: boolean;
    created_at: string;
    updated_at: string;
}

interface SamlProvider {
    id: string;
    tenant_id: string;
    name: string;
    slug: string;
    provider_type: string;
    idp_entity_id: string;
    idp_sso_url: string;
    idp_slo_url: string | null;
    idp_metadata_url: string | null;
    idp_signing_certificate: string;
    sp_entity_id: string;
    nameid_format: string;
    sso_binding: string;
    attribute_email: string;
    attribute_name: string;
    auto_provision: boolean;
    default_role: string;
    default_department_id: string | null;
    email_domains: string[];
    trust_idp_mfa: boolean;
    enabled: boolean;
    created_at: string;
    updated_at: string;
}

interface OidcFormData {
    name: string;
    slug: string;
    provider_type: string;
    issuer_url: string;
    client_id: string;
    client_secret: string;
    scopes: string;
    auto_provision: boolean;
    default_role: string;
    email_domains: string;
    trust_idp_mfa: boolean;
    enabled: boolean;
}

interface SamlFormData {
    name: string;
    slug: string;
    provider_type: string;
    idp_entity_id: string;
    idp_sso_url: string;
    idp_slo_url: string;
    idp_metadata_url: string;
    idp_signing_certificate: string;
    nameid_format: string;
    sso_binding: string;
    attribute_email: string;
    attribute_name: string;
    auto_provision: boolean;
    default_role: string;
    email_domains: string;
    trust_idp_mfa: boolean;
    enabled: boolean;
}

// ==================== Constants ====================

const EMPTY_OIDC_FORM: OidcFormData = {
    name: '',
    slug: '',
    provider_type: 'generic',
    issuer_url: '',
    client_id: '',
    client_secret: '',
    scopes: 'openid email profile',
    auto_provision: false,
    default_role: 'Employee',
    email_domains: '',
    trust_idp_mfa: true,
    enabled: true,
};

const EMPTY_SAML_FORM: SamlFormData = {
    name: '',
    slug: '',
    provider_type: 'generic',
    idp_entity_id: '',
    idp_sso_url: '',
    idp_slo_url: '',
    idp_metadata_url: '',
    idp_signing_certificate: '',
    nameid_format: 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress',
    sso_binding: 'HTTP-POST',
    attribute_email: 'email',
    attribute_name: 'displayName',
    auto_provision: false,
    default_role: 'Employee',
    email_domains: '',
    trust_idp_mfa: true,
    enabled: true,
};

const OIDC_PROVIDER_TYPES = [
    { value: 'google', label: 'Google Workspace' },
    { value: 'microsoft', label: 'Microsoft Entra ID' },
    { value: 'okta', label: 'Okta' },
    { value: 'generic', label: 'Generic OIDC' },
];

const SAML_PROVIDER_TYPES = [
    { value: 'okta', label: 'Okta' },
    { value: 'azure', label: 'Azure AD / Entra ID' },
    { value: 'adfs', label: 'ADFS' },
    { value: 'google', label: 'Google Workspace' },
    { value: 'generic', label: 'Generic SAML' },
];

const NAMEID_FORMATS = [
    { value: 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress', label: 'Email Address' },
    { value: 'urn:oasis:names:tc:SAML:2.0:nameid-format:persistent', label: 'Persistent' },
    { value: 'urn:oasis:names:tc:SAML:2.0:nameid-format:transient', label: 'Transient' },
    { value: 'urn:oasis:names:tc:SAML:1.1:nameid-format:unspecified', label: 'Unspecified' },
];

type Tab = 'oidc' | 'saml' | 'mappings';

// ==================== Shared Components ====================

function StatusMessages({ error, success, setError, setSuccess }: {
    error: string;
    success: string;
    setError: (v: string) => void;
    setSuccess: (v: string) => void;
}) {
    return (
        <>
            {error && (
                <div className="rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 p-4">
                    <div className="flex items-center gap-2">
                        <XCircle className="w-5 h-5 text-red-500" />
                        <p className="text-sm text-red-700 dark:text-red-400">{error}</p>
                        <button onClick={() => setError('')} className="ml-auto">
                            <X className="w-4 h-4 text-red-400" />
                        </button>
                    </div>
                </div>
            )}
            {success && (
                <div className="rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 p-4">
                    <div className="flex items-center gap-2">
                        <CheckCircle className="w-5 h-5 text-green-500" />
                        <p className="text-sm text-green-700 dark:text-green-400">{success}</p>
                        <button onClick={() => setSuccess('')} className="ml-auto">
                            <X className="w-4 h-4 text-green-400" />
                        </button>
                    </div>
                </div>
            )}
        </>
    );
}

function ToggleField({ label, description, value, onChange }: {
    label: string;
    description: string;
    value: boolean;
    onChange: () => void;
}) {
    return (
        <div className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
            <div>
                <p className="text-sm font-medium text-gray-900 dark:text-white">{label}</p>
                <p className="text-xs text-gray-500 dark:text-gray-400">{description}</p>
            </div>
            <button
                type="button"
                onClick={onChange}
                className={clsx(
                    'relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition-colors ml-4',
                    value ? 'bg-primary-600' : 'bg-gray-300 dark:bg-gray-600'
                )}
            >
                <span
                    className={clsx(
                        'inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform',
                        value ? 'translate-x-6' : 'translate-x-1'
                    )}
                />
            </button>
        </div>
    );
}

function ProviderCard({ provider, isAdmin, onToggle, onTest, onEdit, onDelete, testingId, deleteConfirm, setDeleteConfirm, subtitle }: {
    provider: { id: string; name: string; provider_type: string; enabled: boolean; auto_provision: boolean; email_domains: string[] };
    isAdmin: boolean;
    onToggle: () => void;
    onTest: () => void;
    onEdit: () => void;
    onDelete: () => void;
    testingId: string | null;
    deleteConfirm: string | null;
    setDeleteConfirm: (v: string | null) => void;
    subtitle: string;
}) {
    return (
        <div className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl p-5">
            <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                    <div className={clsx('p-2.5 rounded-xl', provider.enabled ? 'bg-green-100 dark:bg-green-900/30' : 'bg-gray-100 dark:bg-gray-700')}>
                        <Shield className={clsx('w-5 h-5', provider.enabled ? 'text-green-600 dark:text-green-400' : 'text-gray-400')} />
                    </div>
                    <div>
                        <h4 className="text-sm font-semibold text-gray-900 dark:text-white">{provider.name}</h4>
                        <p className="text-xs text-gray-500 dark:text-gray-400">{provider.provider_type} &middot; {subtitle}</p>
                        {provider.email_domains.length > 0 && (
                            <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">Domains: {provider.email_domains.join(', ')}</p>
                        )}
                    </div>
                </div>
                <div className="flex items-center gap-2">
                    <span className={clsx('px-2 py-0.5 rounded-full text-xs font-medium', provider.enabled ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400')}>
                        {provider.enabled ? 'Active' : 'Disabled'}
                    </span>
                    {provider.auto_provision && (
                        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">Auto-Provision</span>
                    )}
                </div>
            </div>
            {isAdmin && (
                <div className="flex items-center gap-2 mt-4 pt-3 border-t border-gray-100 dark:border-gray-700">
                    <button onClick={onToggle} className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-gray-600 dark:text-gray-400 bg-gray-100 dark:bg-gray-700 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors">
                        {provider.enabled ? <ToggleRight className="w-3.5 h-3.5" /> : <ToggleLeft className="w-3.5 h-3.5" />}
                        {provider.enabled ? 'Disable' : 'Enable'}
                    </button>
                    <button onClick={onTest} disabled={testingId === provider.id} className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20 rounded-lg hover:bg-blue-100 dark:hover:bg-blue-900/40 transition-colors">
                        {testingId === provider.id ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <TestTube className="w-3.5 h-3.5" />}
                        Test
                    </button>
                    <button onClick={onEdit} className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-gray-600 dark:text-gray-400 bg-gray-100 dark:bg-gray-700 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors">
                        Edit
                    </button>
                    {deleteConfirm === provider.id ? (
                        <div className="flex items-center gap-1 ml-auto">
                            <span className="text-xs text-red-600 dark:text-red-400 mr-1">Delete?</span>
                            <button onClick={onDelete} className="p-1.5 text-red-600 bg-red-50 dark:bg-red-900/20 rounded-lg hover:bg-red-100">
                                <Check className="w-3.5 h-3.5" />
                            </button>
                            <button onClick={() => setDeleteConfirm(null)} className="p-1.5 text-gray-500 bg-gray-100 dark:bg-gray-700 rounded-lg hover:bg-gray-200">
                                <X className="w-3.5 h-3.5" />
                            </button>
                        </div>
                    ) : (
                        <button onClick={() => setDeleteConfirm(provider.id)} className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 rounded-lg hover:bg-red-100 dark:hover:bg-red-900/40 transition-colors ml-auto">
                            <Trash2 className="w-3.5 h-3.5" />
                            Delete
                        </button>
                    )}
                </div>
            )}
        </div>
    );
}

// ==================== Main Component ====================

export function SsoSettings() {
    const authFetch = useAuthFetch();
    const { user } = useAuth();
    const [activeTab, setActiveTab] = useState<Tab>('oidc');

    // OIDC state
    const [oidcProviders, setOidcProviders] = useState<OidcProvider[]>([]);
    const [oidcLoading, setOidcLoading] = useState(true);
    const [oidcSaving, setOidcSaving] = useState(false);
    const [showOidcForm, setShowOidcForm] = useState(false);
    const [oidcEditingId, setOidcEditingId] = useState<string | null>(null);
    const [oidcForm, setOidcForm] = useState<OidcFormData>(EMPTY_OIDC_FORM);
    const [showSecret, setShowSecret] = useState(false);

    // SAML state
    const [samlProviders, setSamlProviders] = useState<SamlProvider[]>([]);
    const [samlLoading, setSamlLoading] = useState(true);
    const [samlSaving, setSamlSaving] = useState(false);
    const [showSamlForm, setShowSamlForm] = useState(false);
    const [samlEditingId, setSamlEditingId] = useState<string | null>(null);
    const [samlForm, setSamlForm] = useState<SamlFormData>(EMPTY_SAML_FORM);
    const [createdSamlProvider, setCreatedSamlProvider] = useState<SamlProvider | null>(null);

    // Shared state
    const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
    const [testingId, setTestingId] = useState<string | null>(null);
    const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
    const [error, setError] = useState('');
    const [success, setSuccess] = useState('');

    const isAdmin = user?.role === 'SuperAdmin' || user?.role === 'Admin';

    useEffect(() => {
        loadOidcProviders();
        loadSamlProviders();
    }, []);

    // ==================== OIDC ====================

    const loadOidcProviders = async () => {
        try {
            const response = await authFetch('/api/oidc/providers');
            if (response.ok) {
                const data = await response.json();
                setOidcProviders(data.providers || []);
            }
        } catch {
            // ignore
        } finally {
            setOidcLoading(false);
        }
    };

    const handleOidcProviderTypeChange = (type: string) => {
        const presets: Record<string, Partial<OidcFormData>> = {
            google: { issuer_url: 'https://accounts.google.com', scopes: 'openid email profile' },
            microsoft: { issuer_url: 'https://login.microsoftonline.com/{tenant_id}/v2.0', scopes: 'openid email profile' },
            okta: { issuer_url: 'https://{your-domain}.okta.com', scopes: 'openid email profile' },
        };
        const preset = presets[type] || {};
        setOidcForm(prev => ({
            ...prev,
            provider_type: type,
            ...(!prev.issuer_url || prev.issuer_url === EMPTY_OIDC_FORM.issuer_url ? preset : {}),
        }));
    };

    const handleOidcSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setOidcSaving(true);
        setError('');
        setSuccess('');
        try {
            const payload = { ...oidcForm, email_domains: oidcForm.email_domains.split(',').map(d => d.trim()).filter(Boolean) };
            const url = oidcEditingId ? `/api/oidc/providers/${oidcEditingId}` : '/api/oidc/providers';
            const method = oidcEditingId ? 'PUT' : 'POST';
            const response = await authFetch(url, { method, body: JSON.stringify(payload) });
            if (response.ok) {
                setSuccess(oidcEditingId ? 'Provider updated successfully.' : 'Provider created successfully.');
                setShowOidcForm(false);
                setOidcEditingId(null);
                setOidcForm(EMPTY_OIDC_FORM);
                await loadOidcProviders();
            } else {
                const data = await response.json();
                setError(data.error || 'Failed to save provider');
            }
        } catch {
            setError('Failed to save provider');
        } finally {
            setOidcSaving(false);
        }
    };

    const handleOidcEdit = (provider: OidcProvider) => {
        setOidcForm({
            name: provider.name,
            slug: provider.slug,
            provider_type: provider.provider_type,
            issuer_url: provider.issuer_url,
            client_id: provider.client_id,
            client_secret: '',
            scopes: provider.scopes,
            auto_provision: provider.auto_provision,
            default_role: provider.default_role,
            email_domains: provider.email_domains.join(', '),
            trust_idp_mfa: provider.trust_idp_mfa,
            enabled: provider.enabled,
        });
        setOidcEditingId(provider.id);
        setShowOidcForm(true);
    };

    const handleOidcDelete = async (id: string) => {
        try {
            const response = await authFetch(`/api/oidc/providers/${id}`, { method: 'DELETE' });
            const data = await response.json();
            if (data.error === 'provider_has_sso_only_users') {
                setError(data.message);
            } else if (data.success) {
                setSuccess('Provider deleted.');
                await loadOidcProviders();
            }
        } catch {
            setError('Failed to delete provider');
        }
        setDeleteConfirm(null);
    };

    const handleOidcTest = async (id: string) => {
        setTestingId(id);
        setTestResult(null);
        try {
            const response = await authFetch(`/api/oidc/providers/${id}/test`, { method: 'POST' });
            const data = await response.json();
            setTestResult({
                success: data.success,
                message: data.success ? `Connected! Authorization endpoint: ${data.authorization_endpoint}` : data.error || 'Connection failed',
            });
        } catch {
            setTestResult({ success: false, message: 'Connection test failed' });
        } finally {
            setTestingId(null);
        }
    };

    const toggleOidcEnabled = async (provider: OidcProvider) => {
        try {
            await authFetch(`/api/oidc/providers/${provider.id}`, { method: 'PUT', body: JSON.stringify({ enabled: !provider.enabled }) });
            await loadOidcProviders();
        } catch {
            setError('Failed to toggle provider');
        }
    };

    // ==================== SAML ====================

    const loadSamlProviders = async () => {
        try {
            const response = await authFetch('/api/saml/providers');
            if (response.ok) {
                const data = await response.json();
                setSamlProviders(data.providers || []);
            }
        } catch {
            // ignore
        } finally {
            setSamlLoading(false);
        }
    };

    const handleSamlSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setSamlSaving(true);
        setError('');
        setSuccess('');
        try {
            const payload = { ...samlForm, email_domains: samlForm.email_domains.split(',').map(d => d.trim()).filter(Boolean) };
            const url = samlEditingId ? `/api/saml/providers/${samlEditingId}` : '/api/saml/providers';
            const method = samlEditingId ? 'PUT' : 'POST';
            const response = await authFetch(url, { method, body: JSON.stringify(payload) });
            if (response.ok) {
                const data = await response.json();
                setSuccess(samlEditingId ? 'SAML provider updated.' : 'SAML provider created.');
                setShowSamlForm(false);
                setSamlEditingId(null);
                setSamlForm(EMPTY_SAML_FORM);
                await loadSamlProviders();
                // Show SP info card for newly created provider
                if (!samlEditingId && data.provider) {
                    setCreatedSamlProvider(data.provider);
                }
            } else {
                const data = await response.json();
                setError(data.error || 'Failed to save SAML provider');
            }
        } catch {
            setError('Failed to save SAML provider');
        } finally {
            setSamlSaving(false);
        }
    };

    const handleSamlEdit = (provider: SamlProvider) => {
        setSamlForm({
            name: provider.name,
            slug: provider.slug,
            provider_type: provider.provider_type,
            idp_entity_id: provider.idp_entity_id,
            idp_sso_url: provider.idp_sso_url,
            idp_slo_url: provider.idp_slo_url || '',
            idp_metadata_url: provider.idp_metadata_url || '',
            idp_signing_certificate: provider.idp_signing_certificate,
            nameid_format: provider.nameid_format,
            sso_binding: provider.sso_binding,
            attribute_email: provider.attribute_email,
            attribute_name: provider.attribute_name,
            auto_provision: provider.auto_provision,
            default_role: provider.default_role,
            email_domains: provider.email_domains.join(', '),
            trust_idp_mfa: provider.trust_idp_mfa,
            enabled: provider.enabled,
        });
        setSamlEditingId(provider.id);
        setShowSamlForm(true);
    };

    const handleSamlDelete = async (id: string) => {
        try {
            const response = await authFetch(`/api/saml/providers/${id}`, { method: 'DELETE' });
            const data = await response.json();
            if (data.error === 'provider_has_sso_only_users') {
                setError(data.message);
            } else if (data.success) {
                setSuccess('SAML provider deleted.');
                await loadSamlProviders();
            }
        } catch {
            setError('Failed to delete SAML provider');
        }
        setDeleteConfirm(null);
    };

    const handleSamlTest = async (id: string) => {
        setTestingId(id);
        setTestResult(null);
        try {
            const response = await authFetch(`/api/saml/providers/${id}/test`, { method: 'POST' });
            const data = await response.json();
            setTestResult({
                success: data.success,
                message: data.success ? `IdP metadata validated: ${data.entity_id || 'OK'}` : data.error || 'Test failed',
            });
        } catch {
            setTestResult({ success: false, message: 'Connection test failed' });
        } finally {
            setTestingId(null);
        }
    };

    const toggleSamlEnabled = async (provider: SamlProvider) => {
        try {
            await authFetch(`/api/saml/providers/${provider.id}`, { method: 'PUT', body: JSON.stringify({ enabled: !provider.enabled }) });
            await loadSamlProviders();
        } catch {
            setError('Failed to toggle provider');
        }
    };

    const copyToClipboard = (text: string) => {
        navigator.clipboard.writeText(text);
        setSuccess('Copied to clipboard.');
        setTimeout(() => setSuccess(''), 2000);
    };

    // ==================== Render ====================

    const loading = oidcLoading || samlLoading;

    if (loading) {
        return (
            <div className="flex items-center justify-center py-12">
                <Loader2 className="w-6 h-6 animate-spin text-primary-500" />
            </div>
        );
    }

    return (
        <div className="space-y-6">
            <div>
                <h2 className="text-lg font-semibold text-gray-900 dark:text-white">Single Sign-On (SSO)</h2>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                    Configure OIDC and SAML identity providers for SSO login
                </p>
            </div>

            {/* Tabs */}
            <div className="border-b border-gray-200 dark:border-gray-700">
                <nav className="-mb-px flex gap-6">
                    {([
                        { key: 'oidc' as Tab, label: 'OIDC Providers', count: oidcProviders.length },
                        { key: 'saml' as Tab, label: 'SAML Providers', count: samlProviders.length },
                        { key: 'mappings' as Tab, label: 'Attribute Mappings' },
                    ]).map(tab => (
                        <button
                            key={tab.key}
                            onClick={() => { setActiveTab(tab.key); setError(''); setSuccess(''); setTestResult(null); }}
                            className={clsx(
                                'py-3 px-1 text-sm font-medium border-b-2 transition-colors',
                                activeTab === tab.key
                                    ? 'border-primary-600 text-primary-600 dark:text-primary-400 dark:border-primary-400'
                                    : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 hover:border-gray-300'
                            )}
                        >
                            {tab.label}
                            {tab.count !== undefined && (
                                <span className={clsx(
                                    'ml-2 px-1.5 py-0.5 rounded-full text-xs',
                                    activeTab === tab.key
                                        ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400'
                                        : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400'
                                )}>
                                    {tab.count}
                                </span>
                            )}
                        </button>
                    ))}
                </nav>
            </div>

            <StatusMessages error={error} success={success} setError={setError} setSuccess={setSuccess} />

            {/* Test Result */}
            {testResult && (
                <div className={clsx('rounded-lg border p-4', testResult.success ? 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800' : 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800')}>
                    <div className="flex items-start gap-2">
                        {testResult.success ? <CheckCircle className="w-5 h-5 text-green-500 mt-0.5" /> : <XCircle className="w-5 h-5 text-red-500 mt-0.5" />}
                        <div>
                            <p className={clsx('text-sm font-medium', testResult.success ? 'text-green-700 dark:text-green-400' : 'text-red-700 dark:text-red-400')}>
                                {testResult.success ? 'Connection Successful' : 'Connection Failed'}
                            </p>
                            <p className={clsx('text-xs mt-1', testResult.success ? 'text-green-600 dark:text-green-500' : 'text-red-600 dark:text-red-500')}>
                                {testResult.message}
                            </p>
                        </div>
                        <button onClick={() => setTestResult(null)} className="ml-auto"><X className="w-4 h-4 text-gray-400" /></button>
                    </div>
                </div>
            )}

            {/* ==================== OIDC Tab ==================== */}
            {activeTab === 'oidc' && (
                <div className="space-y-4">
                    {isAdmin && !showOidcForm && (
                        <div className="flex justify-end">
                            <button
                                onClick={() => { setOidcForm(EMPTY_OIDC_FORM); setOidcEditingId(null); setShowOidcForm(true); }}
                                className="flex items-center gap-2 px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 transition-colors text-sm font-medium"
                            >
                                <Plus className="w-4 h-4" />
                                Add OIDC Provider
                            </button>
                        </div>
                    )}

                    {showOidcForm && (
                        <div className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl p-6">
                            <h3 className="text-md font-semibold text-gray-900 dark:text-white mb-4">
                                {oidcEditingId ? 'Edit OIDC Provider' : 'Add OIDC Provider'}
                            </h3>
                            <form onSubmit={handleOidcSubmit} className="space-y-4">
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Provider Type</label>
                                        <select value={oidcForm.provider_type} onChange={(e) => handleOidcProviderTypeChange(e.target.value)} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm">
                                            {OIDC_PROVIDER_TYPES.map(t => <option key={t.value} value={t.value}>{t.label}</option>)}
                                        </select>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Display Name *</label>
                                        <input type="text" required value={oidcForm.name} onChange={(e) => setOidcForm(prev => ({ ...prev, name: e.target.value }))} placeholder="e.g. Google Workspace" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Slug *</label>
                                        <input type="text" required value={oidcForm.slug} onChange={(e) => setOidcForm(prev => ({ ...prev, slug: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '') }))} placeholder="e.g. google" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Issuer URL *</label>
                                        <input type="url" required value={oidcForm.issuer_url} onChange={(e) => setOidcForm(prev => ({ ...prev, issuer_url: e.target.value }))} placeholder="https://accounts.google.com" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Client ID *</label>
                                        <input type="text" required value={oidcForm.client_id} onChange={(e) => setOidcForm(prev => ({ ...prev, client_id: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Client Secret {oidcEditingId ? '(leave blank to keep)' : '*'}</label>
                                        <div className="relative">
                                            <input type={showSecret ? 'text' : 'password'} required={!oidcEditingId} value={oidcForm.client_secret} onChange={(e) => setOidcForm(prev => ({ ...prev, client_secret: e.target.value }))} className="w-full px-3 py-2 pr-10 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                            <button type="button" onClick={() => setShowSecret(!showSecret)} className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600">
                                                {showSecret ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                            </button>
                                        </div>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Email Domains</label>
                                        <input type="text" value={oidcForm.email_domains} onChange={(e) => setOidcForm(prev => ({ ...prev, email_domains: e.target.value }))} placeholder="acme.com, example.org" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">Comma-separated. Used to auto-discover SSO on login page.</p>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Default Role</label>
                                        <select value={oidcForm.default_role} onChange={(e) => setOidcForm(prev => ({ ...prev, default_role: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm">
                                            <option value="Employee">Employee</option>
                                            <option value="Manager">Manager</option>
                                            <option value="Admin">Admin</option>
                                        </select>
                                    </div>
                                </div>
                                <div className="space-y-3 pt-2">
                                    <ToggleField label="Auto-Provision Users" description="Automatically create accounts for new SSO users." value={oidcForm.auto_provision} onChange={() => setOidcForm(prev => ({ ...prev, auto_provision: !prev.auto_provision }))} />
                                    {oidcForm.auto_provision && (
                                        <div className="rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 p-3">
                                            <div className="flex items-center gap-2">
                                                <AlertTriangle className="w-4 h-4 text-amber-500" />
                                                <p className="text-xs text-amber-700 dark:text-amber-400">Anyone with a valid identity from this provider can create an account.</p>
                                            </div>
                                        </div>
                                    )}
                                    <ToggleField label="Trust IdP MFA" description="Skip ClovaLink 2FA for users authenticated via this provider." value={oidcForm.trust_idp_mfa} onChange={() => setOidcForm(prev => ({ ...prev, trust_idp_mfa: !prev.trust_idp_mfa }))} />
                                    <ToggleField label="Enabled" description="Enable or disable this provider." value={oidcForm.enabled} onChange={() => setOidcForm(prev => ({ ...prev, enabled: !prev.enabled }))} />
                                </div>
                                <div className="flex items-center justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
                                    <button type="button" onClick={() => { setShowOidcForm(false); setOidcEditingId(null); setOidcForm(EMPTY_OIDC_FORM); }} className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors">Cancel</button>
                                    <button type="submit" disabled={oidcSaving} className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-primary-600 rounded-lg hover:bg-primary-700 disabled:opacity-50 transition-colors">
                                        {oidcSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
                                        {oidcEditingId ? 'Update' : 'Create'} Provider
                                    </button>
                                </div>
                            </form>
                        </div>
                    )}

                    {oidcProviders.length === 0 && !showOidcForm ? (
                        <div className="text-center py-12 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl">
                            <Globe className="w-12 h-12 mx-auto text-gray-400 dark:text-gray-500 mb-3" />
                            <h3 className="text-sm font-medium text-gray-900 dark:text-white">No OIDC Providers</h3>
                            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">Add an OIDC provider to enable OpenID Connect SSO.</p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {oidcProviders.map((provider) => (
                                <ProviderCard
                                    key={provider.id}
                                    provider={provider}
                                    isAdmin={isAdmin}
                                    onToggle={() => toggleOidcEnabled(provider)}
                                    onTest={() => handleOidcTest(provider.id)}
                                    onEdit={() => handleOidcEdit(provider)}
                                    onDelete={() => handleOidcDelete(provider.id)}
                                    testingId={testingId}
                                    deleteConfirm={deleteConfirm}
                                    setDeleteConfirm={setDeleteConfirm}
                                    subtitle={provider.issuer_url}
                                />
                            ))}
                        </div>
                    )}
                </div>
            )}

            {/* ==================== SAML Tab ==================== */}
            {activeTab === 'saml' && (
                <div className="space-y-4">
                    {isAdmin && !showSamlForm && (
                        <div className="flex justify-end">
                            <button
                                onClick={() => { setSamlForm(EMPTY_SAML_FORM); setSamlEditingId(null); setShowSamlForm(true); setCreatedSamlProvider(null); }}
                                className="flex items-center gap-2 px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 transition-colors text-sm font-medium"
                            >
                                <Plus className="w-4 h-4" />
                                Add SAML Provider
                            </button>
                        </div>
                    )}

                    {/* SP Info Card (shown after creation) */}
                    {createdSamlProvider && (
                        <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-xl p-5 space-y-3">
                            <h4 className="text-sm font-semibold text-blue-800 dark:text-blue-300">Service Provider Info (configure in your IdP)</h4>
                            {[
                                { label: 'SP Entity ID', value: createdSamlProvider.sp_entity_id },
                                { label: 'ACS URL', value: `${API_URL}/api/auth/saml/acs` },
                                { label: 'Metadata URL', value: `${API_URL}/api/auth/saml/metadata/${createdSamlProvider.id}` },
                            ].map(item => (
                                <div key={item.label} className="flex items-center justify-between bg-white dark:bg-gray-800 rounded-lg p-3">
                                    <div>
                                        <p className="text-xs font-medium text-gray-500 dark:text-gray-400">{item.label}</p>
                                        <p className="text-sm text-gray-900 dark:text-white font-mono break-all">{item.value}</p>
                                    </div>
                                    <button onClick={() => copyToClipboard(item.value)} className="p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
                                        <Copy className="w-4 h-4" />
                                    </button>
                                </div>
                            ))}
                            <button onClick={() => setCreatedSamlProvider(null)} className="text-xs text-blue-600 dark:text-blue-400 hover:underline">Dismiss</button>
                        </div>
                    )}

                    {showSamlForm && (
                        <div className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl p-6">
                            <h3 className="text-md font-semibold text-gray-900 dark:text-white mb-4">
                                {samlEditingId ? 'Edit SAML Provider' : 'Add SAML Provider'}
                            </h3>
                            <form onSubmit={handleSamlSubmit} className="space-y-4">
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Provider Type</label>
                                        <select value={samlForm.provider_type} onChange={(e) => setSamlForm(prev => ({ ...prev, provider_type: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm">
                                            {SAML_PROVIDER_TYPES.map(t => <option key={t.value} value={t.value}>{t.label}</option>)}
                                        </select>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Display Name *</label>
                                        <input type="text" required value={samlForm.name} onChange={(e) => setSamlForm(prev => ({ ...prev, name: e.target.value }))} placeholder="e.g. Okta SAML" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Slug *</label>
                                        <input type="text" required value={samlForm.slug} onChange={(e) => setSamlForm(prev => ({ ...prev, slug: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '') }))} placeholder="e.g. okta-saml" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">IdP Entity ID *</label>
                                        <input type="text" required value={samlForm.idp_entity_id} onChange={(e) => setSamlForm(prev => ({ ...prev, idp_entity_id: e.target.value }))} placeholder="https://idp.example.com/entity" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">IdP SSO URL *</label>
                                        <input type="url" required value={samlForm.idp_sso_url} onChange={(e) => setSamlForm(prev => ({ ...prev, idp_sso_url: e.target.value }))} placeholder="https://idp.example.com/sso" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">IdP SLO URL</label>
                                        <input type="url" value={samlForm.idp_slo_url} onChange={(e) => setSamlForm(prev => ({ ...prev, idp_slo_url: e.target.value }))} placeholder="Optional" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">IdP Metadata URL</label>
                                        <input type="url" value={samlForm.idp_metadata_url} onChange={(e) => setSamlForm(prev => ({ ...prev, idp_metadata_url: e.target.value }))} placeholder="Optional — for metadata auto-fetch" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">NameID Format</label>
                                        <select value={samlForm.nameid_format} onChange={(e) => setSamlForm(prev => ({ ...prev, nameid_format: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm">
                                            {NAMEID_FORMATS.map(f => <option key={f.value} value={f.value}>{f.label}</option>)}
                                        </select>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">SSO Binding</label>
                                        <select value={samlForm.sso_binding} onChange={(e) => setSamlForm(prev => ({ ...prev, sso_binding: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm">
                                            <option value="HTTP-POST">HTTP-POST</option>
                                            <option value="HTTP-Redirect">HTTP-Redirect</option>
                                        </select>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Email Attribute Name</label>
                                        <input type="text" value={samlForm.attribute_email} onChange={(e) => setSamlForm(prev => ({ ...prev, attribute_email: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Display Name Attribute</label>
                                        <input type="text" value={samlForm.attribute_name} onChange={(e) => setSamlForm(prev => ({ ...prev, attribute_name: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Email Domains</label>
                                        <input type="text" value={samlForm.email_domains} onChange={(e) => setSamlForm(prev => ({ ...prev, email_domains: e.target.value }))} placeholder="acme.com, example.org" className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm" />
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">Comma-separated. Used for SSO discovery on login page.</p>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Default Role</label>
                                        <select value={samlForm.default_role} onChange={(e) => setSamlForm(prev => ({ ...prev, default_role: e.target.value }))} className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm">
                                            <option value="Employee">Employee</option>
                                            <option value="Manager">Manager</option>
                                            <option value="Admin">Admin</option>
                                        </select>
                                    </div>
                                </div>

                                {/* Certificate (full width) */}
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        IdP Signing Certificate (PEM) *
                                    </label>
                                    <textarea
                                        required={!samlEditingId}
                                        value={samlForm.idp_signing_certificate}
                                        onChange={(e) => setSamlForm(prev => ({ ...prev, idp_signing_certificate: e.target.value }))}
                                        rows={6}
                                        placeholder="-----BEGIN CERTIFICATE-----&#10;MIIDxTCCA...&#10;-----END CERTIFICATE-----"
                                        className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm font-mono"
                                    />
                                    {samlEditingId && (
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">Leave blank to keep existing certificate.</p>
                                    )}
                                </div>

                                <div className="space-y-3 pt-2">
                                    <ToggleField label="Auto-Provision Users" description="Automatically create accounts for new SSO users." value={samlForm.auto_provision} onChange={() => setSamlForm(prev => ({ ...prev, auto_provision: !prev.auto_provision }))} />
                                    {samlForm.auto_provision && (
                                        <div className="rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 p-3">
                                            <div className="flex items-center gap-2">
                                                <AlertTriangle className="w-4 h-4 text-amber-500" />
                                                <p className="text-xs text-amber-700 dark:text-amber-400">Anyone with a valid identity from this provider can create an account.</p>
                                            </div>
                                        </div>
                                    )}
                                    <ToggleField label="Trust IdP MFA" description="Skip ClovaLink 2FA for users authenticated via this provider." value={samlForm.trust_idp_mfa} onChange={() => setSamlForm(prev => ({ ...prev, trust_idp_mfa: !prev.trust_idp_mfa }))} />
                                    <ToggleField label="Enabled" description="Enable or disable this provider." value={samlForm.enabled} onChange={() => setSamlForm(prev => ({ ...prev, enabled: !prev.enabled }))} />
                                </div>
                                <div className="flex items-center justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
                                    <button type="button" onClick={() => { setShowSamlForm(false); setSamlEditingId(null); setSamlForm(EMPTY_SAML_FORM); }} className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors">Cancel</button>
                                    <button type="submit" disabled={samlSaving} className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-primary-600 rounded-lg hover:bg-primary-700 disabled:opacity-50 transition-colors">
                                        {samlSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
                                        {samlEditingId ? 'Update' : 'Create'} Provider
                                    </button>
                                </div>
                            </form>
                        </div>
                    )}

                    {samlProviders.length === 0 && !showSamlForm ? (
                        <div className="text-center py-12 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl">
                            <Shield className="w-12 h-12 mx-auto text-gray-400 dark:text-gray-500 mb-3" />
                            <h3 className="text-sm font-medium text-gray-900 dark:text-white">No SAML Providers</h3>
                            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">Add a SAML 2.0 provider for enterprise SSO (ADFS, Okta, Azure AD).</p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {samlProviders.map((provider) => (
                                <div key={provider.id}>
                                    <ProviderCard
                                        provider={provider}
                                        isAdmin={isAdmin}
                                        onToggle={() => toggleSamlEnabled(provider)}
                                        onTest={() => handleSamlTest(provider.id)}
                                        onEdit={() => handleSamlEdit(provider)}
                                        onDelete={() => handleSamlDelete(provider.id)}
                                        testingId={testingId}
                                        deleteConfirm={deleteConfirm}
                                        setDeleteConfirm={setDeleteConfirm}
                                        subtitle={provider.idp_entity_id}
                                    />
                                    {/* SP info inline */}
                                    <div className="mt-1 ml-14 flex flex-wrap gap-3 text-[11px] text-gray-400 dark:text-gray-500">
                                        <span>SP Entity ID: <span className="font-mono">{provider.sp_entity_id}</span></span>
                                        <span>|</span>
                                        <button onClick={() => copyToClipboard(`${API_URL}/api/auth/saml/metadata/${provider.id}`)} className="hover:text-gray-600 dark:hover:text-gray-300 underline">
                                            Copy Metadata URL
                                        </button>
                                        <span>|</span>
                                        <button onClick={() => copyToClipboard(`${API_URL}/api/auth/saml/acs`)} className="hover:text-gray-600 dark:hover:text-gray-300 underline">
                                            Copy ACS URL
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            )}

            {/* ==================== Attribute Mappings Tab ==================== */}
            {activeTab === 'mappings' && (
                <SsoAttributeMappings
                    oidcProviders={oidcProviders}
                    samlProviders={samlProviders}
                />
            )}
        </div>
    );
}
