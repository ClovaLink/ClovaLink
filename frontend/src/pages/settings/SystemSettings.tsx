import { useState, useEffect } from 'react';
import { Save, Check, Loader2, AlertTriangle, Power, RefreshCw, ExternalLink, Github, Package, ArrowUp, CheckCircle2 } from 'lucide-react';
import { useGlobalSettings } from '../../context/GlobalSettingsContext';
import { useAuth } from '../../context/AuthContext';
import clsx from 'clsx';

interface VersionInfo {
    current_version: string;
    latest_version?: string;
    update_available: boolean;
    release_url?: string;
    release_notes?: string;
    published_at?: string;
    check_error?: string;
}

export function SystemSettings() {
    const { settings, updateSettings } = useGlobalSettings();
    const { token } = useAuth();
    
    const [maintenanceMode, setMaintenanceMode] = useState(settings.maintenance_mode);
    const [maintenanceMessage, setMaintenanceMessage] = useState(settings.maintenance_message);
    const [githubRepo, setGithubRepo] = useState(settings.github_repo || '');
    const [isSaving, setIsSaving] = useState(false);
    const [saveSuccess, setSaveSuccess] = useState(false);
    
    // Version check state
    const [versionInfo, setVersionInfo] = useState<VersionInfo | null>(null);
    const [isCheckingVersion, setIsCheckingVersion] = useState(false);
    const [versionError, setVersionError] = useState<string | null>(null);

    const hasChanges = 
        maintenanceMode !== settings.maintenance_mode ||
        maintenanceMessage !== settings.maintenance_message ||
        githubRepo !== (settings.github_repo || '');

    useEffect(() => {
        setMaintenanceMode(settings.maintenance_mode);
        setMaintenanceMessage(settings.maintenance_message);
        setGithubRepo(settings.github_repo || '');
    }, [settings]);

    // Check version on mount
    useEffect(() => {
        checkVersion();
    }, []);

    const checkVersion = async () => {
        setIsCheckingVersion(true);
        setVersionError(null);
        
        try {
            const response = await fetch('/api/admin/version', {
                headers: {
                    'Authorization': `Bearer ${token}`,
                },
            });
            
            if (!response.ok) {
                throw new Error('Failed to check version');
            }
            
            const data = await response.json();
            setVersionInfo(data);
        } catch (err) {
            setVersionError(err instanceof Error ? err.message : 'Failed to check version');
        } finally {
            setIsCheckingVersion(false);
        }
    };

    const handleSave = async () => {
        setIsSaving(true);
        setSaveSuccess(false);
        
        const success = await updateSettings({
            maintenance_mode: maintenanceMode,
            maintenance_message: maintenanceMessage,
            github_repo: githubRepo || null,
        });
        
        setIsSaving(false);
        if (success) {
            setSaveSuccess(true);
            setTimeout(() => setSaveSuccess(false), 3000);
            // Refresh version info after saving repo
            if (githubRepo) {
                checkVersion();
            }
        }
    };

    const handleToggleMaintenance = () => {
        if (maintenanceMode) {
            // Turning off - just toggle
            setMaintenanceMode(false);
        } else {
            // Turning on - confirm first
            if (confirm('Are you sure you want to enable maintenance mode? Users will be unable to access the system.')) {
                setMaintenanceMode(true);
            }
        }
    };

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-lg font-semibold text-gray-900 dark:text-white">System</h2>
                    <p className="text-sm text-gray-500 dark:text-gray-400">System-wide controls and maintenance settings</p>
                </div>
                <button
                    onClick={handleSave}
                    disabled={!hasChanges || isSaving}
                    className={clsx(
                        "flex items-center px-4 py-2 rounded-lg text-sm font-medium transition-all",
                        hasChanges && !isSaving
                            ? "bg-primary-600 text-white hover:bg-primary-700"
                            : "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
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
            </div>

            {/* Maintenance Mode Card */}
            <div className={clsx(
                "rounded-xl border shadow-sm overflow-hidden transition-colors",
                maintenanceMode 
                    ? "bg-amber-50 dark:bg-amber-900/20 border-amber-300 dark:border-amber-700"
                    : "bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700"
            )}>
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                        <div className={clsx(
                            "p-2 rounded-lg",
                            maintenanceMode
                                ? "bg-amber-100 dark:bg-amber-900/50"
                                : "bg-gray-100 dark:bg-gray-700"
                        )}>
                            <AlertTriangle className={clsx(
                                "w-5 h-5",
                                maintenanceMode
                                    ? "text-amber-600 dark:text-amber-400"
                                    : "text-gray-500 dark:text-gray-400"
                            )} />
                        </div>
                        <div>
                            <h3 className="font-medium text-gray-900 dark:text-white">Maintenance Mode</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                Temporarily disable access for all users except SuperAdmins
                            </p>
                        </div>
                    </div>
                    
                    {/* Toggle Switch */}
                    <button
                        onClick={handleToggleMaintenance}
                        className={clsx(
                            "relative inline-flex h-7 w-14 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2",
                            maintenanceMode
                                ? "bg-amber-500"
                                : "bg-gray-300 dark:bg-gray-600"
                        )}
                    >
                        <span
                            className={clsx(
                                "inline-flex items-center justify-center h-5 w-5 transform rounded-full bg-white shadow-md transition-transform",
                                maintenanceMode ? "translate-x-8" : "translate-x-1"
                            )}
                        >
                            <Power className={clsx(
                                "w-3 h-3",
                                maintenanceMode ? "text-amber-500" : "text-gray-400"
                            )} />
                        </span>
                    </button>
                </div>
                
                <div className="p-6 space-y-4">
                    {maintenanceMode && (
                        <div className="flex items-start gap-3 p-4 bg-amber-100 dark:bg-amber-900/30 rounded-lg">
                            <AlertTriangle className="w-5 h-5 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
                            <div>
                                <p className="text-sm font-medium text-amber-800 dark:text-amber-200">
                                    Maintenance mode is currently active
                                </p>
                                <p className="text-xs text-amber-700 dark:text-amber-300 mt-1">
                                    Only SuperAdmins can access the system. Remember to save changes to apply.
                                </p>
                            </div>
                        </div>
                    )}
                    
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            Maintenance Message
                        </label>
                        <textarea
                            value={maintenanceMessage}
                            onChange={(e) => setMaintenanceMessage(e.target.value)}
                            rows={3}
                            placeholder="We are currently performing scheduled maintenance. Please check back soon."
                            className="w-full px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-primary-500"
                        />
                        <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                            This message will be shown to users when maintenance mode is active
                        </p>
                    </div>

                    {/* Preview */}
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            Preview
                        </label>
                        <div className="border border-gray-200 dark:border-gray-600 rounded-lg overflow-hidden">
                            <div className="bg-amber-500 px-4 py-3 flex items-center gap-3">
                                <AlertTriangle className="w-5 h-5 text-white" />
                                <span className="text-white font-medium text-sm">System Maintenance</span>
                            </div>
                            <div className="p-6 bg-gray-50 dark:bg-gray-900 text-center">
                                <p className="text-gray-600 dark:text-gray-400">
                                    {maintenanceMessage || 'We are currently performing scheduled maintenance. Please check back soon.'}
                                </p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Version & Updates Card */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                        <div className="p-2 rounded-lg bg-primary-100 dark:bg-primary-900/50">
                            <Package className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                        </div>
                        <div>
                            <h3 className="font-medium text-gray-900 dark:text-white">Version & Updates</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                Current version and update notifications
                            </p>
                        </div>
                    </div>
                </div>
                
                <div className="p-6 space-y-6">
                    {/* Current Version Display */}
                    <div className="flex items-center justify-between p-4 bg-gray-50 dark:bg-gray-900/50 rounded-lg">
                        <div className="flex items-center gap-4">
                            <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-primary-500 to-primary-700 flex items-center justify-center">
                                <span className="text-white font-bold text-lg">CL</span>
                            </div>
                            <div>
                                <h4 className="font-semibold text-gray-900 dark:text-white">ClovaLink</h4>
                                <div className="flex items-center gap-2 mt-1">
                                    <span className="text-sm text-gray-600 dark:text-gray-400">Version</span>
                                    <span className="px-2 py-0.5 bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300 rounded text-sm font-mono font-medium">
                                        v{versionInfo?.current_version || '...'}
                                    </span>
                                </div>
                            </div>
                        </div>
                        
                        <button
                            onClick={checkVersion}
                            disabled={isCheckingVersion}
                            className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
                        >
                            <RefreshCw className={clsx("w-4 h-4", isCheckingVersion && "animate-spin")} />
                            {isCheckingVersion ? 'Checking...' : 'Check for Updates'}
                        </button>
                    </div>

                    {/* Update Status */}
                    {versionInfo && !versionInfo.check_error && (
                        <div className={clsx(
                            "flex items-start gap-3 p-4 rounded-lg",
                            versionInfo.update_available
                                ? "bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800"
                                : "bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800"
                        )}>
                            {versionInfo.update_available ? (
                                <>
                                    <ArrowUp className="w-5 h-5 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
                                    <div className="flex-1">
                                        <p className="text-sm font-medium text-amber-800 dark:text-amber-200">
                                            Update Available: v{versionInfo.latest_version}
                                        </p>
                                        <p className="text-xs text-amber-700 dark:text-amber-300 mt-1">
                                            {versionInfo.published_at && (
                                                <>Released {new Date(versionInfo.published_at).toLocaleDateString()}</>
                                            )}
                                        </p>
                                        {versionInfo.release_notes && (
                                            <p className="text-xs text-amber-700 dark:text-amber-300 mt-2 line-clamp-3">
                                                {versionInfo.release_notes}
                                            </p>
                                        )}
                                        {versionInfo.release_url && (
                                            <a
                                                href={versionInfo.release_url}
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                className="inline-flex items-center gap-1.5 mt-3 text-sm font-medium text-amber-700 dark:text-amber-300 hover:text-amber-900 dark:hover:text-amber-100"
                                            >
                                                <ExternalLink className="w-4 h-4" />
                                                View Release on GitHub
                                            </a>
                                        )}
                                    </div>
                                </>
                            ) : (
                                <>
                                    <CheckCircle2 className="w-5 h-5 text-green-600 dark:text-green-400 flex-shrink-0 mt-0.5" />
                                    <div>
                                        <p className="text-sm font-medium text-green-800 dark:text-green-200">
                                            You're up to date!
                                        </p>
                                        <p className="text-xs text-green-700 dark:text-green-300 mt-1">
                                            ClovaLink v{versionInfo.current_version} is the latest version
                                        </p>
                                    </div>
                                </>
                            )}
                        </div>
                    )}

                    {/* Error State */}
                    {(versionError || versionInfo?.check_error) && (
                        <div className="flex items-start gap-3 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-lg">
                            <AlertTriangle className="w-5 h-5 text-gray-500 dark:text-gray-400 flex-shrink-0 mt-0.5" />
                            <div>
                                <p className="text-sm text-gray-600 dark:text-gray-400">
                                    {versionError || versionInfo?.check_error}
                                </p>
                                {!githubRepo && (
                                    <p className="text-xs text-gray-500 dark:text-gray-500 mt-1">
                                        Configure a GitHub repository below to enable update checks
                                    </p>
                                )}
                            </div>
                        </div>
                    )}

                    {/* GitHub Repository Configuration */}
                    <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            <div className="flex items-center gap-2">
                                <Github className="w-4 h-4" />
                                GitHub Repository
                            </div>
                        </label>
                        <div className="flex gap-3">
                            <input
                                type="text"
                                value={githubRepo}
                                onChange={(e) => setGithubRepo(e.target.value)}
                                placeholder="owner/repository (e.g., clovalink/clovalink)"
                                className="flex-1 px-4 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-primary-500 font-mono text-sm"
                            />
                        </div>
                        <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                            Enter your GitHub repository to check for new releases. Format: <code className="px-1 py-0.5 bg-gray-100 dark:bg-gray-800 rounded">owner/repo</code>
                        </p>
                    </div>
                </div>
            </div>

        </div>
    );
}

