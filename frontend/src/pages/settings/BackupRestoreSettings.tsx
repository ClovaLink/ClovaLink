import { BackupRestore } from '../../components/BackupRestore';

export function BackupRestoreSettings() {
    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Global Backup & Restore</h1>
                <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
                    Export and import global settings including app configuration, default email templates, and branding. Backups are encrypted with a passphrase you choose.
                </p>
            </div>
            <BackupRestore type="global" />
        </div>
    );
}
