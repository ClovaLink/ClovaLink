import { useState, useEffect, useCallback } from 'react';
import { 
    Mail, Edit2, Check, AlertTriangle, RotateCcw, Upload, Clock, UserPlus, 
    RefreshCw, Share2, ShieldAlert, HardDrive, KeyRound, UserCheck, LucideIcon 
} from 'lucide-react';
import { EmailTemplateEditor } from './EmailTemplateEditor';
import clsx from 'clsx';

interface EmailTemplate {
    template_key: string;
    name: string;
    subject: string;
    body_html: string;
    body_text?: string | null;
    variables: string[];
    is_customized?: boolean;
    global_subject?: string;
    global_body_html?: string;
    global_body_text?: string | null;
    updated_at?: string;
}

interface TenantEmailTemplatesProps {
    tenantId: string;
    authFetch: (url: string, options?: RequestInit) => Promise<Response>;
}

// Template category icons and descriptions
const TEMPLATE_CATEGORIES: Record<string, { Icon: LucideIcon; color: string; description: string }> = {
    file_upload: { Icon: Upload, color: 'blue', description: 'Sent when files are uploaded to a request' },
    request_expiring: { Icon: Clock, color: 'amber', description: 'Sent when file requests are about to expire' },
    user_created: { Icon: UserPlus, color: 'green', description: 'Sent to admins when new users are added' },
    role_changed: { Icon: RefreshCw, color: 'purple', description: 'Sent when user roles are updated' },
    file_shared: { Icon: Share2, color: 'blue', description: 'Sent when files are shared with users' },
    compliance_alert: { Icon: ShieldAlert, color: 'red', description: 'Sent for compliance-related alerts' },
    storage_warning: { Icon: HardDrive, color: 'orange', description: 'Sent when storage quota is running low' },
    password_reset: { Icon: KeyRound, color: 'indigo', description: 'Sent for password reset requests' },
    welcome: { Icon: UserCheck, color: 'teal', description: 'Sent when new users are invited' },
};

export function TenantEmailTemplates({ tenantId, authFetch }: TenantEmailTemplatesProps) {
    const [templates, setTemplates] = useState<EmailTemplate[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [editingTemplate, setEditingTemplate] = useState<EmailTemplate | null>(null);
    const [successMessage, setSuccessMessage] = useState<string | null>(null);

    const fetchTemplates = useCallback(async () => {
        setIsLoading(true);
        setError(null);
        try {
            const response = await authFetch('/api/settings/email-templates');
            if (!response.ok) throw new Error('Failed to fetch templates');
            const data = await response.json();
            // Map the response to include variables as array
            const mapped = data.map((t: any) => ({
                ...t,
                variables: Array.isArray(t.variables) ? t.variables : [],
            }));
            setTemplates(mapped);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to load templates');
        } finally {
            setIsLoading(false);
        }
    }, [authFetch]);

    useEffect(() => {
        fetchTemplates();
    }, [fetchTemplates, tenantId]);

    const handleSave = async (data: { subject: string; body_html: string; body_text?: string }) => {
        if (!editingTemplate) return;
        
        try {
            const response = await authFetch(`/api/settings/email-templates/${editingTemplate.template_key}`, {
                method: 'PUT',
                body: JSON.stringify(data),
            });
            
            if (!response.ok) throw new Error('Failed to save template');
            
            setSuccessMessage(`Template "${editingTemplate.name}" saved successfully`);
            setTimeout(() => setSuccessMessage(null), 3000);
            setEditingTemplate(null);
            fetchTemplates();
        } catch (err) {
            throw err;
        }
    };

    const handleReset = async () => {
        if (!editingTemplate) return;
        
        try {
            const response = await authFetch(`/api/settings/email-templates/${editingTemplate.template_key}`, {
                method: 'DELETE',
            });
            
            if (!response.ok) throw new Error('Failed to reset template');
            
            setSuccessMessage(`Template "${editingTemplate.name}" reset to default`);
            setTimeout(() => setSuccessMessage(null), 3000);
            setEditingTemplate(null);
            fetchTemplates();
        } catch (err) {
            throw err;
        }
    };

    const getTemplateInfo = (key: string) => {
        return TEMPLATE_CATEGORIES[key] || { Icon: Mail, color: 'gray', description: 'Email notification template' };
    };

    if (isLoading) {
        return (
            <div className="flex items-center justify-center py-12">
                <div className="w-8 h-8 border-4 border-primary-200 border-t-primary-600 rounded-full animate-spin" />
            </div>
        );
    }

    return (
        <div className="max-w-6xl mx-auto space-y-6">
            {/* Header */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                    <h3 className="text-lg font-medium text-gray-900 dark:text-white flex items-center gap-2">
                        <Mail className="w-5 h-5 text-primary-600" />
                        Email Templates
                    </h3>
                    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                        Customize email templates for this company. Templates can override global defaults or use the system defaults.
                    </p>
                </div>

                {/* Success Message */}
                {successMessage && (
                    <div className="mx-6 mt-4 flex items-center gap-2 p-3 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300 rounded-lg">
                        <Check className="w-5 h-5" />
                        {successMessage}
                    </div>
                )}

                {/* Error Message */}
                {error && (
                    <div className="mx-6 mt-4 flex items-center gap-2 p-3 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 rounded-lg">
                        <AlertTriangle className="w-5 h-5" />
                        {error}
                    </div>
                )}

                {/* Templates List */}
                <div className="divide-y divide-gray-200 dark:divide-gray-700">
                    {templates.map((template) => {
                        const info = getTemplateInfo(template.template_key);
                        const IconComponent = info.Icon;
                        return (
                            <div
                                key={template.template_key}
                                className="p-4 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
                            >
                                <div className="flex items-start justify-between">
                                    <div className="flex items-start gap-3">
                                        <div className={clsx(
                                            "w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0",
                                            info.color === 'blue' && "bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400",
                                            info.color === 'amber' && "bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400",
                                            info.color === 'green' && "bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400",
                                            info.color === 'purple' && "bg-purple-100 dark:bg-purple-900/30 text-purple-600 dark:text-purple-400",
                                            info.color === 'red' && "bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400",
                                            info.color === 'orange' && "bg-orange-100 dark:bg-orange-900/30 text-orange-600 dark:text-orange-400",
                                            info.color === 'indigo' && "bg-indigo-100 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400",
                                            info.color === 'teal' && "bg-teal-100 dark:bg-teal-900/30 text-teal-600 dark:text-teal-400",
                                            info.color === 'gray' && "bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
                                        )}>
                                            <IconComponent className="w-5 h-5" />
                                        </div>
                                        <div>
                                            <div className="flex items-center gap-2">
                                                <h4 className="font-medium text-gray-900 dark:text-white">
                                                    {template.name}
                                                </h4>
                                                {template.is_customized ? (
                                                    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300">
                                                        Customized
                                                    </span>
                                                ) : (
                                                    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400">
                                                        Using Default
                                                    </span>
                                                )}
                                            </div>
                                            <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                                                {info.description}
                                            </p>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        {template.is_customized && (
                                            <button
                                                onClick={async () => {
                                                    try {
                                                        await authFetch(`/api/settings/email-templates/${template.template_key}`, {
                                                            method: 'DELETE',
                                                        });
                                                        setSuccessMessage(`Template "${template.name}" reset to default`);
                                                        setTimeout(() => setSuccessMessage(null), 3000);
                                                        fetchTemplates();
                                                    } catch (err) {
                                                        setError('Failed to reset template');
                                                    }
                                                }}
                                                className="flex items-center gap-1 px-2 py-1 text-xs text-amber-600 dark:text-amber-400 hover:bg-amber-50 dark:hover:bg-amber-900/20 rounded transition-colors"
                                                title="Reset to default"
                                            >
                                                <RotateCcw className="w-3 h-3" />
                                            </button>
                                        )}
                                        <button
                                            onClick={() => setEditingTemplate(template)}
                                            className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-primary-600 dark:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-lg transition-colors"
                                        >
                                            <Edit2 className="w-4 h-4" />
                                            Edit
                                        </button>
                                    </div>
                                </div>
                                
                                {/* Subject Preview */}
                                <div className="mt-3 ml-11">
                                    <div className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                                        Subject:
                                    </div>
                                    <div className={clsx(
                                        "text-sm px-3 py-2 rounded-md font-mono",
                                        template.is_customized
                                            ? "text-blue-700 dark:text-blue-300 bg-blue-50 dark:bg-blue-900/20"
                                            : "text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-900/50"
                                    )}>
                                        {template.subject}
                                    </div>
                                </div>

                                {/* Variables */}
                                {template.variables && template.variables.length > 0 && (
                                    <div className="mt-2 ml-11 flex flex-wrap gap-1">
                                        {template.variables.map((v: string) => (
                                            <span
                                                key={v}
                                                className="px-1.5 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded font-mono"
                                            >
                                                {`{{${v}}}`}
                                            </span>
                                        ))}
                                    </div>
                                )}
                            </div>
                        );
                    })}
                </div>
            </div>

            {/* Info Box */}
            <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
                <h4 className="text-sm font-medium text-blue-800 dark:text-blue-300">About Email Templates</h4>
                <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                    <strong>Customized:</strong> Templates you've edited will be used for all emails sent from this company.<br/>
                    <strong>Using Default:</strong> Templates using the global default set by the system administrator.<br/>
                    <strong>Variables:</strong> Use {`{{variable_name}}`} to insert dynamic content like user names, file names, etc.
                </p>
            </div>

            {/* Editor Modal */}
            {editingTemplate && (
                <EmailTemplateEditor
                    template={editingTemplate}
                    onSave={handleSave}
                    onReset={handleReset}
                    onClose={() => setEditingTemplate(null)}
                    canReset={editingTemplate.is_customized}
                />
            )}
        </div>
    );
}

