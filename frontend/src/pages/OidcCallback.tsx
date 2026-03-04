import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useAuth } from '../context/AuthContext';

const API_URL = import.meta.env.VITE_API_URL || '';

export function OidcCallback() {
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const { refreshUser } = useAuth();
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        const token = searchParams.get('token');
        const errorParam = searchParams.get('error');

        if (errorParam) {
            setError(errorParam === 'no_account'
                ? 'No account found. Contact your administrator.'
                : errorParam === 'oidc_error'
                    ? searchParams.get('message') || 'SSO authentication failed.'
                    : 'Authentication failed. Please try again.');
            return;
        }

        if (!token) {
            setError('No authentication token received.');
            return;
        }

        // Store token and load user data
        const handleToken = async () => {
            try {
                // Default to sessionStorage (can be changed later via remember-me)
                sessionStorage.setItem('auth_token', token);

                // Verify the token by calling /api/auth/me
                const response = await fetch(`${API_URL}/api/auth/me`, {
                    headers: { Authorization: `Bearer ${token}` },
                });

                if (!response.ok) {
                    throw new Error('Token validation failed');
                }

                // Token is valid — trigger a full auth refresh
                await refreshUser(token);
                navigate('/', { replace: true });
            } catch {
                sessionStorage.removeItem('auth_token');
                setError('Authentication failed. Please try again.');
            }
        };

        handleToken();
    }, [searchParams, navigate, refreshUser]);

    if (error) {
        return (
            <div className="min-h-screen bg-white flex flex-col justify-center py-12 sm:px-6 lg:px-8">
                <div className="sm:mx-auto sm:w-full sm:max-w-md">
                    <div className="flex justify-center">
                        <img src="/logo.svg" alt="ClovaLink" className="h-48 w-auto" />
                    </div>
                    <div className="mt-8 bg-white py-8 px-4 shadow-xl sm:rounded-lg sm:px-10 border border-gray-100">
                        <div className="rounded-md bg-red-50 border border-red-200 p-4">
                            <p className="text-sm font-medium text-red-800">SSO Login Failed</p>
                            <p className="text-sm text-red-700 mt-1">{error}</p>
                        </div>
                        <button
                            onClick={() => navigate('/login', { replace: true })}
                            className="mt-4 w-full flex justify-center py-2.5 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-primary-600 hover:bg-primary-700 transition-colors"
                        >
                            Back to Login
                        </button>
                    </div>
                </div>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-white flex flex-col items-center justify-center">
            <div className="flex flex-col items-center gap-3">
                <div className="w-8 h-8 border-4 border-primary-200 border-t-primary-600 rounded-full animate-spin" />
                <span className="text-sm text-gray-500">Completing sign in...</span>
            </div>
        </div>
    );
}
