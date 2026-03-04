import { useState, useEffect, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useAuth } from '../context/AuthContext';
import { Lock, Mail, AlertCircle, ShieldAlert, LogIn } from 'lucide-react';

const API_URL = import.meta.env.VITE_API_URL || '';

interface SsoProvider {
    id: string;
    name: string;
    slug: string;
    provider_type: string;
    protocol: 'oidc' | 'saml';
}

// Provider icon/color mapping
function getProviderStyle(providerType: string) {
    switch (providerType) {
        case 'google':
            return { bg: 'bg-white border border-gray-300 hover:bg-gray-50', text: 'text-gray-700' };
        case 'microsoft':
            return { bg: 'bg-[#2F2F2F] hover:bg-[#404040]', text: 'text-white' };
        case 'okta':
            return { bg: 'bg-[#007DC1] hover:bg-[#006BA1]', text: 'text-white' };
        default:
            return { bg: 'bg-gray-700 hover:bg-gray-800', text: 'text-white' };
    }
}

export function Login() {
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [code, setCode] = useState('');
    const [show2FA, setShow2FA] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [isSuspended, setIsSuspended] = useState(false);
    const [rememberMe, setRememberMe] = useState(false);
    const [ssoProviders, setSsoProviders] = useState<SsoProvider[]>([]);
    const [ssoOnly, setSsoOnly] = useState(false);
    const [ssoLoading, setSsoLoading] = useState(false);
    const { login } = useAuth();
    const [searchParams] = useSearchParams();

    // Handle error from SSO redirect
    useEffect(() => {
        const errorParam = searchParams.get('error');
        if (errorParam === 'no_account') {
            setError('No account found for this SSO identity. Contact your administrator.');
        } else if (errorParam === 'no_email') {
            setError('SSO provider did not return an email address.');
        } else if (errorParam === 'suspended') {
            setIsSuspended(true);
            setError('Your account is suspended. Contact your administrator.');
        } else if (errorParam === 'oidc_error' || errorParam === 'saml_error') {
            setError(searchParams.get('message') || 'SSO authentication failed.');
        }
    }, [searchParams]);

    // Discover SSO providers when email changes
    const discoverProviders = useCallback(async (emailValue: string) => {
        if (!emailValue || !emailValue.includes('@')) {
            setSsoProviders([]);
            setSsoOnly(false);
            return;
        }

        setSsoLoading(true);
        try {
            const response = await fetch(`${API_URL}/api/auth/oidc/providers?email=${encodeURIComponent(emailValue)}`);
            if (response.ok) {
                const data = await response.json();
                setSsoProviders(data.providers || []);
                setSsoOnly(data.sso_only || false);
            }
        } catch {
            // Silently ignore — SSO discovery is optional
        } finally {
            setSsoLoading(false);
        }
    }, []);

    const handleEmailBlur = () => {
        discoverProviders(email);
    };

    const handleSsoLogin = (provider: SsoProvider) => {
        const protocol = provider.protocol || 'oidc';
        window.location.href = `${API_URL}/api/auth/${protocol}/authorize/${provider.id}`;
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        setError('');
        setIsSuspended(false);

        try {
            const result = await login(email, password, code, rememberMe);

            // Check for suspended account or company
            if (result && (result.error === 'account_suspended' || result.error === 'company_suspended')) {
                setIsSuspended(true);
                setError(result.message || 'Access denied. Please contact your administrator.');
                setLoading(false);
                return;
            }

            // SSO required — show SSO buttons
            if (result && result.error === 'sso_required') {
                if (result.providers && result.providers.length > 0) {
                    setSsoProviders(result.providers);
                    setSsoOnly(true);
                }
                setError(result.message || 'This account uses SSO. Please sign in with your identity provider.');
                setLoading(false);
                return;
            }

            if (result && result.require_2fa) {
                setShow2FA(true);
                setLoading(false);
                return;
            }
            // Navigation handled by AuthContext
        } catch (err: any) {
            console.error('Login error:', err);
            setError('Invalid email, password, or 2FA code. Please try again.');
            setLoading(false);
        }
    };

    return (
        <div className="min-h-screen bg-white flex flex-col justify-center py-12 sm:px-6 lg:px-8">
            <div className="sm:mx-auto sm:w-full sm:max-w-md">
                {/* Logo */}
                <div className="flex justify-center">
                    <img src="/logo.svg" alt="ClovaLink" className="h-48 w-auto" />
                </div>
                <h2 className="mt-6 text-center text-3xl font-extrabold text-gray-900">
                    Sign in to your account
                </h2>
                <p className="mt-2 text-center text-sm text-gray-600">
                    Secure file sharing and collaboration platform
                </p>
            </div>

            <div className="mt-8 sm:mx-auto sm:w-full sm:max-w-md">
                <div className="bg-white py-8 px-4 shadow-xl sm:rounded-lg sm:px-10 border border-gray-100">
                    <form className="space-y-6" onSubmit={handleSubmit}>
                        {/* Error Message */}
                        {error && (
                            <div className={`rounded-md p-4 ${isSuspended ? 'bg-orange-50 border border-orange-200' : 'bg-red-50 border border-red-200'}`}>
                                <div className="flex">
                                    <div className="flex-shrink-0">
                                        {isSuspended ? (
                                            <ShieldAlert className="h-5 w-5 text-orange-500" />
                                        ) : (
                                            <AlertCircle className="h-5 w-5 text-red-400" />
                                        )}
                                    </div>
                                    <div className="ml-3">
                                        <h3 className={`text-sm font-medium ${isSuspended ? 'text-orange-800' : 'text-red-800'}`}>
                                            {isSuspended ? 'Account Suspended' : 'Login Failed'}
                                        </h3>
                                        <p className={`text-sm mt-1 ${isSuspended ? 'text-orange-700' : 'text-red-700'}`}>
                                            {error}
                                        </p>
                                    </div>
                                </div>
                            </div>
                        )}

                        {!show2FA ? (
                            <>
                                {/* Email Field */}
                                <div>
                                    <label htmlFor="email" className="block text-sm font-medium text-gray-700">
                                        Email address
                                    </label>
                                    <div className="mt-1 relative">
                                        <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                                            <Mail className="h-5 w-5 text-gray-400" />
                                        </div>
                                        <input
                                            id="email"
                                            name="email"
                                            type="email"
                                            autoComplete="email"
                                            required
                                            value={email}
                                            onChange={(e) => setEmail(e.target.value)}
                                            onBlur={handleEmailBlur}
                                            className="appearance-none block w-full pl-10 pr-3 py-2 border border-gray-300 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-primary-500 focus:border-primary-500 sm:text-sm text-gray-900 bg-white"
                                            placeholder="you@company.com"
                                        />
                                    </div>
                                </div>

                                {/* SSO Buttons */}
                                {ssoProviders.length > 0 && (
                                    <div className="space-y-3">
                                        {!ssoOnly && (
                                            <div className="relative">
                                                <div className="absolute inset-0 flex items-center">
                                                    <div className="w-full border-t border-gray-300" />
                                                </div>
                                                <div className="relative flex justify-center text-sm">
                                                    <span className="px-2 bg-white text-gray-500">Sign in with SSO</span>
                                                </div>
                                            </div>
                                        )}
                                        {ssoProviders.map((provider) => {
                                            const style = getProviderStyle(provider.provider_type);
                                            return (
                                                <button
                                                    key={provider.id}
                                                    type="button"
                                                    onClick={() => handleSsoLogin(provider)}
                                                    className={`w-full flex items-center justify-center gap-2 py-2.5 px-4 rounded-md shadow-sm text-sm font-medium transition-colors ${style.bg} ${style.text}`}
                                                >
                                                    <LogIn className="h-4 w-4" />
                                                    Sign in with {provider.name}
                                                </button>
                                            );
                                        })}
                                        {ssoOnly && (
                                            <p className="text-xs text-center text-gray-500">
                                                Your organization requires SSO login.
                                            </p>
                                        )}
                                    </div>
                                )}

                                {/* Password Field (hidden when SSO-only) */}
                                {!ssoOnly && (
                                    <>
                                        {ssoProviders.length > 0 && (
                                            <div className="relative">
                                                <div className="absolute inset-0 flex items-center">
                                                    <div className="w-full border-t border-gray-300" />
                                                </div>
                                                <div className="relative flex justify-center text-sm">
                                                    <span className="px-2 bg-white text-gray-500">Or with password</span>
                                                </div>
                                            </div>
                                        )}
                                        <div>
                                            <label htmlFor="password" className="block text-sm font-medium text-gray-700">
                                                Password
                                            </label>
                                            <div className="mt-1 relative">
                                                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                                                    <Lock className="h-5 w-5 text-gray-400" />
                                                </div>
                                                <input
                                                    id="password"
                                                    name="password"
                                                    type="password"
                                                    autoComplete="current-password"
                                                    required={!ssoOnly}
                                                    value={password}
                                                    onChange={(e) => setPassword(e.target.value)}
                                                    className="appearance-none block w-full pl-10 pr-3 py-2 border border-gray-300 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-primary-500 focus:border-primary-500 sm:text-sm text-gray-900 bg-white"
                                                    placeholder="••••••••"
                                                />
                                            </div>
                                        </div>

                                        {/* Remember Me & Forgot Password */}
                                        <div className="flex items-center justify-between">
                                            <div className="flex items-center">
                                                <input
                                                    id="remember-me"
                                                    name="remember-me"
                                                    type="checkbox"
                                                    checked={rememberMe}
                                                    onChange={(e) => setRememberMe(e.target.checked)}
                                                    className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                                                />
                                                <label htmlFor="remember-me" className="ml-2 block text-sm text-gray-900">
                                                    Remember me
                                                </label>
                                            </div>

                                            <div className="text-sm">
                                                <button
                                                    type="button"
                                                    onClick={() => setError('Please contact your administrator to reset your password.')}
                                                    className="font-medium text-primary-600 hover:text-primary-500"
                                                >
                                                    Forgot password?
                                                </button>
                                            </div>
                                        </div>
                                    </>
                                )}
                            </>
                        ) : (
                            /* 2FA Field */
                            <div>
                                <label htmlFor="code" className="block text-sm font-medium text-gray-700">
                                    Authentication Code
                                </label>
                                <div className="mt-1 relative">
                                    <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                                        <Lock className="h-5 w-5 text-gray-400" />
                                    </div>
                                        <input
                                            id="code"
                                            name="code"
                                            type="text"
                                            required
                                            value={code}
                                            onChange={(e) => setCode(e.target.value)}
                                            className="appearance-none block w-full pl-10 pr-3 py-2 border border-gray-300 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-primary-500 focus:border-primary-500 sm:text-sm text-gray-900 bg-white"
                                            placeholder="123456"
                                        />
                                </div>
                                <p className="mt-2 text-sm text-gray-500">
                                    Enter the 6-digit code from your authenticator app.
                                </p>
                            </div>
                        )}

                        {/* Submit Button (hidden when SSO-only and no 2FA) */}
                        {(!ssoOnly || show2FA) && (
                            <div>
                                <button
                                    type="submit"
                                    disabled={loading}
                                    className="w-full flex justify-center py-2.5 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-primary-600 hover:bg-primary-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-primary-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                                >
                                    {loading ? (
                                        <div className="flex items-center">
                                            <svg className="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                                                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                            </svg>
                                            {show2FA ? 'Verifying...' : 'Signing in...'}
                                        </div>
                                    ) : (
                                        show2FA ? 'Verify Code' : 'Sign in'
                                    )}
                                </button>
                            </div>
                        )}
                    </form>
                </div>

                {/* Footer */}
                <p className="mt-6 text-center text-xs text-gray-500">
                    &copy; {new Date().getFullYear()} ClovaLink. All rights reserved.
                </p>
            </div>
        </div>
    );
}
