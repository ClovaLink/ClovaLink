import { useState, useEffect, useRef } from 'react';
import { useAuth, useAuthFetch } from '../context/AuthContext';
import { useTenant } from '../context/TenantContext';
import { 
    User, 
    Shield, 
    Download, 
    Smartphone, 
    CheckCircle, 
    AlertCircle, 
    Camera, 
    Lock, 
    Mail, 
    Edit2, 
    Save, 
    X,
    Monitor,
    Clock,
    Trash2,
    Globe,
    Bell
} from 'lucide-react';
import clsx from 'clsx';
import { NotificationPreferences } from '../components/NotificationPreferences';
import { ImageCropModal } from '../components/ImageCropModal';
import { PasswordInput, usePasswordPolicy, validatePassword } from '../components/PasswordInput';
import { DiscordConnection } from '../components/DiscordConnection';

interface Session {
    id: string;
    device_info: string | null;
    ip_address: string | null;
    last_active_at: string;
    created_at: string;
}

export function Profile() {
    const { user, refreshUser } = useAuth();
    const authFetch = useAuthFetch();
    const { currentCompany } = useTenant();
    const fileInputRef = useRef<HTMLInputElement>(null);

    // Profile editing state
    const [isEditingProfile, setIsEditingProfile] = useState(false);
    const [editName, setEditName] = useState(user?.name || '');
    const [editEmail, setEditEmail] = useState(user?.email || '');
    const [isSavingProfile, setIsSavingProfile] = useState(false);
    const [profileError, setProfileError] = useState('');
    const [profileSuccess, setProfileSuccess] = useState('');
    const [email2FACode, setEmail2FACode] = useState('');
    const [email2FARequired, setEmail2FARequired] = useState(false);

    // Avatar state
    const [isUploadingAvatar, setIsUploadingAvatar] = useState(false);
    const [avatarError, setAvatarError] = useState('');
    const [showCropModal, setShowCropModal] = useState(false);
    const [selectedImage, setSelectedImage] = useState<string | null>(null);
    const [avatarCacheBuster, setAvatarCacheBuster] = useState(Date.now());
    const [avatarImgError, setAvatarImgError] = useState(false);

    // Password state
    const [showPasswordForm, setShowPasswordForm] = useState(false);
    const [currentPassword, setCurrentPassword] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [password2FACode, setPassword2FACode] = useState('');
    const [password2FARequired, setPassword2FARequired] = useState(false);
    const [isChangingPassword, setIsChangingPassword] = useState(false);
    const [passwordError, setPasswordError] = useState('');
    const [passwordSuccess, setPasswordSuccess] = useState('');
    const [passwordErrors, setPasswordErrors] = useState<string[]>([]);
    const { policy: passwordPolicy } = usePasswordPolicy();

    // Export state
    const [isExporting, setIsExporting] = useState(false);
    const [exportError, setExportError] = useState('');

    // 2FA state
    const [isSettingUp2FA, setIsSettingUp2FA] = useState(false);
    const [qrCode, setQrCode] = useState('');
    const [secret, setSecret] = useState('');
    const [verifyCode, setVerifyCode] = useState('');
    const [setupError, setSetupError] = useState('');
    const [setupSuccess, setSetupSuccess] = useState(false);

    // Sessions state
    const [sessions, setSessions] = useState<Session[]>([]);
    const [isLoadingSessions, setIsLoadingSessions] = useState(false);
    const [revokingSessionId, setRevokingSessionId] = useState<string | null>(null);

    useEffect(() => {
        if (user) {
            setEditName(user.name);
            setEditEmail(user.email);
            fetchSessions();
        }
    }, [user]);

    // Reset avatar error when avatar_url changes
    useEffect(() => {
        setAvatarImgError(false);
    }, [user?.avatar_url]);

    const fetchSessions = async () => {
        setIsLoadingSessions(true);
        try {
            const response = await authFetch('/api/users/me/sessions');
            if (response.ok) {
                const data = await response.json();
                setSessions(data.sessions || []);
            }
        } catch (error) {
            console.error('Failed to fetch sessions:', error);
        } finally {
            setIsLoadingSessions(false);
        }
    };

    const handleAvatarClick = () => {
        fileInputRef.current?.click();
    };

    const handleAvatarChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;

        // Validate file type
        if (!file.type.startsWith('image/')) {
            setAvatarError('Please select an image file');
            return;
        }

        // Validate file size (5MB max)
        if (file.size > 5 * 1024 * 1024) {
            setAvatarError('Image must be less than 5MB');
            return;
        }

        setAvatarError('');
        // Create object URL and show crop modal
        setSelectedImage(URL.createObjectURL(file));
        setShowCropModal(true);
        
        // Reset file input so same file can be selected again
        if (fileInputRef.current) {
            fileInputRef.current.value = '';
        }
    };

    const handleCropComplete = async (croppedBlob: Blob) => {
        setShowCropModal(false);
        setSelectedImage(null);
        setIsUploadingAvatar(true);
        setAvatarError('');

        try {
            const formData = new FormData();
            formData.append('avatar', croppedBlob, 'avatar.png');

            const response = await authFetch('/api/users/me/avatar', {
                method: 'POST',
                body: formData,
            });

            if (response.ok) {
                await refreshUser();
                // Update cache buster to force avatar refresh
                setAvatarCacheBuster(Date.now());
                setAvatarImgError(false);
            } else {
                setAvatarError('Failed to upload avatar');
            }
        } catch (error) {
            setAvatarError('Failed to upload avatar');
        } finally {
            setIsUploadingAvatar(false);
        }
    };

    const handleCropCancel = () => {
        setShowCropModal(false);
        if (selectedImage) {
            URL.revokeObjectURL(selectedImage);
        }
        setSelectedImage(null);
    };

    const handleSaveProfile = async () => {
        const isEmailChanging = editEmail !== user?.email;
        
        // If email is changing and 2FA is required but no code provided
        if (isEmailChanging && email2FARequired && !email2FACode) {
            setProfileError('Please enter your 2FA code to change email');
            return;
        }
        
        setIsSavingProfile(true);
        setProfileError('');
        setProfileSuccess('');

        try {
            const response = await authFetch('/api/users/me/profile', {
                method: 'PUT',
                body: JSON.stringify({
                    name: editName !== user?.name ? editName : undefined,
                    email: isEmailChanging ? editEmail : undefined,
                    totp_code: isEmailChanging && email2FACode ? email2FACode : undefined,
                }),
            });

            if (response.ok) {
                setProfileSuccess('Profile updated successfully');
                setIsEditingProfile(false);
                setEmail2FACode('');
                setEmail2FARequired(false);
                await refreshUser();
            } else if (response.status === 409) {
                setProfileError('Email is already in use');
            } else if (response.status === 403) {
                // 2FA is required for email change
                setEmail2FARequired(true);
                setProfileError('2FA verification required to change email. Enter your authenticator code.');
            } else if (response.status === 401) {
                setProfileError('Invalid 2FA code. Please try again.');
            } else {
                setProfileError('Failed to update profile');
            }
        } catch (error) {
            setProfileError('An error occurred');
        } finally {
            setIsSavingProfile(false);
        }
    };

    const handleChangePassword = async () => {
        if (newPassword !== confirmPassword) {
            setPasswordError('Passwords do not match');
            return;
        }

        // Validate against password policy
        if (passwordPolicy) {
            const errors = validatePassword(newPassword, passwordPolicy);
            if (errors.length > 0) {
                setPasswordErrors(errors);
                setPasswordError('Password does not meet requirements');
                return;
            }
        }

        // If 2FA is required but no code provided
        if (password2FARequired && !password2FACode) {
            setPasswordError('Please enter your 2FA code');
            return;
        }

        setIsChangingPassword(true);
        setPasswordError('');
        setPasswordErrors([]);
        setPasswordSuccess('');

        try {
            const response = await authFetch('/api/users/me/password', {
                method: 'PUT',
                body: JSON.stringify({
                    current_password: currentPassword,
                    new_password: newPassword,
                    totp_code: password2FACode || undefined,
                }),
            });

            const data = await response.json();

            if (response.ok && data.success) {
                setPasswordSuccess('Password changed successfully');
                setShowPasswordForm(false);
                setCurrentPassword('');
                setNewPassword('');
                setConfirmPassword('');
                setPassword2FACode('');
                setPassword2FARequired(false);
            } else if (data.error === '2fa_required' || data.require_2fa) {
                // 2FA is required - show the 2FA input
                setPassword2FARequired(true);
                setPasswordError('2FA verification required. Enter your authenticator code.');
            } else if (data.error === 'invalid_2fa_code') {
                setPasswordError('Invalid 2FA code. Please try again.');
            } else if (response.status === 401) {
                setPasswordError('Current password is incorrect');
            } else if (response.status === 400) {
                setPasswordError('Password does not meet security requirements');
            } else {
                setPasswordError(data.message || 'Failed to change password');
            }
        } catch (error) {
            setPasswordError('An error occurred');
        } finally {
            setIsChangingPassword(false);
        }
    };

    const handleRevokeSession = async (sessionId: string) => {
        setRevokingSessionId(sessionId);
        try {
            const response = await authFetch(`/api/users/me/sessions/${sessionId}`, {
                method: 'DELETE',
            });

            if (response.ok) {
                setSessions(sessions.filter(s => s.id !== sessionId));
            }
        } catch (error) {
            console.error('Failed to revoke session:', error);
        } finally {
            setRevokingSessionId(null);
        }
    };

    const handleExportData = async () => {
        setIsExporting(true);
        setExportError('');
        try {
            const response = await authFetch('/api/users/me/export');
            if (response.ok) {
                const blob = await response.blob();
                const url = window.URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `clovalink-export-${user?.id}.json`;
                document.body.appendChild(a);
                a.click();
                window.URL.revokeObjectURL(url);
                document.body.removeChild(a);
            } else {
                setExportError('Failed to export data.');
            }
        } catch (error) {
            setExportError('An error occurred during export.');
        } finally {
            setIsExporting(false);
        }
    };

    const start2FASetup = async () => {
        setIsSettingUp2FA(true);
        setSetupError('');
        setSetupSuccess(false);
        try {
            const response = await authFetch('/api/auth/2fa/setup', { method: 'POST' });
            if (response.ok) {
                const data = await response.json();
                setQrCode(data.qr_code);
                setSecret(data.secret);
            } else {
                setSetupError('Failed to initiate 2FA setup.');
            }
        } catch (error) {
            setSetupError('An error occurred.');
        }
    };

    const verify2FASetup = async () => {
        setSetupError('');
        try {
            const response = await authFetch('/api/auth/2fa/verify', {
                method: 'POST',
                body: JSON.stringify({ code: verifyCode, secret }),
            });

            if (response.ok) {
                setSetupSuccess(true);
                setIsSettingUp2FA(false);
                refreshUser();
            } else {
                setSetupError('Invalid code. Please try again.');
            }
        } catch (error) {
            setSetupError('Verification failed.');
        }
    };

    const formatDate = (dateString: string) => {
        return new Date(dateString).toLocaleDateString('en-US', {
            month: 'short',
            day: 'numeric',
            year: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
    };

    const getInitials = (name: string) => {
        return name.split(' ').map(n => n[0]).join('').toUpperCase().slice(0, 2);
    };

    if (!user) return null;

    return (
        <div className="max-w-4xl mx-auto space-y-6 p-4 md:p-0">
            {/* Profile Header Card */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="p-6">
                    <div className="flex flex-col md:flex-row md:items-center gap-4">
                        {/* Avatar */}
                        <div className="relative">
                            <div 
                                onClick={handleAvatarClick}
                                className={clsx(
                                    "h-24 w-24 rounded-full border-4 border-gray-200 dark:border-gray-700 flex items-center justify-center cursor-pointer transition-all",
                                    "bg-primary-100 dark:bg-primary-900/30 text-primary-600 dark:text-primary-400",
                                    isUploadingAvatar && "opacity-50"
                                )}
                            >
                                {user.avatar_url && !avatarImgError ? (
                                    <img 
                                        src={`${user.avatar_url}?t=${avatarCacheBuster}`} 
                                        alt=""
                                        className="h-full w-full rounded-full object-cover"
                                        onError={() => setAvatarImgError(true)}
                                    />
                                ) : (
                                    <span className="text-2xl font-semibold">{getInitials(user.name)}</span>
                                )}
                                <div className="absolute inset-0 bg-black/40 rounded-full flex items-center justify-center opacity-0 hover:opacity-100 transition-opacity">
                                    <Camera className="w-6 h-6 text-white" />
                                </div>
                            </div>
                            <input
                                ref={fileInputRef}
                                type="file"
                                accept="image/*"
                                onChange={handleAvatarChange}
                                className="hidden"
                            />
                        </div>

                        {/* Name and Role */}
                        <div className="flex-1">
                            {isEditingProfile ? (
                                <div className="space-y-3">
                                    <input
                                        type="text"
                                        value={editName}
                                        onChange={(e) => setEditName(e.target.value)}
                                        className="text-xl font-bold w-full px-3 py-1 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        placeholder="Your name"
                                    />
                                    <input
                                        type="email"
                                        value={editEmail}
                                        onChange={(e) => {
                                            setEditEmail(e.target.value);
                                            // Reset 2FA requirement if email is changed back to original
                                            if (e.target.value === user?.email) {
                                                setEmail2FARequired(false);
                                                setEmail2FACode('');
                                            }
                                        }}
                                        className="text-sm w-full px-3 py-1 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        placeholder="your.email@example.com"
                                    />
                                    {email2FARequired && editEmail !== user?.email && (
                                        <div className="mt-2 p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
                                            <div className="flex items-center gap-2 text-amber-700 dark:text-amber-300 text-xs mb-2">
                                                <Shield className="w-4 h-4" />
                                                2FA verification required to change email
                                            </div>
                                            <input
                                                type="text"
                                                value={email2FACode}
                                                onChange={(e) => setEmail2FACode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-center text-lg tracking-widest font-mono"
                                                placeholder="000000"
                                                maxLength={6}
                                                autoComplete="one-time-code"
                                            />
                                        </div>
                                    )}
                                </div>
                            ) : (
                                <>
                                    <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{user.name}</h1>
                                    <p className="text-gray-500 dark:text-gray-400 flex items-center gap-2">
                                        <Mail className="w-4 h-4" />
                                        {user.email}
                                    </p>
                                </>
                            )}
                            <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-primary-100 dark:bg-primary-900/30 text-primary-800 dark:text-primary-300 mt-2">
                                {user.role}
                            </span>
                        </div>

                        {/* Edit Button */}
                        <div className="flex gap-2">
                            {isEditingProfile ? (
                                <>
                                    <button
                                        onClick={() => {
                                            setIsEditingProfile(false);
                                            setEditName(user.name);
                                            setEditEmail(user.email);
                                            setEmail2FACode('');
                                            setEmail2FARequired(false);
                                            setProfileError('');
                                        }}
                                        className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                                    >
                                        <X className="w-4 h-4" />
                                    </button>
                                    <button
                                        onClick={handleSaveProfile}
                                        disabled={isSavingProfile}
                                        className="px-4 py-2 bg-primary-600 text-white rounded-lg text-sm font-medium hover:bg-primary-700 disabled:opacity-50 transition-colors flex items-center gap-2"
                                    >
                                        <Save className="w-4 h-4" />
                                        {isSavingProfile ? 'Saving...' : 'Save'}
                                    </button>
                                </>
                            ) : (
                                <button
                                    onClick={() => setIsEditingProfile(true)}
                                    className="px-4 py-2 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors flex items-center gap-2"
                                >
                                    <Edit2 className="w-4 h-4" />
                                    Edit Profile
                                </button>
                            )}
                        </div>
                    </div>

                    {/* Messages */}
                    {avatarError && (
                        <div className="mt-4 p-3 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 rounded-lg text-sm flex items-center gap-2">
                            <AlertCircle className="w-4 h-4" />
                            {avatarError}
                        </div>
                    )}
                    {profileError && (
                        <div className="mt-4 p-3 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 rounded-lg text-sm flex items-center gap-2">
                            <AlertCircle className="w-4 h-4" />
                            {profileError}
                        </div>
                    )}
                    {profileSuccess && (
                        <div className="mt-4 p-3 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300 rounded-lg text-sm flex items-center gap-2">
                            <CheckCircle className="w-4 h-4" />
                            {profileSuccess}
                        </div>
                    )}
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {/* Security Settings */}
                <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                    <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2">
                        <Shield className="w-5 h-5 text-primary-500" />
                        <h2 className="text-lg font-medium text-gray-900 dark:text-white">Security</h2>
                    </div>
                    <div className="p-6 space-y-6">
                        {/* Change Password */}
                        <div>
                            <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-2 flex items-center gap-2">
                                <Lock className="w-4 h-4" />
                                Password
                            </h3>
                            
                            {passwordSuccess && (
                                <div className="mb-4 p-3 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300 rounded-lg flex items-center gap-2 text-sm">
                                    <CheckCircle className="w-4 h-4" />
                                    {passwordSuccess}
                                </div>
                            )}

                            {!showPasswordForm ? (
                                <button
                                    onClick={() => setShowPasswordForm(true)}
                                    className="w-full px-4 py-2 bg-gray-100 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                                >
                                    Change Password
                                </button>
                            ) : (
                                <div className="space-y-3 bg-gray-50 dark:bg-gray-700/30 p-4 rounded-lg">
                                    <div>
                                        <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">Current Password</label>
                                        <input
                                            type="password"
                                            value={currentPassword}
                                            onChange={(e) => setCurrentPassword(e.target.value)}
                                            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                        />
                                    </div>
                                    <PasswordInput
                                        value={newPassword}
                                        onChange={(val) => {
                                            setNewPassword(val);
                                            setPasswordErrors([]);
                                        }}
                                        policy={passwordPolicy}
                                        label="New Password"
                                        showRequirements={true}
                                        error={passwordErrors}
                                    />
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Confirm New Password</label>
                                        <input
                                            type="password"
                                            value={confirmPassword}
                                            onChange={(e) => setConfirmPassword(e.target.value)}
                                            placeholder="••••••••"
                                            className={clsx(
                                                "w-full px-4 py-2.5 border rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white",
                                                confirmPassword && newPassword !== confirmPassword
                                                    ? "border-red-500"
                                                    : "border-gray-300 dark:border-gray-600"
                                            )}
                                        />
                                        {confirmPassword && newPassword !== confirmPassword && (
                                            <p className="text-xs text-red-600 dark:text-red-400 mt-1">Passwords do not match</p>
                                        )}
                                    </div>

                                    {password2FARequired && (
                                        <div className="p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg">
                                            <label className="block text-xs font-medium text-amber-800 dark:text-amber-300 mb-2">
                                                <Smartphone className="w-3 h-3 inline mr-1" />
                                                2FA Verification Required
                                            </label>
                                            <input
                                                type="text"
                                                value={password2FACode}
                                                onChange={(e) => setPassword2FACode(e.target.value)}
                                                placeholder="Enter 6-digit code"
                                                maxLength={6}
                                                className="w-full px-3 py-2 border border-amber-300 dark:border-amber-600 rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-center tracking-widest font-mono"
                                            />
                                            <p className="text-xs text-amber-700 dark:text-amber-400 mt-1">
                                                Enter the code from your authenticator app
                                            </p>
                                        </div>
                                    )}

                                    {passwordError && (
                                        <p className="text-xs text-red-600 dark:text-red-400">{passwordError}</p>
                                    )}

                                    <div className="flex gap-2">
                                        <button
                                            onClick={() => {
                                                setShowPasswordForm(false);
                                                setCurrentPassword('');
                                                setNewPassword('');
                                                setConfirmPassword('');
                                                setPassword2FACode('');
                                                setPassword2FARequired(false);
                                                setPasswordError('');
                                            }}
                                            className="flex-1 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-lg transition-colors"
                                        >
                                            Cancel
                                        </button>
                                        <button
                                            onClick={handleChangePassword}
                                            disabled={isChangingPassword || !currentPassword || !newPassword || !confirmPassword || (password2FARequired && !password2FACode)}
                                            className="flex-1 px-4 py-2 bg-primary-600 text-white rounded-lg text-sm font-medium hover:bg-primary-700 disabled:opacity-50"
                                        >
                                            {isChangingPassword ? 'Changing...' : 'Change Password'}
                                        </button>
                                    </div>
                                </div>
                            )}
                        </div>

                        {/* 2FA */}
                        <div>
                            <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-2 flex items-center gap-2">
                                <Smartphone className="w-4 h-4" />
                                Two-Factor Authentication
                            </h3>

                            {setupSuccess && (
                                <div className="mb-4 p-3 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300 rounded-lg flex items-center gap-2 text-sm">
                                    <CheckCircle className="w-4 h-4" />
                                    2FA enabled successfully!
                                </div>
                            )}

                            {!isSettingUp2FA ? (
                                <button
                                    onClick={start2FASetup}
                                    className="w-full px-4 py-2 bg-gray-100 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                                >
                                    Setup 2FA
                                </button>
                            ) : (
                                <div className="space-y-4 bg-gray-50 dark:bg-gray-700/30 p-4 rounded-lg">
                                    <div className="text-center">
                                        {qrCode && (
                                            <img src={`data:image/png;base64,${qrCode}`} alt="2FA QR Code" className="mx-auto mb-4 rounded-lg bg-white p-2" />
                                        )}
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">Scan with your authenticator app</p>
                                        <p className="text-xs font-mono bg-gray-100 dark:bg-gray-800 p-1 rounded select-all">{secret}</p>
                                    </div>

                                    <div>
                                        <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">Verification Code</label>
                                        <div className="flex gap-2">
                                            <input
                                                type="text"
                                                value={verifyCode}
                                                onChange={(e) => setVerifyCode(e.target.value)}
                                                placeholder="123456"
                                                className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-sm bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                            />
                                            <button
                                                onClick={verify2FASetup}
                                                disabled={!verifyCode}
                                                className="px-4 py-2 bg-primary-600 text-white rounded-lg text-sm font-medium hover:bg-primary-700 disabled:opacity-50"
                                            >
                                                Verify
                                            </button>
                                        </div>
                                    </div>

                                    {setupError && (
                                        <p className="text-xs text-red-600 dark:text-red-400">{setupError}</p>
                                    )}

                                    <button
                                        onClick={() => setIsSettingUp2FA(false)}
                                        className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 underline w-full text-center"
                                    >
                                        Cancel
                                    </button>
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                {/* Data Privacy - Only show if data export is enabled for this tenant */}
                {currentCompany.data_export_enabled !== false && (
                    <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                        <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2">
                            <Download className="w-5 h-5 text-blue-500" />
                            <h2 className="text-lg font-medium text-gray-900 dark:text-white">Data Privacy</h2>
                        </div>
                        <div className="p-6">
                            <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-2">Export Your Data</h3>
                            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
                                Download a copy of your personal data.
                            </p>

                            {exportError && (
                                <div className="mb-4 p-3 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 rounded-lg flex items-center gap-2 text-sm">
                                    <AlertCircle className="w-4 h-4" />
                                    {exportError}
                                </div>
                            )}

                            <button
                                onClick={handleExportData}
                                disabled={isExporting}
                                className="w-full px-4 py-2 bg-gray-100 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
                            >
                                <Download className={clsx("w-4 h-4", isExporting && "animate-bounce")} />
                                {isExporting ? 'Exporting...' : 'Export My Data'}
                            </button>
                        </div>
                    </div>
                )}
            </div>

            {/* Active Sessions */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <Monitor className="w-5 h-5 text-purple-500" />
                        <h2 className="text-lg font-medium text-gray-900 dark:text-white">Active Sessions</h2>
                    </div>
                    <span className="text-sm text-gray-500 dark:text-gray-400">
                        {sessions.length} active
                    </span>
                </div>
                <div className="p-6">
                    {isLoadingSessions ? (
                        <div className="flex justify-center py-4">
                            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
                        </div>
                    ) : sessions.length === 0 ? (
                        <p className="text-sm text-gray-500 dark:text-gray-400 text-center py-4">
                            No active sessions found
                        </p>
                    ) : (
                        <div className="space-y-3">
                            {sessions.map((session) => (
                                <div
                                    key={session.id}
                                    className="flex items-center justify-between p-4 rounded-lg bg-gray-50 dark:bg-gray-700/50"
                                >
                                    <div className="flex items-center gap-4">
                                        <div className="p-2 rounded-lg bg-gray-200 dark:bg-gray-600">
                                            <Monitor className="w-5 h-5 text-gray-600 dark:text-gray-300" />
                                        </div>
                                        <div>
                                            <p className="text-sm font-medium text-gray-900 dark:text-white">
                                                {session.device_info || 'Unknown Device'}
                                            </p>
                                            <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400 mt-1">
                                                {session.ip_address && (
                                                    <span className="flex items-center gap-1">
                                                        <Globe className="w-3 h-3" />
                                                        {session.ip_address}
                                                    </span>
                                                )}
                                                <span className="flex items-center gap-1">
                                                    <Clock className="w-3 h-3" />
                                                    {formatDate(session.last_active_at)}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                    <button
                                        onClick={() => handleRevokeSession(session.id)}
                                        disabled={revokingSessionId === session.id}
                                        className="p-2 text-gray-400 hover:text-red-600 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-lg transition-colors disabled:opacity-50"
                                        title="Revoke Session"
                                    >
                                        {revokingSessionId === session.id ? (
                                            <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full"></div>
                                        ) : (
                                            <Trash2 className="w-4 h-4" />
                                        )}
                                    </button>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* Notification Preferences */}
            <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                    <div className="flex items-center gap-2">
                        <Bell className="w-5 h-5 text-orange-500" />
                        <h2 className="text-lg font-medium text-gray-900 dark:text-white">Notification Preferences</h2>
                    </div>
                    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                        Customize how you receive notifications
                    </p>
                </div>
                <div className="p-0">
                    <NotificationPreferences compact />
                </div>
            </div>

            {/* Connected Accounts */}
            <div className="space-y-4">
                <h2 className="text-lg font-medium text-gray-900 dark:text-white">Connected Accounts</h2>
                <DiscordConnection />
            </div>

            {/* Image Crop Modal */}
            {showCropModal && selectedImage && (
                <ImageCropModal
                    image={selectedImage}
                    onCropComplete={handleCropComplete}
                    onCancel={handleCropCancel}
                />
            )}
        </div>
    );
}
