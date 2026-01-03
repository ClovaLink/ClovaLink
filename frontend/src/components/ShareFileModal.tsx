import { useState, useEffect, useCallback } from 'react';
import { X, Link2, Copy, Check, Globe, Lock, Calendar, AlertCircle, Loader2, Folder, FileText, Users, User, Search, X as XIcon, Building } from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch } from '../context/AuthContext';

interface ShareableUser {
    id: string;
    name: string;
    email: string;
    department_id: string | null;
    department_name: string | null;
    role: string;
}

interface ShareFileModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: {
        id: string;
        name: string;
        type?: 'folder' | 'image' | 'document' | 'video' | 'audio' | string;
    };
    companyId: string;
    complianceMode?: string;
}

export function ShareFileModal({ isOpen, onClose, file, companyId, complianceMode }: ShareFileModalProps) {
    const isFolder = file.type === 'folder';
    const authFetch = useAuthFetch();
    const [shareType, setShareType] = useState<'link' | 'user'>('link');
    const [isPublic, setIsPublic] = useState(false);
    const [hasExpiration, setHasExpiration] = useState(false);
    const [expirationDays, setExpirationDays] = useState(7);
    const [isCreating, setIsCreating] = useState(false);
    const [shareLink, setShareLink] = useState<string | null>(null);
    const [copied, setCopied] = useState(false);
    const [error, setError] = useState<string | null>(null);
    
    // User-specific sharing state
    const [searchQuery, setSearchQuery] = useState('');
    const [shareableUsers, setShareableUsers] = useState<ShareableUser[]>([]);
    const [selectedUser, setSelectedUser] = useState<ShareableUser | null>(null);
    const [searchLoading, setSearchLoading] = useState(false);
    const [userShareSuccess, setUserShareSuccess] = useState(false);

    // Check if public sharing is blocked by compliance mode
    const isComplianceMode = complianceMode && ['HIPAA', 'SOC2', 'GDPR'].includes(complianceMode);

    // Search for shareable users
    const searchUsers = useCallback(async (query: string) => {
        if (!query.trim()) {
            setShareableUsers([]);
            return;
        }
        
        setSearchLoading(true);
        try {
            const res = await authFetch(`/api/users/${companyId}/shareable?search=${encodeURIComponent(query)}`);
            if (res.ok) {
                const data = await res.json();
                setShareableUsers(data.users || []);
            }
        } catch {
            // Ignore search errors
        } finally {
            setSearchLoading(false);
        }
    }, [authFetch, companyId]);

    // Debounced search
    useEffect(() => {
        const timer = setTimeout(() => {
            searchUsers(searchQuery);
        }, 300);
        return () => clearTimeout(timer);
    }, [searchQuery, searchUsers]);

    const handleCreateShare = async () => {
        setIsCreating(true);
        setError(null);

        try {
            const body: Record<string, unknown> = {
                is_public: shareType === 'user' ? false : isPublic,
                expires_in_days: hasExpiration ? expirationDays : null,
            };
            
            // Add user-specific sharing if a user is selected
            if (shareType === 'user' && selectedUser) {
                body.shared_with_user_id = selectedUser.id;
            }
            
            const response = await authFetch(`/api/files/${companyId}/${file.id}/share`, {
                method: 'POST',
                body: JSON.stringify(body),
            });

            if (response.ok) {
                const data = await response.json();
                if (shareType === 'user' && selectedUser) {
                    setUserShareSuccess(true);
                } else {
                    setShareLink(data.link);
                }
            } else if (response.status === 403) {
                if (shareType === 'user') {
                    setError('You cannot share with this user. They may be in a different department.');
                } else {
                    setError('Public sharing is not allowed in your compliance mode.');
                }
            } else {
                setError('Failed to create share. Please try again.');
            }
        } catch (err) {
            setError('An error occurred. Please try again.');
        } finally {
            setIsCreating(false);
        }
    };

    const handleCopy = async () => {
        if (shareLink) {
            await navigator.clipboard.writeText(shareLink);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    const handleClose = () => {
        setShareLink(null);
        setIsPublic(false);
        setHasExpiration(false);
        setExpirationDays(7);
        setError(null);
        setCopied(false);
        setShareType('link');
        setSearchQuery('');
        setShareableUsers([]);
        setSelectedUser(null);
        setUserShareSuccess(false);
        onClose();
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-[60] overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
                {/* Backdrop */}
                <div className="fixed inset-0 bg-black/50 transition-opacity" onClick={handleClose} />

                {/* Modal */}
                <div className="relative bg-white dark:bg-gray-800 rounded-xl shadow-2xl w-full max-w-md transform transition-all">
                    {/* Header */}
                    <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-primary-100 dark:bg-primary-900/30 rounded-lg">
                                {isFolder ? (
                                    <Folder className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                                ) : (
                                    <Link2 className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                                )}
                            </div>
                            <div>
                                <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                                    Share {isFolder ? 'Folder' : 'File'}
                                </h3>
                                <p className="text-sm text-gray-500 dark:text-gray-400 truncate max-w-[250px]">{file.name}</p>
                            </div>
                        </div>
                        <button
                            onClick={handleClose}
                            className="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    {/* Content */}
                    <div className="p-6">
                        {/* Success States */}
                        {shareLink ? (
                            // Success state - show the link
                            <div className="space-y-4">
                                <div className="p-4 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
                                    <div className="flex items-center gap-2 text-green-700 dark:text-green-300 text-sm font-medium mb-2">
                                        <Check className="w-4 h-4" />
                                        Share link created!
                                    </div>
                                    <p className="text-xs text-green-600 dark:text-green-400">
                                        {isPublic 
                                            ? `Anyone with this link can download the ${isFolder ? 'folder as a zip file' : 'file'}.` 
                                            : `Only logged-in users from your organization can access this ${isFolder ? 'folder' : 'file'}.`}
                                    </p>
                                    {isFolder && (
                                        <p className="text-xs text-green-600 dark:text-green-400 mt-1">
                                            📁 Folder contents will be automatically zipped when downloaded.
                                        </p>
                                    )}
                                </div>

                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        value={shareLink}
                                        readOnly
                                        className="flex-1 px-3 py-2 text-sm bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-700 dark:text-gray-300"
                                    />
                                    <button
                                        onClick={handleCopy}
                                        className={clsx(
                                            "px-4 py-2 rounded-lg font-medium text-sm transition-all flex items-center gap-2",
                                            copied
                                                ? "bg-green-600 text-white"
                                                : "bg-primary-600 text-white hover:bg-primary-700"
                                        )}
                                    >
                                        {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                                        {copied ? 'Copied!' : 'Copy'}
                                    </button>
                                </div>

                                <button
                                    onClick={handleClose}
                                    className="w-full py-2 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 transition-colors"
                                >
                                    Done
                                </button>
                            </div>
                        ) : userShareSuccess ? (
                            // User share success state
                            <div className="space-y-4">
                                <div className="p-4 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
                                    <div className="flex items-center gap-2 text-green-700 dark:text-green-300 text-sm font-medium mb-2">
                                        <Check className="w-4 h-4" />
                                        Shared with {selectedUser?.name}!
                                    </div>
                                    <p className="text-xs text-green-600 dark:text-green-400">
                                        {selectedUser?.name} can now access "{file.name}" and will be notified.
                                    </p>
                                </div>
                                <button
                                    onClick={handleClose}
                                    className="w-full py-2 text-sm font-medium text-white bg-primary-600 rounded-lg hover:bg-primary-700 transition-colors"
                                >
                                    Done
                                </button>
                            </div>
                        ) : (
                            // Configuration state
                            <div className="space-y-5">
                                {/* Share Type Tabs */}
                                <div className="flex border-b border-gray-200 dark:border-gray-700">
                                    <button
                                        onClick={() => setShareType('link')}
                                        className={clsx(
                                            "flex-1 py-2 text-sm font-medium border-b-2 transition-colors",
                                            shareType === 'link'
                                                ? "border-primary-500 text-primary-600 dark:text-primary-400"
                                                : "border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                                        )}
                                    >
                                        <div className="flex items-center justify-center gap-2">
                                            <Link2 className="w-4 h-4" />
                                            Share Link
                                        </div>
                                    </button>
                                    <button
                                        onClick={() => setShareType('user')}
                                        className={clsx(
                                            "flex-1 py-2 text-sm font-medium border-b-2 transition-colors",
                                            shareType === 'user'
                                                ? "border-primary-500 text-primary-600 dark:text-primary-400"
                                                : "border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                                        )}
                                    >
                                        <div className="flex items-center justify-center gap-2">
                                            <User className="w-4 h-4" />
                                            Share with User
                                        </div>
                                    </button>
                                </div>

                                {/* User Sharing */}
                                {shareType === 'user' && (
                                    <div className="space-y-3">
                                        {/* Selected User Display */}
                                        {selectedUser ? (
                                            <div className="flex items-center justify-between p-3 bg-primary-50 dark:bg-primary-900/20 border border-primary-200 dark:border-primary-800 rounded-lg">
                                                <div className="flex items-center gap-3">
                                                    <div className="w-8 h-8 rounded-full bg-primary-100 dark:bg-primary-800 flex items-center justify-center text-primary-700 dark:text-primary-300 text-sm font-medium">
                                                        {selectedUser.name?.charAt(0)?.toUpperCase() || '?'}
                                                    </div>
                                                    <div>
                                                        <p className="text-sm font-medium text-gray-900 dark:text-white">{selectedUser.name}</p>
                                                        <p className="text-xs text-gray-500 dark:text-gray-400">{selectedUser.email}</p>
                                                    </div>
                                                </div>
                                                <button
                                                    onClick={() => setSelectedUser(null)}
                                                    className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                                                >
                                                    <XIcon className="w-4 h-4" />
                                                </button>
                                            </div>
                                        ) : (
                                            <>
                                                {/* Search Input */}
                                                <div className="relative">
                                                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
                                                    <input
                                                        type="text"
                                                        value={searchQuery}
                                                        onChange={(e) => setSearchQuery(e.target.value)}
                                                        placeholder="Search users by name or email..."
                                                        className="w-full pl-10 pr-4 py-2.5 text-sm bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                                                    />
                                                    {searchLoading && (
                                                        <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 animate-spin" />
                                                    )}
                                                </div>

                                                {/* Search Results */}
                                                {shareableUsers.length > 0 && (
                                                    <div className="max-h-48 overflow-y-auto border border-gray-200 dark:border-gray-700 rounded-lg divide-y divide-gray-100 dark:divide-gray-700">
                                                        {shareableUsers.map((user) => (
                                                            <button
                                                                key={user.id}
                                                                onClick={() => {
                                                                    setSelectedUser(user);
                                                                    setSearchQuery('');
                                                                    setShareableUsers([]);
                                                                }}
                                                                className="w-full flex items-center gap-3 p-3 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors text-left"
                                                            >
                                                                <div className="w-8 h-8 rounded-full bg-gray-100 dark:bg-gray-700 flex items-center justify-center text-gray-600 dark:text-gray-300 text-sm font-medium">
                                                                    {user.name?.charAt(0)?.toUpperCase() || '?'}
                                                                </div>
                                                                <div className="flex-1 min-w-0">
                                                                    <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{user.name}</p>
                                                                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{user.email}</p>
                                                                </div>
                                                                {user.department_name && (
                                                                    <span className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
                                                                        <Building className="w-3 h-3" />
                                                                        {user.department_name}
                                                                    </span>
                                                                )}
                                                            </button>
                                                        ))}
                                                    </div>
                                                )}

                                                {/* Empty State */}
                                                {searchQuery && !searchLoading && shareableUsers.length === 0 && (
                                                    <div className="text-center py-4 text-sm text-gray-500 dark:text-gray-400">
                                                        No users found. Try a different search term.
                                                    </div>
                                                )}

                                                {/* Hint */}
                                                {!searchQuery && (
                                                    <p className="text-xs text-gray-500 dark:text-gray-400 text-center">
                                                        Search for a user in your department to share this {isFolder ? 'folder' : 'file'} with them.
                                                    </p>
                                                )}
                                            </>
                                        )}
                                    </div>
                                )}

                                {/* Link Access Type - only show for link sharing */}
                                {shareType === 'link' && (
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                                        Who can access this link?
                                    </label>
                                    <div className="grid grid-cols-2 gap-3">
                                        <button
                                            onClick={() => setIsPublic(false)}
                                            disabled={isCreating}
                                            className={clsx(
                                                "flex flex-col items-center gap-2 p-4 rounded-lg border-2 transition-all",
                                                !isPublic
                                                    ? "border-primary-500 bg-primary-50 dark:bg-primary-900/20"
                                                    : "border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600"
                                            )}
                                        >
                                            <Lock className={clsx("w-6 h-6", !isPublic ? "text-primary-600 dark:text-primary-400" : "text-gray-400")} />
                                            <span className={clsx("text-sm font-medium", !isPublic ? "text-primary-700 dark:text-primary-300" : "text-gray-600 dark:text-gray-400")}>
                                                Organization Only
                                            </span>
                                            <span className="text-xs text-gray-500 dark:text-gray-500 text-center">
                                                Must be logged in
                                            </span>
                                        </button>

                                        <button
                                            onClick={() => !isComplianceMode && setIsPublic(true)}
                                            disabled={isCreating || !!isComplianceMode}
                                            className={clsx(
                                                "flex flex-col items-center gap-2 p-4 rounded-lg border-2 transition-all",
                                                isComplianceMode && "opacity-50 cursor-not-allowed",
                                                isPublic
                                                    ? "border-primary-500 bg-primary-50 dark:bg-primary-900/20"
                                                    : "border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600"
                                            )}
                                        >
                                            <Globe className={clsx("w-6 h-6", isPublic ? "text-primary-600 dark:text-primary-400" : "text-gray-400")} />
                                            <span className={clsx("text-sm font-medium", isPublic ? "text-primary-700 dark:text-primary-300" : "text-gray-600 dark:text-gray-400")}>
                                                Anyone with Link
                                            </span>
                                            <span className="text-xs text-gray-500 dark:text-gray-500 text-center">
                                                {isComplianceMode ? 'Blocked by compliance' : 'No login required'}
                                            </span>
                                        </button>
                                    </div>
                                </div>
                                )}

                                {/* Expiration */}
                                <div>
                                    <div className="flex items-center justify-between mb-3">
                                        <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                                            Set expiration date?
                                        </label>
                                        <button
                                            onClick={() => setHasExpiration(!hasExpiration)}
                                            disabled={isCreating}
                                            className={clsx(
                                                "relative w-11 h-6 rounded-full transition-colors",
                                                hasExpiration ? "bg-primary-600" : "bg-gray-200 dark:bg-gray-700"
                                            )}
                                        >
                                            <span
                                                className={clsx(
                                                    "absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform",
                                                    hasExpiration && "translate-x-5"
                                                )}
                                            />
                                        </button>
                                    </div>

                                    {hasExpiration && (
                                        <div className="flex items-center gap-3 p-3 bg-gray-50 dark:bg-gray-900 rounded-lg">
                                            <Calendar className="w-5 h-5 text-gray-400" />
                                            <span className="text-sm text-gray-600 dark:text-gray-400">Expires in</span>
                                            <select
                                                value={expirationDays}
                                                onChange={(e) => setExpirationDays(Number(e.target.value))}
                                                disabled={isCreating}
                                                className="flex-1 px-3 py-1.5 text-sm bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                                            >
                                                <option value={1}>1 day</option>
                                                <option value={3}>3 days</option>
                                                <option value={7}>7 days</option>
                                                <option value={14}>14 days</option>
                                                <option value={30}>30 days</option>
                                                <option value={90}>90 days</option>
                                            </select>
                                        </div>
                                    )}
                                </div>

                                {/* Error */}
                                {error && (
                                    <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg flex items-start gap-2">
                                        <AlertCircle className="w-4 h-4 text-red-500 mt-0.5" />
                                        <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
                                    </div>
                                )}

                                {/* Actions */}
                                <div className="flex gap-3 pt-2">
                                    <button
                                        onClick={handleClose}
                                        disabled={isCreating}
                                        className="flex-1 px-4 py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        onClick={handleCreateShare}
                                        disabled={isCreating || (shareType === 'user' && !selectedUser)}
                                        className="flex-1 px-4 py-2.5 text-sm font-medium text-white bg-primary-600 rounded-lg hover:bg-primary-700 transition-colors flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        {isCreating ? (
                                            <>
                                                <Loader2 className="w-4 h-4 animate-spin" />
                                                {shareType === 'user' ? 'Sharing...' : 'Creating...'}
                                            </>
                                        ) : shareType === 'user' ? (
                                            <>
                                                <User className="w-4 h-4" />
                                                Share with User
                                            </>
                                        ) : (
                                            <>
                                                <Link2 className="w-4 h-4" />
                                                Create Link
                                            </>
                                        )}
                                    </button>
                                </div>
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}

