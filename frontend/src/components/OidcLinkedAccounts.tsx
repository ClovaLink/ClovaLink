import { useState, useEffect, useCallback } from 'react';
import { Link2, Unlink, Loader2, Shield, AlertCircle, Globe } from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch } from '../context/AuthContext';

const API_URL = import.meta.env.VITE_API_URL || '';

interface SsoIdentity {
    id: string;
    provider_id: string;
    provider_name: string;
    provider_slug: string;
    provider_type: string;
    protocol: 'oidc' | 'saml';
    email: string | null;
    display_name: string | null;
    // OIDC fields
    oidc_email?: string | null;
    oidc_name?: string | null;
    // SAML fields
    saml_email?: string | null;
    saml_name?: string | null;
    last_login_at: string | null;
    login_count: number;
    created_at: string;
}

interface SsoProvider {
    id: string;
    name: string;
    slug: string;
    provider_type: string;
    protocol: 'oidc' | 'saml';
}

export function OidcLinkedAccounts() {
    const authFetch = useAuthFetch();
    const [identities, setIdentities] = useState<SsoIdentity[]>([]);
    const [providers, setProviders] = useState<SsoProvider[]>([]);
    const [loading, setLoading] = useState(true);
    const [unlinking, setUnlinking] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    const loadData = useCallback(async () => {
        try {
            const [oidcIdentRes, samlIdentRes, oidcProvRes, samlProvRes] = await Promise.all([
                authFetch('/api/auth/oidc/identities'),
                authFetch('/api/auth/saml/identities').catch(() => null),
                authFetch('/api/oidc/providers').catch(() => null),
                authFetch('/api/saml/providers').catch(() => null),
            ]);

            const allIdentities: SsoIdentity[] = [];

            if (oidcIdentRes.ok) {
                const data = await oidcIdentRes.json();
                (data.identities || []).forEach((i: any) => {
                    allIdentities.push({
                        ...i,
                        protocol: 'oidc',
                        email: i.oidc_email,
                        display_name: i.oidc_name,
                    });
                });
            }

            if (samlIdentRes?.ok) {
                const data = await samlIdentRes.json();
                (data.identities || []).forEach((i: any) => {
                    allIdentities.push({
                        ...i,
                        protocol: 'saml',
                        email: i.saml_email,
                        display_name: i.saml_name,
                    });
                });
            }

            setIdentities(allIdentities);

            const allProviders: SsoProvider[] = [];

            if (oidcProvRes?.ok) {
                const data = await oidcProvRes.json();
                (data.providers || []).forEach((p: any) => {
                    allProviders.push({ ...p, protocol: 'oidc' });
                });
            }

            if (samlProvRes?.ok) {
                const data = await samlProvRes.json();
                (data.providers || []).forEach((p: any) => {
                    allProviders.push({ ...p, protocol: 'saml' });
                });
            }

            setProviders(allProviders);
        } catch {
            // Silently fail — SSO may not be configured
        } finally {
            setLoading(false);
        }
    }, [authFetch]);

    useEffect(() => {
        loadData();
    }, [loadData]);

    const handleLink = (provider: SsoProvider) => {
        window.location.href = `${API_URL}/api/auth/${provider.protocol}/link/${provider.id}`;
    };

    const handleUnlink = async (identity: SsoIdentity) => {
        setUnlinking(identity.id);
        setError(null);
        try {
            const response = await authFetch(`/api/auth/${identity.protocol}/unlink/${identity.id}`, { method: 'DELETE' });
            const data = await response.json();
            if (data.error === 'cannot_unlink') {
                setError(data.message);
            } else if (data.success) {
                await loadData();
            }
        } catch {
            setError('Failed to unlink account');
        } finally {
            setUnlinking(null);
        }
    };

    // Don't render if SSO isn't configured (no providers, no identities)
    if (!loading && providers.length === 0 && identities.length === 0) {
        return null;
    }

    if (loading) {
        return (
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
                <div className="flex items-center gap-2 text-gray-500">
                    <Loader2 className="w-4 h-4 animate-spin" />
                    <span className="text-sm">Loading SSO accounts...</span>
                </div>
            </div>
        );
    }

    // Find providers not yet linked (match by both ID and protocol)
    const linkedKeys = new Set(identities.map(i => `${i.protocol}:${i.provider_id}`));
    const unlinkedProviders = providers.filter(p => !linkedKeys.has(`${p.protocol}:${p.id}`));

    return (
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6 space-y-4">
            <div className="flex items-center gap-2">
                <Globe className="w-5 h-5 text-gray-500 dark:text-gray-400" />
                <h3 className="text-sm font-semibold text-gray-900 dark:text-white">SSO Linked Accounts</h3>
            </div>

            {error && (
                <div className="flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                    <AlertCircle className="w-4 h-4 text-red-500 flex-shrink-0" />
                    <p className="text-xs text-red-700 dark:text-red-400">{error}</p>
                </div>
            )}

            {/* Linked Identities */}
            {identities.length > 0 && (
                <div className="space-y-2">
                    {identities.map((identity) => (
                        <div
                            key={`${identity.protocol}-${identity.id}`}
                            className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg"
                        >
                            <div className="flex items-center gap-3">
                                <div className="p-2 bg-green-100 dark:bg-green-900/30 rounded-lg">
                                    <Shield className="w-4 h-4 text-green-600 dark:text-green-400" />
                                </div>
                                <div>
                                    <div className="flex items-center gap-2">
                                        <p className="text-sm font-medium text-gray-900 dark:text-white">
                                            {identity.provider_name}
                                        </p>
                                        <span className={clsx(
                                            'px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase',
                                            identity.protocol === 'saml'
                                                ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400'
                                                : 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'
                                        )}>
                                            {identity.protocol}
                                        </span>
                                    </div>
                                    <p className="text-xs text-gray-500 dark:text-gray-400">
                                        {identity.email || identity.display_name || 'Linked'}
                                        {identity.login_count > 0 && ` \u00b7 ${identity.login_count} login${identity.login_count !== 1 ? 's' : ''}`}
                                    </p>
                                </div>
                            </div>
                            <button
                                onClick={() => handleUnlink(identity)}
                                disabled={unlinking === identity.id}
                                className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 rounded-lg hover:bg-red-100 dark:hover:bg-red-900/40 transition-colors disabled:opacity-50"
                            >
                                {unlinking === identity.id ? (
                                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                ) : (
                                    <Unlink className="w-3.5 h-3.5" />
                                )}
                                Unlink
                            </button>
                        </div>
                    ))}
                </div>
            )}

            {/* Available Providers to Link */}
            {unlinkedProviders.length > 0 && (
                <div className="space-y-2">
                    {identities.length > 0 && (
                        <p className="text-xs text-gray-500 dark:text-gray-400 pt-1">
                            Available to link:
                        </p>
                    )}
                    {unlinkedProviders.map((provider) => (
                        <button
                            key={`${provider.protocol}-${provider.id}`}
                            onClick={() => handleLink(provider)}
                            className="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                        >
                            <div className="flex items-center gap-3">
                                <div className="p-2 bg-gray-200 dark:bg-gray-600 rounded-lg">
                                    <Globe className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                                </div>
                                <div className="text-left">
                                    <div className="flex items-center gap-2">
                                        <p className="text-sm font-medium text-gray-900 dark:text-white">
                                            {provider.name}
                                        </p>
                                        <span className={clsx(
                                            'px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase',
                                            provider.protocol === 'saml'
                                                ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400'
                                                : 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'
                                        )}>
                                            {provider.protocol}
                                        </span>
                                    </div>
                                    <p className="text-xs text-gray-500 dark:text-gray-400">
                                        Not linked
                                    </p>
                                </div>
                            </div>
                            <div className="flex items-center gap-1.5 text-xs font-medium text-primary-600 dark:text-primary-400">
                                <Link2 className="w-3.5 h-3.5" />
                                Link
                            </div>
                        </button>
                    ))}
                </div>
            )}

            {identities.length === 0 && unlinkedProviders.length === 0 && (
                <p className="text-sm text-gray-500 dark:text-gray-400">
                    No SSO providers are configured for your organization.
                </p>
            )}
        </div>
    );
}
