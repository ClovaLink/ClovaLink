import { Link } from 'react-router-dom';
import { ExternalLink } from 'lucide-react';
import { Logo } from './Logo';
import { useSettings } from '../context/SettingsContext';
import { useGlobalSettings } from '../context/GlobalSettingsContext';

export function Footer({ collapsed = false }: { collapsed?: boolean }) {
    const { complianceMode, isComplianceActive } = useSettings();
    const { settings } = useGlobalSettings();

    const complianceTag = isComplianceActive && complianceMode !== 'Standard' && complianceMode !== 'None' && (
        <span className="text-[10px] sm:text-xs text-primary-600 dark:text-primary-400">
            {complianceMode === 'HIPAA' && 'This system is configured for HIPAA compliance. All PHI access is logged and audited.'}
            {complianceMode === 'SOX' && 'This system is configured for SOX compliance. Document versioning is enforced and all changes are logged.'}
            {complianceMode === 'GDPR' && 'This system is configured for GDPR compliance. Personal data is protected according to EU regulations.'}
        </span>
    );

    if (collapsed) {
        return (
            <footer className="mt-auto border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 transition-all duration-300">
                {complianceTag && (
                    <div className="px-4 py-2 text-center">
                        {complianceTag}
                    </div>
                )}
            </footer>
        );
    }

    return (
        <footer className="mt-auto border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 transition-all duration-300">
            <div className="px-4 py-4 sm:px-6 sm:py-6">
                <div className="flex flex-col items-center gap-3 sm:gap-4 md:flex-row md:gap-6">
                    {/* Logo */}
                    <Logo className="h-5 sm:h-6 w-auto text-gray-900 dark:text-white flex-shrink-0" compact />

                    {/* Content */}
                    <div className="flex flex-col items-center md:items-start gap-2 sm:gap-3 flex-1">
                        {/* Open Source Attribution */}
                        <div className="text-xs sm:text-sm text-center md:text-left">
                            <span className="text-gray-600 dark:text-gray-400">{settings.footer_attribution} </span>
                            <a
                                href="https://clovalink.org"
                                target="_blank"
                                rel="noopener noreferrer"
                                className="text-primary-600 dark:text-primary-400 hover:underline inline-flex items-center gap-1"
                            >
                                <ExternalLink className="w-3 h-3" />
                            </a>
                        </div>

                        {/* Links */}
                        <div className="flex flex-wrap items-center justify-center md:justify-start gap-2 sm:gap-4 text-xs sm:text-sm">
                            <Link
                                to="/quickstart"
                                className="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
                            >
                                Quickstart
                            </Link>
                            <span className="text-gray-300 dark:text-gray-600">|</span>
                            <Link
                                to="/help"
                                className="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
                            >
                                Help
                            </Link>
                            <span className="text-gray-300 dark:text-gray-600">|</span>
                            <Link
                                to="/privacy"
                                className="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
                            >
                                Privacy Policy
                            </Link>
                            <span className="hidden sm:inline text-gray-300 dark:text-gray-600">|</span>
                            <Link
                                to="/terms"
                                className="text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
                            >
                                Terms of Service
                            </Link>
                        </div>

                        {/* Disclaimer */}
                        <p className="text-[10px] sm:text-xs text-center md:text-left text-gray-500 dark:text-gray-500">
                            {settings.footer_disclaimer}
                            {complianceTag && <span className="block mt-1">{complianceTag}</span>}
                        </p>
                    </div>
                </div>
            </div>
        </footer>
    );
}
