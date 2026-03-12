import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { Settings, Image, Shield, ShieldCheck, Building2, ArrowRight, BookOpen, Wrench, Mail, Keyboard, Globe, HardDrive } from 'lucide-react';
import { useAuth } from '../../context/AuthContext';
import clsx from 'clsx';

const TABS = [
    { id: 'general', label: 'General', path: '/settings/general', icon: Settings },
    { id: 'branding', label: 'Branding', path: '/settings/branding', icon: Image },
    { id: 'pages', label: 'Pages', path: '/settings/pages', icon: BookOpen },
    { id: 'email-templates', label: 'Email Templates', path: '/settings/email-templates', icon: Mail },
    { id: 'shortcuts', label: 'Shortcuts', path: '/settings/shortcuts', icon: Keyboard },
    { id: 'system', label: 'System', path: '/settings/system', icon: Wrench },
    { id: 'virus-scan', label: 'Virus Scan', path: '/settings/virus-scan', icon: ShieldCheck },
    { id: 'sso', label: 'SSO', path: '/settings/sso', icon: Globe },
    { id: 'backup', label: 'Backup', path: '/settings/backup', icon: HardDrive },
    { id: 'admin', label: 'Administration', path: '/settings/admin', icon: Shield },
];

export function SettingsLayout() {
    const { user, tenant } = useAuth();
    const navigate = useNavigate();

    // Non-SuperAdmin users get redirected to Company Details (per-tenant settings)
    if (!user || user.role !== 'SuperAdmin') {
        return (
            <div className="max-w-4xl mx-auto py-12 px-4 sm:px-6 lg:px-8">
                <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-8 text-center">
                    <div className="mx-auto h-12 w-12 bg-primary-100 dark:bg-primary-900/30 rounded-full flex items-center justify-center mb-4">
                        <Building2 className="h-6 w-6 text-primary-600 dark:text-primary-400" />
                    </div>
                    <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">
                        Company Settings
                    </h2>
                    <p className="text-gray-500 dark:text-gray-400 mb-8 max-w-md mx-auto">
                        Company settings, departments, and compliance configurations are managed on the Company Details page.
                    </p>
                    <button
                        onClick={() => navigate(`/companies/${encodeURIComponent(tenant?.name || '')}`)}
                        className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-primary-600 hover:bg-primary-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-primary-500"
                    >
                        Go to Company Details
                        <ArrowRight className="ml-2 h-4 w-4" />
                    </button>
                </div>
            </div>
        );
    }

    return (
        <div className="h-full flex flex-col">
            {/* Header with Tabs */}
            <div className="flex-shrink-0 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
                <div className="px-4 sm:px-8 pt-4 sm:pt-6 pb-0">
                    <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white">Global Settings</h1>
                    <p className="mt-1 text-xs sm:text-sm text-gray-500 dark:text-gray-400 mb-4 sm:mb-6">
                        Configure system-wide preferences for all companies
                    </p>
                    
                    {/* Horizontal Tab Navigation - scrollable on mobile */}
                    <div className="overflow-x-auto overflow-y-hidden -mx-4 px-4 sm:mx-0 sm:px-0">
                        <nav className="flex gap-1 -mb-px min-w-max">
                            {TABS.map((tab) => {
                                const Icon = tab.icon;
                                return (
                                    <NavLink
                                        key={tab.id}
                                        to={tab.path}
                                        className={({ isActive }) =>
                                            clsx(
                                                "flex items-center gap-1.5 sm:gap-2 px-3 sm:px-4 py-2.5 sm:py-3 text-xs sm:text-sm font-medium border-b-2 transition-colors whitespace-nowrap",
                                                isActive
                                                    ? "border-primary-500 text-primary-600 dark:text-primary-400"
                                                    : "border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600"
                                            )
                                        }
                                    >
                                        <Icon className="w-4 h-4" />
                                        <span className="hidden xs:inline sm:inline">{tab.label}</span>
                                    </NavLink>
                                );
                            })}
                        </nav>
                    </div>
                </div>
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-y-auto bg-gray-50 dark:bg-gray-900 p-4 sm:p-8">
                <div className="max-w-6xl mx-auto">
                    <Outlet />
                </div>
            </div>
        </div>
    );
}

