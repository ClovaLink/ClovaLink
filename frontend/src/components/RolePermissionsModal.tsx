import { useState, useEffect } from 'react';
import {
    X,
    Shield,
    File,
    Users,
    Settings,
    Activity,
    Building2,
    CheckCircle,
    Check,
    Lock,
    Unlock,
    Save,
    Info
} from 'lucide-react';
import { useAuthFetch } from '../context/AuthContext';
import clsx from 'clsx';

interface Role {
    id: string;
    tenant_id: string | null;
    name: string;
    description: string | null;
    base_role: string;
    is_system: boolean;
}

interface Permission {
    permission: string;
    granted: boolean;
    inherited: boolean;
}

interface RolePermissionsModalProps {
    role: Role;
    isOpen: boolean;
    onClose: () => void;
    onSave: () => void;
    canEdit: boolean;
}

// Permission categories for grouping
const PERMISSION_CATEGORIES = {
    files: {
        label: 'Files',
        icon: File,
        permissions: ['files.view', 'files.upload', 'files.download', 'files.delete', 'files.share'],
    },
    requests: {
        label: 'File Requests',
        icon: File,
        permissions: ['requests.create', 'requests.view'],
    },
    users: {
        label: 'Users',
        icon: Users,
        permissions: ['users.view', 'users.invite', 'users.edit', 'users.delete'],
    },
    roles: {
        label: 'Roles',
        icon: Shield,
        permissions: ['roles.view', 'roles.manage'],
    },
    audit: {
        label: 'Audit',
        icon: Activity,
        permissions: ['audit.view', 'audit.export'],
    },
    settings: {
        label: 'Settings',
        icon: Settings,
        permissions: ['settings.view', 'settings.edit'],
    },
    tenants: {
        label: 'Companies',
        icon: Building2,
        permissions: ['tenants.manage'],
    },
    approvals: {
        label: 'Approvals',
        icon: CheckCircle,
        permissions: ['approvals.view', 'approvals.manage'],
    },
};

const PERMISSION_LABELS: Record<string, string> = {
    'files.view': 'View Files',
    'files.upload': 'Upload Files',
    'files.download': 'Download Files',
    'files.delete': 'Delete Files',
    'files.share': 'Share Files',
    'requests.create': 'Create Requests',
    'requests.view': 'View Requests',
    'users.view': 'View Users',
    'users.invite': 'Invite Users',
    'users.edit': 'Edit Users',
    'users.delete': 'Delete Users',
    'roles.view': 'View Roles',
    'roles.manage': 'Manage Roles',
    'audit.view': 'View Audit Logs',
    'audit.export': 'Export Audit Logs',
    'settings.view': 'View Settings',
    'settings.edit': 'Edit Settings',
    'tenants.manage': 'Manage Companies',
    'approvals.view': 'View Approvals',
    'approvals.manage': 'Manage Approvals',
};

export function RolePermissionsModal({
    role,
    isOpen,
    onClose,
    onSave,
    canEdit,
}: RolePermissionsModalProps) {
    const authFetch = useAuthFetch();
    const [permissions, setPermissions] = useState<Permission[]>([]);
    const [allPermissions, setAllPermissions] = useState<string[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [hasChanges, setHasChanges] = useState(false);

    useEffect(() => {
        if (isOpen && role) {
            fetchPermissions();
        }
    }, [isOpen, role]);

    const fetchPermissions = async () => {
        setIsLoading(true);
        try {
            const response = await authFetch(`/api/roles/${role.id}/permissions`);
            if (response.ok) {
                const data = await response.json();
                setPermissions(data.permissions || []);
                setAllPermissions(data.all_permissions || []);
            }
        } catch (error) {
            console.error('Failed to fetch permissions', error);
        } finally {
            setIsLoading(false);
        }
    };

    const handleTogglePermission = (permissionKey: string) => {
        if (!canEdit) return;

        setPermissions(prev => prev.map(p => {
            if (p.permission === permissionKey) {
                return { ...p, granted: !p.granted, inherited: false };
            }
            return p;
        }));
        setHasChanges(true);
    };

    const handleSave = async () => {
        setIsSaving(true);
        try {
            // Only send non-inherited permissions (custom changes)
            const permissionUpdates = permissions
                .filter(p => !p.inherited)
                .map(p => ({ permission: p.permission, granted: p.granted }));

            const response = await authFetch(`/api/roles/${role.id}/permissions`, {
                method: 'PUT',
                body: JSON.stringify({ permissions: permissionUpdates }),
            });

            if (response.ok) {
                setHasChanges(false);
                onSave();
            }
        } catch (error) {
            console.error('Failed to save permissions', error);
        } finally {
            setIsSaving(false);
        }
    };

    const getPermission = (key: string): Permission | undefined => {
        return permissions.find(p => p.permission === key);
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-2xl w-full max-h-[90vh] flex flex-col">
                {/* Header */}
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
                    <div>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
                            <Shield className="w-5 h-5 text-primary-600" />
                            {role.name} Permissions
                        </h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                            Base level: <span className="font-medium">{role.base_role}</span>
                            {role.is_system && (
                                <span className="ml-2 inline-flex items-center text-amber-600 dark:text-amber-400">
                                    <Lock className="w-3 h-3 mr-1" />
                                    System Role
                                </span>
                            )}
                        </p>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Content */}
                <div className="flex-1 overflow-y-auto p-6">
                    {isLoading ? (
                        <div className="flex items-center justify-center h-48">
                            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
                        </div>
                    ) : (
                        <div className="space-y-6">
                            {/* Legend */}
                            <div className="flex items-center gap-4 text-xs text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-900/50 p-3 rounded-lg">
                                <span className="flex items-center gap-1">
                                    <div className="w-3 h-3 rounded bg-emerald-500"></div>
                                    Granted
                                </span>
                                <span className="flex items-center gap-1">
                                    <div className="w-3 h-3 rounded bg-gray-300 dark:bg-gray-600"></div>
                                    Not Granted
                                </span>
                                <span className="flex items-center gap-1">
                                    <Lock className="w-3 h-3" />
                                    Inherited from base role
                                </span>
                            </div>

                            {/* Permission Categories */}
                            {Object.entries(PERMISSION_CATEGORIES).map(([key, category]) => {
                                const CategoryIcon = category.icon;
                                const categoryPermissions = category.permissions
                                    .map(p => getPermission(p))
                                    .filter((p): p is Permission => p !== undefined);

                                if (categoryPermissions.length === 0) return null;

                                return (
                                    <div key={key} className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
                                        <div className="bg-gray-50 dark:bg-gray-900/50 px-4 py-3 flex items-center gap-2">
                                            <CategoryIcon className="w-4 h-4 text-gray-500" />
                                            <h4 className="font-medium text-gray-900 dark:text-white text-sm">
                                                {category.label}
                                            </h4>
                                        </div>
                                        <div className="divide-y divide-gray-100 dark:divide-gray-700/50">
                                            {categoryPermissions.map((perm) => (
                                                <div
                                                    key={perm.permission}
                                                    className={clsx(
                                                        "px-4 py-3 flex items-center justify-between",
                                                        canEdit && !perm.inherited && "cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700/50"
                                                    )}
                                                    onClick={() => !perm.inherited && handleTogglePermission(perm.permission)}
                                                >
                                                    <div className="flex items-center gap-2">
                                                        <span className="text-sm text-gray-700 dark:text-gray-300">
                                                            {PERMISSION_LABELS[perm.permission] || perm.permission}
                                                        </span>
                                                        {perm.inherited && (
                                                            <Lock className="w-3 h-3 text-gray-400" />
                                                        )}
                                                    </div>
                                                    <div className="flex items-center gap-2">
                                                        {!canEdit || perm.inherited ? (
                                                            <div
                                                                className={clsx(
                                                                    "w-8 h-5 rounded-full flex items-center px-0.5",
                                                                    perm.granted
                                                                        ? "bg-emerald-500/30"
                                                                        : "bg-gray-200 dark:bg-gray-700"
                                                                )}
                                                            >
                                                                <div
                                                                    className={clsx(
                                                                        "w-4 h-4 rounded-full transition-transform",
                                                                        perm.granted
                                                                            ? "bg-emerald-500 translate-x-3"
                                                                            : "bg-gray-400 dark:bg-gray-500"
                                                                    )}
                                                                />
                                                            </div>
                                                        ) : (
                                                            <button
                                                                type="button"
                                                                className={clsx(
                                                                    "w-10 h-6 rounded-full flex items-center px-0.5 transition-colors",
                                                                    perm.granted
                                                                        ? "bg-emerald-500"
                                                                        : "bg-gray-200 dark:bg-gray-700"
                                                                )}
                                                            >
                                                                <div
                                                                    className={clsx(
                                                                        "w-5 h-5 rounded-full bg-white shadow transition-transform",
                                                                        perm.granted ? "translate-x-4" : ""
                                                                    )}
                                                                />
                                                            </button>
                                                        )}
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    </div>
                                );
                            })}

                            {/* Info about inherited permissions */}
                            {!canEdit && (
                                <div className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                                    <Info className="w-5 h-5 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
                                    <div className="text-sm text-amber-800 dark:text-amber-200">
                                        <p className="font-medium">View Only</p>
                                        <p className="mt-0.5">
                                            You don't have permission to modify this role.
                                        </p>
                                    </div>
                                </div>
                            )}

                            {/* SuperAdmin notice */}
                            {role.base_role === 'SuperAdmin' && (
                                <div className="flex items-start gap-3 p-4 bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-800 rounded-lg">
                                    <Shield className="w-5 h-5 text-purple-600 dark:text-purple-400 flex-shrink-0 mt-0.5" />
                                    <div className="text-sm text-purple-800 dark:text-purple-200">
                                        <p className="font-medium">SuperAdmin Role</p>
                                        <p className="mt-0.5">
                                            This role has all permissions by default. SuperAdmin is the highest privilege level.
                                        </p>
                                    </div>
                                </div>
                            )}
                        </div>
                    )}
                </div>

                {/* Footer */}
                <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
                    <div className="text-sm text-gray-500 dark:text-gray-400">
                        {hasChanges && (
                            <span className="text-amber-600 dark:text-amber-400">
                                You have unsaved changes
                            </span>
                        )}
                    </div>
                    <div className="flex items-center gap-3">
                        <button
                            onClick={onClose}
                            className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                        >
                            {canEdit ? 'Cancel' : 'Close'}
                        </button>
                        {canEdit && (
                            <button
                                onClick={handleSave}
                                disabled={!hasChanges || isSaving}
                                className="flex items-center px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 text-sm font-medium disabled:opacity-50 transition-colors"
                            >
                                <Save className="w-4 h-4 mr-2" />
                                {isSaving ? 'Saving...' : 'Save Changes'}
                            </button>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}

