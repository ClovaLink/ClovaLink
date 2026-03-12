import { useState, useRef, useEffect, useCallback } from 'react';
import {
    Download, Upload, Shield, Eye, Loader2, CheckCircle, AlertTriangle,
    Lock, FileText, X, Building2, Settings, Database,
    Clock, User, Trash2, Info, HardDrive, RefreshCw, Calendar, RotateCcw,
    Code, ChevronDown, ChevronUp
} from 'lucide-react';
import { useAuth } from '../context/AuthContext';
import { PasswordConfirmModal } from './PasswordConfirmModal';
import clsx from 'clsx';

interface BackupRestoreProps {
    type: 'global' | 'tenant';
    tenantId?: string;
}

// Categorized sections with human-readable names
const SECTION_CATEGORIES = [
    {
        name: 'Organization',
        icon: Building2,
        sections: [
            { key: 'users', label: 'Users', description: 'Accounts, roles, department assignments' },
            { key: 'departments', label: 'Departments', description: 'Organization structure' },
            { key: 'roles', label: 'Custom Roles', description: 'Roles and permission grants' },
        ],
    },
    {
        name: 'Settings',
        icon: Settings,
        sections: [
            { key: 'tenant_core', label: 'Core Settings', description: 'Compliance, SMTP, auth, storage config' },
            { key: 'settings_audit', label: 'Audit Settings', description: 'What activities get logged' },
            { key: 'settings_virus_scan', label: 'Virus Scan', description: 'ClamAV scan configuration' },
            { key: 'settings_ai', label: 'AI Config', description: 'AI provider and model settings' },
            { key: 'settings_discord', label: 'Discord', description: 'Webhook integration settings' },
        ],
    },
    {
        name: 'Security',
        icon: Shield,
        sections: [
            { key: 'sso_oidc', label: 'OIDC Providers', description: 'OpenID Connect SSO providers' },
            { key: 'sso_saml', label: 'SAML Providers', description: 'SAML 2.0 SSO providers' },
            { key: 'sso_mappings', label: 'SSO Mappings', description: 'Attribute-to-role mappings' },
            { key: 'sso_identities', label: 'SSO Accounts', description: 'Linked SSO user accounts' },
        ],
    },
    {
        name: 'Workflow',
        icon: CheckCircle,
        sections: [
            { key: 'approval_policies', label: 'Approval Policies', description: 'Document approval rules' },
            { key: 'email_templates', label: 'Email Templates', description: 'Custom template overrides' },
            { key: 'notification_settings', label: 'Notifications', description: 'Per-event notification config' },
        ],
    },
];

const OPTIONAL_SECTIONS = [
    { key: 'file_metadata', label: 'File Metadata', description: 'All file records and shares (not actual files)' },
    { key: 'audit_logs', label: 'Audit Logs', description: 'Activity history' },
    { key: 'approval_history', label: 'Approval History', description: 'Approval request decisions' },
];

const ALL_CORE_KEYS = SECTION_CATEGORIES.flatMap(c => c.sections.map(s => s.key));

const GLOBAL_SECTION_CATEGORIES = [
    {
        name: 'Global',
        icon: Settings,
        sections: [
            { key: 'global_settings', label: 'Global Settings', description: 'Platform-wide configuration' },
            { key: 'global_email_templates', label: 'Email Templates', description: 'Default email templates' },
        ],
    },
];
const ALL_GLOBAL_KEYS = GLOBAL_SECTION_CATEGORIES.flatMap(c => c.sections.map(s => s.key));

interface SavedBackup {
    id: string;
    filename: string;
    size_bytes: number;
    is_auto_backup: boolean;
    status: string;
    error_message?: string;
    duration_ms?: number;
    created_at: string;
}

export function BackupRestore({ type, tenantId }: BackupRestoreProps) {
    const { token, user } = useAuth();
    const fileInputRef = useRef<HTMLInputElement>(null);
    const isSuperAdmin = user?.role === 'SuperAdmin';

    // Export state
    const [exportPassphrase, setExportPassphrase] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [selectedSections, setSelectedSections] = useState<string[]>(ALL_CORE_KEYS);
    const [selectedGlobalSections, setSelectedGlobalSections] = useState<string[]>(ALL_GLOBAL_KEYS);
    const [selectedOptional, setSelectedOptional] = useState<string[]>([]);
    const [includeSecrets, setIncludeSecrets] = useState(false);
    const [exportDest, setExportDest] = useState<'download' | 'storage'>('download');
    const [isExporting, setIsExporting] = useState(false);
    const [exportError, setExportError] = useState<string | null>(null);
    const [exportSuccess, setExportSuccess] = useState(false);

    // Pagination params for large sections
    const [auditDays, setAuditDays] = useState(90);
    const [fileLimit, setFileLimit] = useState(50000);
    const [approvalDays, setApprovalDays] = useState(90);

    // Section counts
    const [sectionCounts, setSectionCounts] = useState<Record<string, number>>({});

    // Saved backups
    const [savedBackups, setSavedBackups] = useState<SavedBackup[]>([]);
    const [loadingSaved, setLoadingSaved] = useState(false);

    // Circuit breaker health
    const [healthState, setHealthState] = useState<string>('closed');
    const [masterKeyConfigured, setMasterKeyConfigured] = useState(true);

    // Scheduled backup settings (SuperAdmin only)
    const [autoBackupEnabled, setAutoBackupEnabled] = useState(false);
    const [autoBackupCron, setAutoBackupCron] = useState('0 2 * * 0');
    const [autoBackupRetention, setAutoBackupRetention] = useState(5);
    const [autoBackupSaving, setAutoBackupSaving] = useState(false);

    // Import state
    const [importFile, setImportFile] = useState<string | null>(null);
    const [importFileName, setImportFileName] = useState<string>('');
    const [importPassphrase, setImportPassphrase] = useState('');
    const [importConfirmPassword, setImportConfirmPassword] = useState('');
    const [isImporting, setIsImporting] = useState(false);
    const [isPreviewing, setIsPreviewing] = useState(false);
    const [importError, setImportError] = useState<string | null>(null);
    const [importSuccess, setImportSuccess] = useState(false);
    const [previewData, setPreviewData] = useState<any>(null);

    // Settings Profile state (SuperAdmin only)
    const defaultProfileJson = type === 'global'
        ? '{\n  "global_settings": {}\n}'
        : '{\n  "tenant_core": {\n    "session_timeout_minutes": 30\n  }\n}';
    const [profileJson, setProfileJson] = useState(defaultProfileJson);
    const [profilePassword, setProfilePassword] = useState('');
    const [profileApplying, setProfileApplying] = useState(false);
    const [profilePreviewing, setProfilePreviewing] = useState(false);
    const [profileResult, setProfileResult] = useState<any>(null);
    const [profileError, setProfileError] = useState<string | null>(null);
    const [profileFieldsOpen, setProfileFieldsOpen] = useState(false);
    const [profileLoading, setProfileLoading] = useState(false);

    // Global backup toggle state
    const [globalBackupEnabled, setGlobalBackupEnabled] = useState(true);
    const [globalToggleSaving, setGlobalToggleSaving] = useState(false);

    // Password confirmation modal (replaces browser prompt())
    const [pwdModal, setPwdModal] = useState<{ resolve: (pwd: string | null) => void } | null>(null);
    const requestPassword = (): Promise<string | null> => {
        return new Promise((resolve) => {
            setPwdModal({ resolve });
        });
    };

    // Clear all sensitive fields on unmount
    useEffect(() => {
        return () => {
            setExportPassphrase('');
            setConfirmPassword('');
            setImportPassphrase('');
            setImportConfirmPassword('');
            setProfilePassword('');
        };
    }, []);

    // Fetch section counts + saved backups + health + auto-backup settings on mount
    useEffect(() => {
        if (type === 'tenant') {
            fetchSectionCounts();
            fetchSavedBackups();
            fetchHealth();
            if (isSuperAdmin && tenantId) fetchAutoBackupSettings();
        }
        if (type === 'global' && isSuperAdmin) {
            fetchGlobalBackupStatus();
            fetchSavedBackups();
            fetchHealth();
            fetchGlobalAutoBackupSettings();
        }
    }, [type, tenantId]);

    const fetchSectionCounts = async () => {
        try {
            const res = await fetch('/api/backup/section-counts', {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) setSectionCounts(await res.json());
        } catch {}
    };

    const fetchSavedBackups = async () => {
        setLoadingSaved(true);
        try {
            const url = type === 'global' ? '/api/backup/saved?mode=global' : '/api/backup/saved';
            const res = await fetch(url, {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) setSavedBackups(await res.json());
        } catch {}
        setLoadingSaved(false);
    };

    const fetchHealth = async () => {
        try {
            const res = await fetch('/api/backup/health', {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) {
                const data = await res.json();
                setHealthState(data.state);
                if (data.master_key_configured !== undefined) {
                    setMasterKeyConfigured(data.master_key_configured);
                }
            }
        } catch {}
    };

    const fetchAutoBackupSettings = async () => {
        try {
            const res = await fetch('/api/tenants/accessible', {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) {
                const tenants = await res.json();
                const tenant = tenants.find((t: any) => t.id === tenantId);
                if (tenant) {
                    setAutoBackupEnabled(tenant.auto_backup_enabled ?? false);
                    setAutoBackupCron(tenant.auto_backup_cron ?? '0 2 * * 0');
                    setAutoBackupRetention(tenant.auto_backup_retention_count ?? 5);
                }
            }
        } catch {}
    };

    const saveAutoBackupSettings = async (updates: Record<string, any>): Promise<boolean> => {
        const pwd = await requestPassword();
        if (!pwd) return false;
        setAutoBackupSaving(true);
        try {
            const res = await fetch(`/api/tenants/${tenantId}/edit`, {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json',
                    'X-Confirm-Password': pwd,
                },
                body: JSON.stringify(updates),
            });
            if (!res.ok) return false;
            return true;
        } catch {
            return false;
        } finally {
            setAutoBackupSaving(false);
        }
    };

    const toggleSection = (key: string) => {
        setSelectedSections(prev =>
            prev.includes(key) ? prev.filter(k => k !== key) : [...prev, key]
        );
    };

    const toggleGlobalSection = (key: string) => {
        setSelectedGlobalSections(prev =>
            prev.includes(key) ? prev.filter(k => k !== key) : [...prev, key]
        );
    };

    const toggleOptional = (key: string) => {
        setSelectedOptional(prev =>
            prev.includes(key) ? prev.filter(k => k !== key) : [...prev, key]
        );
    };

    const fetchGlobalAutoBackupSettings = async () => {
        try {
            const res = await fetch('/api/backup/global/schedule', {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) {
                const data = await res.json();
                setAutoBackupEnabled(data.enabled ?? false);
                setAutoBackupCron(data.cron ?? '0 3 * * 0');
                setAutoBackupRetention(data.retention_count ?? 5);
            }
        } catch {}
    };

    const saveGlobalAutoBackupSettings = async (updates: Record<string, any>): Promise<boolean> => {
        const pwd = await requestPassword();
        if (!pwd) return false;
        setAutoBackupSaving(true);
        try {
            const res = await fetch('/api/backup/global/schedule', {
                method: 'PUT',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json',
                    'X-Confirm-Password': pwd,
                },
                body: JSON.stringify(updates),
            });
            if (!res.ok) return false;
            return true;
        } catch {
            return false;
        } finally {
            setAutoBackupSaving(false);
        }
    };

    const handleExport = async () => {
        if (exportPassphrase.length < 12) {
            setExportError('Passphrase must be at least 12 characters');
            return;
        }
        if (!confirmPassword) {
            setExportError('Please confirm your account password');
            return;
        }

        setIsExporting(true);
        setExportError(null);
        setExportSuccess(false);

        try {
            const queryParams = type === 'tenant'
                ? `?sections=${selectedSections.join(',')}&include_optional=${selectedOptional.join(',')}&include_secrets=${includeSecrets}&audit_days=${auditDays}&file_limit=${fileLimit}&approval_days=${approvalDays}`
                : `?sections=${selectedGlobalSections.join(',')}`;

            if (exportDest === 'storage') {
                // Save to storage backend
                const saveEndpoint = type === 'global' ? '/api/backup/global/save' : '/api/backup/save';
                const response = await fetch(`${saveEndpoint}${queryParams}`, {
                    method: 'POST',
                    headers: {
                        'Authorization': `Bearer ${token}`,
                        'X-Confirm-Password': confirmPassword,
                        'X-Backup-Passphrase': exportPassphrase,
                    },
                });

                if (!response.ok) {
                    if (response.status === 401) throw new Error('Incorrect account password');
                    if (response.status === 403) throw new Error('Insufficient permissions');
                    if (response.status === 429) throw new Error('Too many concurrent operations. Try again shortly.');
                    if (response.status === 503) throw new Error('Backup service temporarily unavailable due to errors.');
                    throw new Error(`Save failed (${response.status})`);
                }

                const data = await response.json();
                if (!data.success) throw new Error('Save failed');

                setExportSuccess(true);
                fetchSavedBackups(); // Refresh list
            } else {
                // Download to browser
                const endpoint = type === 'global'
                    ? `/api/backup/global/export${queryParams}`
                    : `/api/backup/export${queryParams}`;

                const response = await fetch(endpoint, {
                    headers: {
                        'Authorization': `Bearer ${token}`,
                        'X-Confirm-Password': confirmPassword,
                        'X-Backup-Passphrase': exportPassphrase,
                    },
                });

                if (!response.ok) {
                    if (response.status === 401) throw new Error('Incorrect account password');
                    if (response.status === 403) throw new Error('Insufficient permissions');
                    if (response.status === 429) throw new Error('Rate limited. Please wait before exporting again.');
                    if (response.status === 503) throw new Error('Backup service temporarily unavailable.');
                    throw new Error(`Export failed (${response.status})`);
                }

                const blob = await response.blob();
                const contentDisposition = response.headers.get('Content-Disposition');
                const filename = contentDisposition?.match(/filename="(.+)"/)?.[1] || 'backup.clovalink.json';

                const url = window.URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = filename;
                document.body.appendChild(a);
                a.click();
                window.URL.revokeObjectURL(url);
                document.body.removeChild(a);

                setExportSuccess(true);
            }

            setTimeout(() => setExportSuccess(false), 5000);
        } catch (err) {
            setExportError(err instanceof Error ? err.message : 'Export failed');
        } finally {
            setExportPassphrase('');
            setConfirmPassword('');
            setIsExporting(false);
        }
    };

    const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = () => {
            setImportFile(reader.result as string);
            setImportFileName(file.name);
            setPreviewData(null);
            setImportError(null);
        };
        reader.readAsText(file);
    };

    const handlePreview = async () => {
        if (!importFile || !importPassphrase || !importConfirmPassword) {
            setImportError('Please provide the backup file, passphrase, and account password');
            return;
        }

        setIsPreviewing(true);
        setImportError(null);

        try {
            const endpoint = type === 'global'
                ? '/api/backup/global/import/preview'
                : '/api/backup/import/preview';

            const response = await fetch(endpoint, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json',
                    'X-Confirm-Password': importConfirmPassword,
                    'X-Backup-Passphrase': importPassphrase,
                },
                body: JSON.stringify({ data: importFile }),
            });

            if (!response.ok) {
                if (response.status === 401) throw new Error('Incorrect account password');
                if (response.status === 429) throw new Error('Too many failed attempts. Backup operations locked.');
                throw new Error(`Preview failed (${response.status})`);
            }

            const data = await response.json();
            if (!data.valid) {
                throw new Error(data.errors?.[0] || 'Invalid backup file');
            }

            setPreviewData(data);
        } catch (err) {
            setImportError(err instanceof Error ? err.message : 'Preview failed');
        } finally {
            setIsPreviewing(false);
        }
    };

    const handleImport = async () => {
        if (!importFile) return;

        setIsImporting(true);
        setImportError(null);

        try {
            const endpoint = type === 'global'
                ? '/api/backup/global/import'
                : '/api/backup/import';

            const response = await fetch(endpoint, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`,
                    'Content-Type': 'application/json',
                    'X-Confirm-Password': importConfirmPassword,
                    'X-Backup-Passphrase': importPassphrase,
                },
                body: JSON.stringify({ data: importFile }),
            });

            if (!response.ok) {
                throw new Error(`Import failed (${response.status})`);
            }

            const data = await response.json();
            if (!data.success) {
                throw new Error(data.error || 'Import failed');
            }

            setImportSuccess(true);
            setImportFile(null);
            setImportFileName('');
            setPreviewData(null);
            setTimeout(() => setImportSuccess(false), 5000);
        } catch (err) {
            setImportError(err instanceof Error ? err.message : 'Import failed');
        } finally {
            setImportPassphrase('');
            setImportConfirmPassword('');
            setIsImporting(false);
        }
    };

    const handleDeleteSaved = async (id: string) => {
        if (!confirm('Delete this saved backup? This cannot be undone.')) return;
        try {
            await fetch(`/api/backup/saved/${id}`, {
                method: 'DELETE',
                headers: { 'Authorization': `Bearer ${token}` },
            });
            fetchSavedBackups();
        } catch {}
    };

    const handleDownloadSaved = async (id: string, filename: string) => {
        try {
            const res = await fetch(`/api/backup/saved/${id}/download`, {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (!res.ok) throw new Error('Download failed');
            const blob = await res.blob();
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = filename;
            document.body.appendChild(a);
            a.click();
            window.URL.revokeObjectURL(url);
            document.body.removeChild(a);
        } catch (err) {
            console.error('Download failed:', err);
        }
    };

    const formatSize = (bytes: number) => {
        if (bytes < 1024) return `${bytes} B`;
        if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
        return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    };

    const fetchGlobalBackupStatus = async () => {
        try {
            const res = await fetch('/api/backup/global/status', {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) {
                const data = await res.json();
                setGlobalBackupEnabled(data.enabled);
            }
        } catch {}
    };

    const toggleGlobalBackup = async (enabled: boolean) => {
        setGlobalToggleSaving(true);
        try {
            const pwd = await requestPassword();
            if (!pwd) { setGlobalToggleSaving(false); return; }
            const res = await fetch('/api/backup/global/toggle', {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${token}`,
                    'X-Confirm-Password': pwd,
                },
                body: JSON.stringify({ enabled }),
            });
            if (res.ok) setGlobalBackupEnabled(enabled);
        } catch {}
        setGlobalToggleSaving(false);
    };

    const loadCurrentSettings = async () => {
        setProfileLoading(true);
        try {
            const mode = type === 'global' ? 'global' : 'tenant';
            const res = await fetch(`/api/backup/current-settings?mode=${mode}`, {
                headers: { 'Authorization': `Bearer ${token}` },
            });
            if (res.ok) {
                const data = await res.json();
                setProfileJson(JSON.stringify(data, null, 2));
                setProfileError(null);
                setProfileResult(null);
            }
        } catch {
            setProfileError('Failed to load current settings');
        }
        setProfileLoading(false);
    };

    const handleProfileApply = async (dryRun: boolean) => {
        setProfileError(null);
        setProfileResult(null);

        // Validate JSON
        let parsed: any;
        try {
            parsed = JSON.parse(profileJson);
        } catch {
            setProfileError('Invalid JSON. Please check your syntax.');
            return;
        }

        if (dryRun) setProfilePreviewing(true);
        else setProfileApplying(true);

        try {
            const endpoint = type === 'global'
                ? '/api/backup/global/apply-settings-profile'
                : '/api/backup/apply-settings-profile';
            const res = await fetch(endpoint, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${token}`,
                    'X-Confirm-Password': profilePassword,
                },
                body: JSON.stringify({ profile: parsed, dry_run: dryRun }),
            });

            const data = await res.json();
            if (!res.ok) {
                setProfileError(res.status === 401 ? 'Invalid password' : res.status === 403 ? 'Access denied' : `Error: ${res.statusText}`);
                return;
            }

            if (!data.success) {
                setProfileError(data.error || 'Failed to apply profile');
                return;
            }

            setProfileResult(data);
            if (!dryRun) setProfilePassword('');
        } catch (err: any) {
            setProfileError(err.message || 'Network error');
        } finally {
            setProfileApplying(false);
            setProfilePreviewing(false);
        }
    };

    return (
        <div className="space-y-6">
            {/* Global Backup Toggle */}
            {type === 'global' && isSuperAdmin && (
                <div className="flex items-center justify-between p-4 bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm">
                    <div className="flex items-center gap-3">
                        <div className="p-2 rounded-lg bg-blue-100 dark:bg-blue-900/50">
                            <Shield className="w-5 h-5 text-blue-600 dark:text-blue-400" />
                        </div>
                        <div>
                            <h3 className="font-medium text-gray-900 dark:text-white">Global Backup</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                {globalBackupEnabled ? 'Backup operations are enabled' : 'Backup operations are disabled'}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={() => toggleGlobalBackup(!globalBackupEnabled)}
                        disabled={globalToggleSaving}
                        className={clsx(
                            "relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2",
                            globalBackupEnabled ? "bg-blue-600" : "bg-gray-200 dark:bg-gray-600",
                            globalToggleSaving && "opacity-50 cursor-not-allowed"
                        )}
                    >
                        <span className={clsx(
                            "pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out",
                            globalBackupEnabled ? "translate-x-5" : "translate-x-0"
                        )} />
                    </button>
                </div>
            )}

            {/* Circuit Breaker Status + Storage Migration Info */}
            {type === 'tenant' && (
                <>
                    {/* Health Status Pill */}
                    <div className="flex items-center gap-3">
                        <span className={clsx(
                            "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium",
                            healthState === 'closed' && "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300",
                            healthState === 'half_open' && "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300",
                            healthState === 'open' && "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300",
                        )}>
                            <span className={clsx(
                                "w-1.5 h-1.5 rounded-full",
                                healthState === 'closed' && "bg-green-500",
                                healthState === 'half_open' && "bg-yellow-500",
                                healthState === 'open' && "bg-red-500",
                            )} />
                            {healthState === 'closed' ? 'Healthy' : healthState === 'half_open' ? 'Degraded' : 'Paused'}
                        </span>
                        <span className="text-xs text-gray-500 dark:text-gray-400">Backup System Status</span>
                    </div>

                    {/* Migration Info Banner */}
                    <div className="flex items-start gap-3 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg border border-blue-200 dark:border-blue-800">
                        <Info className="w-5 h-5 text-blue-600 dark:text-blue-400 flex-shrink-0 mt-0.5" />
                        <p className="text-sm text-blue-800 dark:text-blue-200">
                            Backups preserve all metadata, settings, users, and organization structure.
                            File content remains on your storage backend (S3/local).
                            To migrate storage, export a backup, copy files to new storage, then import the backup on the new instance.
                        </p>
                    </div>
                </>
            )}

            {/* Export Card */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
                    <div className="p-2 rounded-lg bg-primary-100 dark:bg-primary-900/50">
                        <Download className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                    </div>
                    <div>
                        <h3 className="font-medium text-gray-900 dark:text-white">Export Backup</h3>
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                            Create an encrypted backup of {type === 'global' ? 'global settings' : 'tenant data'}
                        </p>
                    </div>
                </div>

                <div className="p-6 space-y-5">
                    {/* Categorized section checkboxes */}
                    {type === 'tenant' ? (
                        <>
                            {SECTION_CATEGORIES.map(category => {
                                const Icon = category.icon;
                                const allSelected = category.sections.every(s => selectedSections.includes(s.key));
                                return (
                                    <div key={category.name} className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
                                        <div className="flex items-center gap-2 px-4 py-2.5 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-700">
                                            <Icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                                            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">{category.name}</span>
                                            <button
                                                onClick={() => {
                                                    const keys = category.sections.map(s => s.key);
                                                    if (allSelected) {
                                                        setSelectedSections(prev => prev.filter(k => !keys.includes(k)));
                                                    } else {
                                                        setSelectedSections(prev => [...new Set([...prev, ...keys])]);
                                                    }
                                                }}
                                                className="ml-auto text-xs text-primary-600 dark:text-primary-400 hover:underline"
                                            >
                                                {allSelected ? 'Deselect all' : 'Select all'}
                                            </button>
                                        </div>
                                        <div className="grid grid-cols-2 gap-0">
                                            {category.sections.map(section => (
                                                <label key={section.key} className="flex items-center gap-2 px-4 py-2 hover:bg-gray-50 dark:hover:bg-gray-700/50 cursor-pointer border-b border-gray-100 dark:border-gray-800 last:border-0">
                                                    <input
                                                        type="checkbox"
                                                        checked={selectedSections.includes(section.key)}
                                                        onChange={() => toggleSection(section.key)}
                                                        className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                                    />
                                                    <div className="flex-1 min-w-0">
                                                        <div className="flex items-center gap-2">
                                                            <span className="text-sm text-gray-900 dark:text-white">{section.label}</span>
                                                            {sectionCounts[section.key] !== undefined && (
                                                                <span className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded text-xs font-mono">
                                                                    {sectionCounts[section.key]}
                                                                </span>
                                                            )}
                                                        </div>
                                                        <span className="text-xs text-gray-500 dark:text-gray-400">{section.description}</span>
                                                    </div>
                                                </label>
                                            ))}
                                        </div>
                                    </div>
                                );
                            })}

                            {/* Optional Large Data Sections */}
                            <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
                                <div className="flex items-center gap-2 px-4 py-2.5 bg-amber-50 dark:bg-amber-900/20 border-b border-gray-200 dark:border-gray-700">
                                    <Database className="w-4 h-4 text-amber-600 dark:text-amber-400" />
                                    <span className="text-sm font-medium text-amber-800 dark:text-amber-200">Large Data (optional)</span>
                                </div>
                                <div className="space-y-0">
                                    {OPTIONAL_SECTIONS.map(section => (
                                        <div key={section.key} className="px-4 py-2 border-b border-gray-100 dark:border-gray-800 last:border-0">
                                            <label className="flex items-center gap-2 cursor-pointer">
                                                <input
                                                    type="checkbox"
                                                    checked={selectedOptional.includes(section.key)}
                                                    onChange={() => toggleOptional(section.key)}
                                                    className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                                />
                                                <div className="flex-1">
                                                    <div className="flex items-center gap-2">
                                                        <span className="text-sm text-gray-900 dark:text-white">{section.label}</span>
                                                        {sectionCounts[section.key] !== undefined && (
                                                            <span className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded text-xs font-mono">
                                                                {sectionCounts[section.key].toLocaleString()}
                                                            </span>
                                                        )}
                                                    </div>
                                                    <span className="text-xs text-gray-500 dark:text-gray-400">{section.description}</span>
                                                </div>
                                            </label>
                                            {/* Pagination controls */}
                                            {selectedOptional.includes(section.key) && (
                                                <div className="ml-6 mt-2 mb-1">
                                                    {section.key === 'audit_logs' && (
                                                        <label className="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400">
                                                            Days to include:
                                                            <input
                                                                type="number"
                                                                value={auditDays}
                                                                onChange={e => setAuditDays(Math.max(1, Math.min(3650, parseInt(e.target.value) || 90)))}
                                                                className="w-20 px-2 py-1 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-sm"
                                                            />
                                                        </label>
                                                    )}
                                                    {section.key === 'file_metadata' && (
                                                        <label className="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400">
                                                            Max records:
                                                            <input
                                                                type="number"
                                                                value={fileLimit}
                                                                onChange={e => setFileLimit(Math.max(1, Math.min(100000, parseInt(e.target.value) || 50000)))}
                                                                className="w-24 px-2 py-1 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-sm"
                                                            />
                                                        </label>
                                                    )}
                                                    {section.key === 'approval_history' && (
                                                        <label className="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400">
                                                            Days to include:
                                                            <input
                                                                type="number"
                                                                value={approvalDays}
                                                                onChange={e => setApprovalDays(Math.max(1, Math.min(3650, parseInt(e.target.value) || 90)))}
                                                                className="w-20 px-2 py-1 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-sm"
                                                            />
                                                        </label>
                                                    )}
                                                </div>
                                            )}
                                        </div>
                                    ))}
                                </div>
                            </div>

                            {/* Include Secrets (SuperAdmin) */}
                            {isSuperAdmin && (
                                <div className="flex items-center gap-2 p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
                                    <input
                                        type="checkbox"
                                        checked={includeSecrets}
                                        onChange={(e) => setIncludeSecrets(e.target.checked)}
                                        className="rounded border-gray-300 text-amber-600 focus:ring-amber-500"
                                    />
                                    <div>
                                        <span className="text-sm font-medium text-amber-800 dark:text-amber-200">Include encrypted secrets</span>
                                        <p className="text-xs text-amber-700 dark:text-amber-300">SMTP passwords, API keys, client secrets (triggers security alert)</p>
                                    </div>
                                </div>
                            )}
                        </>
                    ) : (
                        /* Global section checkboxes */
                        <>
                            {GLOBAL_SECTION_CATEGORIES.map(category => {
                                const Icon = category.icon;
                                const allSelected = category.sections.every(s => selectedGlobalSections.includes(s.key));
                                return (
                                    <div key={category.name} className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
                                        <div className="flex items-center gap-2 px-4 py-2.5 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-700">
                                            <Icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                                            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">{category.name}</span>
                                            <button
                                                onClick={() => {
                                                    const keys = category.sections.map(s => s.key);
                                                    if (allSelected) {
                                                        setSelectedGlobalSections(prev => prev.filter(k => !keys.includes(k)));
                                                    } else {
                                                        setSelectedGlobalSections(prev => [...new Set([...prev, ...keys])]);
                                                    }
                                                }}
                                                className="ml-auto text-xs text-primary-600 dark:text-primary-400 hover:underline"
                                            >
                                                {allSelected ? 'Deselect all' : 'Select all'}
                                            </button>
                                        </div>
                                        <div className="grid grid-cols-2 gap-0">
                                            {category.sections.map(section => (
                                                <label key={section.key} className="flex items-center gap-2 px-4 py-2 hover:bg-gray-50 dark:hover:bg-gray-700/50 cursor-pointer border-b border-gray-100 dark:border-gray-800 last:border-0">
                                                    <input
                                                        type="checkbox"
                                                        checked={selectedGlobalSections.includes(section.key)}
                                                        onChange={() => toggleGlobalSection(section.key)}
                                                        className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                                    />
                                                    <div className="flex-1 min-w-0">
                                                        <div className="flex items-center gap-2">
                                                            <span className="text-sm text-gray-900 dark:text-white">{section.label}</span>
                                                        </div>
                                                        <span className="text-xs text-gray-500 dark:text-gray-400">{section.description}</span>
                                                    </div>
                                                </label>
                                            ))}
                                        </div>
                                    </div>
                                );
                            })}
                        </>
                    )}

                    {/* Export Destination */}
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            Export Destination
                        </label>
                        <div className="flex gap-3">
                            <label className={clsx(
                                "flex-1 flex items-center gap-2 p-3 rounded-lg border-2 cursor-pointer transition-colors",
                                exportDest === 'download'
                                    ? "border-primary-500 bg-primary-50 dark:bg-primary-900/20"
                                    : "border-gray-200 dark:border-gray-700 hover:border-gray-300"
                            )}>
                                <input
                                    type="radio"
                                    checked={exportDest === 'download'}
                                    onChange={() => setExportDest('download')}
                                    className="text-primary-600"
                                />
                                <Download className="w-4 h-4 text-gray-500" />
                                <span className="text-sm text-gray-900 dark:text-white">Download to Browser</span>
                            </label>
                            <label className={clsx(
                                "flex-1 flex items-center gap-2 p-3 rounded-lg border-2 cursor-pointer transition-colors",
                                exportDest === 'storage'
                                    ? "border-primary-500 bg-primary-50 dark:bg-primary-900/20"
                                    : "border-gray-200 dark:border-gray-700 hover:border-gray-300"
                            )}>
                                <input
                                    type="radio"
                                    checked={exportDest === 'storage'}
                                    onChange={() => setExportDest('storage')}
                                    className="text-primary-600"
                                />
                                <HardDrive className="w-4 h-4 text-gray-500" />
                                <span className="text-sm text-gray-900 dark:text-white">Save to Storage Backend</span>
                            </label>
                        </div>
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                <Lock className="w-3.5 h-3.5 inline mr-1" />
                                Backup Passphrase
                            </label>
                            <input
                                type="password"
                                value={exportPassphrase}
                                onChange={(e) => setExportPassphrase(e.target.value)}
                                placeholder="Min 12 characters"
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                            />
                            <p className="mt-1 text-xs text-gray-500">You'll need this to decrypt the backup</p>
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                <Shield className="w-3.5 h-3.5 inline mr-1" />
                                Confirm Account Password
                            </label>
                            <input
                                type="password"
                                value={confirmPassword}
                                onChange={(e) => setConfirmPassword(e.target.value)}
                                placeholder="Your current password"
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                            />
                        </div>
                    </div>

                    {exportError && (
                        <div className="flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 rounded-lg text-red-700 dark:text-red-300 text-sm">
                            <AlertTriangle className="w-4 h-4 flex-shrink-0" />
                            {exportError}
                        </div>
                    )}

                    {exportSuccess && (
                        <div className="flex items-center gap-2 p-3 bg-green-50 dark:bg-green-900/20 rounded-lg text-green-700 dark:text-green-300 text-sm">
                            <CheckCircle className="w-4 h-4 flex-shrink-0" />
                            {exportDest === 'storage' ? 'Backup saved to storage successfully' : 'Backup exported and downloaded successfully'}
                        </div>
                    )}

                    <button
                        onClick={handleExport}
                        disabled={isExporting || exportPassphrase.length < 12 || !confirmPassword}
                        className={clsx(
                            "flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all",
                            isExporting || exportPassphrase.length < 12 || !confirmPassword
                                ? "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
                                : "bg-primary-600 text-white hover:bg-primary-700"
                        )}
                    >
                        {isExporting ? <Loader2 className="w-4 h-4 animate-spin" /> : exportDest === 'storage' ? <HardDrive className="w-4 h-4" /> : <Download className="w-4 h-4" />}
                        {isExporting ? 'Exporting...' : exportDest === 'storage' ? 'Save to Storage' : 'Export Backup'}
                    </button>
                </div>
            </div>

            {/* Saved Backups */}
            {(type === 'tenant' || (type === 'global' && isSuperAdmin)) && (
                <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                    <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <div className="p-2 rounded-lg bg-gray-100 dark:bg-gray-700">
                                <HardDrive className="w-5 h-5 text-gray-600 dark:text-gray-400" />
                            </div>
                            <div>
                                <h3 className="font-medium text-gray-900 dark:text-white">Saved Backups</h3>
                                <p className="text-xs text-gray-500 dark:text-gray-400">Backups stored on your storage backend</p>
                            </div>
                        </div>
                        <button onClick={fetchSavedBackups} className="p-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg">
                            <RefreshCw className={clsx("w-4 h-4 text-gray-500", loadingSaved && "animate-spin")} />
                        </button>
                    </div>

                    {savedBackups.length === 0 ? (
                        <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">
                            No saved backups yet. Use "Save to Storage Backend" when exporting.
                        </div>
                    ) : (
                        <div className="divide-y divide-gray-200 dark:divide-gray-700">
                            {savedBackups.map(backup => (
                                <div key={backup.id} className="px-6 py-3 flex items-center gap-4">
                                    <div className="flex-shrink-0">
                                        {backup.is_auto_backup
                                            ? <Clock className="w-4 h-4 text-blue-500" />
                                            : <User className="w-4 h-4 text-gray-500" />
                                        }
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2">
                                            <span className="text-sm text-gray-900 dark:text-white truncate">{backup.filename}</span>
                                            {backup.is_auto_backup && (
                                                <span className="px-1.5 py-0.5 text-xs rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300">auto</span>
                                            )}
                                            <span className={clsx(
                                                "px-1.5 py-0.5 text-xs rounded",
                                                backup.status === 'completed' ? "bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300" : "bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300"
                                            )}>
                                                {backup.status}
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                            <span>{formatSize(backup.size_bytes)}</span>
                                            {backup.duration_ms && <span>{backup.duration_ms}ms</span>}
                                            <span>{new Date(backup.created_at).toLocaleString()}</span>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-1">
                                        <button
                                            onClick={() => handleDownloadSaved(backup.id, backup.filename)}
                                            className="p-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 rounded text-gray-500 hover:text-gray-700"
                                            title="Download"
                                        >
                                            <Download className="w-4 h-4" />
                                        </button>
                                        <button
                                            onClick={() => handleDeleteSaved(backup.id)}
                                            className="p-1.5 hover:bg-red-50 dark:hover:bg-red-900/20 rounded text-gray-400 hover:text-red-600"
                                            title="Delete"
                                        >
                                            <Trash2 className="w-4 h-4" />
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            )}

            {/* Scheduled Backup Settings (SuperAdmin only) */}
            {isSuperAdmin && (
                <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                    <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
                        <div className="p-2 rounded-lg bg-purple-100 dark:bg-purple-900/50">
                            <Calendar className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                        </div>
                        <div>
                            <h3 className="font-medium text-gray-900 dark:text-white">Scheduled Backups</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                {type === 'global'
                                    ? 'Automatically back up global settings on a schedule'
                                    : 'Automatically back up this tenant on a schedule'}
                            </p>
                        </div>
                    </div>
                    <div className="p-6 space-y-5">
                        {/* Master key warning */}
                        {!masterKeyConfigured && (
                            <div className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
                                <AlertTriangle className="w-5 h-5 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
                                <div>
                                    <p className="text-sm font-medium text-amber-800 dark:text-amber-200">BACKUP_MASTER_KEY not configured</p>
                                    <p className="text-xs text-amber-700 dark:text-amber-300 mt-1">
                                        Auto-backups require <code className="px-1 py-0.5 bg-amber-100 dark:bg-amber-900/40 rounded text-xs">BACKUP_MASTER_KEY</code> to
                                        be set in your server environment. This key encrypts the system-generated passphrase stored in the database.
                                        Manual export/import works without it.
                                    </p>
                                    <p className="text-xs text-amber-600 dark:text-amber-400 mt-1">
                                        Generate one with: <code className="px-1 py-0.5 bg-amber-100 dark:bg-amber-900/40 rounded text-xs">openssl rand -base64 48</code>
                                    </p>
                                </div>
                            </div>
                        )}

                        {/* Enable toggle */}
                        <div className="flex items-center justify-between">
                            <div>
                                <label className="text-sm font-medium text-gray-900 dark:text-white">Enable Auto-Backup</label>
                                <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                    Backups run automatically and are saved to storage
                                </p>
                            </div>
                            <button
                                onClick={async () => {
                                    const newVal = !autoBackupEnabled;
                                    const fn = type === 'global' ? saveGlobalAutoBackupSettings : saveAutoBackupSettings;
                                    const key = type === 'global' ? 'enabled' : 'auto_backup_enabled';
                                    const ok = await fn({ [key]: newVal });
                                    if (ok) setAutoBackupEnabled(newVal);
                                }}
                                disabled={autoBackupSaving || (!masterKeyConfigured && !autoBackupEnabled)}
                                className={clsx(
                                    "relative inline-flex h-6 w-11 flex-shrink-0 rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out",
                                    (!masterKeyConfigured && !autoBackupEnabled) ? "bg-gray-200 dark:bg-gray-600 cursor-not-allowed opacity-50" :
                                    autoBackupEnabled ? "bg-primary-600 cursor-pointer" : "bg-gray-200 dark:bg-gray-600 cursor-pointer"
                                )}
                            >
                                <span className={clsx(
                                    "pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out",
                                    autoBackupEnabled ? "translate-x-5" : "translate-x-0"
                                )} />
                            </button>
                        </div>

                        {autoBackupEnabled && (
                            <>
                                {/* Schedule select */}
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Schedule</label>
                                    <select
                                        value={autoBackupCron}
                                        onChange={async (e) => {
                                            const val = e.target.value;
                                            const prev = autoBackupCron;
                                            setAutoBackupCron(val); // show selection immediately
                                            const fn = type === 'global' ? saveGlobalAutoBackupSettings : saveAutoBackupSettings;
                                            const key = type === 'global' ? 'cron' : 'auto_backup_cron';
                                            const ok = await fn({ [key]: val });
                                            if (!ok) setAutoBackupCron(prev); // revert on failure/cancel
                                        }}
                                        className="w-full px-3 py-2 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg text-sm text-gray-900 dark:text-white"
                                    >
                                        <option value="0 2 * * *">Daily at 2:00 AM</option>
                                        <option value="0 2 * * 0">Weekly — Sunday at 2:00 AM</option>
                                        <option value="0 3 * * 0">Weekly — Sunday at 3:00 AM</option>
                                        <option value="0 2 * * 1">Weekly — Monday at 2:00 AM</option>
                                        <option value="0 2 1 * *">Monthly — 1st at 2:00 AM</option>
                                        <option value="0 2 15 * *">Monthly — 15th at 2:00 AM</option>
                                    </select>
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                                        Cron: <code className="px-1 py-0.5 bg-gray-100 dark:bg-gray-700 rounded text-xs">{autoBackupCron}</code>
                                    </p>
                                </div>

                                {/* Retention */}
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        Retention — keep last
                                    </label>
                                    <div className="flex items-center gap-2">
                                        <input
                                            type="number"
                                            min={1}
                                            max={100}
                                            value={autoBackupRetention}
                                            onChange={(e) => {
                                                const val = Math.max(1, Math.min(100, parseInt(e.target.value) || 5));
                                                setAutoBackupRetention(val);
                                            }}
                                            onBlur={async () => {
                                                const fn = type === 'global' ? saveGlobalAutoBackupSettings : saveAutoBackupSettings;
                                                const key = type === 'global' ? 'retention_count' : 'auto_backup_retention_count';
                                                const ok = await fn({ [key]: autoBackupRetention });
                                                if (!ok) {
                                                    // Revert by re-fetching
                                                    if (type === 'global') fetchGlobalAutoBackupSettings();
                                                    else fetchAutoBackupSettings();
                                                }
                                            }}
                                            className="w-20 px-3 py-2 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg text-sm text-gray-900 dark:text-white"
                                        />
                                        <span className="text-sm text-gray-500 dark:text-gray-400">backups</span>
                                    </div>
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                                        Oldest auto-backups are deleted when this limit is exceeded
                                    </p>
                                </div>

                                {/* Info note */}
                                <div className="flex items-start gap-2 p-3 bg-purple-50 dark:bg-purple-900/20 rounded-lg text-sm text-purple-800 dark:text-purple-200">
                                    <Info className="w-4 h-4 flex-shrink-0 mt-0.5" />
                                    <span>
                                        {type === 'global'
                                            ? 'Auto-backups include all global sections. Uses a system-generated passphrase, encrypted at rest with BACKUP_MASTER_KEY. Backups are distributed with random jitter to avoid load spikes.'
                                            : 'Auto-backups include core sections only (no large data). Uses a system-generated passphrase, encrypted at rest with BACKUP_MASTER_KEY. Backups are distributed with random jitter to avoid load spikes.'}
                                    </span>
                                </div>
                            </>
                        )}
                    </div>
                </div>
            )}

            {/* Import Card */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
                    <div className="p-2 rounded-lg bg-amber-100 dark:bg-amber-900/50">
                        <Upload className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                    </div>
                    <div>
                        <h3 className="font-medium text-gray-900 dark:text-white">Import / Restore</h3>
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                            Restore from a previously exported backup file
                        </p>
                    </div>
                </div>

                <div className="p-6 space-y-4">
                    {/* File Upload */}
                    <div>
                        <input
                            ref={fileInputRef}
                            type="file"
                            accept=".clovalink.json,.json"
                            onChange={handleFileSelect}
                            className="hidden"
                        />
                        {importFile ? (
                            <div className="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-900/50 rounded-lg">
                                <div className="flex items-center gap-2">
                                    <FileText className="w-4 h-4 text-gray-500" />
                                    <span className="text-sm text-gray-900 dark:text-white">{importFileName}</span>
                                </div>
                                <button
                                    onClick={() => { setImportFile(null); setImportFileName(''); setPreviewData(null); }}
                                    className="p-1 hover:bg-gray-200 dark:hover:bg-gray-700 rounded"
                                >
                                    <X className="w-4 h-4 text-gray-500" />
                                </button>
                            </div>
                        ) : (
                            <button
                                onClick={() => fileInputRef.current?.click()}
                                className="w-full p-8 border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-lg hover:border-primary-400 dark:hover:border-primary-500 transition-colors text-center"
                            >
                                <Upload className="w-8 h-8 text-gray-400 mx-auto mb-2" />
                                <p className="text-sm text-gray-600 dark:text-gray-400">Click to select a .clovalink.json file</p>
                            </button>
                        )}
                    </div>

                    {importFile && (
                        <>
                            <div className="grid grid-cols-2 gap-4">
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        <Lock className="w-3.5 h-3.5 inline mr-1" />
                                        Backup Passphrase
                                    </label>
                                    <input
                                        type="password"
                                        value={importPassphrase}
                                        onChange={(e) => setImportPassphrase(e.target.value)}
                                        placeholder="Passphrase used during export"
                                        className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                    />
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        <Shield className="w-3.5 h-3.5 inline mr-1" />
                                        Confirm Account Password
                                    </label>
                                    <input
                                        type="password"
                                        value={importConfirmPassword}
                                        onChange={(e) => setImportConfirmPassword(e.target.value)}
                                        placeholder="Your current password"
                                        className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                                    />
                                </div>
                            </div>

                            <div className="flex gap-3">
                                <button
                                    onClick={handlePreview}
                                    disabled={isPreviewing || !importPassphrase || !importConfirmPassword}
                                    className={clsx(
                                        "flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all",
                                        isPreviewing || !importPassphrase || !importConfirmPassword
                                            ? "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
                                            : "bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-300 dark:hover:bg-gray-500"
                                    )}
                                >
                                    {isPreviewing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Eye className="w-4 h-4" />}
                                    Preview Changes
                                </button>

                                {previewData && (
                                    <button
                                        onClick={handleImport}
                                        disabled={isImporting}
                                        className={clsx(
                                            "flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all",
                                            isImporting
                                                ? "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
                                                : "bg-amber-600 text-white hover:bg-amber-700"
                                        )}
                                    >
                                        {isImporting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Upload className="w-4 h-4" />}
                                        {isImporting ? 'Importing...' : 'Apply Backup'}
                                    </button>
                                )}
                            </div>
                        </>
                    )}

                    {/* Preview Results */}
                    {previewData && (
                        <div className="p-4 bg-gray-50 dark:bg-gray-900/50 rounded-lg space-y-3">
                            <div className="flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-white">
                                <Eye className="w-4 h-4" />
                                Import Preview
                            </div>
                            <div className="text-xs text-gray-500 dark:text-gray-400 space-y-1">
                                <p>Version: {previewData.meta?.clovalink_version} | Exported: {previewData.meta?.exported_at ? new Date(previewData.meta.exported_at).toLocaleString() : 'Unknown'}</p>
                                {previewData.meta?.tenant_name && <p>Source tenant: {previewData.meta.tenant_name}</p>}
                            </div>
                            <div className="space-y-2">
                                {Object.entries(previewData.sections || previewData.changes || {}).map(([key, value]: [string, any]) => (
                                    <div key={key} className="flex items-center justify-between p-2 bg-white dark:bg-gray-800 rounded text-sm">
                                        <span className="text-gray-700 dark:text-gray-300">{key}</span>
                                        <span className="text-xs text-gray-500 dark:text-gray-400 font-mono">
                                            {typeof value === 'object' ? JSON.stringify(value) : String(value)}
                                        </span>
                                    </div>
                                ))}
                            </div>
                            {previewData.warnings?.length > 0 && (
                                <div className="space-y-1">
                                    {previewData.warnings.map((w: string, i: number) => (
                                        <div key={i} className="flex items-center gap-2 text-xs text-amber-700 dark:text-amber-300">
                                            <AlertTriangle className="w-3 h-3 flex-shrink-0" />
                                            {w}
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    )}

                    {importError && (
                        <div className="flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 rounded-lg text-red-700 dark:text-red-300 text-sm">
                            <AlertTriangle className="w-4 h-4 flex-shrink-0" />
                            {importError}
                        </div>
                    )}

                    {importSuccess && (
                        <div className="flex items-center gap-2 p-3 bg-green-50 dark:bg-green-900/20 rounded-lg text-green-700 dark:text-green-300 text-sm">
                            <CheckCircle className="w-4 h-4 flex-shrink-0" />
                            Backup imported successfully. Settings have been restored.
                        </div>
                    )}
                </div>
            </div>

            {/* Settings Profile Editor (SuperAdmin only) */}
            {isSuperAdmin && (
                <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                    <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
                        <div className="p-2 rounded-lg bg-indigo-100 dark:bg-indigo-900/50">
                            <Code className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                        </div>
                        <div>
                            <h3 className="font-medium text-gray-900 dark:text-white">Settings Profile</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                Apply a partial JSON config — declarative merge, like NixOS
                            </p>
                        </div>
                    </div>

                    <div className="p-6 space-y-4">
                        {/* Info Banner */}
                        <div className="flex items-start gap-2 p-3 bg-indigo-50 dark:bg-indigo-900/20 rounded-lg text-sm text-indigo-800 dark:text-indigo-200">
                            <Info className="w-4 h-4 flex-shrink-0 mt-0.5" />
                            <span>
                                Define only the fields you want to change. This is a declarative merge —
                                unspecified fields remain unchanged. Use "Load Current Settings" to start from your
                                actual config. Redacted fields (***REDACTED***) are automatically skipped on apply.
                            </span>
                        </div>

                        {/* JSON Editor */}
                        <div>
                            <div className="flex items-center justify-between mb-1">
                                <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                                    Profile JSON
                                </label>
                                <button
                                    onClick={loadCurrentSettings}
                                    disabled={profileLoading}
                                    className="flex items-center gap-1.5 px-3 py-1 text-xs font-medium text-indigo-600 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/30 rounded-lg hover:bg-indigo-100 dark:hover:bg-indigo-900/50 transition-colors disabled:opacity-50"
                                >
                                    {profileLoading ? <Loader2 className="w-3 h-3 animate-spin" /> : <RefreshCw className="w-3 h-3" />}
                                    Load Current Settings
                                </button>
                            </div>
                            <textarea
                                value={profileJson}
                                onChange={(e) => { setProfileJson(e.target.value); setProfileResult(null); setProfileError(null); }}
                                rows={12}
                                spellCheck={false}
                                className="w-full font-mono text-sm bg-gray-900 text-green-400 border border-gray-700 rounded-lg p-4 focus:ring-2 focus:ring-indigo-500 focus:border-transparent resize-y"
                                placeholder='{ "tenant_core": { ... } }'
                            />
                        </div>

                        {/* Available Fields Reference */}
                        <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
                            <button
                                onClick={() => setProfileFieldsOpen(!profileFieldsOpen)}
                                className="w-full flex items-center justify-between px-4 py-2.5 bg-gray-50 dark:bg-gray-900/50 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
                            >
                                <span>Available Sections</span>
                                {profileFieldsOpen ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                            </button>
                            {profileFieldsOpen && (
                                <div className="p-4 grid grid-cols-2 gap-2 text-xs">
                                    {(type === 'global'
                                        ? [{ key: 'global_settings', description: 'Global key-value settings' },
                                           { key: 'global_email_templates', description: 'System email templates' }]
                                        : SECTION_CATEGORIES.flatMap(c => c.sections)
                                    ).map(s => (
                                        <div key={s.key} className="flex items-center gap-2 p-2 bg-gray-50 dark:bg-gray-900/30 rounded">
                                            <code className="text-indigo-600 dark:text-indigo-400 font-mono">{s.key}</code>
                                            <span className="text-gray-500 dark:text-gray-400">— {s.description}</span>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>

                        {/* Password + Actions */}
                        <div className="flex items-end gap-3 flex-wrap">
                            <div className="flex-1 min-w-[200px]">
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    Confirm Password
                                </label>
                                <div className="relative">
                                    <Lock className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
                                    <input
                                        type="password"
                                        value={profilePassword}
                                        onChange={(e) => setProfilePassword(e.target.value)}
                                        placeholder="Your account password"
                                        className="w-full pl-9 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-900 text-sm text-gray-900 dark:text-white focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                                    />
                                </div>
                            </div>

                            <button
                                onClick={() => handleProfileApply(true)}
                                disabled={profilePreviewing || !profilePassword}
                                className={clsx(
                                    "flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all",
                                    profilePreviewing || !profilePassword
                                        ? "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
                                        : "bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-300 dark:hover:bg-gray-500"
                                )}
                            >
                                {profilePreviewing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Eye className="w-4 h-4" />}
                                Preview
                            </button>

                            <button
                                onClick={() => handleProfileApply(false)}
                                disabled={profileApplying || !profilePassword}
                                className={clsx(
                                    "flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all",
                                    profileApplying || !profilePassword
                                        ? "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
                                        : "bg-indigo-600 text-white hover:bg-indigo-700"
                                )}
                            >
                                {profileApplying ? <Loader2 className="w-4 h-4 animate-spin" /> : <Code className="w-4 h-4" />}
                                {profileApplying ? 'Applying...' : 'Apply Profile'}
                            </button>
                        </div>

                        {/* Results */}
                        {profileResult && (
                            <div className={clsx(
                                "p-4 rounded-lg text-sm space-y-2",
                                profileResult.dry_run
                                    ? "bg-blue-50 dark:bg-blue-900/20 text-blue-800 dark:text-blue-200"
                                    : "bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-200"
                            )}>
                                <div className="flex items-center gap-2 font-medium">
                                    <CheckCircle className="w-4 h-4" />
                                    {profileResult.dry_run ? 'Preview — no changes applied' : 'Profile applied successfully'}
                                </div>
                                <div className="space-y-1">
                                    {Object.entries(profileResult.results || {}).map(([key, value]: [string, any]) => (
                                        <div key={key} className="flex items-center justify-between p-2 bg-white/50 dark:bg-gray-800/50 rounded text-xs">
                                            <code className="font-mono">{key}</code>
                                            <span className="text-gray-500 dark:text-gray-400 font-mono truncate max-w-[60%]">
                                                {typeof value === 'object' ? JSON.stringify(value) : String(value)}
                                            </span>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        )}

                        {profileError && (
                            <div className="flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 rounded-lg text-red-700 dark:text-red-300 text-sm">
                                <AlertTriangle className="w-4 h-4 flex-shrink-0" />
                                {profileError}
                            </div>
                        )}
                    </div>
                </div>
            )}
            <PasswordConfirmModal
                isOpen={!!pwdModal}
                onConfirm={(pwd) => {
                    pwdModal?.resolve(pwd);
                    setPwdModal(null);
                }}
                onCancel={() => {
                    pwdModal?.resolve(null);
                    setPwdModal(null);
                }}
            />
        </div>
    );
}
