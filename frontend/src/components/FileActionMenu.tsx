import React, { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import {
    Eye, Download, Trash2, Star, Edit2, Share2,
    Lock, Unlock, History, Move, Info, Building2, Sparkles, MessageSquare, FileSearch, Copy,
    Layers, FolderMinus, ChevronRight, Plus
} from 'lucide-react';
import clsx from 'clsx';

export interface FileItem {
    id: string;
    name: string;
    type: 'folder' | 'image' | 'document' | 'video' | 'audio' | 'group';
    size?: string;
    size_bytes?: number;
    modified: string;
    created_at?: string;
    owner: string;
    owner_id?: string;
    owner_avatar?: string;
    is_starred?: boolean;
    is_locked?: boolean;
    locked_by?: string;
    locked_at?: string;
    lock_requires_role?: string;
    has_lock_password?: boolean;
    visibility?: 'department' | 'private';
    department_id?: string;
    content_type?: string;
    storage_path?: string;
    is_company_folder?: boolean;
    color?: string;
    file_count?: number;
}

interface FileGroup {
    id: string;
    name: string;
    color?: string;
}

interface FileActionMenuProps {
    file: FileItem;
    companyId: string;
    complianceMode?: string;
    canLockFiles: boolean;
    canViewActivity: boolean;
    canDelete: boolean;
    canShare: boolean;  // Only owner, manager, or admin can share
    currentUserId?: string;  // Current user's ID for ownership checks
    currentUserRole?: string;  // Current user's role for lock requirement checks
    canUseAi?: boolean;  // Whether user has access to AI features
    aiEnabled?: boolean;  // Whether AI is enabled for the tenant
    groups?: FileGroup[];  // Available groups to add files to
    isInsideGroup?: boolean;  // Whether we're viewing files inside a group
    onPreview: (file: FileItem) => void;
    onShare: (file: FileItem) => void;
    onDownload: (file: FileItem) => void;
    onStar: (file: FileItem) => void;
    onRename: (file: FileItem) => void;
    onLock: (file: FileItem) => void;
    onActivity: (file: FileItem) => void;
    onMove: (file: FileItem) => void;
    onCopy: (file: FileItem) => void;
    onDelete: (file: FileItem) => void;
    onProperties: (file: FileItem) => void;
    onToggleCompanyFolder?: (file: FileItem) => void;
    onAiSummarize?: (file: FileItem) => void;
    onAiQuestion?: (file: FileItem) => void;
    onAddToGroup?: (file: FileItem, groupId: string) => void;
    onRemoveFromGroup?: (file: FileItem) => void;
    onCreateGroupFromFile?: (file: FileItem) => void;  // Opens modal to create group with this file
    buttonRef?: { current: HTMLButtonElement | null };
}

const menuItemClass = "flex items-center w-full px-4 py-2 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700";
const dividerClass = "border-t border-gray-100 dark:border-gray-700 my-1";

export function FileActionMenu({
    file,
    companyId,
    complianceMode,
    canLockFiles,
    canViewActivity,
    canDelete,
    canShare,
    currentUserId,
    currentUserRole,
    canUseAi,
    aiEnabled,
    groups = [],
    isInsideGroup = false,
    onPreview,
    onShare,
    onDownload,
    onStar,
    onRename,
    onLock,
    onActivity,
    onMove,
    onCopy,
    onDelete,
    onProperties,
    onToggleCompanyFolder,
    onAiSummarize,
    onAiQuestion,
    onAddToGroup,
    onRemoveFromGroup,
    onCreateGroupFromFile,
    buttonRef,
}: FileActionMenuProps) {
    const isFile = file.type !== 'folder';
    // Case-insensitive check for compliance mode (backend may return "HIPAA", "Hipaa", etc.)
    const isComplianceMode = ['hipaa', 'soc2', 'gdpr'].includes(complianceMode?.toLowerCase() || '');
    
    // Role hierarchy for lock requirement checks
    const getRoleLevel = (role: string) => {
        switch(role) {
            case 'SuperAdmin': return 100;
            case 'Admin': return 80;
            case 'Manager': return 60;
            case 'Employee': return 40;
            default: return 20;
        }
    };
    
    // Check if user can access locked file
    const isOwner = currentUserId && file.owner_id === currentUserId;
    const isLocker = currentUserId && file.locked_by === currentUserId;
    
    // Check if user's role meets the lock requirement
    const meetsRoleRequirement = () => {
        if (!file.lock_requires_role) return false; // No role specified = only owner/locker can access
        if (!currentUserRole) return false;
        // SuperAdmin/Admin bypass all locks
        if (currentUserRole === 'SuperAdmin' || currentUserRole === 'Admin') return true;
        return getRoleLevel(currentUserRole) >= getRoleLevel(file.lock_requires_role);
    };
    
    const canAccessLockedFile = !file.is_locked || isLocker || isOwner || meetsRoleRequirement();
    
    // Check if AI actions are available (file must be text-based)
    const isTextFile = isFile && (
        ['text/plain', 'text/markdown', 'text/csv', 'text/html', 'application/json', 'text/xml'].some(
            type => file.content_type?.startsWith(type.split('/')[0]) || file.content_type === type
        ) || file.name?.match(/\.(txt|md|json|xml|csv|html|htm|js|ts|jsx|tsx|py|rs|go|java|c|cpp|h|css|scss|yaml|yml)$/i)
    );
    const showAiActions = aiEnabled && canUseAi && isTextFile && canAccessLockedFile;

    const [position, setPosition] = useState({ top: 0, right: 0 });

    useEffect(() => {
        if (buttonRef?.current) {
            const rect = buttonRef.current.getBoundingClientRect();
            // Calculate position - menu appears below button, aligned to right edge
            const menuWidth = 192; // w-48 = 12rem = 192px
            const menuHeight = 350; // approximate max height
            
            let top = rect.bottom + 8;
            let right = window.innerWidth - rect.right;
            
            // Check if menu would go off-screen bottom
            if (top + menuHeight > window.innerHeight) {
                // Position above the button instead
                top = rect.top - menuHeight - 8;
                if (top < 0) top = 8; // Minimum top padding
            }
            
            // Check if menu would go off-screen right
            if (right < 8) right = 8;
            
            setPosition({ top, right });
        }
    }, [buttonRef]);

    const menuContent = (
        <div 
            className="fixed w-48 bg-white dark:bg-gray-800 rounded-md shadow-xl z-[100] border border-gray-100 dark:border-gray-700 ring-1 ring-black ring-opacity-5 text-left"
            style={{ top: position.top, right: position.right }}
        >
            <div className="py-1">
                {/* Preview - Files only, requires access to locked files */}
                {isFile && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onPreview(file)}>
                        <Eye className="w-4 h-4 mr-2 text-gray-400" /> Preview
                    </button>
                )}

                {/* Share - Only if user can share AND can access locked file */}
                {canShare && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onShare(file)}>
                        <Share2 className="w-4 h-4 mr-2 text-gray-400" /> Share
                    </button>
                )}

                {/* Star - requires access to locked files */}
                {canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onStar(file)}>
                        <Star className={clsx("w-4 h-4 mr-2", file.is_starred ? "text-yellow-400 fill-current" : "text-gray-400")} />
                        {file.is_starred ? "Unstar" : "Star"}
                    </button>
                )}

                {/* Download - requires access to locked files */}
                {canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onDownload(file)}>
                        <Download className="w-4 h-4 mr-2 text-gray-400" /> Download
                    </button>
                )}

                {/* AI Actions - Only for text files when AI is enabled */}
                {showAiActions && onAiSummarize && (
                    <button className={menuItemClass} onClick={() => onAiSummarize(file)}>
                        <Sparkles className="w-4 h-4 mr-2 text-purple-500" /> Summarize
                    </button>
                )}
                {showAiActions && onAiQuestion && (
                    <button className={menuItemClass} onClick={() => onAiQuestion(file)}>
                        <MessageSquare className="w-4 h-4 mr-2 text-purple-500" /> Ask AI
                    </button>
                )}

                {/* Rename - requires access to locked files */}
                {canAccessLockedFile && !file.is_locked && (
                    <button className={menuItemClass} onClick={() => onRename(file)}>
                        <Edit2 className="w-4 h-4 mr-2 text-gray-400" /> Rename
                    </button>
                )}

                <div className={dividerClass}></div>

                {/* Lock/Unlock - Only for Manager, Admin, SuperAdmin who can access the file */}
                {canLockFiles && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onLock(file)}>
                        {file.is_locked ? (
                            <>
                                <Unlock className="w-4 h-4 mr-2 text-green-500" /> Unlock
                            </>
                        ) : (
                            <>
                                <Lock className="w-4 h-4 mr-2 text-orange-500" /> Lock
                            </>
                        )}
                    </button>
                )}

                {/* Recent Activity - Only for Admin, SuperAdmin */}
                {canViewActivity && (
                    <button className={menuItemClass} onClick={() => onActivity(file)}>
                        <History className="w-4 h-4 mr-2 text-gray-400" /> Recent Activity
                    </button>
                )}

                {/* Move To - Only if not locked and user can access */}
                {!file.is_locked && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onMove(file)}>
                        <Move className="w-4 h-4 mr-2 text-gray-400" /> Move To...
                    </button>
                )}

                {/* Copy - Only for files (not folders/groups), requires access to locked files */}
                {file.type !== 'folder' && file.type !== 'group' && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onCopy(file)}>
                        <Copy className="w-4 h-4 mr-2 text-gray-400" /> Copy
                    </button>
                )}

                {/* Create Group from File - Only for files */}
                {file.type !== 'folder' && file.type !== 'group' && onCreateGroupFromFile && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onCreateGroupFromFile(file)}>
                        <Plus className="w-4 h-4 mr-2 text-green-500" /> Create Group...
                    </button>
                )}

                {/* Add to Group - Only for files, when groups are available */}
                {file.type !== 'folder' && file.type !== 'group' && onAddToGroup && groups.length > 0 && canAccessLockedFile && (
                    <div className="relative group/addgroup">
                        <button className={`${menuItemClass} justify-between`}>
                            <span className="flex items-center">
                                <Layers className="w-4 h-4 mr-2 text-gray-400" /> Add to Group
                            </span>
                            <ChevronRight className="w-3 h-3 text-gray-400" />
                        </button>
                        <div className="absolute left-full top-0 ml-0.5 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 py-1 opacity-0 invisible group-hover/addgroup:opacity-100 group-hover/addgroup:visible transition-all z-[60]">
                            {groups.map(g => (
                                <button
                                    key={g.id}
                                    className={menuItemClass}
                                    onClick={() => onAddToGroup(file, g.id)}
                                >
                                    <div className="w-3 h-3 rounded-full mr-2" style={{ backgroundColor: g.color || '#3B82F6' }} />
                                    <span className="truncate">{g.name}</span>
                                </button>
                            ))}
                        </div>
                    </div>
                )}

                {/* Remove from Group - When viewing inside a group */}
                {file.type !== 'folder' && file.type !== 'group' && isInsideGroup && onRemoveFromGroup && (
                    <button className={menuItemClass} onClick={() => onRemoveFromGroup(file)}>
                        <FolderMinus className="w-4 h-4 mr-2 text-orange-400" /> Remove from Group
                    </button>
                )}

                {/* Toggle Company Folder - Folders only, Admin+ */}
                {file.type === 'folder' && onToggleCompanyFolder && canLockFiles && (
                    <button className={`${menuItemClass} whitespace-nowrap`} onClick={() => onToggleCompanyFolder(file)}>
                        <Building2 className={clsx("w-4 h-4 mr-2 flex-shrink-0", file.is_company_folder ? "text-blue-500" : "text-gray-400")} />
                        {file.is_company_folder ? "Unset Company" : "Set as Company"}
                    </button>
                )}

                {/* Properties - requires access to locked files */}
                {canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onProperties(file)}>
                        <Info className="w-4 h-4 mr-2 text-gray-400" /> Properties
                    </button>
                )}

                <div className={dividerClass}></div>

                {/* Delete - Only owner or Admin/SuperAdmin, not locked or user can unlock */}
                {canDelete && (!file.is_locked || canAccessLockedFile) && (
                    <button
                        className="flex items-center w-full px-4 py-2 text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
                        onClick={() => onDelete(file)}
                    >
                        <Trash2 className="w-4 h-4 mr-2" /> Delete
                    </button>
                )}

                {/* Locked file message - shown to users who can't access */}
                {file.is_locked && !canAccessLockedFile && (
                    <div className="px-4 py-2 text-xs text-red-400 dark:text-red-500 italic">
                        File is locked - access denied
                    </div>
                )}
            </div>
        </div>
    );

    // If buttonRef is provided, use portal for fixed positioning
    // Otherwise fall back to absolute positioning (backwards compatible)
    if (buttonRef) {
        return createPortal(menuContent, document.body);
    }

    // Fallback to original absolute positioning
    return (
        <div className="absolute right-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-md shadow-xl z-20 border border-gray-100 dark:border-gray-700 ring-1 ring-black ring-opacity-5 text-left">
            <div className="py-1">
                {/* Preview - Files only, requires access to locked files */}
                {isFile && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onPreview(file)}>
                        <Eye className="w-4 h-4 mr-2 text-gray-400" /> Preview
                    </button>
                )}

                {/* Share - Only if user can share AND can access locked file */}
                {canShare && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onShare(file)}>
                        <Share2 className="w-4 h-4 mr-2 text-gray-400" /> Share
                    </button>
                )}

                {/* Star - requires access to locked files */}
                {canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onStar(file)}>
                        <Star className={clsx("w-4 h-4 mr-2", file.is_starred ? "text-yellow-400 fill-current" : "text-gray-400")} />
                        {file.is_starred ? "Unstar" : "Star"}
                    </button>
                )}

                {/* Download - requires access to locked files */}
                {canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onDownload(file)}>
                        <Download className="w-4 h-4 mr-2 text-gray-400" /> Download
                    </button>
                )}

                {/* AI Actions - Only for text files when AI is enabled */}
                {showAiActions && onAiSummarize && (
                    <button className={menuItemClass} onClick={() => onAiSummarize(file)}>
                        <Sparkles className="w-4 h-4 mr-2 text-purple-500" /> Summarize
                    </button>
                )}
                {showAiActions && onAiQuestion && (
                    <button className={menuItemClass} onClick={() => onAiQuestion(file)}>
                        <MessageSquare className="w-4 h-4 mr-2 text-purple-500" /> Ask AI
                    </button>
                )}

                {/* Rename - requires access to locked files */}
                {canAccessLockedFile && !file.is_locked && (
                    <button className={menuItemClass} onClick={() => onRename(file)}>
                        <Edit2 className="w-4 h-4 mr-2 text-gray-400" /> Rename
                    </button>
                )}

                <div className={dividerClass}></div>

                {/* Lock/Unlock - Only for Manager, Admin, SuperAdmin who can access the file */}
                {canLockFiles && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onLock(file)}>
                        {file.is_locked ? (
                            <>
                                <Unlock className="w-4 h-4 mr-2 text-green-500" /> Unlock
                            </>
                        ) : (
                            <>
                                <Lock className="w-4 h-4 mr-2 text-orange-500" /> Lock
                            </>
                        )}
                    </button>
                )}

                {/* Recent Activity - Only for Admin, SuperAdmin */}
                {canViewActivity && (
                    <button className={menuItemClass} onClick={() => onActivity(file)}>
                        <History className="w-4 h-4 mr-2 text-gray-400" /> Recent Activity
                    </button>
                )}

                {/* Move To - Only if not locked and user can access */}
                {!file.is_locked && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onMove(file)}>
                        <Move className="w-4 h-4 mr-2 text-gray-400" /> Move To...
                    </button>
                )}

                {/* Copy - Only for files (not folders/groups), requires access to locked files */}
                {file.type !== 'folder' && file.type !== 'group' && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onCopy(file)}>
                        <Copy className="w-4 h-4 mr-2 text-gray-400" /> Copy
                    </button>
                )}

                {/* Create Group from File - Only for files */}
                {file.type !== 'folder' && file.type !== 'group' && onCreateGroupFromFile && canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onCreateGroupFromFile(file)}>
                        <Plus className="w-4 h-4 mr-2 text-green-500" /> Create Group...
                    </button>
                )}

                {/* Add to Group - Only for files, when groups are available */}
                {file.type !== 'folder' && file.type !== 'group' && onAddToGroup && groups.length > 0 && canAccessLockedFile && (
                    <div className="relative group/addgroup2">
                        <button className={`${menuItemClass} justify-between`}>
                            <span className="flex items-center">
                                <Layers className="w-4 h-4 mr-2 text-gray-400" /> Add to Group
                            </span>
                            <ChevronRight className="w-3 h-3 text-gray-400" />
                        </button>
                        <div className="absolute left-full top-0 ml-0.5 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 py-1 opacity-0 invisible group-hover/addgroup2:opacity-100 group-hover/addgroup2:visible transition-all z-[60]">
                            {groups.map(g => (
                                <button
                                    key={g.id}
                                    className={menuItemClass}
                                    onClick={() => onAddToGroup(file, g.id)}
                                >
                                    <div className="w-3 h-3 rounded-full mr-2" style={{ backgroundColor: g.color || '#3B82F6' }} />
                                    <span className="truncate">{g.name}</span>
                                </button>
                            ))}
                        </div>
                    </div>
                )}

                {/* Remove from Group - When viewing inside a group */}
                {file.type !== 'folder' && file.type !== 'group' && isInsideGroup && onRemoveFromGroup && (
                    <button className={menuItemClass} onClick={() => onRemoveFromGroup(file)}>
                        <FolderMinus className="w-4 h-4 mr-2 text-orange-400" /> Remove from Group
                    </button>
                )}

                {/* Toggle Company Folder - Folders only, Admin+ */}
                {file.type === 'folder' && onToggleCompanyFolder && canLockFiles && (
                    <button className={menuItemClass} onClick={() => onToggleCompanyFolder(file)}>
                        <Building2 className={clsx("w-4 h-4 mr-2", file.is_company_folder ? "text-blue-500" : "text-gray-400")} />
                        {file.is_company_folder ? "Remove Company Folder" : "Mark as Company Folder"}
                    </button>
                )}

                {/* Properties - requires access to locked files */}
                {canAccessLockedFile && (
                    <button className={menuItemClass} onClick={() => onProperties(file)}>
                        <Info className="w-4 h-4 mr-2 text-gray-400" /> Properties
                    </button>
                )}

                <div className={dividerClass}></div>

                {/* Delete - Only owner or Admin/SuperAdmin, not locked or user can unlock */}
                {canDelete && (!file.is_locked || canAccessLockedFile) && (
                    <button
                        className="flex items-center w-full px-4 py-2 text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
                        onClick={() => onDelete(file)}
                    >
                        <Trash2 className="w-4 h-4 mr-2" /> Delete
                    </button>
                )}

                {/* Locked file message - shown to users who can't access */}
                {file.is_locked && !canAccessLockedFile && (
                    <div className="px-4 py-2 text-xs text-red-400 dark:text-red-500 italic">
                        File is locked - access denied
                    </div>
                )}
            </div>
        </div>
    );
}
