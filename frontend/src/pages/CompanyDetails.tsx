import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import {
    Building2,
    ArrowLeft,
    Save,
    Trash2,
    Users,
    HardDrive,
    Shield,
    Activity,
    CheckCircle,
    XCircle,
    Globe,
    Calendar,
    Settings,
    Plus,
    X,
    Lock,
    Info,
    Bell,
    Mail,
    BellRing,
    AlertTriangle,
    Ban,
    Play
} from 'lucide-react';
import { useAuthFetch, useAuth } from '../context/AuthContext';
import { useGlobalSettings } from '../context/GlobalSettingsContext';
import { useSettings, getComplianceEnforcementSummary, ComplianceMode } from '../context/SettingsContext';
import { InviteUserModal, UserData } from '../components/InviteUserModal';
import { ComplianceBanner } from '../components/ComplianceBanner';
import { ComplianceBadge } from '../components/ComplianceBadge';
import { LockedToggle } from '../components/LockedField';
import { TenantEmailTemplates } from '../components/TenantEmailTemplates';
import { TenantAiSettings } from '../components/TenantAiSettings';
import { TenantDiscordSettings } from '../components/TenantDiscordSettings';
import clsx from 'clsx';

interface Tenant {
    id: string;
    name: string;
    domain: string;
    plan: string;
    status: string;
    compliance_mode: string;
    data_export_enabled?: boolean;
    user_count?: number;
    storage_used_bytes?: number;
    storage_quota_bytes?: number;
    max_upload_size_bytes?: number;
    retention_policy_days?: number;
    created_at: string;
    updated_at?: string;
    smtp_host?: string;
    smtp_port?: number;
    smtp_username?: string;
    smtp_password?: string;
    smtp_from?: string;
    smtp_secure?: boolean;
    enable_totp?: boolean;
    mfa_required?: boolean;
    session_timeout_minutes?: number;
    public_sharing_enabled?: boolean;
}

interface Department {
    id: string;
    name: string;
    user_count: number;
}

export function CompanyDetails() {
    const { slug } = useParams<{ slug: string }>();
    const navigate = useNavigate();
    const authFetch = useAuthFetch();
    const { refreshUser, tenant, user: currentUser } = useAuth();
    const { formatDate } = useGlobalSettings();
    const { restrictions, canModifySetting, refreshRestrictions, setComplianceMode } = useSettings();

    const [company, setCompany] = useState<Tenant | null>(null);
    const [departments, setDepartments] = useState<Department[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [activeTab, setActiveTab] = useState<'overview' | 'settings' | 'departments' | 'users' | 'audit' | 'notifications' | 'email-templates' | 'ai' | 'discord'>('overview');
    
    // Notification settings state
    const [notificationSettings, setNotificationSettings] = useState<any[]>([]);
    const [notificationsByRole, setNotificationsByRole] = useState<Record<string, any[]>>({});
    const [globalNotificationSettings, setGlobalNotificationSettings] = useState<any[]>([]);
    const [selectedNotificationRole, setSelectedNotificationRole] = useState<string | null>(null); // null = All Users (global)
    const [isLoadingNotifications, setIsLoadingNotifications] = useState(false);
    const [isSavingNotifications, setIsSavingNotifications] = useState(false);
    const availableRoles = ['Admin', 'Manager', 'Employee'];
    const [isSaving, setIsSaving] = useState(false);
    const [isSavingAudit, setIsSavingAudit] = useState(false);
    const [auditSettingsLocked, setAuditSettingsLocked] = useState(false);

    // Edit states
    const [editName, setEditName] = useState('');
    const [editDomain, setEditDomain] = useState('');
    const [editQuota, setEditQuota] = useState(1);
    const [editMaxUpload, setEditMaxUpload] = useState(1073741824); // 1GB default in bytes
    const [editCompliance, setEditCompliance] = useState('');
    const [editRetention, setEditRetention] = useState(30);
    const [editDataExportEnabled, setEditDataExportEnabled] = useState(true);
    const [editStatus, setEditStatus] = useState('');
    
    // Blocked extensions
    const [blockedExtensions, setBlockedExtensions] = useState<string[]>([]);
    const [newExtension, setNewExtension] = useState('');

    // Password policy
    const [passwordPolicy, setPasswordPolicy] = useState({
        min_length: 8,
        require_uppercase: true,
        require_lowercase: true,
        require_number: true,
        require_special: false,
        max_age_days: null as number | null,
        prevent_reuse: 0,
    });
    const [isSavingPasswordPolicy, setIsSavingPasswordPolicy] = useState(false);

    // IP restrictions
    const [ipRestrictions, setIpRestrictions] = useState({
        mode: 'disabled',
        allowlist: [] as string[],
        blocklist: [] as string[],
    });
    const [newAllowlistIp, setNewAllowlistIp] = useState('');
    const [newBlocklistIp, setNewBlocklistIp] = useState('');
    const [isSavingIpRestrictions, setIsSavingIpRestrictions] = useState(false);

    // SMTP states
    const [editSmtpHost, setEditSmtpHost] = useState('');
    const [editSmtpPort, setEditSmtpPort] = useState(587);
    const [editSmtpUsername, setEditSmtpUsername] = useState('');
    const [editSmtpPassword, setEditSmtpPassword] = useState('');
    const [editSmtpFrom, setEditSmtpFrom] = useState('');
    const [editSmtpSecure, setEditSmtpSecure] = useState(true);
    const [isTestingSmtp, setIsTestingSmtp] = useState(false);

    // Auth states
    const [editEnableTotp, setEditEnableTotp] = useState(false);

    // Audit settings states
    const [auditLogLogins, setAuditLogLogins] = useState(true);
    const [auditLogFileOperations, setAuditLogFileOperations] = useState(true);
    const [auditLogUserChanges, setAuditLogUserChanges] = useState(true);
    const [auditLogSettingsChanges, setAuditLogSettingsChanges] = useState(true);
    const [auditLogRoleChanges, setAuditLogRoleChanges] = useState(true);
    const [auditRetentionDays, setAuditRetentionDays] = useState(90);
    
    // Check if MFA toggle is locked due to compliance
    const isMfaLocked = restrictions?.mfa_locked || false;
    const isPublicSharingLocked = restrictions?.public_sharing_locked || false;
    const isRetentionLocked = restrictions?.retention_policy_locked || false;
    const minRetentionDays = restrictions?.min_retention_days || null;

    // Department states
    const [newDeptName, setNewDeptName] = useState('');
    const [isAddingDept, setIsAddingDept] = useState(false);

    // User states
    const [users, setUsers] = useState<any[]>([]);
    const [selectedUser, setSelectedUser] = useState<any>(null);
    const [isInviteModalOpen, setIsInviteModalOpen] = useState(false);

    // Danger zone states
    const [showSuspendConfirm, setShowSuspendConfirm] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [suspendReason, setSuspendReason] = useState('');
    const [deleteConfirmName, setDeleteConfirmName] = useState('');
    const [isSuspending, setIsSuspending] = useState(false);
    const [isDeleting, setIsDeleting] = useState(false);

    useEffect(() => {
        if (slug) {
            fetchCompanyDetails();
            fetchAuditSettings();
        }
    }, [slug]);

    useEffect(() => {
        if (activeTab === 'notifications' && company?.id) {
            fetchNotificationSettings(company.id, selectedNotificationRole);
        }
    }, [activeTab, company?.id, selectedNotificationRole]);

    const fetchCompanyDetails = async () => {
        try {
            // Use different endpoint based on role
            // SuperAdmin can search all tenants, others use their accessible tenants
            const isSuperAdmin = currentUser?.role === 'SuperAdmin';
            const endpoint = isSuperAdmin 
                ? `/api/tenants?search=${encodeURIComponent(slug || '')}`
                : `/api/tenants/accessible`;
            
            const response = await authFetch(endpoint);
            if (response.ok) {
                const data = await response.json();
                // Find exact match by name
                const found = data.find((t: Tenant) => t.name === decodeURIComponent(slug || ''));
                if (found) {
                    setCompany(found);
                    setEditName(found.name);
                    setEditDomain(found.domain);
                    setEditQuota(found.storage_quota_bytes ? Math.round(found.storage_quota_bytes / (1024 * 1024 * 1024 * 1024)) : 1);
                    setEditMaxUpload(found.max_upload_size_bytes || 1073741824); // Default 1GB
                    // Normalize compliance_mode: backend uses 'Standard', frontend uses 'none' for standard mode
                    const mode = (found.compliance_mode || 'Standard').toLowerCase();
                    setEditCompliance(mode === 'standard' ? 'none' : mode);
                    setEditRetention(found.retention_policy_days || 30);
                    setEditDataExportEnabled(found.data_export_enabled !== false); // Default true
                    setEditStatus(found.status);

                    setEditSmtpHost(found.smtp_host || '');
                    setEditSmtpPort(found.smtp_port || 587);
                    setEditSmtpUsername(found.smtp_username || '');
                    setEditSmtpPassword(found.smtp_password || '');
                    setEditSmtpFrom(found.smtp_from || '');
                    setEditSmtpSecure(found.smtp_secure !== false); // Default true

                    setEditEnableTotp(found.enable_totp || false);


                    // Fetch departments and users once we have the ID
                    fetchDepartments(found.id);
                    fetchUsers(found.id);
                    fetchBlockedExtensions();
                    fetchPasswordPolicy();
                    fetchIpRestrictions();
                }
            }
        } catch (error) {
            console.error('Failed to fetch company details', error);
        } finally {
            setIsLoading(false);
        }
    };

    const fetchDepartments = async (tenantId: string) => {
        try {
            const res = await authFetch(`/api/departments?tenant_id=${tenantId}`);
            if (res.ok) {
                const data = await res.json();
                setDepartments(data);
            }
        } catch (error) {
            console.error('Failed to fetch departments', error);
        }
    };

    const fetchBlockedExtensions = async () => {
        try {
            const res = await authFetch('/api/settings/blocked-extensions');
            if (res.ok) {
                const data = await res.json();
                setBlockedExtensions(data.blocked_extensions || []);
            }
        } catch (error) {
            console.error('Failed to fetch blocked extensions', error);
        }
    };

    const saveBlockedExtensions = async (extensions: string[]) => {
        try {
            await authFetch('/api/settings/blocked-extensions', {
                method: 'PUT',
                body: JSON.stringify({ blocked_extensions: extensions }),
            });
        } catch (error) {
            console.error('Failed to save blocked extensions', error);
        }
    };

    const fetchPasswordPolicy = async () => {
        try {
            const res = await authFetch('/api/settings/password-policy');
            if (res.ok) {
                const data = await res.json();
                setPasswordPolicy(data);
            }
        } catch (error) {
            console.error('Failed to fetch password policy', error);
        }
    };

    const savePasswordPolicy = async () => {
        setIsSavingPasswordPolicy(true);
        try {
            await authFetch('/api/settings/password-policy', {
                method: 'PUT',
                body: JSON.stringify(passwordPolicy),
            });
        } catch (error) {
            console.error('Failed to save password policy', error);
        } finally {
            setIsSavingPasswordPolicy(false);
        }
    };

    const fetchIpRestrictions = async () => {
        try {
            const res = await authFetch('/api/settings/ip-restrictions');
            if (res.ok) {
                const data = await res.json();
                setIpRestrictions(data);
            }
        } catch (error) {
            console.error('Failed to fetch IP restrictions', error);
        }
    };

    const saveIpRestrictions = async () => {
        setIsSavingIpRestrictions(true);
        try {
            await authFetch('/api/settings/ip-restrictions', {
                method: 'PUT',
                body: JSON.stringify(ipRestrictions),
            });
        } catch (error) {
            console.error('Failed to save IP restrictions', error);
        } finally {
            setIsSavingIpRestrictions(false);
        }
    };

    const handleSaveSettings = async () => {
        if (!company) return;
        setIsSaving(true);
        try {
            const response = await authFetch(`/api/tenants/${company.id}`, {
                method: 'PUT',
                body: JSON.stringify({
                    name: editName,
                    domain: editDomain,
                    storage_quota_bytes: editQuota * 1024 * 1024 * 1024 * 1024,
                    max_upload_size_bytes: editMaxUpload,
                    // Convert frontend 'none' to backend 'Standard' 
                    compliance_mode: editCompliance === 'none' ? 'Standard' : editCompliance.toUpperCase(),
                    retention_policy_days: editRetention,
                    data_export_enabled: editDataExportEnabled,
                    smtp_host: editSmtpHost,
                    smtp_port: editSmtpPort,
                    smtp_username: editSmtpUsername,
                    smtp_password: editSmtpPassword,
                    smtp_from: editSmtpFrom,
                    smtp_secure: editSmtpSecure,
                    enable_totp: editEnableTotp
                }),
            });

            if (response.ok) {
                const updated = await response.json();
                setCompany({ ...company, ...updated });
                // Reload all company data to refresh UI
                await fetchCompanyDetails();
                // If this is the current tenant, immediately update global compliance state
                if (tenant && company.id === tenant.id) {
                    // Directly update the compliance mode in SettingsContext for immediate UI update
                    const newMode = editCompliance === 'none' ? 'Standard' : editCompliance.toUpperCase();
                    const mappedMode: ComplianceMode = 
                        newMode === 'HIPAA' ? 'HIPAA' :
                        newMode === 'SOX' || newMode === 'SOC2' ? 'SOX' :
                        newMode === 'GDPR' ? 'GDPR' : 'Standard';
                    setComplianceMode(mappedMode);
                    // Also refresh restrictions and user data
                    await refreshRestrictions();
                    await refreshUser();
                }
                alert('Settings saved successfully!');
                // If name changed, navigate to new slug
                if (editName !== company.name) {
                    navigate(`/companies/${encodeURIComponent(editName)}`, { replace: true });
                }
            } else {
                const errorText = await response.text();
                console.error('Failed to update company:', response.status, errorText);
                alert(`Failed to save settings: ${response.status} ${response.statusText}`);
            }
        } catch (error) {
            console.error('Failed to update company', error);
            alert('Failed to save settings. Check console for details.');
        } finally {
            setIsSaving(false);
        }
    };

    const handleSuspendCompany = async () => {
        if (!company) return;
        setIsSuspending(true);
        try {
            const response = await authFetch(`/api/tenants/${company.id}/suspend`, {
                method: 'POST',
                body: JSON.stringify({ reason: suspendReason }),
            });

            if (response.ok) {
                setCompany({ ...company, status: 'suspended' });
                setEditStatus('suspended');
                setShowSuspendConfirm(false);
                setSuspendReason('');
                alert('Company suspended successfully');
            } else {
                const errorText = await response.text();
                alert(`Failed to suspend company: ${errorText}`);
            }
        } catch (error) {
            console.error('Failed to suspend company', error);
            alert('An error occurred while suspending the company.');
        } finally {
            setIsSuspending(false);
        }
    };

    const handleUnsuspendCompany = async () => {
        if (!company) return;
        setIsSuspending(true);
        try {
            const response = await authFetch(`/api/tenants/${company.id}/unsuspend`, {
                method: 'POST',
            });

            if (response.ok) {
                setCompany({ ...company, status: 'active' });
                setEditStatus('active');
                alert('Company unsuspended successfully');
            } else {
                const errorText = await response.text();
                alert(`Failed to unsuspend company: ${errorText}`);
            }
        } catch (error) {
            console.error('Failed to unsuspend company', error);
            alert('An error occurred while unsuspending the company.');
        } finally {
            setIsSuspending(false);
        }
    };

    const handleDeleteCompany = async () => {
        if (!company || deleteConfirmName !== company.name) return;
        setIsDeleting(true);
        try {
            const response = await authFetch(`/api/tenants/${company.id}`, {
                method: 'DELETE',
            });

            if (response.ok) {
                alert('Company deleted successfully');
                navigate('/companies');
            } else {
                const errorText = await response.text();
                alert(`Failed to delete company: ${errorText}`);
            }
        } catch (error) {
            console.error('Failed to delete company', error);
            alert('An error occurred while deleting the company.');
        } finally {
            setIsDeleting(false);
        }
    };

    const handleAddDepartment = async () => {
        if (!company || !newDeptName.trim()) return;
        try {
            const response = await authFetch(`/api/departments?tenant_id=${company.id}`, {
                method: 'POST',
                body: JSON.stringify({ name: newDeptName }),
            });

            if (response.ok) {
                setNewDeptName('');
                setIsAddingDept(false);
                fetchDepartments(company.id);
            }
        } catch (error) {
            console.error('Failed to add department', error);
        }
    };

    const handleTestSmtp = async () => {
        if (!company) return;
        setIsTestingSmtp(true);
        try {
            const response = await authFetch(`/api/tenants/${company.id}/smtp/test`, {
                method: 'POST',
                body: JSON.stringify({
                    host: editSmtpHost,
                    port: editSmtpPort,
                    username: editSmtpUsername,
                    password: editSmtpPassword,
                    secure: editSmtpSecure
                }),
            });

            if (response.ok) {
                alert('SMTP Connection Successful!');
            } else {
                alert('SMTP Connection Failed. Please check your settings.');
            }
        } catch (error) {
            console.error('Failed to test SMTP', error);
            alert('An error occurred while testing SMTP.');
        } finally {
            setIsTestingSmtp(false);
        }
    };

    const fetchUsers = async (tenantId: string) => {
        try {
            const res = await authFetch(`/api/users?tenant_id=${tenantId}`);
            if (res.ok) {
                const data = await res.json();
                setUsers(data);
            }
        } catch (error) {
            console.error('Failed to fetch users', error);
        }
    };

    const fetchAuditSettings = async () => {
        try {
            const res = await authFetch('/api/audit-settings');
            if (res.ok) {
                const data = await res.json();
                setAuditLogLogins(data.log_logins ?? true);
                setAuditLogFileOperations(data.log_file_operations ?? true);
                setAuditLogUserChanges(data.log_user_changes ?? true);
                setAuditLogSettingsChanges(data.log_settings_changes ?? true);
                setAuditLogRoleChanges(data.log_role_changes ?? true);
                setAuditRetentionDays(data.retention_days ?? 90);
                setAuditSettingsLocked(data.compliance_locked ?? false);
            }
        } catch (error) {
            console.error('Failed to fetch audit settings', error);
        }
    };

    const fetchNotificationSettings = async (tenantId: string, role?: string | null) => {
        setIsLoadingNotifications(true);
        try {
            const url = role 
                ? `/api/tenants/${tenantId}/notification-settings?role=${role}`
                : `/api/tenants/${tenantId}/notification-settings`;
            const res = await authFetch(url);
            if (res.ok) {
                const data = await res.json();
                if (role) {
                    // Role-specific response
                    setNotificationSettings(data.settings || []);
                } else {
                    // Full response with global and by_role
                    setGlobalNotificationSettings(data.global || []);
                    setNotificationsByRole(data.by_role || {});
                    setNotificationSettings(data.global || []);
                }
            }
        } catch (error) {
            console.error('Failed to fetch notification settings', error);
        } finally {
            setIsLoadingNotifications(false);
        }
    };

    const handleSaveNotificationSetting = async (eventType: string, field: string, value: boolean) => {
        if (!company) return;
        setIsSavingNotifications(true);
        try {
            const res = await authFetch(`/api/tenants/${company.id}/notification-settings`, {
                method: 'PUT',
                body: JSON.stringify({
                    role: selectedNotificationRole,
                    settings: [{
                        event_type: eventType,
                        [field]: value
                    }]
                })
            });
            if (res.ok) {
                const data = await res.json();
                setNotificationSettings(data.settings || data.global || []);
            }
        } catch (error) {
            console.error('Failed to save notification setting', error);
        } finally {
            setIsSavingNotifications(false);
        }
    };

    const notificationLabels: Record<string, { label: string; description: string }> = {
        file_upload: { label: 'File Uploads', description: 'Notifications when files are uploaded to file requests' },
        request_expiring: { label: 'Expiring Requests', description: 'Reminders when file requests are about to expire' },
        user_action: { label: 'User Actions', description: 'Notifications about new users and role changes' },
        compliance_alert: { label: 'Compliance Alerts', description: 'Important compliance-related notifications' },
        storage_warning: { label: 'Storage Warnings', description: 'Alerts when storage quota is running low' },
        file_shared: { label: 'File Sharing', description: 'Notifications when files are shared' }
    };

    const handleSaveAuditSettings = async () => {
        setIsSavingAudit(true);
        try {
            const response = await authFetch('/api/audit-settings', {
                method: 'PUT',
                body: JSON.stringify({
                    log_logins: auditLogLogins,
                    log_file_operations: auditLogFileOperations,
                    log_user_changes: auditLogUserChanges,
                    log_settings_changes: auditLogSettingsChanges,
                    log_role_changes: auditLogRoleChanges,
                    retention_days: auditRetentionDays,
                }),
            });

            if (response.ok) {
                alert('Audit settings saved successfully!');
            } else {
                alert('Failed to save audit settings.');
            }
        } catch (error) {
            console.error('Failed to save audit settings', error);
            alert('An error occurred while saving audit settings.');
        } finally {
            setIsSavingAudit(false);
        }
    };

    const handleUserSubmit = async (data: UserData) => {
        if (!company) return;

        try {
            let response;
            if (selectedUser) {
                // Update existing user
                response = await authFetch(`/api/users/${selectedUser.id}`, {
                    method: 'PUT',
                    body: JSON.stringify({
                        name: data.name,
                        role: data.role,
                        department_id: data.department_id || null,
                        allowed_tenant_ids: data.allowed_tenant_ids,
                    }),
                });
            } else {
                // Create new user
                response = await authFetch('/api/users', {
                    method: 'POST',
                    body: JSON.stringify({
                        ...data,
                        tenant_id: company.id
                    }),
                });
            }

            if (response.ok) {
                setIsInviteModalOpen(false);
                setSelectedUser(null);
                fetchUsers(company.id);
                // Refresh company details to update user count
                fetchCompanyDetails();
            }
        } catch (error) {
            console.error('Failed to save user', error);
        }
    };

    const handleEditUser = (user: any) => {
        setSelectedUser(user);
        setIsInviteModalOpen(true);
    };

    const formatBytes = (bytes?: number) => {
        if (!bytes) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    };

    if (isLoading) {
        return (
            <div className="flex items-center justify-center h-96">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
            </div>
        );
    }

    if (!company) {
        return (
            <div className="text-center py-12">
                <h2 className="text-xl font-semibold text-gray-900 dark:text-white">Company not found</h2>
                <button onClick={() => navigate('/companies')} className="mt-4 text-primary-600 hover:underline">
                    Back to Companies
                </button>
            </div>
        );
    }

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
                <div className="flex items-center gap-4">
                    <button
                        onClick={() => navigate('/companies')}
                        className="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-full transition-colors"
                    >
                        <ArrowLeft className="w-5 h-5 text-gray-500" />
                    </button>
                    <div className="h-12 w-12 rounded-xl bg-gradient-to-br from-primary-500 to-primary-600 flex items-center justify-center text-white shadow-lg">
                        <Building2 className="w-6 h-6" />
                    </div>
                    <div>
                        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{company.name}</h1>
                        <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
                            <Globe className="w-3 h-3" />
                            {company.domain}
                            <span className="mx-1">•</span>
                            <span className={clsx(
                                "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium",
                                company.status === 'active'
                                    ? "bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400"
                                    : "bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400"
                            )}>
                                {company.status.charAt(0).toUpperCase() + company.status.slice(1)}
                            </span>
                        </div>
                    </div>
                </div>
                <div className="flex items-center gap-3">
                    <button
                        onClick={() => setActiveTab('settings')}
                        className="px-4 py-2 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 text-gray-700 dark:text-gray-200 rounded-lg text-sm font-medium hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm transition-all"
                    >
                        Edit Details
                    </button>
                </div>
            </div>

            {/* Tabs */}
            <div className="border-b border-gray-200 dark:border-gray-700">
                <nav className="-mb-px flex space-x-8 overflow-x-auto">
                    {['overview', 'settings', 'departments', 'users', 'notifications', 'email-templates', 'ai', 'discord', 'audit'].map((tab) => (
                        <button
                            key={tab}
                            onClick={() => setActiveTab(tab as any)}
                            className={clsx(
                                "whitespace-nowrap py-4 px-1 border-b-2 font-medium text-sm transition-colors",
                                activeTab === tab
                                    ? "border-primary-500 text-primary-600 dark:text-primary-400"
                                    : "border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                            )}
                        >
                            {tab === 'audit' ? 'Audit Settings' : tab === 'ai' ? 'AI' : tab.charAt(0).toUpperCase() + tab.slice(1)}
                        </button>
                    ))}
                </nav>
            </div>

            {/* Content */}
            <div className="min-h-[400px]">
                {activeTab === 'overview' && (
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                        {/* Stats Cards */}
                        <div className="bg-white dark:bg-gray-800 p-6 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm">
                            <div className="flex items-center justify-between mb-4">
                                <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">Total Users</h3>
                                <Users className="w-5 h-5 text-primary-500" />
                            </div>
                            <div className="text-3xl font-bold text-gray-900 dark:text-white">{company.user_count || 0}</div>
                            <p className="text-xs text-gray-500 mt-1">Active accounts</p>
                        </div>

                        <div className="bg-white dark:bg-gray-800 p-6 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm">
                            <div className="flex items-center justify-between mb-4">
                                <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">Storage Used</h3>
                                <HardDrive className="w-5 h-5 text-blue-500" />
                            </div>
                            <div className="text-3xl font-bold text-gray-900 dark:text-white">{formatBytes(company.storage_used_bytes || 0)}</div>
                            <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5 mt-3">
                                <div
                                    className="bg-blue-500 h-1.5 rounded-full"
                                    style={{ width: `${Math.min(((company.storage_used_bytes || 0) / (company.storage_quota_bytes || 1)) * 100, 100)}%` }}
                                ></div>
                            </div>
                            <p className="text-xs text-gray-500 mt-1">of {formatBytes(company.storage_quota_bytes)} quota</p>
                        </div>

                        <div className="bg-white dark:bg-gray-800 p-6 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm">
                            <div className="flex items-center justify-between mb-4">
                                <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">Compliance</h3>
                                <Shield className="w-5 h-5 text-green-500" />
                            </div>
                            <div className="text-3xl font-bold text-gray-900 dark:text-white">
                                {(() => {
                                    const mode = company.compliance_mode?.toUpperCase();
                                    if (mode === 'HIPAA') return 'HIPAA';
                                    if (mode === 'GDPR') return 'GDPR';
                                    if (mode === 'SOX' || mode === 'SOC2') return 'SOX';
                                    return 'Standard';
                                })()}
                            </div>
                            <p className="text-xs text-gray-500 mt-1">Current mode</p>
                        </div>

                        {/* Details Section */}
                        <div className="md:col-span-2 bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                            <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h3 className="text-lg font-medium text-gray-900 dark:text-white">Company Information</h3>
                            </div>
                            <div className="p-6 grid grid-cols-1 md:grid-cols-2 gap-6">
                                <div>
                                    <label className="text-xs font-medium text-gray-500 uppercase tracking-wider">Company Name</label>
                                    <p className="mt-1 text-sm font-medium text-gray-900 dark:text-white">{company.name}</p>
                                </div>
                                <div>
                                    <label className="text-xs font-medium text-gray-500 uppercase tracking-wider">Domain</label>
                                    <p className="mt-1 text-sm font-medium text-gray-900 dark:text-white">{company.domain}</p>
                                </div>
                                <div>
                                    <label className="text-xs font-medium text-gray-500 uppercase tracking-wider">Created At</label>
                                    <p className="mt-1 text-sm font-medium text-gray-900 dark:text-white">
                                        {formatDate(company.created_at)}
                                    </p>
                                </div>
                                <div>
                                    <label className="text-xs font-medium text-gray-500 uppercase tracking-wider">Status</label>
                                    <div className="mt-1">
                                        <span className={clsx(
                                            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium",
                                            company.status === 'active'
                                                ? "bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-300"
                                                : "bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-300"
                                        )}>
                                            {company.status.toUpperCase()}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'settings' && (
                    <div className="max-w-2xl space-y-6">
                        {/* Compliance Banner */}
                        {editCompliance && editCompliance !== 'none' && editCompliance !== 'Standard' && (
                            <ComplianceBanner mode={editCompliance} />
                        )}

                        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                        <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                            <h3 className="text-lg font-medium text-gray-900 dark:text-white">Edit Configuration</h3>
                        </div>
                        <div className="p-6 space-y-6">
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Company Name</label>
                                <input
                                    type="text"
                                    value={editName}
                                    onChange={(e) => setEditName(e.target.value)}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                />
                            </div>

                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Domain</label>
                                <input
                                    type="text"
                                    value={editDomain}
                                    onChange={(e) => setEditDomain(e.target.value)}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                />
                            </div>

                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Storage Quota (TB)</label>
                                <input
                                    type="number"
                                    min="1"
                                    max="100"
                                    value={editQuota}
                                    onChange={(e) => setEditQuota(Number(e.target.value))}
                                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                />
                            </div>

                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Max Upload Size</label>
                                <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                                    Maximum file size per upload
                                </p>
                                <div className="flex flex-wrap gap-2">
                                    {[
                                        { value: 104857600, label: '100 MB' },
                                        { value: 262144000, label: '250 MB' },
                                        { value: 524288000, label: '500 MB' },
                                        { value: 1073741824, label: '1 GB' },
                                        { value: 2147483648, label: '2 GB' },
                                        { value: 5368709120, label: '5 GB' },
                                    ].map(({ value, label }) => (
                                        <button
                                            key={value}
                                            type="button"
                                            onClick={() => setEditMaxUpload(value)}
                                            className={clsx(
                                                "px-3 py-1.5 text-sm rounded-lg border transition-colors",
                                                editMaxUpload === value
                                                    ? "bg-primary-100 dark:bg-primary-900/30 border-primary-500 text-primary-700 dark:text-primary-300"
                                                    : "bg-white dark:bg-gray-700 border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:border-primary-300"
                                            )}
                                        >
                                            {label}
                                        </button>
                                    ))}
                                </div>
                            </div>

                            <div>
                                <div className="flex items-center justify-between mb-1">
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Compliance Mode</label>
                                    {editCompliance && editCompliance !== 'none' && editCompliance !== 'Standard' && (
                                        <ComplianceBadge mode={editCompliance} size="sm" />
                                    )}
                                </div>
                                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                                    {['none', 'hipaa', 'gdpr', 'sox'].map((mode) => (
                                        <div
                                            key={mode}
                                            onClick={() => setEditCompliance(mode)}
                                            className={clsx(
                                                "relative rounded-lg border p-4 cursor-pointer flex flex-col hover:border-primary-300 dark:hover:border-primary-500 transition-colors",
                                                editCompliance === mode ? "bg-primary-50 dark:bg-primary-900/20 border-primary-500 ring-1 ring-primary-500" : "border-gray-300 dark:border-gray-600"
                                            )}
                                        >
                                            <div className="flex items-center justify-between">
                                                <span className="block text-sm font-medium text-gray-900 dark:text-white">
                                                    {mode === 'none' ? 'Standard' : mode.toUpperCase()}
                                                </span>
                                                {editCompliance === mode && <CheckCircle className="h-5 w-5 text-primary-600 dark:text-primary-400" />}
                                            </div>
                                            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                                                {mode === 'hipaa' && 'Enforces PHI logging, auto-logout, and strict access controls.'}
                                                {mode === 'sox' && 'Enforces audit trails, change management, and security monitoring.'}
                                                {mode === 'gdpr' && 'Enforces data privacy, consent management, and right-to-be-forgotten.'}
                                                {mode === 'none' && 'Standard security features without specific regulatory enforcement.'}
                                            </p>
                                        </div>
                                    ))}
                                </div>
                                {/* Enforcement Summary */}
                                {editCompliance && editCompliance !== 'none' && editCompliance !== 'Standard' && (
                                    <div className="mt-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700">
                                        <h4 className="text-sm font-medium text-gray-900 dark:text-white mb-2 flex items-center gap-2">
                                            <Info className="w-4 h-4" />
                                            Controls enforced in {editCompliance.toUpperCase()} mode:
                                        </h4>
                                        <ul className="space-y-1">
                                            {getComplianceEnforcementSummary(editCompliance).map((item, idx) => (
                                                <li key={idx} className="text-xs text-gray-600 dark:text-gray-400 flex items-start gap-2">
                                                    <CheckCircle className="w-3 h-3 text-green-500 mt-0.5 flex-shrink-0" />
                                                    {item}
                                                </li>
                                            ))}
                                        </ul>
                                    </div>
                                )}
                            </div>

                            <div>
                                <div className="flex items-center justify-between mb-1">
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Recycle Bin Retention</label>
                                    {minRetentionDays && (
                                        <span className="text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1">
                                            <Lock className="w-3 h-3" />
                                            Min {minRetentionDays} days required
                                        </span>
                                    )}
                                </div>
                                <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                                    How long deleted files stay in trash before permanent removal
                                </p>
                                <div className="grid grid-cols-2 gap-4 sm:grid-cols-6">
                                    {[
                                        { value: 30, label: '30 days' },
                                        { value: 90, label: '90 days' },
                                        { value: 365, label: '1 year' },
                                        { value: 2190, label: '6 years' },
                                        { value: 2555, label: '7 years' },
                                        { value: 0, label: 'Never' },
                                    ].map(({ value, label }) => {
                                        // 0 (Never) is always allowed - it's the most conservative option
                                        const isDisabled = value !== 0 && minRetentionDays ? value < minRetentionDays : false;
                                        return (
                                        <div
                                            key={value}
                                            onClick={() => !isDisabled && setEditRetention(value)}
                                            className={clsx(
                                                "relative rounded-lg border p-3 flex flex-col items-center justify-center transition-colors",
                                                isDisabled 
                                                    ? "cursor-not-allowed opacity-50 border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800"
                                                    : "cursor-pointer hover:border-primary-300 dark:hover:border-primary-500",
                                                editRetention === value && !isDisabled
                                                    ? "bg-primary-50 dark:bg-primary-900/20 border-primary-500 ring-1 ring-primary-500"
                                                    : "border-gray-300 dark:border-gray-600"
                                            )}
                                        >
                                            <span className={clsx(
                                                "text-sm font-bold",
                                                isDisabled ? "text-gray-400 dark:text-gray-600" : "text-gray-900 dark:text-white"
                                            )}>{label}</span>
                                            {editRetention === value && !isDisabled && (
                                                <div className="absolute top-1 right-1">
                                                    <CheckCircle className="h-3 w-3 text-primary-600 dark:text-primary-400" />
                                                </div>
                                            )}
                                            {isDisabled && (
                                                <div className="absolute top-1 right-1">
                                                    <Lock className="h-3 w-3 text-gray-400 dark:text-gray-600" />
                                                </div>
                                            )}
                                        </div>
                                    )})}
                                </div>
                            </div>

                            {/* Data Export Toggle */}
                            <div className="flex items-center justify-between p-4 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700">
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Allow User Data Export</label>
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                                        When enabled, users can export their personal data from their profile
                                    </p>
                                </div>
                                <button
                                    onClick={() => setEditDataExportEnabled(!editDataExportEnabled)}
                                    className={clsx(
                                        "relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2",
                                        editDataExportEnabled
                                            ? "bg-primary-600"
                                            : "bg-gray-300 dark:bg-gray-600"
                                    )}
                                >
                                    <span
                                        className={clsx(
                                            "inline-block h-4 w-4 transform rounded-full bg-white transition-transform",
                                            editDataExportEnabled ? "translate-x-6" : "translate-x-1"
                                        )}
                                    />
                                </button>
                            </div>

                            {/* Blocked File Extensions */}
                            <div className="border-t border-gray-200 dark:border-gray-700 pt-6">
                                <h4 className="text-md font-medium text-gray-900 dark:text-white mb-2">Blocked File Extensions</h4>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
                                    Block certain file types from being uploaded. Users will be notified when they try to upload blocked file types.
                                </p>
                                <div className="flex flex-wrap gap-2 mb-3">
                                    {blockedExtensions.map((ext, index) => (
                                        <span
                                            key={index}
                                            className="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300"
                                        >
                                            .{ext}
                                            <button
                                                onClick={() => {
                                                    const updated = blockedExtensions.filter((_, i) => i !== index);
                                                    setBlockedExtensions(updated);
                                                    saveBlockedExtensions(updated);
                                                }}
                                                className="ml-2 hover:text-red-600 dark:hover:text-red-200"
                                            >
                                                <X className="w-3 h-3" />
                                            </button>
                                        </span>
                                    ))}
                                    {blockedExtensions.length === 0 && (
                                        <span className="text-sm text-gray-400 dark:text-gray-500 italic">No extensions blocked</span>
                                    )}
                                </div>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        value={newExtension}
                                        onChange={(e) => setNewExtension(e.target.value.replace(/[^a-zA-Z0-9]/g, '').toLowerCase())}
                                        onKeyDown={(e) => {
                                            if (e.key === 'Enter' && newExtension) {
                                                e.preventDefault();
                                                if (!blockedExtensions.includes(newExtension)) {
                                                    const updated = [...blockedExtensions, newExtension];
                                                    setBlockedExtensions(updated);
                                                    saveBlockedExtensions(updated);
                                                }
                                                setNewExtension('');
                                            }
                                        }}
                                        placeholder="Enter extension (e.g., exe)"
                                        className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                    />
                                    <button
                                        onClick={() => {
                                            if (newExtension && !blockedExtensions.includes(newExtension)) {
                                                const updated = [...blockedExtensions, newExtension];
                                                setBlockedExtensions(updated);
                                                saveBlockedExtensions(updated);
                                                setNewExtension('');
                                            }
                                        }}
                                        className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 text-sm font-medium"
                                    >
                                        Block
                                    </button>
                                </div>
                                <div className="mt-3 flex flex-wrap gap-2">
                                    <span className="text-xs text-gray-500 dark:text-gray-400 mr-2">Quick add:</span>
                                    {['exe', 'bat', 'sh', 'cmd', 'msi', 'dll', 'scr', 'js', 'vbs', 'ps1'].map(ext => (
                                        !blockedExtensions.includes(ext) && (
                                            <button
                                                key={ext}
                                                onClick={() => {
                                                    const updated = [...blockedExtensions, ext];
                                                    setBlockedExtensions(updated);
                                                    saveBlockedExtensions(updated);
                                                }}
                                                className="px-2 py-1 text-xs bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600"
                                            >
                                                +.{ext}
                                            </button>
                                        )
                                    ))}
                                </div>
                            </div>

                            {/* Password Policy Section */}
                            <div className="border-t border-gray-200 dark:border-gray-700 pt-6">
                                <div className="flex items-center justify-between mb-4">
                                    <div>
                                        <h4 className="text-md font-medium text-gray-900 dark:text-white">Password Policy</h4>
                                        <p className="text-sm text-gray-500 dark:text-gray-400">
                                            Configure password requirements for all users in this company
                                        </p>
                                    </div>
                                    <button
                                        onClick={savePasswordPolicy}
                                        disabled={isSavingPasswordPolicy}
                                        className="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 text-sm font-medium disabled:opacity-50 flex items-center gap-2"
                                    >
                                        {isSavingPasswordPolicy ? (
                                            <>
                                                <span className="animate-spin h-4 w-4 border-2 border-white border-t-transparent rounded-full" />
                                                Saving...
                                            </>
                                        ) : (
                                            <>
                                                <Save className="w-4 h-4" />
                                                Save Policy
                                            </>
                                        )}
                                    </button>
                                </div>
                                
                                <div className="space-y-4">
                                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                        <div>
                                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                                Minimum Length
                                            </label>
                                            <input
                                                type="number"
                                                min={4}
                                                max={128}
                                                value={passwordPolicy.min_length}
                                                onChange={(e) => setPasswordPolicy({ ...passwordPolicy, min_length: parseInt(e.target.value) || 8 })}
                                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                            />
                                        </div>
                                        <div>
                                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                                Prevent Password Reuse
                                            </label>
                                            <select
                                                value={passwordPolicy.prevent_reuse}
                                                onChange={(e) => setPasswordPolicy({ ...passwordPolicy, prevent_reuse: parseInt(e.target.value) })}
                                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                            >
                                                <option value={0}>Disabled</option>
                                                <option value={3}>Last 3 passwords</option>
                                                <option value={6}>Last 6 passwords</option>
                                                <option value={12}>Last 12 passwords</option>
                                                <option value={24}>Last 24 passwords</option>
                                            </select>
                                        </div>
                                    </div>
                                    
                                    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                                        <label className="flex items-center p-3 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={passwordPolicy.require_uppercase}
                                                onChange={(e) => setPasswordPolicy({ ...passwordPolicy, require_uppercase: e.target.checked })}
                                                className="form-checkbox h-4 w-4 text-primary-600 rounded border-gray-300 dark:border-gray-600"
                                            />
                                            <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">Uppercase</span>
                                        </label>
                                        <label className="flex items-center p-3 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={passwordPolicy.require_lowercase}
                                                onChange={(e) => setPasswordPolicy({ ...passwordPolicy, require_lowercase: e.target.checked })}
                                                className="form-checkbox h-4 w-4 text-primary-600 rounded border-gray-300 dark:border-gray-600"
                                            />
                                            <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">Lowercase</span>
                                        </label>
                                        <label className="flex items-center p-3 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={passwordPolicy.require_number}
                                                onChange={(e) => setPasswordPolicy({ ...passwordPolicy, require_number: e.target.checked })}
                                                className="form-checkbox h-4 w-4 text-primary-600 rounded border-gray-300 dark:border-gray-600"
                                            />
                                            <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">Number</span>
                                        </label>
                                        <label className="flex items-center p-3 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={passwordPolicy.require_special}
                                                onChange={(e) => setPasswordPolicy({ ...passwordPolicy, require_special: e.target.checked })}
                                                className="form-checkbox h-4 w-4 text-primary-600 rounded border-gray-300 dark:border-gray-600"
                                            />
                                            <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">Special char</span>
                                        </label>
                                    </div>
                                    
                                    <div className="p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg border border-blue-200 dark:border-blue-800">
                                        <p className="text-sm text-blue-700 dark:text-blue-300">
                                            <strong>Current requirements:</strong> {passwordPolicy.min_length}+ characters
                                            {passwordPolicy.require_uppercase && ', uppercase'}
                                            {passwordPolicy.require_lowercase && ', lowercase'}
                                            {passwordPolicy.require_number && ', number'}
                                            {passwordPolicy.require_special && ', special character'}
                                        </p>
                                    </div>
                                </div>
                            </div>

                            {/* IP Restrictions Section */}
                            <div className="border-t border-gray-200 dark:border-gray-700 pt-6">
                                <div className="flex items-center justify-between mb-4">
                                    <div>
                                        <h4 className="text-md font-medium text-gray-900 dark:text-white">IP Restrictions</h4>
                                        <p className="text-sm text-gray-500 dark:text-gray-400">
                                            Restrict access based on IP addresses (supports CIDR notation)
                                        </p>
                                    </div>
                                    <button
                                        onClick={saveIpRestrictions}
                                        disabled={isSavingIpRestrictions}
                                        className="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 text-sm font-medium disabled:opacity-50 flex items-center gap-2"
                                    >
                                        {isSavingIpRestrictions ? (
                                            <>
                                                <span className="animate-spin h-4 w-4 border-2 border-white border-t-transparent rounded-full" />
                                                Saving...
                                            </>
                                        ) : (
                                            <>
                                                <Save className="w-4 h-4" />
                                                Save Restrictions
                                            </>
                                        )}
                                    </button>
                                </div>
                                
                                <div className="space-y-4">
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            Restriction Mode
                                        </label>
                                        <select
                                            value={ipRestrictions.mode}
                                            onChange={(e) => setIpRestrictions({ ...ipRestrictions, mode: e.target.value })}
                                            className="w-full max-w-xs px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        >
                                            <option value="disabled">Disabled</option>
                                            <option value="allowlist_only">Allowlist Only (block all except listed)</option>
                                            <option value="blocklist_only">Blocklist Only (allow all except listed)</option>
                                            <option value="both">Both (must be on allowlist and not on blocklist)</option>
                                        </select>
                                    </div>
                                    
                                    {ipRestrictions.mode !== 'disabled' && (
                                        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                                            {/* Allowlist */}
                                            {(ipRestrictions.mode === 'allowlist_only' || ipRestrictions.mode === 'both') && (
                                                <div>
                                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                        Allowed IPs
                                                    </label>
                                                    <div className="space-y-2">
                                                        {ipRestrictions.allowlist.map((ip, index) => (
                                                            <div key={index} className="flex items-center gap-2 p-2 bg-green-50 dark:bg-green-900/20 rounded border border-green-200 dark:border-green-800">
                                                                <span className="flex-1 text-sm text-green-800 dark:text-green-300 font-mono">{ip}</span>
                                                                <button
                                                                    onClick={() => {
                                                                        const updated = ipRestrictions.allowlist.filter((_, i) => i !== index);
                                                                        setIpRestrictions({ ...ipRestrictions, allowlist: updated });
                                                                    }}
                                                                    className="text-green-600 hover:text-green-800 dark:text-green-400 dark:hover:text-green-200"
                                                                >
                                                                    <X className="w-4 h-4" />
                                                                </button>
                                                            </div>
                                                        ))}
                                                        {ipRestrictions.allowlist.length === 0 && (
                                                            <p className="text-sm text-gray-400 italic">No IPs in allowlist</p>
                                                        )}
                                                        <div className="flex gap-2">
                                                            <input
                                                                type="text"
                                                                value={newAllowlistIp}
                                                                onChange={(e) => setNewAllowlistIp(e.target.value)}
                                                                placeholder="192.168.1.0/24"
                                                                className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-green-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm font-mono"
                                                                onKeyDown={(e) => {
                                                                    if (e.key === 'Enter' && newAllowlistIp.trim()) {
                                                                        e.preventDefault();
                                                                        if (!ipRestrictions.allowlist.includes(newAllowlistIp.trim())) {
                                                                            setIpRestrictions({
                                                                                ...ipRestrictions,
                                                                                allowlist: [...ipRestrictions.allowlist, newAllowlistIp.trim()]
                                                                            });
                                                                        }
                                                                        setNewAllowlistIp('');
                                                                    }
                                                                }}
                                                            />
                                                            <button
                                                                onClick={() => {
                                                                    if (newAllowlistIp.trim() && !ipRestrictions.allowlist.includes(newAllowlistIp.trim())) {
                                                                        setIpRestrictions({
                                                                            ...ipRestrictions,
                                                                            allowlist: [...ipRestrictions.allowlist, newAllowlistIp.trim()]
                                                                        });
                                                                        setNewAllowlistIp('');
                                                                    }
                                                                }}
                                                                className="px-3 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 text-sm"
                                                            >
                                                                <Plus className="w-4 h-4" />
                                                            </button>
                                                        </div>
                                                    </div>
                                                </div>
                                            )}

                                            {/* Blocklist */}
                                            {(ipRestrictions.mode === 'blocklist_only' || ipRestrictions.mode === 'both') && (
                                                <div>
                                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                        Blocked IPs
                                                    </label>
                                                    <div className="space-y-2">
                                                        {ipRestrictions.blocklist.map((ip, index) => (
                                                            <div key={index} className="flex items-center gap-2 p-2 bg-red-50 dark:bg-red-900/20 rounded border border-red-200 dark:border-red-800">
                                                                <span className="flex-1 text-sm text-red-800 dark:text-red-300 font-mono">{ip}</span>
                                                                <button
                                                                    onClick={() => {
                                                                        const updated = ipRestrictions.blocklist.filter((_, i) => i !== index);
                                                                        setIpRestrictions({ ...ipRestrictions, blocklist: updated });
                                                                    }}
                                                                    className="text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-200"
                                                                >
                                                                    <X className="w-4 h-4" />
                                                                </button>
                                                            </div>
                                                        ))}
                                                        {ipRestrictions.blocklist.length === 0 && (
                                                            <p className="text-sm text-gray-400 italic">No IPs in blocklist</p>
                                                        )}
                                                        <div className="flex gap-2">
                                                            <input
                                                                type="text"
                                                                value={newBlocklistIp}
                                                                onChange={(e) => setNewBlocklistIp(e.target.value)}
                                                                placeholder="10.0.0.0/8"
                                                                className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-red-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm font-mono"
                                                                onKeyDown={(e) => {
                                                                    if (e.key === 'Enter' && newBlocklistIp.trim()) {
                                                                        e.preventDefault();
                                                                        if (!ipRestrictions.blocklist.includes(newBlocklistIp.trim())) {
                                                                            setIpRestrictions({
                                                                                ...ipRestrictions,
                                                                                blocklist: [...ipRestrictions.blocklist, newBlocklistIp.trim()]
                                                                            });
                                                                        }
                                                                        setNewBlocklistIp('');
                                                                    }
                                                                }}
                                                            />
                                                            <button
                                                                onClick={() => {
                                                                    if (newBlocklistIp.trim() && !ipRestrictions.blocklist.includes(newBlocklistIp.trim())) {
                                                                        setIpRestrictions({
                                                                            ...ipRestrictions,
                                                                            blocklist: [...ipRestrictions.blocklist, newBlocklistIp.trim()]
                                                                        });
                                                                        setNewBlocklistIp('');
                                                                    }
                                                                }}
                                                                className="px-3 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 text-sm"
                                                            >
                                                                <Plus className="w-4 h-4" />
                                                            </button>
                                                        </div>
                                                    </div>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                    
                                    {ipRestrictions.mode === 'disabled' && (
                                        <div className="p-3 bg-gray-50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-700">
                                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                                IP restrictions are currently disabled. All IP addresses can access this company.
                                            </p>
                                        </div>
                                    )}
                                    
                                    {ipRestrictions.mode !== 'disabled' && (
                                        <div className="p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
                                            <p className="text-sm text-amber-700 dark:text-amber-300">
                                                <AlertTriangle className="w-4 h-4 inline mr-1" />
                                                <strong>Warning:</strong> Be careful not to lock yourself out! Make sure your current IP is in the allowlist before enabling restrictions.
                                            </p>
                                        </div>
                                    )}
                                </div>
                            </div>

                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Encryption Standard</label>
                                <div className="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4 border border-gray-200 dark:border-gray-600 flex items-center">
                                    <Shield className="h-5 w-5 text-green-600 dark:text-green-400 mr-3" />
                                    <div>
                                        <p className="text-sm font-medium text-gray-900 dark:text-white">AES-256-GCM (High Security)</p>
                                        <p className="text-xs text-gray-500 dark:text-gray-400">Data at rest and in transit is encrypted.</p>
                                    </div>
                                </div>
                            </div>

                            <div className="border-t border-gray-200 dark:border-gray-700 pt-6">
                                <h4 className="text-md font-medium text-gray-900 dark:text-white mb-4">SMTP Configuration</h4>
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div className="md:col-span-2">
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">SMTP Host</label>
                                        <input
                                            type="text"
                                            value={editSmtpHost}
                                            onChange={(e) => setEditSmtpHost(e.target.value)}
                                            placeholder="smtp.example.com"
                                            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Port</label>
                                        <input
                                            type="number"
                                            value={editSmtpPort}
                                            onChange={(e) => setEditSmtpPort(Number(e.target.value))}
                                            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        />
                                    </div>
                                    <div className="flex items-center pt-6">
                                        <label className="flex items-center cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={editSmtpSecure}
                                                onChange={(e) => setEditSmtpSecure(e.target.checked)}
                                                className="form-checkbox h-5 w-5 text-primary-600 rounded border-gray-300 dark:border-gray-600 dark:bg-gray-700"
                                            />
                                            <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">Use Secure Connection (TLS)</span>
                                        </label>
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Username</label>
                                        <input
                                            type="text"
                                            value={editSmtpUsername}
                                            onChange={(e) => setEditSmtpUsername(e.target.value)}
                                            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Password</label>
                                        <input
                                            type="password"
                                            value={editSmtpPassword}
                                            onChange={(e) => setEditSmtpPassword(e.target.value)}
                                            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        />
                                    </div>
                                    <div className="md:col-span-2">
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">From Email</label>
                                        <input
                                            type="email"
                                            value={editSmtpFrom}
                                            onChange={(e) => setEditSmtpFrom(e.target.value)}
                                            placeholder="noreply@example.com"
                                            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        />
                                    </div>
                                    <div className="md:col-span-2">
                                        <button
                                            type="button"
                                            onClick={handleTestSmtp}
                                            disabled={isTestingSmtp || !editSmtpHost}
                                            className="text-sm text-primary-600 hover:text-primary-700 font-medium"
                                        >
                                            {isTestingSmtp ? 'Testing...' : 'Test Connection'}
                                        </button>
                                    </div>
                                </div>
                            </div>

                            <div className="border-t border-gray-200 dark:border-gray-700 pt-6">
                                <h4 className="text-md font-medium text-gray-900 dark:text-white mb-4">Authentication Settings</h4>
                                <div className="space-y-4">
                                    <LockedToggle
                                        label="Enable Two-Factor Authentication (TOTP)"
                                        description="Require users to set up an authenticator app."
                                        checked={editEnableTotp || isMfaLocked}
                                        onChange={(checked) => setEditEnableTotp(checked)}
                                        locked={isMfaLocked}
                                        reason="MFA is required by compliance mode and cannot be disabled."
                                    />
                                </div>
                            </div>

                            <div className="pt-4 flex justify-end">
                                <button
                                    onClick={handleSaveSettings}
                                    disabled={isSaving}
                                    className="flex items-center px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50"
                                >
                                    <Save className="w-4 h-4 mr-2" />
                                    {isSaving ? 'Saving...' : 'Save Changes'}
                                </button>
                            </div>
                        </div>

                        {/* Danger Zone - SuperAdmin only */}
                        {currentUser?.role === 'SuperAdmin' && company && tenant?.id !== company.id && (
                            <div className="bg-white dark:bg-gray-800 rounded-xl border-2 border-red-300 dark:border-red-800 shadow-sm overflow-hidden">
                                <div className="px-6 py-4 border-b border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20">
                                    <h3 className="text-lg font-medium text-red-700 dark:text-red-400 flex items-center gap-2">
                                        <AlertTriangle className="w-5 h-5" />
                                        Danger Zone
                                    </h3>
                                    <p className="text-sm text-red-600 dark:text-red-400 mt-1">
                                        These actions are irreversible. Please proceed with caution.
                                    </p>
                                </div>
                                <div className="p-6 space-y-4">
                                    {/* Suspend/Unsuspend Company */}
                                    <div className="flex items-center justify-between p-4 border border-gray-200 dark:border-gray-700 rounded-lg">
                                        <div>
                                            <h4 className="text-sm font-medium text-gray-900 dark:text-white">
                                                {company.status === 'suspended' ? 'Unsuspend Company' : 'Suspend Company'}
                                            </h4>
                                            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                                                {company.status === 'suspended' 
                                                    ? 'Restore access to this company and all its users.'
                                                    : 'Block all users from accessing this company. Data will be preserved.'}
                                            </p>
                                        </div>
                                        {company.status === 'suspended' ? (
                                            <button
                                                onClick={handleUnsuspendCompany}
                                                disabled={isSuspending}
                                                className="flex items-center px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 disabled:opacity-50 text-sm font-medium"
                                            >
                                                <Play className="w-4 h-4 mr-2" />
                                                {isSuspending ? 'Processing...' : 'Unsuspend'}
                                            </button>
                                        ) : (
                                            <button
                                                onClick={() => setShowSuspendConfirm(true)}
                                                disabled={isSuspending}
                                                className="flex items-center px-4 py-2 bg-amber-600 text-white rounded-lg hover:bg-amber-700 disabled:opacity-50 text-sm font-medium"
                                            >
                                                <Ban className="w-4 h-4 mr-2" />
                                                Suspend
                                            </button>
                                        )}
                                    </div>

                                    {/* Suspend Confirmation */}
                                    {showSuspendConfirm && (
                                        <div className="p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg space-y-3">
                                            <p className="text-sm text-amber-800 dark:text-amber-200">
                                                Are you sure you want to suspend <strong>{company.name}</strong>? All users will be blocked from accessing this company.
                                            </p>
                                            <div>
                                                <label className="block text-xs font-medium text-amber-700 dark:text-amber-300 mb-1">
                                                    Reason (optional)
                                                </label>
                                                <textarea
                                                    value={suspendReason}
                                                    onChange={(e) => setSuspendReason(e.target.value)}
                                                    placeholder="Enter a reason for suspension..."
                                                    rows={2}
                                                    className="w-full px-3 py-2 text-sm border border-amber-300 dark:border-amber-700 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-white resize-none"
                                                />
                                            </div>
                                            <div className="flex gap-2">
                                                <button
                                                    onClick={handleSuspendCompany}
                                                    disabled={isSuspending}
                                                    className="flex-1 px-4 py-2 bg-amber-600 text-white rounded-lg text-sm font-medium hover:bg-amber-700 disabled:opacity-50"
                                                >
                                                    {isSuspending ? 'Suspending...' : 'Confirm Suspend'}
                                                </button>
                                                <button
                                                    onClick={() => { setShowSuspendConfirm(false); setSuspendReason(''); }}
                                                    className="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-200 rounded-lg text-sm font-medium hover:bg-gray-300 dark:hover:bg-gray-600"
                                                >
                                                    Cancel
                                                </button>
                                            </div>
                                        </div>
                                    )}

                                    {/* Delete Company */}
                                    <div className="flex items-center justify-between p-4 border border-red-200 dark:border-red-800 rounded-lg bg-red-50/50 dark:bg-red-900/10">
                                        <div>
                                            <h4 className="text-sm font-medium text-red-700 dark:text-red-400">
                                                Delete Company
                                            </h4>
                                            <p className="text-xs text-red-600 dark:text-red-400 mt-1">
                                                Permanently delete this company and ALL associated data. This cannot be undone.
                                            </p>
                                        </div>
                                        <button
                                            onClick={() => setShowDeleteConfirm(true)}
                                            disabled={isDeleting}
                                            className="flex items-center px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 text-sm font-medium"
                                        >
                                            <Trash2 className="w-4 h-4 mr-2" />
                                            Delete
                                        </button>
                                    </div>

                                    {/* Delete Confirmation */}
                                    {showDeleteConfirm && (
                                        <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg space-y-3">
                                            <div className="flex items-start gap-2 text-sm text-red-800 dark:text-red-200">
                                                <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                                                <p>
                                                    <strong>Warning:</strong> This action cannot be undone. All users ({company.user_count || 0}) and files will be permanently deleted.
                                                </p>
                                            </div>
                                            <div>
                                                <label className="block text-xs font-medium text-red-700 dark:text-red-300 mb-1">
                                                    Type <span className="font-mono bg-red-100 dark:bg-red-900/50 px-1 rounded">{company.name}</span> to confirm
                                                </label>
                                                <input
                                                    type="text"
                                                    value={deleteConfirmName}
                                                    onChange={(e) => setDeleteConfirmName(e.target.value)}
                                                    placeholder={company.name}
                                                    className="w-full px-3 py-2 text-sm border border-red-300 dark:border-red-700 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-white font-mono"
                                                />
                                            </div>
                                            <div className="flex gap-2">
                                                <button
                                                    onClick={handleDeleteCompany}
                                                    disabled={isDeleting || deleteConfirmName !== company.name}
                                                    className="flex-1 px-4 py-2 bg-red-600 text-white rounded-lg text-sm font-medium hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed"
                                                >
                                                    {isDeleting ? 'Deleting...' : 'Permanently Delete Company'}
                                                </button>
                                                <button
                                                    onClick={() => { setShowDeleteConfirm(false); setDeleteConfirmName(''); }}
                                                    className="px-4 py-2 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-200 rounded-lg text-sm font-medium hover:bg-gray-300 dark:hover:bg-gray-600"
                                                >
                                                    Cancel
                                                </button>
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </div>
                        )}
                        </div>
                    </div>
                )}

                {activeTab === 'departments' && (
                    <div className="space-y-6">
                        <div className="flex justify-between items-center">
                            <h3 className="text-lg font-medium text-gray-900 dark:text-white">Departments</h3>
                            <button
                                onClick={() => setIsAddingDept(true)}
                                className="flex items-center px-3 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 text-sm font-medium"
                            >
                                <Plus className="w-4 h-4 mr-2" />
                                Add Department
                            </button>
                        </div>

                        {isAddingDept && (
                            <div className="bg-gray-50 dark:bg-gray-800/50 p-4 rounded-lg border border-gray-200 dark:border-gray-700 flex items-end gap-4">
                                <div className="flex-1">
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Department Name</label>
                                    <input
                                        type="text"
                                        value={newDeptName}
                                        onChange={(e) => setNewDeptName(e.target.value)}
                                        placeholder="e.g. Engineering"
                                        className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                    />
                                </div>
                                <button
                                    onClick={handleAddDepartment}
                                    disabled={!newDeptName.trim()}
                                    className="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50 text-sm font-medium"
                                >
                                    Add
                                </button>
                                <button
                                    onClick={() => {
                                        setIsAddingDept(false);
                                        setNewDeptName('');
                                    }}
                                    className="px-4 py-2 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 text-sm font-medium"
                                >
                                    Cancel
                                </button>
                            </div>
                        )}

                        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                            <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                                <thead className="bg-gray-50 dark:bg-gray-900/50">
                                    <tr>
                                        <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Name</th>
                                        <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Users</th>
                                        <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Actions</th>
                                    </tr>
                                </thead>
                                <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                                    {departments.length === 0 ? (
                                        <tr>
                                            <td colSpan={3} className="px-6 py-8 text-center text-gray-500 dark:text-gray-400">
                                                No departments found. Add one to get started.
                                            </td>
                                        </tr>
                                    ) : (
                                        departments.map((dept) => (
                                            <tr key={dept.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/50">
                                                <td className="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900 dark:text-white">
                                                    {dept.name}
                                                </td>
                                                <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                                                    {dept.user_count || 0} users
                                                </td>
                                                <td className="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                                                    <button className="text-red-600 hover:text-red-900 dark:hover:text-red-400">
                                                        <Trash2 className="w-4 h-4" />
                                                    </button>
                                                </td>
                                            </tr>
                                        ))
                                    )}
                                </tbody>
                            </table>
                        </div>
                    </div>
                )}

                {activeTab === 'users' && (
                    <div className="space-y-6">
                        <div className="flex justify-between items-center">
                            <h3 className="text-lg font-medium text-gray-900 dark:text-white">Users</h3>
                            <button
                                onClick={() => {
                                    setSelectedUser(null);
                                    setIsInviteModalOpen(true);
                                }}
                                className="flex items-center px-3 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 text-sm font-medium"
                            >
                                <Plus className="w-4 h-4 mr-2" />
                                Add User
                            </button>
                        </div>

                        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                            <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                                <thead className="bg-gray-50 dark:bg-gray-900/50">
                                    <tr>
                                        <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">User</th>
                                        <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Role</th>
                                        <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Status</th>
                                        <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Actions</th>
                                    </tr>
                                </thead>
                                <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                                    {users.length === 0 ? (
                                        <tr>
                                            <td colSpan={4} className="px-6 py-8 text-center text-gray-500 dark:text-gray-400">
                                                No users found. Add one to get started.
                                            </td>
                                        </tr>
                                    ) : (
                                        users.map((user) => (
                                            <tr key={user.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/50">
                                                <td className="px-6 py-4 whitespace-nowrap">
                                                    <div className="flex items-center">
                                                        <div className="h-8 w-8 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-primary-700 dark:text-primary-400 font-medium text-sm">
                                                            {user.name.charAt(0).toUpperCase()}
                                                        </div>
                                                        <div className="ml-3">
                                                            <div className="text-sm font-medium text-gray-900 dark:text-white">{user.name}</div>
                                                            <div className="text-xs text-gray-500 dark:text-gray-400">{user.email}</div>
                                                        </div>
                                                    </div>
                                                </td>
                                                <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                                                    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-300">
                                                        {user.role}
                                                    </span>
                                                </td>
                                                <td className="px-6 py-4 whitespace-nowrap">
                                                    <span className={clsx(
                                                        "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium",
                                                        user.status === 'active'
                                                            ? "bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-300"
                                                            : "bg-gray-100 dark:bg-gray-800 text-gray-800 dark:text-gray-300"
                                                    )}>
                                                        {user.status}
                                                    </span>
                                                </td>
                                                <td className="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                                                    <button
                                                        onClick={() => handleEditUser(user)}
                                                        className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                                                    >
                                                        <Settings className="w-4 h-4" />
                                                    </button>
                                                </td>
                                            </tr>
                                        ))
                                    )}
                                </tbody>
                            </table>
                        </div>
                    </div>
                )}

                {activeTab === 'notifications' && (
                    <div className="max-w-3xl space-y-6">
                        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                            <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h3 className="text-lg font-medium text-gray-900 dark:text-white flex items-center gap-2">
                                    <Bell className="w-5 h-5 text-primary-600" />
                                    Company Notification Settings
                                </h3>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                                    Configure notification policies per role. SuperAdmins are exempt from company controls.
                                </p>
                            </div>
                            
                            {/* Role Tabs */}
                            <div className="px-6 py-3 bg-gray-50 dark:bg-gray-800/50 border-b border-gray-200 dark:border-gray-700">
                                <div className="flex flex-wrap gap-2">
                                    <button
                                        onClick={() => setSelectedNotificationRole(null)}
                                        className={clsx(
                                            "px-4 py-2 rounded-lg text-sm font-medium transition-colors",
                                            selectedNotificationRole === null
                                                ? "bg-primary-600 text-white"
                                                : "bg-white dark:bg-gray-700 text-gray-700 dark:text-gray-200 border border-gray-200 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-600"
                                        )}
                                    >
                                        All Users (Default)
                                    </button>
                                    {availableRoles.map((role) => (
                                        <button
                                            key={role}
                                            onClick={() => setSelectedNotificationRole(role)}
                                            className={clsx(
                                                "px-4 py-2 rounded-lg text-sm font-medium transition-colors",
                                                selectedNotificationRole === role
                                                    ? "bg-primary-600 text-white"
                                                    : "bg-white dark:bg-gray-700 text-gray-700 dark:text-gray-200 border border-gray-200 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-600"
                                            )}
                                        >
                                            {role}
                                            {notificationsByRole[role]?.length > 0 && (
                                                <span className="ml-2 text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 px-1.5 py-0.5 rounded">
                                                    Custom
                                                </span>
                                            )}
                                        </button>
                                    ))}
                                </div>
                                {selectedNotificationRole && (
                                    <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                                        Settings here override "All Users" defaults for {selectedNotificationRole} role.
                                        Inherited settings are marked.
                                    </p>
                                )}
                            </div>
                            
                            {isLoadingNotifications ? (
                                <div className="p-6 space-y-4">
                                    {[...Array(4)].map((_, i) => (
                                        <div key={i} className="animate-pulse flex items-center justify-between">
                                            <div className="space-y-2">
                                                <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded w-32"></div>
                                                <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded w-48"></div>
                                            </div>
                                            <div className="flex space-x-4">
                                                <div className="h-6 w-12 bg-gray-200 dark:bg-gray-700 rounded"></div>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            ) : (
                                <div className="divide-y divide-gray-200 dark:divide-gray-700">
                                    {notificationSettings.map((setting) => {
                                        const labelInfo = notificationLabels[setting.event_type] || { 
                                            label: setting.event_type, 
                                            description: '' 
                                        };
                                        const isInherited = setting.inherited === true;
                                        return (
                                            <div key={setting.event_type} className={clsx(
                                                "p-6",
                                                isInherited && "bg-gray-50/50 dark:bg-gray-800/30"
                                            )}>
                                                <div className="flex items-start justify-between">
                                                    <div className="flex-1">
                                                        <div className="flex items-center gap-2">
                                                            <h4 className="text-sm font-medium text-gray-900 dark:text-white">
                                                                {labelInfo.label}
                                                            </h4>
                                                            {isInherited && (
                                                                <span className="text-xs bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400 px-2 py-0.5 rounded">
                                                                    Inherited
                                                                </span>
                                                            )}
                                                        </div>
                                                        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                                                            {labelInfo.description}
                                                        </p>
                                                    </div>
                                                    <div className="ml-4">
                                                        <label className="flex items-center space-x-2 cursor-pointer">
                                                            <input
                                                                type="checkbox"
                                                                checked={setting.enabled}
                                                                onChange={(e) => handleSaveNotificationSetting(setting.event_type, 'enabled', e.target.checked)}
                                                                disabled={isSavingNotifications}
                                                                className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                                                            />
                                                            <span className="text-sm text-gray-600 dark:text-gray-300">Enabled</span>
                                                        </label>
                                                    </div>
                                                </div>
                                                
                                                {setting.enabled && (
                                                    <div className="mt-4 pt-4 border-t border-gray-100 dark:border-gray-700">
                                                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                                            {/* Enforce Email */}
                                                            <div className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                                                                <div className="flex items-center space-x-2">
                                                                    <Mail className="w-4 h-4 text-gray-400" />
                                                                    <div>
                                                                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Force Email</span>
                                                                        <p className="text-xs text-gray-500 dark:text-gray-400">Users cannot disable</p>
                                                                    </div>
                                                                </div>
                                                                <input
                                                                    type="checkbox"
                                                                    checked={setting.email_enforced}
                                                                    onChange={(e) => handleSaveNotificationSetting(setting.event_type, 'email_enforced', e.target.checked)}
                                                                    disabled={isSavingNotifications}
                                                                    className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                                                                />
                                                            </div>
                                                            
                                                            {/* Enforce In-App */}
                                                            <div className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                                                                <div className="flex items-center space-x-2">
                                                                    <BellRing className="w-4 h-4 text-gray-400" />
                                                                    <div>
                                                                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Force In-App</span>
                                                                        <p className="text-xs text-gray-500 dark:text-gray-400">Users cannot disable</p>
                                                                    </div>
                                                                </div>
                                                                <input
                                                                    type="checkbox"
                                                                    checked={setting.in_app_enforced}
                                                                    onChange={(e) => handleSaveNotificationSetting(setting.event_type, 'in_app_enforced', e.target.checked)}
                                                                    disabled={isSavingNotifications}
                                                                    className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                                                                />
                                                            </div>
                                                            
                                                            {/* Default Email */}
                                                            <div className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                                                                <div className="flex items-center space-x-2">
                                                                    <Mail className="w-4 h-4 text-gray-400" />
                                                                    <div>
                                                                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Default Email</span>
                                                                        <p className="text-xs text-gray-500 dark:text-gray-400">For new users</p>
                                                                    </div>
                                                                </div>
                                                                <input
                                                                    type="checkbox"
                                                                    checked={setting.default_email}
                                                                    onChange={(e) => handleSaveNotificationSetting(setting.event_type, 'default_email', e.target.checked)}
                                                                    disabled={isSavingNotifications || setting.email_enforced}
                                                                    className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500 disabled:opacity-50"
                                                                />
                                                            </div>
                                                            
                                                            {/* Default In-App */}
                                                            <div className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                                                                <div className="flex items-center space-x-2">
                                                                    <BellRing className="w-4 h-4 text-gray-400" />
                                                                    <div>
                                                                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Default In-App</span>
                                                                        <p className="text-xs text-gray-500 dark:text-gray-400">For new users</p>
                                                                    </div>
                                                                </div>
                                                                <input
                                                                    type="checkbox"
                                                                    checked={setting.default_in_app}
                                                                    onChange={(e) => handleSaveNotificationSetting(setting.event_type, 'default_in_app', e.target.checked)}
                                                                    disabled={isSavingNotifications || setting.in_app_enforced}
                                                                    className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500 disabled:opacity-50"
                                                                />
                                                            </div>
                                                        </div>
                                                    </div>
                                                )}
                                            </div>
                                        );
                                    })}
                                </div>
                            )}
                        </div>
                        
                        <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
                            <div className="flex">
                                <Info className="w-5 h-5 text-blue-500 mr-3 flex-shrink-0 mt-0.5" />
                                <div>
                                    <h4 className="text-sm font-medium text-blue-800 dark:text-blue-300">How it works</h4>
                                    <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                                        <strong>Role-based:</strong> Select a role to configure specific settings. Role settings override "All Users" defaults.<br/>
                                        <strong>SuperAdmins:</strong> Are exempt from company controls (they manage the platform).<br/>
                                        <strong>Force Email/In-App:</strong> When enabled, users of that role cannot disable this delivery method.<br/>
                                        <strong>Inherited:</strong> Settings marked as inherited come from "All Users" defaults.
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'email-templates' && company && (
                    <TenantEmailTemplates
                        tenantId={company.id}
                        authFetch={authFetch}
                    />
                )}

                {activeTab === 'ai' && company && (
                    <TenantAiSettings
                        tenantId={company.id}
                        authFetch={authFetch}
                    />
                )}

                {activeTab === 'discord' && company && (
                    <TenantDiscordSettings
                        tenantId={company.id}
                        authFetch={authFetch}
                    />
                )}

                {activeTab === 'audit' && (
                    <div className="max-w-2xl space-y-6">
                        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                            <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h3 className="text-lg font-medium text-gray-900 dark:text-white flex items-center gap-2">
                                    <Activity className="w-5 h-5 text-primary-600" />
                                    Audit Log Configuration
                                </h3>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                                    Configure which activities are tracked in your audit logs
                                </p>
                            </div>
                            <div className="p-6 space-y-4">
                                <LockedToggle
                                    label="Log Login Events"
                                    description="Track user login attempts, successes, and failures"
                                    checked={auditLogLogins || auditSettingsLocked}
                                    onChange={setAuditLogLogins}
                                    locked={auditSettingsLocked}
                                    reason="Audit logging is required by compliance mode and cannot be disabled."
                                />

                                <LockedToggle
                                    label="Log File Operations"
                                    description="Track file uploads, downloads, deletions, and shares"
                                    checked={auditLogFileOperations || auditSettingsLocked}
                                    onChange={setAuditLogFileOperations}
                                    locked={auditSettingsLocked}
                                    reason="Audit logging is required by compliance mode and cannot be disabled."
                                />

                                <LockedToggle
                                    label="Log User Changes"
                                    description="Track user creation, updates, and deletions"
                                    checked={auditLogUserChanges || auditSettingsLocked}
                                    onChange={setAuditLogUserChanges}
                                    locked={auditSettingsLocked}
                                    reason="Audit logging is required by compliance mode and cannot be disabled."
                                />

                                <LockedToggle
                                    label="Log Settings Changes"
                                    description="Track changes to company and system settings"
                                    checked={auditLogSettingsChanges || auditSettingsLocked}
                                    onChange={setAuditLogSettingsChanges}
                                    locked={auditSettingsLocked}
                                    reason="Audit logging is required by compliance mode and cannot be disabled."
                                />

                                <LockedToggle
                                    label="Log Role Changes"
                                    description="Track role assignments and permission modifications"
                                    checked={auditLogRoleChanges || auditSettingsLocked}
                                    onChange={setAuditLogRoleChanges}
                                    locked={auditSettingsLocked}
                                    reason="Audit logging is required by compliance mode and cannot be disabled."
                                />
                            </div>
                        </div>

                        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                            <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h3 className="text-lg font-medium text-gray-900 dark:text-white">Log Retention</h3>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                                    How long to keep audit logs before automatic deletion
                                </p>
                            </div>
                            <div className="p-6">
                                <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
                                    {[30, 60, 90, 180, 365, 730].map((days) => (
                                        <div
                                            key={days}
                                            onClick={() => setAuditRetentionDays(days)}
                                            className={clsx(
                                                "relative rounded-lg border p-4 cursor-pointer flex flex-col items-center justify-center hover:border-primary-300 dark:hover:border-primary-500 transition-colors",
                                                auditRetentionDays === days
                                                    ? "bg-primary-50 dark:bg-primary-900/20 border-primary-500 ring-1 ring-primary-500"
                                                    : "border-gray-300 dark:border-gray-600"
                                            )}
                                        >
                                            <span className="text-xl font-bold text-gray-900 dark:text-white">{days}</span>
                                            <span className="text-xs text-gray-500 dark:text-gray-400">days</span>
                                            {auditRetentionDays === days && (
                                                <div className="absolute top-1 right-1">
                                                    <CheckCircle className="h-4 w-4 text-primary-600 dark:text-primary-400" />
                                                </div>
                                            )}
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </div>

                        <div className="flex justify-end">
                            <button
                                onClick={handleSaveAuditSettings}
                                disabled={isSavingAudit}
                                className="flex items-center px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50"
                            >
                                <Save className="w-4 h-4 mr-2" />
                                {isSavingAudit ? 'Saving...' : 'Save Audit Settings'}
                            </button>
                        </div>

                        <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
                            <p className="text-sm text-blue-800 dark:text-blue-200">
                                <strong>Note:</strong> View and export your audit logs from the{' '}
                                <a href="/audit-logs" className="underline hover:no-underline">Audit Logs</a> page.
                            </p>
                        </div>
                    </div>
                )}
            </div>

            <InviteUserModal
                isOpen={isInviteModalOpen}
                onClose={() => {
                    setIsInviteModalOpen(false);
                    setSelectedUser(null);
                }}
                onSubmit={handleUserSubmit}
                targetTenantId={company.id}
                initialData={selectedUser ? {
                    name: selectedUser.name,
                    email: selectedUser.email,
                    role: selectedUser.role,
                    department_id: selectedUser.department_id || '',
                    allowed_department_ids: selectedUser.allowed_department_ids || [],
                    password: '', // Password not editable here
                    allowed_tenant_ids: selectedUser.allowed_tenant_ids || [],
                } : undefined}
            />
        </div>
    );
}
