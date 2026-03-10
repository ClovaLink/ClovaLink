import { useState, useEffect } from 'react';
import { Bell, Mail, BellRing, Info } from 'lucide-react';
import { useAuthFetch, useAuth } from '../context/AuthContext';

interface NotificationPreference {
    id: string;
    user_id: string;
    event_type: string;
    email_enabled: boolean;
    in_app_enabled: boolean;
}

interface CompanySetting {
    enabled: boolean;
    email_enforced: boolean;
    in_app_enforced: boolean;
    default_email: boolean;
    default_in_app: boolean;
}

interface PreferencesWithCompany {
    preferences: NotificationPreference[];
    company_settings: Record<string, CompanySetting>;
    is_exempt?: boolean;  // SuperAdmins are exempt from company controls
    user_role?: string;
}

interface PreferenceLabel {
    event_type: string;
    label: string;
    description: string;
}

const DEFAULT_LABELS: PreferenceLabel[] = [
    { event_type: 'file_upload', label: 'File Uploads', description: 'Notifications when files are uploaded to your file requests' },
    { event_type: 'request_expiring', label: 'Expiring Requests', description: 'Reminders when your file requests are about to expire' },
    { event_type: 'user_action', label: 'User Actions', description: 'Notifications about new users and role changes' },
    { event_type: 'compliance_alert', label: 'Compliance Alerts', description: 'Important compliance-related notifications' },
    { event_type: 'storage_warning', label: 'Storage Warnings', description: 'Alerts when storage quota is running low' },
    { event_type: 'file_shared', label: 'File Sharing', description: 'Notifications when files are shared with you' },
    { event_type: 'approval', label: 'Approvals', description: 'Notifications about document approvals requiring your action' }
];

interface NotificationPreferencesProps {
    compact?: boolean;
}

export function NotificationPreferences({ compact = false }: NotificationPreferencesProps) {
    const { user } = useAuth();
    const authFetch = useAuthFetch();
    const [preferences, setPreferences] = useState<NotificationPreference[]>([]);
    const [companySettings, setCompanySettings] = useState<Record<string, CompanySetting>>({});
    const [isExempt, setIsExempt] = useState(false);
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);

    useEffect(() => {
        fetchPreferences();
    }, []);

    const fetchPreferences = async () => {
        setIsLoading(true);
        try {
            const res = await authFetch('/api/notifications/preferences-with-company');
            if (res.ok) {
                const data: PreferencesWithCompany = await res.json();
                setPreferences(data.preferences || []);
                setCompanySettings(data.company_settings || {});
                setIsExempt(data.is_exempt || false);
            }
        } catch (error) {
            console.error('Failed to fetch preferences', error);
        } finally {
            setIsLoading(false);
        }
    };

    const handleUpdate = async (eventType: string, field: 'email_enabled' | 'in_app_enabled', value: boolean) => {
        setIsSaving(true);
        try {
            const res = await authFetch('/api/notifications/preferences', {
                method: 'PUT',
                body: JSON.stringify({
                    preferences: [{
                        event_type: eventType,
                        [field]: value
                    }]
                })
            });
            if (res.ok) {
                const updated = await res.json();
                setPreferences(updated);
            }
        } catch (error) {
            console.error('Failed to update preference', error);
        } finally {
            setIsSaving(false);
        }
    };

    const canAccessPreference = (eventType: string) => {
        const adminOnlyTypes = ['user_action', 'compliance_alert', 'storage_warning'];
        if (adminOnlyTypes.includes(eventType)) {
            return user?.role === 'SuperAdmin' || user?.role === 'Admin';
        }
        return true;
    };

    const isEventEnabled = (eventType: string) => {
        // SuperAdmins are exempt - all event types are enabled
        if (isExempt) return true;
        const companySetting = companySettings[eventType];
        return companySetting?.enabled !== false;
    };

    const isEmailEnforced = (eventType: string) => {
        // SuperAdmins are exempt - nothing is enforced
        if (isExempt) return false;
        return companySettings[eventType]?.email_enforced === true;
    };

    const isInAppEnforced = (eventType: string) => {
        // SuperAdmins are exempt - nothing is enforced
        if (isExempt) return false;
        return companySettings[eventType]?.in_app_enforced === true;
    };

    const getPreference = (eventType: string) => {
        return preferences.find(p => p.event_type === eventType);
    };

    // Filter to only show enabled event types that user can access
    const visibleLabels = DEFAULT_LABELS.filter(label => 
        canAccessPreference(label.event_type) && isEventEnabled(label.event_type)
    );

    if (isLoading) {
        return (
            <div className={`bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 ${compact ? 'p-4' : 'p-6'}`}>
                <div className="animate-pulse space-y-4">
                    {[...Array(4)].map((_, i) => (
                        <div key={i} className="flex items-center justify-between">
                            <div className="space-y-2">
                                <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded w-32"></div>
                                <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded w-48"></div>
                            </div>
                            <div className="flex space-x-4">
                                <div className="h-6 w-12 bg-gray-200 dark:bg-gray-700 rounded"></div>
                                <div className="h-6 w-12 bg-gray-200 dark:bg-gray-700 rounded"></div>
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        );
    }

    if (visibleLabels.length === 0) {
        return (
            <div className={`bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 ${compact ? 'p-4' : 'p-6'}`}>
                <div className="text-center py-6">
                    <Bell className="w-12 h-12 mx-auto text-gray-300 dark:text-gray-600 mb-3" />
                    <p className="text-gray-500 dark:text-gray-400">No notification settings available</p>
                    <p className="text-sm text-gray-400 dark:text-gray-500 mt-1">
                        Your company administrator has disabled all notifications
                    </p>
                </div>
            </div>
        );
    }

    return (
        <div className={`bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden`}>
            {!compact && (
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                    <h3 className="text-lg font-medium text-gray-900 dark:text-white flex items-center gap-2">
                        <Bell className="w-5 h-5 text-primary-600" />
                        Notification Preferences
                    </h3>
                    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                        Customize how you receive notifications
                    </p>
                    {isExempt && (
                        <p className="text-xs text-green-600 dark:text-green-400 mt-2 flex items-center gap-1">
                            <Info className="w-3 h-3" />
                            As a SuperAdmin, you have full control over your notification preferences.
                        </p>
                    )}
                </div>
            )}
            
            <div className="divide-y divide-gray-200 dark:divide-gray-700">
                {visibleLabels.map((label) => {
                    const pref = getPreference(label.event_type);
                    const emailEnforced = isEmailEnforced(label.event_type);
                    const inAppEnforced = isInAppEnforced(label.event_type);
                    
                    return (
                        <div key={label.event_type} className={compact ? 'p-4' : 'p-6'}>
                            <div className="flex items-start justify-between">
                                <div className="flex-1">
                                    <h4 className="text-sm font-medium text-gray-900 dark:text-white">
                                        {label.label}
                                    </h4>
                                    {!compact && (
                                        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                                            {label.description}
                                        </p>
                                    )}
                                </div>
                                
                                <div className="flex items-center space-x-6 ml-4">
                                    {/* In-App Toggle - Hidden if enforced */}
                                    {!inAppEnforced && (
                                        <label className="flex items-center space-x-2 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={pref?.in_app_enabled ?? true}
                                                onChange={(e) => handleUpdate(label.event_type, 'in_app_enabled', e.target.checked)}
                                                disabled={isSaving}
                                                className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                                            />
                                            <BellRing className="w-4 h-4 text-gray-400" />
                                            <span className="text-sm text-gray-600 dark:text-gray-300">In-app</span>
                                        </label>
                                    )}
                                    
                                    {/* Email Toggle - Hidden if enforced */}
                                    {!emailEnforced && (
                                        <label className="flex items-center space-x-2 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={pref?.email_enabled ?? true}
                                                onChange={(e) => handleUpdate(label.event_type, 'email_enabled', e.target.checked)}
                                                disabled={isSaving}
                                                className="w-4 h-4 text-primary-600 border-gray-300 rounded focus:ring-primary-500"
                                            />
                                            <Mail className="w-4 h-4 text-gray-400" />
                                            <span className="text-sm text-gray-600 dark:text-gray-300">Email</span>
                                        </label>
                                    )}
                                    
                                    {/* Show indicator if both are enforced */}
                                    {emailEnforced && inAppEnforced && (
                                        <span className="text-xs text-gray-400 dark:text-gray-500 flex items-center gap-1">
                                            <Info className="w-3 h-3" />
                                            Set by admin
                                        </span>
                                    )}
                                </div>
                            </div>
                        </div>
                    );
                })}
            </div>
            
            {!compact && Object.keys(companySettings).length > 0 && (
                <div className="px-6 py-4 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700">
                    <p className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
                        <Info className="w-3 h-3" />
                        Some settings may be managed by your company administrator
                    </p>
                </div>
            )}
        </div>
    );
}
