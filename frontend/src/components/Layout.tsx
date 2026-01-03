import { Outlet, NavLink, useNavigate, useLocation } from 'react-router-dom';
import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { LayoutDashboard, Users, FileText, Settings, Building2, Search, ChevronDown, LogOut, Puzzle, Folder, User, Menu, X, Link2, Shield, Activity, HelpCircle, Share2 } from 'lucide-react';
import { NotificationBell } from './NotificationBell';
import clsx from 'clsx';
import { useAuth, useAuthFetch } from '../context/AuthContext';
import { useSettings } from '../context/SettingsContext';
import { useTenant } from '../context/TenantContext';
import { useTheme } from '../context/ThemeContext';
import { useExtensions } from '../context/ExtensionContext';
import { useKeyboardShortcutsContext } from '../context/KeyboardShortcutsContext';
import { useKeyboardShortcuts, Shortcut } from '../hooks/useKeyboardShortcuts';
import { ShortcutActionId } from '../hooks/shortcutPresets';
import { Sun, Moon } from 'lucide-react';
import { Logo } from './Logo';
import { ExtensionPanel } from './ExtensionPanel';
import { Footer } from './Footer';
import { Avatar } from './Avatar';

interface SearchResult {
  id: string;
  name: string;
  description?: string;
  result_type: 'company' | 'user' | 'file';
  link: string;
}

interface SearchResults {
  companies: SearchResult[];
  users: SearchResult[];
  files: SearchResult[];
  total: number;
}

// Permission-based navigation - each item specifies which permission is required
// null permission means always visible (with authentication)
// 'superadmin_only' is a special flag for SuperAdmin-exclusive features
interface NavItem {
    name: string;
    href: string;
    icon: typeof LayoutDashboard;
    permission: string | null;
    superAdminOnly?: boolean;
}

const NAVIGATION: NavItem[] = [
    { name: 'Dashboard', href: '/', icon: LayoutDashboard, permission: null }, // Always visible
    { name: 'Companies', href: '/companies', icon: Building2, permission: 'tenants.manage' },
    { name: 'Users', href: '/users', icon: Users, permission: 'users.view' },
    { name: 'Files', href: '/files', icon: FileText, permission: 'files.view' },
    { name: 'Requests', href: '/file-requests', icon: Link2, permission: 'requests.view' },
    { name: 'Shared', href: '/shared-with-me', icon: Share2, permission: 'files.view' },
    { name: 'Security', href: '/security', icon: Shield, permission: 'audit.view' },
    { name: 'Performance', href: '/performance', icon: Activity, permission: null, superAdminOnly: true },
    { name: 'Settings', href: '/settings', icon: Settings, permission: 'settings.view' },
];

export function Layout() {
    const { user, logout, hasPermission } = useAuth();
    const { complianceMode } = useSettings();
    
    // Dynamic search placeholder based on role
    const getSearchPlaceholder = () => {
        switch (user?.role) {
            case 'SuperAdmin':
                return 'Search companies, users, or files...';
            case 'Admin':
            case 'Manager':
                return 'Search users or files...';
            default:
                return 'Search files...';
        }
    };
    const { currentCompany, companies, setCurrentCompany } = useTenant();
    const { theme, toggleTheme } = useTheme();
    const { uiComponents } = useExtensions();
    const { toggleHelp, isHelpOpen, getResolvedBinding } = useKeyboardShortcutsContext();
    const authFetch = useAuthFetch();
    const navigate = useNavigate();
    const location = useLocation();
    
    // Helper to get binding for an action from the current preset
    const getBinding = (actionId: ShortcutActionId) => {
        const binding = getResolvedBinding(actionId);
        return binding ? { keys: binding.keys, isSequence: binding.isSequence } : null;
    };
    const [isCompanyDropdownOpen, setIsCompanyDropdownOpen] = useState(false);
    const [activeExtensionPanel, setActiveExtensionPanel] = useState<string | null>(null);
    const [isMobileSidebarOpen, setIsMobileSidebarOpen] = useState(false);
    const [searchQuery, setSearchQuery] = useState('');
    const [searchResults, setSearchResults] = useState<SearchResults | null>(null);
    const [isSearching, setIsSearching] = useState(false);
    const [showSearchResults, setShowSearchResults] = useState(false);
    const [securityAlertCount, setSecurityAlertCount] = useState(0);
    const dropdownRef = useRef<HTMLDivElement>(null);
    const searchRef = useRef<HTMLDivElement>(null);
    const searchInputRef = useRef<HTMLInputElement>(null);
    const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    // Keyboard shortcuts - read bindings from current preset
    const shortcuts: Shortcut[] = useMemo(() => {
        const allShortcuts: Shortcut[] = [];
        
        // Navigation shortcuts
        const navDashboard = getBinding('nav.dashboard');
        if (navDashboard) {
            allShortcuts.push({
                id: 'nav.dashboard',
                keys: navDashboard.keys,
                description: 'Go to Dashboard',
                category: 'navigation',
                action: () => navigate('/'),
                isSequence: navDashboard.isSequence,
            });
        }
        
        const navFiles = getBinding('nav.files');
        if (navFiles) {
            allShortcuts.push({
                id: 'nav.files',
                keys: navFiles.keys,
                description: 'Go to Files',
                category: 'navigation',
                action: () => navigate('/files'),
                isSequence: navFiles.isSequence,
                enabled: hasPermission('files.view'),
            });
        }
        
        const navUsers = getBinding('nav.users');
        if (navUsers) {
            allShortcuts.push({
                id: 'nav.users',
                keys: navUsers.keys,
                description: 'Go to Users',
                category: 'navigation',
                action: () => navigate('/users'),
                isSequence: navUsers.isSequence,
                enabled: hasPermission('users.view'),
            });
        }
        
        const navCompanies = getBinding('nav.companies');
        if (navCompanies) {
            allShortcuts.push({
                id: 'nav.companies',
                keys: navCompanies.keys,
                description: 'Go to Companies',
                category: 'navigation',
                action: () => navigate('/companies'),
                isSequence: navCompanies.isSequence,
                enabled: hasPermission('tenants.manage'),
            });
        }
        
        const navSettings = getBinding('nav.settings');
        if (navSettings) {
            allShortcuts.push({
                id: 'nav.settings',
                keys: navSettings.keys,
                description: 'Go to Settings',
                category: 'navigation',
                action: () => navigate('/settings'),
                isSequence: navSettings.isSequence,
                enabled: hasPermission('settings.view'),
            });
        }
        
        const navProfile = getBinding('nav.profile');
        if (navProfile) {
            allShortcuts.push({
                id: 'nav.profile',
                keys: navProfile.keys,
                description: 'Go to Profile',
                category: 'navigation',
                action: () => navigate('/profile'),
                isSequence: navProfile.isSequence,
            });
        }
        
        const navNotifications = getBinding('nav.notifications');
        if (navNotifications) {
            allShortcuts.push({
                id: 'nav.notifications',
                keys: navNotifications.keys,
                description: 'Go to Notifications',
                category: 'navigation',
                action: () => navigate('/notifications'),
                isSequence: navNotifications.isSequence,
            });
        }
        
        // UI control shortcuts
        const uiSearch = getBinding('ui.search');
        if (uiSearch) {
            allShortcuts.push({
                id: 'ui.search',
                keys: uiSearch.keys,
                description: 'Focus search',
                category: 'ui',
                action: () => searchInputRef.current?.focus(),
                isSequence: uiSearch.isSequence,
            });
        }
        
        const uiTheme = getBinding('ui.theme');
        if (uiTheme) {
            allShortcuts.push({
                id: 'ui.theme',
                keys: uiTheme.keys,
                description: 'Toggle dark/light theme',
                category: 'ui',
                action: () => toggleTheme(),
                isSequence: uiTheme.isSequence,
            });
        }
        
        const uiHelp = getBinding('ui.help');
        if (uiHelp) {
            allShortcuts.push({
                id: 'ui.help',
                keys: uiHelp.keys,
                description: 'Show keyboard shortcuts',
                category: 'ui',
                action: () => toggleHelp(),
                isSequence: uiHelp.isSequence,
            });
        }
        
        const uiClose = getBinding('ui.close');
        if (uiClose) {
            allShortcuts.push({
                id: 'ui.close',
                keys: uiClose.keys,
                description: 'Close modal or dropdown',
                category: 'ui',
                action: () => {
                    // Close any open dropdowns/panels
                    if (isHelpOpen) return; // Let the modal handle its own escape
                    setIsCompanyDropdownOpen(false);
                    setShowSearchResults(false);
                    setActiveExtensionPanel(null);
                    setIsMobileSidebarOpen(false);
                },
                isSequence: uiClose.isSequence,
            });
        }
        
        return allShortcuts;
    }, [navigate, hasPermission, toggleTheme, toggleHelp, isHelpOpen, getBinding]);

    useKeyboardShortcuts(shortcuts);

    // Fetch security alert badge count
    useEffect(() => {
        const fetchSecurityBadge = async () => {
            if (!user || !hasPermission('audit.view')) return;
            try {
                const response = await authFetch('/api/security/alerts/badge');
                if (response.ok) {
                    const data = await response.json();
                    setSecurityAlertCount(data.count || 0);
                }
            } catch (error) {
                console.error('Failed to fetch security badge:', error);
            }
        };
        fetchSecurityBadge();
        // Refresh every 5 minutes
        const interval = setInterval(fetchSecurityBadge, 5 * 60 * 1000);
        return () => clearInterval(interval);
    }, [user, authFetch, hasPermission]);

    // Close dropdown when clicking outside
    useEffect(() => {
        function handleClickOutside(event: MouseEvent) {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsCompanyDropdownOpen(false);
            }
            if (searchRef.current && !searchRef.current.contains(event.target as Node)) {
                setShowSearchResults(false);
            }
        }
        document.addEventListener("mousedown", handleClickOutside);
        return () => {
            document.removeEventListener("mousedown", handleClickOutside);
        };
    }, []);

    // Debounced search
    const handleSearch = useCallback(async (query: string) => {
        if (query.length < 2) {
            setSearchResults(null);
            setShowSearchResults(false);
            return;
        }

        setIsSearching(true);
        try {
            const response = await authFetch(`/api/search?q=${encodeURIComponent(query)}&limit=5`);
            if (response.ok) {
                const data = await response.json();
                setSearchResults(data);
                setShowSearchResults(true);
            }
        } catch (error) {
            console.error('Search failed:', error);
        } finally {
            setIsSearching(false);
        }
    }, [authFetch]);

    const onSearchInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const value = e.target.value;
        setSearchQuery(value);

        if (searchTimeoutRef.current) {
            clearTimeout(searchTimeoutRef.current);
        }

        searchTimeoutRef.current = setTimeout(() => {
            handleSearch(value);
        }, 300);
    };

    const handleResultClick = (result: SearchResult) => {
        setShowSearchResults(false);
        setSearchQuery('');
        navigate(result.link);
    };

    const getResultIcon = (type: string) => {
        switch (type) {
            case 'company':
                return <Building2 className="w-4 h-4" />;
            case 'user':
                return <User className="w-4 h-4" />;
            case 'file':
                return <Folder className="w-4 h-4" />;
            default:
                return <FileText className="w-4 h-4" />;
        }
    };

    // Get user initials
    const getInitials = (name: string) => {
        return name
            .split(' ')
            .map(n => n[0])
            .join('')
            .toUpperCase()
            .slice(0, 2);
    };

    return (
        <div className="flex h-screen w-full bg-gray-50 dark:bg-gray-900 transition-colors duration-200">
            {/* Mobile Sidebar Overlay */}
            {isMobileSidebarOpen && (
                <div 
                    className="fixed inset-0 z-40 bg-black/50 md:hidden"
                    onClick={() => setIsMobileSidebarOpen(false)}
                />
            )}

            {/* Sidebar */}
            <aside className={clsx(
                "fixed md:relative z-50 w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col transition-all duration-300 h-full",
                isMobileSidebarOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"
            )}>
                <div className="h-auto py-4 flex items-center justify-between px-6 border-b border-gray-200 dark:border-gray-700">
                    <div className="flex items-center space-x-2 flex-1 justify-center">
                        <Logo className="h-40 text-black dark:text-white" />
                    </div>
                    {/* Mobile close button */}
                    <button
                        onClick={() => setIsMobileSidebarOpen(false)}
                        className="md:hidden p-2 rounded-md text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 absolute top-4 right-4"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>
                {/* Navigation - filtered by user role */}
                <nav className="flex-1 px-3 py-4 space-y-1 overflow-y-auto">
                    {NAVIGATION
                        .filter((item) => {
                            // SuperAdmin-only items
                            if (item.superAdminOnly) {
                                return user?.role === 'SuperAdmin';
                            }
                            // No permission required (like Dashboard)
                            if (!item.permission) {
                                return true;
                            }
                            // Check if user has the required permission
                            return hasPermission(item.permission);
                        })
                        .map((item) => (
                            <NavLink
                                key={item.name}
                                to={item.href}
                                onClick={() => setIsMobileSidebarOpen(false)}
                                className={({ isActive }) =>
                                    clsx(
                                        "flex items-center px-3 py-3 text-sm font-medium rounded-lg transition-colors min-h-[44px]",
                                        isActive
                                            ? "bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-400"
                                            : "text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white"
                                    )
                                }
                            >
                                <item.icon className={clsx("mr-3 h-5 w-5 flex-shrink-0")} />
                                <span className="flex-1">{item.name}</span>
                                {item.name === 'Security' && securityAlertCount > 0 && (
                                    <span className="ml-2 px-2 py-0.5 text-xs font-bold bg-red-500 text-white rounded-full">
                                        {securityAlertCount > 99 ? '99+' : securityAlertCount}
                                    </span>
                                )}
                            </NavLink>
                        ))}

{/* Extension Sidebar Items */}
                    {uiComponents.sidebar.length > 0 && (
                        <>
                            <div className="my-3 border-t border-gray-200 dark:border-gray-700" />
                            <p className="px-3 text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2">
                                Extensions
                            </p>
                            {uiComponents.sidebar.map((item) => (
                                <button
                                    key={item.id}
                                    onClick={() => { setActiveExtensionPanel(item.id); setIsMobileSidebarOpen(false); }}
                                    className={clsx(
                                        "w-full flex items-center px-3 py-3 text-sm font-medium rounded-lg transition-colors min-h-[44px]",
                                        activeExtensionPanel === item.id
                                            ? "bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-400"
                                            : "text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white"
                                    )}
                                >
                                    {item.icon ? (
                                        <img src={item.icon} alt="" className="mr-3 h-5 w-5 flex-shrink-0" />
                                    ) : (
                                        <Puzzle className="mr-3 h-5 w-5 flex-shrink-0" />
                                    )}
                                    {item.name}
                                </button>
                            ))}
                        </>
                    )}
                </nav>

                {/* Keyboard Shortcuts Help Button */}
                <div className="px-3 pb-2">
                    <button
                        onClick={toggleHelp}
                        className="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm font-medium rounded-lg text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200 transition-colors border border-gray-200 dark:border-gray-600"
                        title="Show keyboard shortcuts (press ? anytime)"
                    >
                        <HelpCircle className="h-4 w-4" />
                        <span>Shortcuts</span>
                    </button>
                </div>

                <div className="p-3 border-t border-gray-200 dark:border-gray-700">
                    <div className="flex items-center p-3 rounded-lg bg-gray-50 dark:bg-gray-700/50 border border-gray-200 dark:border-gray-700">
                        <Avatar 
                            src={user?.avatar_url} 
                            name={user?.name || 'User'} 
                            size="md"
                        />
                        <div className="ml-3 min-w-0 flex-1">
                            <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{user?.name || 'User'}</p>
                            <NavLink 
                                to="/profile" 
                                onClick={() => setIsMobileSidebarOpen(false)}
                                className="text-xs text-gray-500 dark:text-gray-400 truncate hover:text-primary-600 dark:hover:text-primary-400 block"
                            >
                                {user?.role || 'Role'}
                            </NavLink>
                        </div>
                        <button
                            onClick={logout}
                            className="p-2 rounded-lg text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-600 transition-colors"
                            title="Logout"
                        >
                            <LogOut className="h-5 w-5" />
                        </button>
                    </div>
                </div>
            </aside>

            {/* Main Content */}
            <main className="flex-1 flex flex-col overflow-hidden">
                <header className="h-16 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4 md:px-6 transition-colors duration-200">
                    {/* Mobile menu button */}
                    <button
                        onClick={() => setIsMobileSidebarOpen(true)}
                        className="md:hidden p-2.5 rounded-lg text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 min-w-[44px] min-h-[44px] flex items-center justify-center"
                    >
                        <Menu className="w-6 h-6" />
                    </button>

                    <div className="flex-1 max-w-lg hidden md:block" ref={searchRef}>
                        <div className="relative">
                            <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                                <Search className="h-5 w-5 text-gray-400 dark:text-gray-500" />
                            </div>
                            <input
                                ref={searchInputRef}
                                type="text"
                                value={searchQuery}
                                onChange={onSearchInputChange}
                                onFocus={() => searchResults && searchResults.total > 0 && setShowSearchResults(true)}
                                className="block w-full pl-10 pr-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md leading-5 bg-gray-50 dark:bg-gray-700 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white focus:outline-none focus:placeholder-gray-400 focus:ring-1 focus:ring-primary-500 focus:border-primary-500 sm:text-sm transition-colors"
                                placeholder={getSearchPlaceholder()}
                            />
                            
                            {/* Search Results Dropdown */}
                            {showSearchResults && searchResults && (
                                <div className="absolute top-full left-0 right-0 mt-2 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 z-50 max-h-96 overflow-y-auto">
                                    {isSearching ? (
                                        <div className="p-4 text-center text-gray-500">
                                            <div className="animate-spin h-5 w-5 border-2 border-primary-500 border-t-transparent rounded-full mx-auto"></div>
                                        </div>
                                    ) : searchResults.total === 0 ? (
                                        <div className="p-4 text-center text-gray-500 dark:text-gray-400">
                                            No results found
                                        </div>
                                    ) : (
                                        <div className="py-2">
                                            {searchResults.companies.length > 0 && (
                                                <div>
                                                    <p className="px-4 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase">Companies</p>
                                                    {searchResults.companies.map((result) => (
                                                        <button
                                                            key={result.id}
                                                            onClick={() => handleResultClick(result)}
                                                            className="w-full flex items-center gap-3 px-4 py-2 hover:bg-gray-100 dark:hover:bg-gray-700 text-left"
                                                        >
                                                            <Building2 className="w-4 h-4 text-gray-400" />
                                                            <div className="min-w-0 flex-1">
                                                                <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{result.name}</p>
                                                                {result.description && (
                                                                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{result.description}</p>
                                                                )}
                                                            </div>
                                                        </button>
                                                    ))}
                                                </div>
                                            )}
                                            {searchResults.users.length > 0 && (
                                                <div>
                                                    <p className="px-4 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase">Users</p>
                                                    {searchResults.users.map((result) => (
                                                        <button
                                                            key={result.id}
                                                            onClick={() => handleResultClick(result)}
                                                            className="w-full flex items-center gap-3 px-4 py-2 hover:bg-gray-100 dark:hover:bg-gray-700 text-left"
                                                        >
                                                            <User className="w-4 h-4 text-gray-400" />
                                                            <div className="min-w-0 flex-1">
                                                                <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{result.name}</p>
                                                                {result.description && (
                                                                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{result.description}</p>
                                                                )}
                                                            </div>
                                                        </button>
                                                    ))}
                                                </div>
                                            )}
                                            {searchResults.files.length > 0 && (
                                                <div>
                                                    <p className="px-4 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase">Files</p>
                                                    {searchResults.files.map((result) => (
                                                        <button
                                                            key={result.id}
                                                            onClick={() => handleResultClick(result)}
                                                            className="w-full flex items-center gap-3 px-4 py-2 hover:bg-gray-100 dark:hover:bg-gray-700 text-left"
                                                        >
                                                            <Folder className="w-4 h-4 text-gray-400" />
                                                            <div className="min-w-0 flex-1">
                                                                <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{result.name}</p>
                                                                {result.description && (
                                                                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{result.description}</p>
                                                                )}
                                                            </div>
                                                        </button>
                                                    ))}
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>

                    <div className="flex items-center space-x-4">
                        <button
                            onClick={toggleTheme}
                            className="p-1 rounded-full text-gray-400 dark:text-gray-500 hover:text-gray-500 dark:hover:text-gray-300 transition-colors"
                        >
                            {theme === 'dark' ? <Sun className="h-6 w-6" /> : <Moon className="h-6 w-6" />}
                        </button>

                        <NotificationBell />
                        
                        <div className="h-6 w-px bg-gray-200 dark:bg-gray-700" />

                        <div className="relative" ref={dropdownRef}>
                            <button
                                onClick={() => setIsCompanyDropdownOpen(!isCompanyDropdownOpen)}
                                className="flex items-center cursor-pointer focus:outline-none"
                            >
                                <span className="text-sm font-medium text-gray-700 dark:text-gray-200 mr-2">
                                    {currentCompany?.name || 'Select Company'}
                                </span>
                                <ChevronDown className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                            </button>

                            {/* Dropdown Menu */}
                            {isCompanyDropdownOpen && (
                                <div className="absolute right-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-md shadow-lg py-1 ring-1 ring-black ring-opacity-5 z-50">
                                    {companies.filter(c => c.status === 'active').map((company) => (
                                        <button
                                            key={company.id}
                                            onClick={() => {
                                                setCurrentCompany(company);
                                                setIsCompanyDropdownOpen(false);
                                            }}
                                            className={clsx(
                                                "block w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700",
                                                currentCompany.id === company.id && "bg-gray-50 dark:bg-gray-700 font-medium"
                                            )}
                                        >
                                            {company.name}
                                        </button>
                                    ))}
                                    {/* Only show Add Company for users with tenants.manage permission */}
                                    {hasPermission('tenants.manage') && (
                                        <>
                                            <div className="border-t border-gray-100 dark:border-gray-700 my-1"></div>
                                            <button 
                                                onClick={() => {
                                                    navigate('/companies');
                                                    setIsCompanyDropdownOpen(false);
                                                }}
                                                className="block w-full text-left px-4 py-2 text-sm text-primary-600 dark:text-primary-400 hover:bg-gray-100 dark:hover:bg-gray-700"
                                            >
                                                + Add Company
                                            </button>
                                        </>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>
                </header>

                <div className="flex-1 overflow-auto bg-gray-50 dark:bg-gray-900 transition-colors duration-200 flex flex-col">
                    <div className="flex-1 p-8">
                        <Outlet />
                    </div>
                    <Footer />
                </div>
                
                {/* Extension Panel Overlay */}
                {activeExtensionPanel && (
                    <ExtensionPanel
                        item={uiComponents.sidebar.find(s => s.id === activeExtensionPanel)!}
                        onClose={() => setActiveExtensionPanel(null)}
                    />
                )}
            </main>
        </div>
    );
}
