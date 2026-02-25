import { AlertTriangle, LogOut, Wrench } from 'lucide-react';
import { useGlobalSettings } from '../context/GlobalSettingsContext';
import { useAuth } from '../context/AuthContext';

export function MaintenanceOverlay() {
    const { settings } = useGlobalSettings();
    const { user, logout } = useAuth();

    // Don't show overlay if:
    // - Maintenance mode is off
    // - User is SuperAdmin (they can still access)
    // - User is not logged in (let them see login page)
    if (!settings.maintenance_mode || user?.role === 'SuperAdmin' || !user) {
        return null;
    }

    return (
        <div className="fixed inset-0 z-[9999] bg-gray-900/95 backdrop-blur-sm flex items-center justify-center p-4">
            <div className="max-w-lg w-full">
                {/* Animated background elements */}
                <div className="absolute inset-0 overflow-hidden pointer-events-none">
                    <div className="absolute top-1/4 left-1/4 w-64 h-64 bg-amber-500/10 rounded-full blur-3xl animate-pulse" />
                    <div className="absolute bottom-1/4 right-1/4 w-48 h-48 bg-amber-500/5 rounded-full blur-3xl animate-pulse delay-1000" />
                </div>

                <div className="relative bg-gray-800 rounded-2xl border border-gray-700 shadow-2xl overflow-hidden">
                    {/* Header bar */}
                    <div className="bg-amber-500 px-6 py-4 flex items-center gap-3">
                        <div className="p-2 bg-amber-600/50 rounded-lg">
                            <Wrench className="w-6 h-6 text-white" />
                        </div>
                        <div>
                            <h2 className="text-lg font-bold text-white">System Maintenance</h2>
                            <p className="text-amber-100 text-sm">We'll be back soon</p>
                        </div>
                    </div>

                    {/* Content */}
                    <div className="p-8 text-center">
                        <div className="mb-6">
                            <div className="inline-flex items-center justify-center w-20 h-20 bg-amber-500/10 rounded-full mb-4">
                                <AlertTriangle className="w-10 h-10 text-amber-400" />
                            </div>
                        </div>

                        <p className="text-gray-300 text-lg mb-6 leading-relaxed">
                            {settings.maintenance_message || 'We are currently performing scheduled maintenance. Please check back soon.'}
                        </p>

                        <div className="flex items-center justify-center gap-2 text-sm text-gray-500">
                            <div className="w-2 h-2 bg-amber-500 rounded-full animate-pulse" />
                            <span>Maintenance in progress</span>
                        </div>
                    </div>

                    {/* Actions */}
                    <div className="px-8 pt-2 pb-6 flex justify-center">
                        <button
                            onClick={logout}
                            className="inline-flex items-center gap-2 px-5 py-2.5 bg-amber-500 hover:bg-amber-600 text-white font-medium rounded-lg transition-colors"
                        >
                            <LogOut className="w-4 h-4" />
                            Log Out
                        </button>
                    </div>

                    {/* Footer */}
                    <div className="px-8 py-4 bg-gray-900/50 border-t border-gray-700">
                        <p className="text-xs text-gray-500 text-center">
                            If you believe this is an error, please contact your system administrator.
                        </p>
                    </div>
                </div>
            </div>
        </div>
    );
}

