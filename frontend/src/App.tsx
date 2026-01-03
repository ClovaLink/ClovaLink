import { lazy, Suspense } from 'react';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { Layout } from './components/Layout';
import { ProtectedRoute, AdminRoute, SuperAdminRoute, RoleProtectedRoute, PermissionProtectedRoute } from './components/ProtectedRoute';

// Eagerly load critical path components
import { Login } from './pages/Login';
import { PublicUpload } from './pages/PublicUpload';
import { PublicDownload } from './pages/PublicDownload';

// Lazy load heavy page components for code splitting
const Dashboard = lazy(() => import('./pages/Dashboard').then(m => ({ default: m.Dashboard })));
const Companies = lazy(() => import('./pages/Companies').then(m => ({ default: m.Companies })));
const CompanyDetails = lazy(() => import('./pages/CompanyDetails').then(m => ({ default: m.CompanyDetails })));
const Users = lazy(() => import('./pages/Users').then(m => ({ default: m.Users })));
const RolesPage = lazy(() => import('./pages/Roles').then(m => ({ default: m.RolesPage })));
const AuditLogsPage = lazy(() => import('./pages/AuditLogs').then(m => ({ default: m.AuditLogsPage })));
const Security = lazy(() => import('./pages/Security').then(m => ({ default: m.Security })));
const FileBrowser = lazy(() => import('./pages/FileBrowser').then(m => ({ default: m.FileBrowser })));
const FileRequests = lazy(() => import('./pages/FileRequests').then(m => ({ default: m.FileRequests })));
const RecycleBin = lazy(() => import('./pages/RecycleBin'));
const SharedWithMe = lazy(() => import('./pages/SharedWithMe').then(m => ({ default: m.SharedWithMe })));
const Help = lazy(() => import('./pages/Help').then(m => ({ default: m.Help })));
const Profile = lazy(() => import('./pages/Profile').then(m => ({ default: m.Profile })));
const Extensions = lazy(() => import('./pages/Extensions').then(m => ({ default: m.Extensions })));
const ExtensionDetails = lazy(() => import('./pages/ExtensionDetails').then(m => ({ default: m.ExtensionDetails })));
const Notifications = lazy(() => import('./pages/Notifications').then(m => ({ default: m.Notifications })));
const PrivacyPolicy = lazy(() => import('./pages/PrivacyPolicy'));
const TermsOfService = lazy(() => import('./pages/TermsOfService'));
const Quickstart = lazy(() => import('./pages/Quickstart'));
const Performance = lazy(() => import('./pages/Performance'));

// Lazy load settings pages
const SettingsLayout = lazy(() => import('./pages/settings').then(m => ({ default: m.SettingsLayout })));
const GeneralSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.GeneralSettings })));
const BrandingSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.BrandingSettings })));
const PagesSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.PagesSettings })));
const SystemSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.SystemSettings })));
const AdminSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.AdminSettings })));
const EmailTemplatesSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.EmailTemplatesSettings })));
const KeyboardShortcutsSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.KeyboardShortcutsSettings })));
const VirusScanSettings = lazy(() => import('./pages/settings').then(m => ({ default: m.VirusScanSettings })));

import { AuthProvider } from './context/AuthContext';
import { SettingsProvider } from './context/SettingsContext';
import { TenantProvider } from './context/TenantContext';
import { ThemeProvider } from './context/ThemeContext';
import { ComplianceProvider } from './context/ComplianceContext';
import { ExtensionProvider } from './context/ExtensionContext';
import { GlobalSettingsProvider } from './context/GlobalSettingsContext';
import { KeyboardShortcutsProvider } from './context/KeyboardShortcutsContext';
import { MaintenanceOverlay } from './components/MaintenanceOverlay';
import { KeyboardShortcutsModal } from './components/KeyboardShortcutsModal';

// Loading fallback component
function PageLoader() {
    return (
        <div className="flex items-center justify-center min-h-[400px]">
            <div className="flex flex-col items-center gap-3">
                <div className="w-8 h-8 border-4 border-primary-200 dark:border-primary-800 border-t-primary-600 dark:border-t-primary-400 rounded-full animate-spin" />
                <span className="text-sm text-gray-500 dark:text-gray-400">Loading...</span>
            </div>
        </div>
    );
}

function App() {
    return (
        <Router>
            <AuthProvider>
                <ThemeProvider>
                    <GlobalSettingsProvider>
                        <MaintenanceOverlay />
                        <SettingsProvider>
                            <TenantProvider>
                                <ComplianceProvider>
                                    <ExtensionProvider>
                                        <KeyboardShortcutsProvider>
                                        <KeyboardShortcutsModal />
                                        <Suspense fallback={<PageLoader />}>
                                            <Routes>
                                            <Route path="/login" element={<Login />} />
                                            <Route path="/upload/:token" element={<PublicUpload />} />
                                            <Route path="/share/:token" element={<PublicDownload />} />
                                            <Route path="/" element={<ProtectedRoute><Layout /></ProtectedRoute>}>
                                                {/* Admin-only routes */}
                                                <Route index element={<AdminRoute><Dashboard /></AdminRoute>} />
                                                <Route path="users" element={<RoleProtectedRoute allowedRoles={['SuperAdmin', 'Admin', 'Manager']}><Users /></RoleProtectedRoute>} />
                                                <Route path="roles" element={<AdminRoute><RolesPage /></AdminRoute>} />
                                                <Route path="audit-logs" element={<AdminRoute><AuditLogsPage /></AdminRoute>} />
                                                <Route path="security" element={<AdminRoute><Security /></AdminRoute>} />
                                                
                                                {/* Permission-based routes */}
                                                <Route path="companies" element={<PermissionProtectedRoute permission="tenants.manage"><Companies /></PermissionProtectedRoute>} />
                                                <Route path="companies/:slug" element={<PermissionProtectedRoute permission="tenants.manage"><CompanyDetails /></PermissionProtectedRoute>} />
                                                
                                                {/* SuperAdmin-only routes */}
                                                <Route path="performance" element={<SuperAdminRoute><Performance /></SuperAdminRoute>} />
                                                
                                                {/* Admin-only settings */}
                                                <Route path="settings" element={<AdminRoute><SettingsLayout /></AdminRoute>}>
                                                    <Route index element={<Navigate to="/settings/general" replace />} />
                                                    <Route path="general" element={<GeneralSettings />} />
                                                    <Route path="branding" element={<BrandingSettings />} />
                                                    <Route path="pages" element={<PagesSettings />} />
                                                    <Route path="email-templates" element={<EmailTemplatesSettings />} />
                                                    <Route path="shortcuts" element={<KeyboardShortcutsSettings />} />
                                                    <Route path="system" element={<SystemSettings />} />
                                                    <Route path="virus-scan" element={<VirusScanSettings />} />
                                                    <Route path="admin" element={<AdminSettings />} />
                                                </Route>
                                                
                                                {/* All authenticated users */}
                                                <Route path="files" element={<FileBrowser />} />
                                                <Route path="file-requests" element={<FileRequests />} />
                                                <Route path="recycle-bin" element={<RecycleBin />} />
                                                <Route path="shared-with-me" element={<SharedWithMe />} />
                                                <Route path="extensions" element={<Extensions />} />
                                                <Route path="extensions/:id" element={<ExtensionDetails />} />
                                                <Route path="profile" element={<Profile />} />
                                                <Route path="notifications" element={<Notifications />} />
                                                <Route path="help" element={<Help />} />
                                                <Route path="privacy" element={<PrivacyPolicy />} />
                                                <Route path="terms" element={<TermsOfService />} />
                                                <Route path="quickstart" element={<Quickstart />} />
                                            </Route>
                                        </Routes>
                                        </Suspense>
                                        </KeyboardShortcutsProvider>
                                    </ExtensionProvider>
                                </ComplianceProvider>
                            </TenantProvider>
                        </SettingsProvider>
                    </GlobalSettingsProvider>
                </ThemeProvider>
            </AuthProvider>
        </Router>
    );
}

export default App;
