import { createContext, useContext, useState, ReactNode, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';

interface User {
    id: string;
    email: string;
    name: string;
    role: string;
    avatar_url?: string;
    department_id?: string;
    allowed_department_ids?: string[];
    dashboard_layout?: any;
    widget_config?: {
        visible_widgets: string[];
        widget_settings: Record<string, any>;
        custom_widgets: string[];
    };
    permissions?: string[];
}

interface Tenant {
    id: string;
    name: string;
    domain: string;
    plan: string;
    compliance_mode: string;
    retention_policy_days?: number;
    data_export_enabled?: boolean;
}

interface AuthContextType {
    user: User | null;
    tenant: Tenant | null;
    token: string | null;
    isAuthenticated: boolean;
    isLoading: boolean;
    login: (email: string, password: string, code?: string, rememberMe?: boolean) => Promise<any>;
    register: (email: string, name: string, password: string, role: string) => Promise<void>;
    logout: () => void;
    switchTenant: (tenantId: string) => Promise<void>;
    refreshUser: () => Promise<void>;
    hasPermission: (permission: string) => boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

const API_URL = import.meta.env.VITE_API_URL || '';

export function AuthProvider({ children }: { children: ReactNode }) {
    const [user, setUser] = useState<User | null>(null);
    const [tenant, setTenant] = useState<Tenant | null>(null);
    const [token, setToken] = useState<string | null>(() => {
        // Check localStorage first (remember me), then sessionStorage
        return localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token');
    });
    const [isLoading, setIsLoading] = useState(true);
    const navigate = useNavigate();

    // Axios-like fetch wrapper with auth
    const authFetch = async (url: string, options: RequestInit = {}) => {
        const isFormData = options.body instanceof FormData;
        const headers = {
            // Don't set Content-Type for FormData - browser sets it with boundary
            ...(!isFormData && { 'Content-Type': 'application/json' }),
            ...(token && { Authorization: `Bearer ${token}` }),
            ...options.headers,
        };

        const response = await fetch(`${API_URL}${url}`, {
            ...options,
            headers,
        });

        if (response.status === 401) {
            // Token expired or invalid
            logout();
            throw new Error('Unauthorized');
        }

        return response;
    };

    const login = async (email: string, password: string, code?: string, rememberMe: boolean = false) => {
        try {
            const response = await fetch(`${API_URL}/api/auth/login`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ email, password, code }),
            });

            if (!response.ok) {
                throw new Error('Login failed');
            }

            const data = await response.json();

            // Check for account or company suspension - return the error response to be handled by caller
            if (data.error === 'account_suspended' || data.error === 'company_suspended') {
                return data;
            }

            if (data.require_2fa) {
                return data;
            }

            // Check if user was switched to a fallback tenant due to primary being suspended
            if (data.primary_tenant_suspended && data.suspended_tenant_name) {
                // Store notification for display after login
                sessionStorage.setItem('tenant_switch_notice', JSON.stringify({
                    suspended_tenant: data.suspended_tenant_name,
                    current_tenant: data.tenant.name
                }));
            }

            setToken(data.token);
            setUser(data.user);
            setTenant(data.tenant);
            
            // Remember me: use localStorage (persists) vs sessionStorage (cleared on tab close)
            if (rememberMe) {
                localStorage.setItem('auth_token', data.token);
                localStorage.setItem('remember_me', 'true');
            } else {
                sessionStorage.setItem('auth_token', data.token);
                localStorage.removeItem('auth_token');
                localStorage.removeItem('remember_me');
            }

            navigate('/');
            return data;
        } catch (error) {
            console.error('Login error:', error);
            throw error;
        }
    };

    const register = async (email: string, name: string, password: string, role: string) => {
        try {
            const response = await fetch(`${API_URL}/api/auth/register`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ email, name, password, role }),
            });

            if (!response.ok) {
                throw new Error('Registration failed');
            }

            const data = await response.json();

            setToken(data.token);
            setUser(data.user);
            setTenant(data.tenant);
            localStorage.setItem('auth_token', data.token);

            navigate('/');
        } catch (error) {
            console.error('Registration error:', error);
            throw error;
        }
    };

    const logout = useCallback(() => {
        setUser(null);
        setTenant(null);
        setToken(null);
        // Clear both storage types
        localStorage.removeItem('auth_token');
        localStorage.removeItem('remember_me');
        sessionStorage.removeItem('auth_token');
        navigate('/login');
    }, [navigate]);

    const switchTenant = async (tenantId: string) => {
        try {
            const response = await authFetch(`/api/tenants/switch/${tenantId}`, {
                method: 'POST',
            });

            if (!response.ok) {
                throw new Error('Tenant switch failed');
            }

            const data = await response.json();

            setToken(data.token);
            setTenant(data.tenant);
            
            // Preserve the storage preference from original login
            if (localStorage.getItem('remember_me') === 'true') {
                localStorage.setItem('auth_token', data.token);
            } else {
                sessionStorage.setItem('auth_token', data.token);
            }

            // Refresh user data with new token
            await refreshUser(data.token);
        } catch (error) {
            console.error('Tenant switch error:', error);
            throw error;
        }
    };

    const refreshUser = async (newToken?: string) => {
        const tokenToUse = newToken || token;
        if (!tokenToUse) {
            setIsLoading(false);
            return;
        }

        try {
            // Manually construct fetch to use the correct token if provided
            const headers: HeadersInit = {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${tokenToUse}`
            };

            const response = await fetch(`${API_URL}/api/auth/me`, {
                headers
            });

            if (!response.ok) {
                throw new Error('Failed to fetch user');
            }

            const data = await response.json();
            setUser(data.user);
            setTenant(data.tenant);
        } catch (error) {
            console.error('Refresh user error:', error);
            logout();
        } finally {
            setIsLoading(false);
        }
    };

    // Auto-authenticate on mount if token exists
    useEffect(() => {
        const initAuth = async () => {
            // Check localStorage first (remember me), then sessionStorage
            const storedToken = localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token');
            if (storedToken) {
                try {
                    const response = await fetch(`${API_URL}/api/auth/me`, {
                        headers: {
                            'Authorization': `Bearer ${storedToken}`,
                        },
                    });

                    if (response.ok) {
                        const data = await response.json();
                        setUser(data.user);
                        setTenant(data.tenant);
                        setToken(storedToken);
                    } else {
                        // Token is invalid - clear both storage types silently
                        localStorage.removeItem('auth_token');
                        localStorage.removeItem('remember_me');
                        sessionStorage.removeItem('auth_token');
                        setToken(null);
                        setUser(null);
                        setTenant(null);
                    }
                } catch (error) {
                    // Network error or invalid token - clear silently
                    console.error('Auth initialization error:', error);
                    localStorage.removeItem('auth_token');
                    localStorage.removeItem('remember_me');
                    sessionStorage.removeItem('auth_token');
                    setToken(null);
                    setUser(null);
                    setTenant(null);
                }
            }
            setIsLoading(false);
        };

        initAuth();
    }, []);

    // Check if the current user has a specific permission
    const hasPermission = useCallback((permission: string): boolean => {
        if (!user) return false;
        // SuperAdmin always has all permissions
        if (user.role === 'SuperAdmin') return true;
        // Check the permissions array
        return user.permissions?.includes(permission) ?? false;
    }, [user]);

    const value = {
        user,
        tenant,
        token,
        isAuthenticated: !!user,
        isLoading,
        login,
        register,
        logout,
        switchTenant,
        refreshUser,
        hasPermission,
    };

    return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
    const context = useContext(AuthContext);
    if (context === undefined) {
        throw new Error('useAuth must be used within an AuthProvider');
    }
    return context;
}

// Export authFetch for use in API calls
export function useAuthFetch() {
    const { token, logout } = useAuth();

    return useCallback(async (url: string, options: RequestInit = {}) => {
        const isFormData = options.body instanceof FormData;
        const headers: Record<string, string> = {
            ...(token && { Authorization: `Bearer ${token}` }),
        };

        // Don't set Content-Type for FormData - browser sets it with boundary
        if (!isFormData) {
            const optionsHeaders = options.headers as Record<string, any> || {};
            if (!('Content-Type' in optionsHeaders) || optionsHeaders['Content-Type'] !== undefined) {
                headers['Content-Type'] = optionsHeaders['Content-Type'] || 'application/json';
            }
        }

        // Add other custom headers (exclude Content-Type as we handled it above)
        const optionsHeaders = options.headers as Record<string, any> || {};
        Object.keys(optionsHeaders).forEach(key => {
            if (key !== 'Content-Type' && optionsHeaders[key] !== undefined) {
                headers[key] = optionsHeaders[key];
            }
        });

        const response = await fetch(`${API_URL}${url}`, {
            ...options,
            headers,
        });

        if (response.status === 401) {
            logout();
            throw new Error('Unauthorized');
        }

        return response;
    }, [token, logout]);
}
