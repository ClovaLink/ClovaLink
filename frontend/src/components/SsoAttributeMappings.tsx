import { useState, useEffect } from 'react';
import {
    Plus,
    Trash2,
    Save,
    X,
    Loader2,
    Check,
    ArrowUpDown,
} from 'lucide-react';
import { useAuthFetch } from '../context/AuthContext';
import clsx from 'clsx';

interface MappingProvider {
    id: string;
    name: string;
    protocol: 'oidc' | 'saml';
}

interface AttributeMapping {
    id: string;
    tenant_id: string;
    protocol: string;
    provider_id: string;
    attribute_name: string;
    attribute_value: string;
    match_type: string;
    target_role: string;
    target_custom_role_id: string | null;
    target_department_id: string | null;
    priority: number;
    enabled: boolean;
    created_at: string;
    updated_at: string;
}

interface MappingFormData {
    attribute_name: string;
    attribute_value: string;
    match_type: string;
    target_role: string;
    target_custom_role_id: string;
    target_department_id: string;
    priority: number;
    enabled: boolean;
}

interface CustomRole {
    id: string;
    name: string;
}

interface Department {
    id: string;
    name: string;
}

const EMPTY_FORM: MappingFormData = {
    attribute_name: '',
    attribute_value: '',
    match_type: 'exact',
    target_role: 'Employee',
    target_custom_role_id: '',
    target_department_id: '',
    priority: 0,
    enabled: true,
};

const BASE_ROLES = ['Employee', 'Manager', 'Admin', 'SuperAdmin'];

interface Props {
    oidcProviders: { id: string; name: string }[];
    samlProviders: { id: string; name: string }[];
}

export function SsoAttributeMappings({ oidcProviders, samlProviders }: Props) {
    const authFetch = useAuthFetch();

    // All providers combined with protocol info
    const allProviders: MappingProvider[] = [
        ...oidcProviders.map(p => ({ ...p, protocol: 'oidc' as const })),
        ...samlProviders.map(p => ({ ...p, protocol: 'saml' as const })),
    ];

    const [selectedProvider, setSelectedProvider] = useState<MappingProvider | null>(allProviders[0] || null);
    const [mappings, setMappings] = useState<AttributeMapping[]>([]);
    const [loading, setLoading] = useState(false);
    const [saving, setSaving] = useState(false);
    const [showForm, setShowForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [form, setForm] = useState<MappingFormData>(EMPTY_FORM);
    const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
    const [error, setError] = useState('');
    const [success, setSuccess] = useState('');

    // Custom roles and departments for dropdowns
    const [customRoles, setCustomRoles] = useState<CustomRole[]>([]);
    const [departments, setDepartments] = useState<Department[]>([]);

    useEffect(() => {
        loadRolesAndDepartments();
    }, []);

    useEffect(() => {
        if (selectedProvider) {
            loadMappings(selectedProvider);
        }
    }, [selectedProvider]);

    const loadRolesAndDepartments = async () => {
        try {
            const [rolesRes, deptsRes] = await Promise.all([
                authFetch('/api/roles').catch(() => null),
                authFetch('/api/departments').catch(() => null),
            ]);
            if (rolesRes?.ok) {
                const data = await rolesRes.json();
                setCustomRoles(data.roles || data || []);
            }
            if (deptsRes?.ok) {
                const data = await deptsRes.json();
                setDepartments(data.departments || data || []);
            }
        } catch {
            // Silently fail
        }
    };

    const loadMappings = async (provider: MappingProvider) => {
        setLoading(true);
        setError('');
        try {
            const response = await authFetch(`/api/sso/mappings/${provider.protocol}/${provider.id}`);
            if (response.ok) {
                const data = await response.json();
                setMappings(data.mappings || []);
            }
        } catch {
            setError('Failed to load mappings');
        } finally {
            setLoading(false);
        }
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!selectedProvider) return;
        setSaving(true);
        setError('');
        setSuccess('');

        try {
            const payload = {
                ...form,
                target_custom_role_id: form.target_custom_role_id || null,
                target_department_id: form.target_department_id || null,
            };

            let response;
            if (editingId) {
                response = await authFetch(`/api/sso/mappings/${editingId}`, {
                    method: 'PUT',
                    body: JSON.stringify(payload),
                });
            } else {
                response = await authFetch(`/api/sso/mappings/${selectedProvider.protocol}/${selectedProvider.id}`, {
                    method: 'POST',
                    body: JSON.stringify(payload),
                });
            }

            if (response.ok) {
                setSuccess(editingId ? 'Mapping updated.' : 'Mapping created.');
                setShowForm(false);
                setEditingId(null);
                setForm(EMPTY_FORM);
                await loadMappings(selectedProvider);
            } else {
                const data = await response.json();
                setError(data.error || 'Failed to save mapping');
            }
        } catch {
            setError('Failed to save mapping');
        } finally {
            setSaving(false);
        }
    };

    const handleEdit = (mapping: AttributeMapping) => {
        setForm({
            attribute_name: mapping.attribute_name,
            attribute_value: mapping.attribute_value,
            match_type: mapping.match_type,
            target_role: mapping.target_role,
            target_custom_role_id: mapping.target_custom_role_id || '',
            target_department_id: mapping.target_department_id || '',
            priority: mapping.priority,
            enabled: mapping.enabled,
        });
        setEditingId(mapping.id);
        setShowForm(true);
    };

    const handleDelete = async (id: string) => {
        try {
            const response = await authFetch(`/api/sso/mappings/${id}`, { method: 'DELETE' });
            if (response.ok) {
                setSuccess('Mapping deleted.');
                if (selectedProvider) await loadMappings(selectedProvider);
            }
        } catch {
            setError('Failed to delete mapping');
        }
        setDeleteConfirm(null);
    };

    const handleProviderChange = (value: string) => {
        const [protocol, id] = value.split(':');
        const provider = allProviders.find(p => p.protocol === protocol && p.id === id);
        setSelectedProvider(provider || null);
        setShowForm(false);
        setEditingId(null);
    };

    if (allProviders.length === 0) {
        return (
            <div className="text-center py-12 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl">
                <ArrowUpDown className="w-12 h-12 mx-auto text-gray-400 dark:text-gray-500 mb-3" />
                <h3 className="text-sm font-medium text-gray-900 dark:text-white">No SSO Providers Configured</h3>
                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                    Add an OIDC or SAML provider first, then configure attribute mappings.
                </p>
            </div>
        );
    }

    return (
        <div className="space-y-4">
            {/* Provider Selector */}
            <div className="flex items-center gap-4">
                <div className="flex-1">
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Provider</label>
                    <select
                        value={selectedProvider ? `${selectedProvider.protocol}:${selectedProvider.id}` : ''}
                        onChange={(e) => handleProviderChange(e.target.value)}
                        className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                    >
                        {allProviders.map(p => (
                            <option key={`${p.protocol}:${p.id}`} value={`${p.protocol}:${p.id}`}>
                                [{p.protocol.toUpperCase()}] {p.name}
                            </option>
                        ))}
                    </select>
                </div>
                {selectedProvider && !showForm && (
                    <button
                        onClick={() => { setForm(EMPTY_FORM); setEditingId(null); setShowForm(true); }}
                        className="flex items-center gap-2 px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 transition-colors text-sm font-medium mt-6"
                    >
                        <Plus className="w-4 h-4" />
                        Add Mapping
                    </button>
                )}
            </div>

            {/* Status Messages */}
            {error && (
                <div className="rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 p-3">
                    <div className="flex items-center gap-2">
                        <p className="text-sm text-red-700 dark:text-red-400">{error}</p>
                        <button onClick={() => setError('')} className="ml-auto"><X className="w-4 h-4 text-red-400" /></button>
                    </div>
                </div>
            )}
            {success && (
                <div className="rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 p-3">
                    <div className="flex items-center gap-2">
                        <p className="text-sm text-green-700 dark:text-green-400">{success}</p>
                        <button onClick={() => setSuccess('')} className="ml-auto"><X className="w-4 h-4 text-green-400" /></button>
                    </div>
                </div>
            )}

            {/* Add/Edit Form */}
            {showForm && selectedProvider && (
                <div className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl p-6">
                    <h3 className="text-md font-semibold text-gray-900 dark:text-white mb-4">
                        {editingId ? 'Edit Mapping' : 'Add Attribute Mapping'}
                    </h3>
                    <form onSubmit={handleSubmit} className="space-y-4">
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Attribute Name *</label>
                                <input
                                    type="text"
                                    required
                                    value={form.attribute_name}
                                    onChange={(e) => setForm(prev => ({ ...prev, attribute_name: e.target.value }))}
                                    placeholder="e.g. groups, roles, memberOf, department"
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                />
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Attribute Value *</label>
                                <input
                                    type="text"
                                    required
                                    value={form.attribute_value}
                                    onChange={(e) => setForm(prev => ({ ...prev, attribute_value: e.target.value }))}
                                    placeholder="e.g. Engineering, IT Admins"
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                />
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Match Type</label>
                                <select
                                    value={form.match_type}
                                    onChange={(e) => setForm(prev => ({ ...prev, match_type: e.target.value }))}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                >
                                    <option value="exact">Exact</option>
                                    <option value="contains">Contains</option>
                                    <option value="regex">Regex</option>
                                </select>
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Base Role *</label>
                                <select
                                    value={form.target_role}
                                    onChange={(e) => setForm(prev => ({ ...prev, target_role: e.target.value }))}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                >
                                    {BASE_ROLES.map(r => <option key={r} value={r}>{r}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Custom Role</label>
                                <select
                                    value={form.target_custom_role_id}
                                    onChange={(e) => setForm(prev => ({ ...prev, target_custom_role_id: e.target.value }))}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                >
                                    <option value="">None</option>
                                    {customRoles.map(r => <option key={r.id} value={r.id}>{r.name}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Target Department</label>
                                <select
                                    value={form.target_department_id}
                                    onChange={(e) => setForm(prev => ({ ...prev, target_department_id: e.target.value }))}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                >
                                    <option value="">None</option>
                                    {departments.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Priority</label>
                                <input
                                    type="number"
                                    value={form.priority}
                                    onChange={(e) => setForm(prev => ({ ...prev, priority: parseInt(e.target.value) || 0 }))}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                />
                                <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">Higher = evaluated first. First match wins.</p>
                            </div>
                            <div className="flex items-end pb-1">
                                <label className="flex items-center gap-2 cursor-pointer">
                                    <button
                                        type="button"
                                        onClick={() => setForm(prev => ({ ...prev, enabled: !prev.enabled }))}
                                        className={clsx(
                                            'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
                                            form.enabled ? 'bg-primary-600' : 'bg-gray-300 dark:bg-gray-600'
                                        )}
                                    >
                                        <span
                                            className={clsx(
                                                'inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform',
                                                form.enabled ? 'translate-x-6' : 'translate-x-1'
                                            )}
                                        />
                                    </button>
                                    <span className="text-sm text-gray-700 dark:text-gray-300">Enabled</span>
                                </label>
                            </div>
                        </div>
                        <div className="flex items-center justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
                            <button type="button" onClick={() => { setShowForm(false); setEditingId(null); setForm(EMPTY_FORM); }} className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors">Cancel</button>
                            <button type="submit" disabled={saving} className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-primary-600 rounded-lg hover:bg-primary-700 disabled:opacity-50 transition-colors">
                                {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
                                {editingId ? 'Update' : 'Create'} Mapping
                            </button>
                        </div>
                    </form>
                </div>
            )}

            {/* Mappings Table */}
            {loading ? (
                <div className="flex items-center justify-center py-12">
                    <Loader2 className="w-6 h-6 animate-spin text-primary-500" />
                </div>
            ) : mappings.length === 0 && !showForm ? (
                <div className="text-center py-8 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl">
                    <ArrowUpDown className="w-10 h-10 mx-auto text-gray-400 dark:text-gray-500 mb-2" />
                    <h3 className="text-sm font-medium text-gray-900 dark:text-white">No Attribute Mappings</h3>
                    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                        Map IdP attributes (groups, roles) to ClovaLink roles and departments.
                    </p>
                </div>
            ) : mappings.length > 0 && (
                <div className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden">
                    <div className="overflow-x-auto">
                        <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                            <thead className="bg-gray-50 dark:bg-gray-700/50">
                                <tr>
                                    <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Attribute</th>
                                    <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Value</th>
                                    <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Match</th>
                                    <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Target Role</th>
                                    <th className="px-4 py-3 text-center text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Priority</th>
                                    <th className="px-4 py-3 text-center text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Status</th>
                                    <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Actions</th>
                                </tr>
                            </thead>
                            <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                                {mappings.sort((a, b) => b.priority - a.priority).map((mapping) => {
                                    const customRole = customRoles.find(r => r.id === mapping.target_custom_role_id);
                                    const dept = departments.find(d => d.id === mapping.target_department_id);
                                    return (
                                        <tr key={mapping.id} className={clsx(!mapping.enabled && 'opacity-50')}>
                                            <td className="px-4 py-3 text-sm text-gray-900 dark:text-white font-medium">{mapping.attribute_name}</td>
                                            <td className="px-4 py-3 text-sm text-gray-700 dark:text-gray-300 font-mono">{mapping.attribute_value}</td>
                                            <td className="px-4 py-3">
                                                <span className="px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300">{mapping.match_type}</span>
                                            </td>
                                            <td className="px-4 py-3 text-sm text-gray-900 dark:text-white">
                                                {mapping.target_role}
                                                {customRole && <span className="text-xs text-gray-500 dark:text-gray-400 ml-1">({customRole.name})</span>}
                                                {dept && <span className="text-xs text-gray-500 dark:text-gray-400 ml-1">/ {dept.name}</span>}
                                            </td>
                                            <td className="px-4 py-3 text-center text-sm text-gray-700 dark:text-gray-300">{mapping.priority}</td>
                                            <td className="px-4 py-3 text-center">
                                                <span className={clsx('px-2 py-0.5 rounded-full text-xs font-medium', mapping.enabled ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400')}>
                                                    {mapping.enabled ? 'On' : 'Off'}
                                                </span>
                                            </td>
                                            <td className="px-4 py-3 text-right">
                                                <div className="flex items-center justify-end gap-1">
                                                    <button onClick={() => handleEdit(mapping)} className="p-1.5 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700">
                                                        <Save className="w-3.5 h-3.5" />
                                                    </button>
                                                    {deleteConfirm === mapping.id ? (
                                                        <div className="flex items-center gap-1">
                                                            <button onClick={() => handleDelete(mapping.id)} className="p-1.5 text-red-600 bg-red-50 dark:bg-red-900/20 rounded-lg hover:bg-red-100">
                                                                <Check className="w-3.5 h-3.5" />
                                                            </button>
                                                            <button onClick={() => setDeleteConfirm(null)} className="p-1.5 text-gray-500 bg-gray-100 dark:bg-gray-700 rounded-lg hover:bg-gray-200">
                                                                <X className="w-3.5 h-3.5" />
                                                            </button>
                                                        </div>
                                                    ) : (
                                                        <button onClick={() => setDeleteConfirm(mapping.id)} className="p-1.5 text-red-500 hover:text-red-700 dark:hover:text-red-400 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/20">
                                                            <Trash2 className="w-3.5 h-3.5" />
                                                        </button>
                                                    )}
                                                </div>
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    </div>
                </div>
            )}
        </div>
    );
}
