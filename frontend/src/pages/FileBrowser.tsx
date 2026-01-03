import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import {
    Folder, FileText, Image as ImageIcon, MoreVertical, Download,
    Trash2, Eye, EyeOff, Upload, Grid, List, Search, Plus, Star, Clock,
    FolderPlus, Edit2, Edit3, Link as LinkIcon, ChevronLeft, ChevronRight, ChevronDown,
    Lock, Unlock, History, FileOutput, Move, Home, Users, CheckSquare, Square, X,
    Building2, MoreHorizontal, Clipboard, Layers
} from 'lucide-react';
import clsx from 'clsx';
import { CreateFileRequestModal, FileRequestData } from '../components/CreateFileRequestModal';
import { UploadProgressModal, UploadFile } from '../components/UploadProgressModal';
import { FilePreviewModal } from '../components/FilePreviewModal';
import { RenameModal } from '../components/RenameModal';
import { NewFolderModal } from '../components/NewFolderModal';
import { FileActivityModal } from '../components/FileActivityModal';
import { LockFileModal, UnlockFileModal } from '../components/LockFileModal';
import { MoveFileModal } from '../components/MoveFileModal';
import { FileActionMenu } from '../components/FileActionMenu';
import { FilePropertiesModal } from '../components/FilePropertiesModal';
import { ShareFileModal } from '../components/ShareFileModal';
import { AiSummaryModal } from '../components/AiSummaryModal';
import { AiQuestionModal } from '../components/AiQuestionModal';
import { CreateGroupModal } from '../components/CreateGroupModal';
import { FileGroupStack } from '../components/FileGroupStack';
import { FileGroupViewer } from '../components/FileGroupViewer';
import { Avatar } from '../components/Avatar';
import { useTenant } from '../context/TenantContext';
import { useAuth, useAuthFetch } from '../context/AuthContext';
import { useKeyboardShortcuts, Shortcut } from '../hooks/useKeyboardShortcuts';
import { useKeyboardShortcutsContext } from '../context/KeyboardShortcutsContext';
import { ShortcutActionId } from '../hooks/shortcutPresets';

interface FileItem {
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
    group_id?: string;
    // Group-specific fields
    color?: string;
    file_count?: number;
    total_size?: number; // Total size in bytes for groups
    parent_path?: string; // For files inside groups
}

interface FileGroup {
    id: string;
    tenant_id: string;
    department_id?: string;
    name: string;
    description?: string;
    color?: string;
    icon?: string;
    created_by: string;
    created_at: string;
    updated_at: string;
    file_count: number;
    total_size: number; // Total size in bytes
    owner_name?: string;
    parent_path?: string; // Folder path where this group lives (null/empty = root)
    // Locking fields
    is_locked?: boolean;
    locked_by?: string;
    locked_at?: string;
    lock_requires_role?: string;
}

interface UserPrefs {
    starred: string[];
    settings: any;
}

export function FileBrowser() {
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
    const [currentPath, setCurrentPath] = useState<string[]>(['Home']);
    const [files, setFiles] = useState<FileItem[]>([]);
    const [starredFiles, setStarredFiles] = useState<string[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [activeMenu, setActiveMenu] = useState<string | null>(null);
    const [activeGroupMenu, setActiveGroupMenu] = useState<string | null>(null);
    const [showMoreStarred, setShowMoreStarred] = useState(false);
    const [previewFile, setPreviewFile] = useState<{ name: string, url: string, type: any } | null>(null);
    const [isRequestModalOpen, setIsRequestModalOpen] = useState(false);
    const [isUploadModalOpen, setIsUploadModalOpen] = useState(false);
    const [uploadFilesList, setUploadFilesList] = useState<UploadFile[]>([]);
    const [isDragging, setIsDragging] = useState(false);
    
    // File view mode: 'department' or 'private'
    const [fileViewMode, setFileViewMode] = useState<'department' | 'private'>('department');
    const [isViewModeOpen, setIsViewModeOpen] = useState(false);
    const viewModeRef = useRef<HTMLDivElement>(null);
    
    // Mobile overflow menu
    const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
    const mobileMenuRef = useRef<HTMLDivElement>(null);
    
    // Department filtering for admins
    const [departments, setDepartments] = useState<{id: string, name: string}[]>([]);
    const [selectedDepartment, setSelectedDepartment] = useState<string | null>(null);

    // File Groups
    const [groups, setGroups] = useState<FileGroup[]>([]);
    const [currentGroup, setCurrentGroup] = useState<FileGroup | null>(null); // When viewing inside a group
    const [isCreateGroupOpen, setIsCreateGroupOpen] = useState(false);
    const [pendingGroupFile, setPendingGroupFile] = useState<FileItem | null>(null); // File to add after group creation
    const [isGroupViewerOpen, setIsGroupViewerOpen] = useState(false);
    const [isGroupViewerMinimized, setIsGroupViewerMinimized] = useState(false);
    const [viewingGroup, setViewingGroup] = useState<FileGroup | null>(null);
    const [groupFiles, setGroupFiles] = useState<any[]>([]); // Files in the currently viewed group
    const [isLoadingGroupFiles, setIsLoadingGroupFiles] = useState(false);

    // Modals
    const [isRenameOpen, setIsRenameOpen] = useState(false);
    const [fileToRename, setFileToRename] = useState<FileItem | null>(null);
    const [isNewFolderOpen, setIsNewFolderOpen] = useState(false);
    const [isActivityModalOpen, setIsActivityModalOpen] = useState(false);
    const [activityFile, setActivityFile] = useState<FileItem | null>(null);
    
    // Lock modals
    const [isLockModalOpen, setIsLockModalOpen] = useState(false);
    const [isUnlockModalOpen, setIsUnlockModalOpen] = useState(false);
    const [lockingFile, setLockingFile] = useState<FileItem | null>(null);
    const [isLocking, setIsLocking] = useState(false);
    
    // Move modal
    const [isMoveModalOpen, setIsMoveModalOpen] = useState(false);
    const [movingFile, setMovingFile] = useState<FileItem | null>(null);
    const [isMoving, setIsMoving] = useState(false);
    
    // Properties modal
    const [isPropertiesModalOpen, setIsPropertiesModalOpen] = useState(false);
    const [propertiesFile, setPropertiesFile] = useState<FileItem | null>(null);
    
    // Share modal
    const [isShareModalOpen, setIsShareModalOpen] = useState(false);
    const [shareFile, setShareFile] = useState<FileItem | null>(null);
    
    // AI modals
    const [isAiSummaryModalOpen, setIsAiSummaryModalOpen] = useState(false);
    const [isAiQuestionModalOpen, setIsAiQuestionModalOpen] = useState(false);
    const [aiFile, setAiFile] = useState<FileItem | null>(null);
    const [aiStatus, setAiStatus] = useState<{ enabled: boolean; hasAccess: boolean }>({ enabled: false, hasAccess: false });
    
    // Drop target state for move
    const [dropTargetId, setDropTargetId] = useState<string | null>(null);
    
    // Bulk selection state
    const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
    const [isSelectionMode, setIsSelectionMode] = useState(false);
    const [isBulkMoveModalOpen, setIsBulkMoveModalOpen] = useState(false);
    const [isBulkMoving, setIsBulkMoving] = useState(false);

    // Clipboard state for copy/paste
    const [clipboardFile, setClipboardFile] = useState<FileItem | null>(null);
    const [isPasting, setIsPasting] = useState(false);

    // Keyboard navigation state
    const [focusedFileIndex, setFocusedFileIndex] = useState<number>(-1);

    // Dynamic items per page based on screen width
    const [itemsPerPage, setItemsPerPage] = useState(24);

    const fileInputRef = useRef<HTMLInputElement>(null);
    const starredDropdownRef = useRef<HTMLDivElement>(null);
    const menuButtonRefs = useRef<Map<string, HTMLButtonElement>>(new Map());
    const { currentCompany } = useTenant();
    const { user } = useAuth();
    const companyId = currentCompany?.id;
    const [searchParams] = useSearchParams();
    const urlPath = searchParams.get('path');

    // Initialize path from URL query parameter (for search result navigation)
    useEffect(() => {
        if (urlPath) {
            // Convert "Work Projects/subfolder" to ["Home", "Work Projects", "subfolder"]
            const pathParts = urlPath.split('/').filter(p => p);
            setCurrentPath(['Home', ...pathParts]);
        }
    }, [urlPath]);

    // Calculate items per page based on screen width for responsive pagination
    useEffect(() => {
        const calculateItemsPerPage = () => {
            const width = window.innerWidth;
            if (width >= 3200) return 64;      // 16 cols x 4 rows
            if (width >= 2800) return 56;      // 14 cols x 4 rows
            if (width >= 2200) return 48;      // 12 cols x 4 rows
            if (width >= 1800) return 40;      // 10 cols x 4 rows
            if (width >= 1536) return 32;      // 8 cols x 4 rows (2xl)
            if (width >= 1280) return 24;      // 6 cols x 4 rows (xl)
            return 24;                          // default
        };
        
        setItemsPerPage(calculateItemsPerPage());
        
        const handleResize = () => setItemsPerPage(calculateItemsPerPage());
        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, []);

    // Fetch departments for filtering (admins only)
    useEffect(() => {
        if (companyId && (user?.role === 'Admin' || user?.role === 'SuperAdmin')) {
            authFetch('/api/departments')
                .then(res => res.ok ? res.json() : [])
                .then(data => setDepartments(data))
                .catch(() => setDepartments([]));
        }
    }, [companyId, user?.role]);
    
    // Fetch AI status for the tenant
    useEffect(() => {
        if (companyId) {
            authFetch('/api/ai/status')
                .then(res => res.ok ? res.json() : { enabled: false, has_access: false })
                .then(data => setAiStatus({ enabled: data.enabled, hasAccess: data.has_access }))
                .catch(() => setAiStatus({ enabled: false, hasAccess: false }));
        }
    }, [companyId]);

    // Fetch files on mount, path change, view mode change, department filter change, or group change
    useEffect(() => {
        if (companyId) {
            fetchFiles();
        }
        // Clear selection when path or view mode changes
        setSelectedFiles(new Set());
        setIsSelectionMode(false);
    }, [companyId, currentPath, fileViewMode, selectedDepartment, currentGroup]);

    // Close menus when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            setActiveMenu(null);
            setActiveGroupMenu(null);
            if (viewModeRef.current && !viewModeRef.current.contains(event.target as Node)) {
                setIsViewModeOpen(false);
            }
            if (starredDropdownRef.current && !starredDropdownRef.current.contains(event.target as Node)) {
                setShowMoreStarred(false);
            }
            if (mobileMenuRef.current && !mobileMenuRef.current.contains(event.target as Node)) {
                setIsMobileMenuOpen(false);
            }
        };
        document.addEventListener('click', handleClickOutside);
        return () => document.removeEventListener('click', handleClickOutside);
    }, []);

    const authFetch = useAuthFetch();

    // Selection helpers
    const toggleFileSelection = (fileId: string) => {
        setSelectedFiles(prev => {
            const newSet = new Set(prev);
            if (newSet.has(fileId)) {
                newSet.delete(fileId);
            } else {
                newSet.add(fileId);
            }
            return newSet;
        });
    };

    const selectAllFiles = () => {
        setSelectedFiles(new Set(files.map(f => f.id)));
    };

    const clearSelection = () => {
        setSelectedFiles(new Set());
        setIsSelectionMode(false);
    };

    // Bulk move handler
    const handleBulkMove = async (targetParentId: string | null, targetDepartmentId: string | null, targetVisibility: string = 'department', _newName?: string) => {
        if (selectedFiles.size === 0 || !companyId) return { success: false, error: 'No files selected' };
        
        // Get only files user has permission to move
        const filesToMove = getSelectedFilesForAction('move');
        
        if (filesToMove.length === 0) {
            setIsBulkMoveModalOpen(false);
            return { success: false, error: 'Cannot move any of the selected files. Locked files cannot be moved.' };
        }
        
        setIsBulkMoving(true);
        
        try {
            let successCount = 0;
            let errorCount = 0;
            let duplicateCount = 0;
            
            for (const file of filesToMove) {
                try {
                    const response = await authFetch(`/api/files/${companyId}/${file.id}/move`, {
                        method: 'PUT',
                        body: JSON.stringify({
                            target_parent_id: targetParentId,
                            target_department_id: targetDepartmentId,
                            target_visibility: targetVisibility
                        })
                    });
                    
                    const result = await response.json();
                    if (response.ok && !result.error) {
                        successCount++;
                    } else if (result.duplicate) {
                        duplicateCount++;
                    } else {
                        errorCount++;
                    }
                } catch {
                    errorCount++;
                }
            }
            
            const skippedCount = selectedFiles.size - filesToMove.length;
            if (errorCount > 0 || skippedCount > 0 || duplicateCount > 0) {
                let message = `Moved ${successCount} file(s).`;
                if (duplicateCount > 0) message += ` ${duplicateCount} skipped (duplicate names).`;
                if (errorCount > 0) message += ` ${errorCount} failed.`;
                if (skippedCount > 0) message += ` ${skippedCount} skipped (locked).`;
                return { success: successCount > 0, error: message };
            }
            
            fetchFiles();
            clearSelection();
            setIsBulkMoveModalOpen(false);
            return { success: true };
        } catch (err) {
            return { success: false, error: 'Failed to move files' };
        } finally {
            setIsBulkMoving(false);
        }
    };

    // Bulk delete handler
    const handleBulkDelete = async () => {
        if (selectedFiles.size === 0 || !companyId) return;
        
        // Get only files user has permission to delete
        const filesToDelete = getSelectedFilesForAction('delete');
        
        if (filesToDelete.length === 0) {
            alert('You do not have permission to delete any of the selected files. Only file owners, Admins, and SuperAdmins can delete files.');
            return;
        }
        
        const skippedCount = selectedFiles.size - filesToDelete.length;
        let confirmMessage = `Are you sure you want to move ${filesToDelete.length} item(s) to the Recycle Bin?`;
        if (skippedCount > 0) {
            confirmMessage += `\n\n${skippedCount} item(s) will be skipped (locked or no permission).`;
        }
        
        if (!confirm(confirmMessage)) {
            return;
        }
        
        let successCount = 0;
        
        for (const file of filesToDelete) {
            const currentPathStr = currentPath.slice(1).join('/');
            const fullPath = currentPathStr ? `${currentPathStr}/${file.name}` : file.name;
            
            try {
                const response = await authFetch(`/api/files/${companyId}/delete`, {
                    method: 'POST',
                    body: JSON.stringify({ path: fullPath })
                });
                
                if (response.ok) {
                    successCount++;
                }
            } catch {
                // Continue with other files
            }
        }
        
        fetchFiles();
        clearSelection();
    };

    const fetchFiles = async () => {
        if (!companyId) {
            console.warn('fetchFiles called without companyId');
            return;
        }
        try {
            // If viewing inside a group, fetch files from group endpoint
            if (currentGroup) {
                const groupFilesRes = await authFetch(`/api/groups/${companyId}/${currentGroup.id}/files`);
                if (groupFilesRes.ok) {
                    const data = await groupFilesRes.json();
                    const groupFiles = (data.files || []).map((f: any) => {
                        const extension = f.name.split('.').pop()?.toLowerCase();
                        let type = 'document';
                        if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(extension)) type = 'image';
                        else if (['mp4', 'webm', 'mov'].includes(extension)) type = 'video';
                        else if (['mp3', 'wav', 'ogg'].includes(extension)) type = 'audio';
                        return { ...f, type, owner: f.owner || 'Unknown' };
                    });
                    setFiles(groupFiles);
                }
                setIsLoading(false);
                return;
            }

            // Construct path from currentPath array (skip "Home")
            const path = currentPath.slice(1).join('/');

            // Fetch files with path, visibility, and optional department filter
            const deptParam = selectedDepartment ? `&department_id=${selectedDepartment}` : '';
            const filesRes = await authFetch(`/api/files/${companyId}?path=${encodeURIComponent(path)}&visibility=${fileViewMode}${deptParam}`);
            if (filesRes.ok) {
                const filesData = await filesRes.json();

                // Fetch user's starred files (per-user, access-controlled)
                const starredRes = await authFetch(`/api/files/${companyId}/starred`);
                let starredData: { starred: string[] } = { starred: [] };
                if (starredRes.ok) {
                    starredData = await starredRes.json();
                }
                setStarredFiles(starredData.starred || []);

                // Merge starred status and map types
                const mergedFiles = filesData.map((f: any) => {
                    const extension = f.name.split('.').pop()?.toLowerCase();
                    // Backend already returns correct 'type' field ('folder' for folders)
                    let type = f.type || 'document';

                    if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(extension)) {
                        type = 'image';
                    } else if (['mp4', 'webm', 'mov'].includes(extension)) {
                        type = 'video';
                    } else if (['mp3', 'wav', 'ogg'].includes(extension)) {
                        type = 'audio';
                    }

                    return {
                        ...f,
                        type,
                        is_starred: (starredData.starred || []).includes(f.id)
                    };
                });

                // Fetch groups at the current path level
                if (fileViewMode === 'department') {
                    // Build query params for groups - filter by current path
                    const groupParams = new URLSearchParams();
                    if (selectedDepartment) groupParams.set('department_id', selectedDepartment);
                    groupParams.set('parent_path', path); // Empty string = root
                    
                    const groupsRes = await authFetch(`/api/groups/${companyId}?${groupParams.toString()}`);
                    if (groupsRes.ok) {
                        const groupsData: FileGroup[] = await groupsRes.json();
                        setGroups(groupsData);
                        
                        // Convert groups to FileItem format and prepend to files
                        // Groups can also be starred (use same starred array)
                        const groupItems: FileItem[] = groupsData.map(g => ({
                            id: g.id,
                            name: g.name,
                            type: 'group' as const,
                            modified: g.updated_at,
                            created_at: g.created_at,
                            owner: g.owner_name || 'Unknown',
                            owner_id: g.created_by,
                            color: g.color,
                            file_count: g.file_count,
                            total_size: g.total_size,
                            is_starred: starredData.starred.includes(g.id),
                            // Locking fields
                            is_locked: g.is_locked,
                            locked_by: g.locked_by,
                            lock_requires_role: g.lock_requires_role,
                        }));
                        setFiles([...groupItems, ...mergedFiles]);
                    } else {
                        setFiles(mergedFiles);
                    }
                } else {
                    setFiles(mergedFiles);
                }
            }
        } catch (error) {
            console.error('Error fetching files:', error);
        } finally {
            setIsLoading(false);
        }
    };

    const handleCreateFolder = async (folderName: string) => {
        try {
            const parentPath = currentPath.slice(1).join('/');
            const response = await authFetch(`/api/folders/${companyId}`, {
                method: 'POST',
                body: JSON.stringify({
                    name: folderName,
                    parent_path: parentPath,
                    visibility: fileViewMode  // Auto-set based on current view mode
                }),
            });

            if (response.ok) {
                fetchFiles();
            }
        } catch (error) {
            console.error('Error creating folder:', error);
        }
    };

    // File Groups
    const handleCreateGroup = async (name: string, description: string, color: string): Promise<string | void> => {
        const response = await authFetch(`/api/groups/${companyId}`, {
            method: 'POST',
            body: JSON.stringify({
                name,
                description: description || null,
                color,
                department_id: selectedDepartment || null,
            }),
        });

        if (!response.ok) {
            const data = await response.json().catch(() => ({}));
            if (response.status === 409) {
                throw new Error('A group with this name already exists');
            }
            throw new Error(data.message || 'Failed to create group');
        }

        const created = await response.json();
        const groupId = created.id;

        // If there's a pending file to add to the new group, add it now
        if (pendingGroupFile && groupId) {
            await handleAddToGroup(pendingGroupFile, groupId);
            setPendingGroupFile(null);
        }

        fetchFiles();
        return groupId;
    };
    
    // Handler for "Create Group..." context menu option
    const handleCreateGroupFromFile = (file: FileItem) => {
        setPendingGroupFile(file);
        setIsCreateGroupOpen(true);
    };

    const handleGroupClick = async (group: FileItem) => {
        // SECURITY: Check if user can access locked group
        if (!canAccessLockedGroup(group)) {
            alert(`Group is locked - access denied${group.lock_requires_role ? ` (requires ${group.lock_requires_role} or higher)` : ''}`);
            return;
        }

        // Find the full group object or construct a minimal one
        const fullGroup = groups.find(g => g.id === group.id) || {
            id: group.id,
            tenant_id: '',
            name: group.name,
            color: group.color,
            file_count: group.file_count || 0,
            total_size: group.total_size || 0,
            created_by: '',
            created_at: group.created_at || '',
            updated_at: group.modified || '',
        };
        
        setViewingGroup(fullGroup);
        setIsGroupViewerOpen(true);
        setGroupFiles([]); // Clear previous files
        setIsLoadingGroupFiles(true);
        
        // Fetch files in this group
        try {
            const res = await authFetch(`/api/groups/${companyId}/${group.id}/files`);
            if (res.ok) {
                const data = await res.json();
                // Check if backend returned access denied error
                if (data.error) {
                    alert(data.error);
                    setIsGroupViewerOpen(false);
                    setViewingGroup(null);
                    setIsLoadingGroupFiles(false);
                    return;
                }
                console.log('Group files API response:', data); // Debug logging
                setGroupFiles(data.files || []);
            } else {
                console.error('Failed to fetch group files:', res.status, res.statusText);
                setGroupFiles([]);
            }
        } catch (error) {
            console.error('Error fetching group files:', error);
            setGroupFiles([]);
        } finally {
            setIsLoadingGroupFiles(false);
        }
    };

    const handleExitGroup = () => {
        setCurrentGroup(null);
        setCurrentPath(['Home']);
    };

    // Group action handlers
    const handleDeleteGroup = async (group: FileItem) => {
        if (!confirm(`Delete group "${group.name}"? Files will be unlinked but not deleted.`)) return;
        
        try {
            const res = await authFetch(`/api/groups/${companyId}/${group.id}`, {
                method: 'DELETE',
            });
            if (res.ok) {
                fetchFiles();
            } else {
                alert('Failed to delete group');
            }
        } catch (error) {
            console.error('Error deleting group:', error);
            alert('Failed to delete group');
        }
        setActiveGroupMenu(null);
    };

    // Group lock/unlock handlers
    const handleLockGroup = (group: FileItem) => {
        setActiveGroupMenu(null);
        // Reuse the file lock modal - set the group as the file to lock
        setLockingFile({ ...group, type: 'group' });
        setIsLockModalOpen(true);
    };

    const handleUnlockGroup = (group: FileItem) => {
        setActiveGroupMenu(null);
        // Reuse the file unlock modal - set the group as the file to unlock
        setLockingFile({ ...group, type: 'group' });
        setIsUnlockModalOpen(true);
    };

    const handleRenameGroup = async (group: FileItem) => {
        const newName = prompt('Enter new group name:', group.name);
        if (!newName || newName === group.name) return;
        
        try {
            const res = await authFetch(`/api/groups/${companyId}/${group.id}`, {
                method: 'PUT',
                body: JSON.stringify({ name: newName }),
            });
            if (res.ok) {
                fetchFiles();
            } else {
                alert('Failed to rename group');
            }
        } catch (error) {
            console.error('Error renaming group:', error);
            alert('Failed to rename group');
        }
        setActiveGroupMenu(null);
    };

    // State and handler for moving groups
    const [groupToMove, setGroupToMove] = useState<FileItem | null>(null);
    const [isMoveGroupModalOpen, setIsMoveGroupModalOpen] = useState(false);

    const handleMoveGroup = (group: FileItem) => {
        setGroupToMove(group);
        setIsMoveGroupModalOpen(true);
        setActiveGroupMenu(null);
    };

    const handleMoveGroupConfirm = async (
        targetParentId: string | null, 
        _targetDepartmentId: string | null, 
        _targetVisibility: string, 
        _newName?: string
    ): Promise<{ success: boolean; error?: string; duplicate?: boolean; conflicting_name?: string; suggested_name?: string }> => {
        if (!groupToMove) return { success: false, error: 'No group selected' };
        
        console.log('Moving group:', groupToMove.id, 'to folder:', targetParentId);
        
        try {
            const response = await authFetch(`/api/groups/${companyId}/${groupToMove.id}/move`, {
                method: 'PUT',
                body: JSON.stringify({ 
                    target_folder_id: targetParentId,
                }),
            });

            const result = await response.json();
            console.log('Move group response:', response.status, result);
            
            if (!response.ok || result.error) {
                return { success: false, error: result.error || result.message || 'Failed to move group' };
            }

            fetchFiles();
            setIsMoveGroupModalOpen(false);
            setGroupToMove(null);
            return { success: true };
        } catch (error) {
            console.error('Error moving group:', error);
            return { success: false, error: 'Failed to move group' };
        }
    };

    // Handle drag start for groups (includes group info for special handling on drop)
    const handleGroupDragStart = (e: React.DragEvent, group: FileItem) => {
        e.stopPropagation();
        const groupWithType = { ...group, type: 'group' as const };
        setDraggedFile(groupWithType);
        e.dataTransfer.setData('application/json', JSON.stringify({ ...group, isGroup: true }));
        e.dataTransfer.effectAllowed = 'move';
        console.log('Group drag started:', group.name, group.id);
    };

    const handleRemoveFromGroup = async (file: FileItem) => {
        try {
            const response = await authFetch(`/api/files/${companyId}/${file.id}/group`, {
                method: 'DELETE',
            });
            if (response.ok) {
                fetchFiles();
            }
        } catch (error) {
            console.error('Error removing file from group:', error);
        }
        setActiveMenu(null);
    };

    // Handlers for FileGroupViewer
    const handleGroupViewerRemove = async (file: any) => {
        try {
            const response = await authFetch(`/api/files/${companyId}/${file.id}/group`, {
                method: 'DELETE',
            });
            if (response.ok) {
                // Remove from local state
                setGroupFiles(prev => prev.filter(f => f.id !== file.id));
                fetchFiles(); // Refresh main file list
            }
        } catch (error) {
            console.error('Error removing file from group:', error);
        }
    };

    const handleGroupViewerPreview = (file: any) => {
        // Minimize group viewer and set up preview
        setIsGroupViewerMinimized(true);
        
        // Convert MIME type to category type expected by FilePreviewModal
        let fileType: 'image' | 'document' | 'video' | 'audio' | 'folder' = 'document';
        const contentType = file.content_type || '';
        if (contentType.startsWith('image/')) fileType = 'image';
        else if (contentType.startsWith('video/')) fileType = 'video';
        else if (contentType.startsWith('audio/')) fileType = 'audio';
        
        setPreviewFile({
            name: file.name,
            url: `/api/download/${companyId}/${file.id}`,
            type: fileType,
        });
    };

    // Handler for when preview closes - expand group viewer if it was minimized
    const handlePreviewClose = () => {
        setPreviewFile(null);
        restoreGroupViewerIfMinimized();
    };

    // Helper to restore group viewer when any modal closes
    const restoreGroupViewerIfMinimized = () => {
        if (isGroupViewerOpen && isGroupViewerMinimized) {
            setIsGroupViewerMinimized(false);
        }
    };

    // Star is intentionally removed from group viewer - star the group instead

    const handleGroupViewerCopy = (file: any) => {
        // Convert to FileItem and set clipboard
        const fileItem: FileItem = {
            id: file.id,
            name: file.name,
            type: 'document',
            size: file.size_bytes ? `${(file.size_bytes / 1024).toFixed(1)} KB` : '--',
            modified: file.created_at || '',
            owner: '',
            content_type: file.content_type,
            storage_path: file.storage_path,
        };
        setClipboardFile(fileItem);
    };

    const handleGroupViewerShare = (file: any) => {
        // Minimize group viewer and open share modal
        setIsGroupViewerMinimized(true);
        const fileItem: FileItem = {
            id: file.id,
            name: file.name,
            type: 'document',
            size: file.size_bytes ? `${(file.size_bytes / 1024).toFixed(1)} KB` : '--',
            modified: file.created_at || '',
            owner: '',
        };
        setShareFile(fileItem);
        setIsShareModalOpen(true);
    };

    const handleGroupViewerProperties = (file: any) => {
        // Minimize group viewer and open properties modal
        setIsGroupViewerMinimized(true);
        const fileItem: FileItem = {
            id: file.id,
            name: file.name,
            type: 'document',
            size: file.size_bytes ? `${(file.size_bytes / 1024).toFixed(1)} KB` : '--',
            size_bytes: file.size_bytes,
            modified: file.created_at || '',
            created_at: file.created_at,
            owner: '',
            content_type: file.content_type,
        };
        setPropertiesFile(fileItem);
        setIsPropertiesModalOpen(true);
    };

    const handleGroupViewerAiSummarize = (file: any) => {
        // Minimize group viewer and open AI summary modal
        setIsGroupViewerMinimized(true);
        const fileItem: FileItem = {
            id: file.id,
            name: file.name,
            type: 'document',
            size: file.size_bytes ? `${(file.size_bytes / 1024).toFixed(1)} KB` : '--',
            modified: file.created_at || '',
            owner: '',
        };
        setAiFile(fileItem);
        setIsAiSummaryModalOpen(true);
    };

    const handleGroupViewerAiQuestion = (file: any) => {
        // Minimize group viewer and open AI question modal
        setIsGroupViewerMinimized(true);
        const fileItem: FileItem = {
            id: file.id,
            name: file.name,
            type: 'document',
            size: file.size_bytes ? `${(file.size_bytes / 1024).toFixed(1)} KB` : '--',
            modified: file.created_at || '',
            owner: '',
        };
        setAiFile(fileItem);
        setIsAiQuestionModalOpen(true);
    };

    const handleGroupViewerDownload = async (file: any) => {
        try {
            const response = await authFetch(`/api/download/${companyId}/${file.id}`);
            if (response.ok) {
                const blob = await response.blob();
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = file.name;
                document.body.appendChild(a);
                a.click();
                document.body.removeChild(a);
                URL.revokeObjectURL(url);
            }
        } catch (error) {
            console.error('Error downloading file:', error);
        }
    };

    const handleGroupViewerMoveToFolder = (file: any) => {
        console.log('handleGroupViewerMoveToFolder called with file:', file);
        // Minimize group viewer and open move modal
        setIsGroupViewerMinimized(true);
        const fileItem: FileItem = {
            id: file.id,
            name: file.name,
            type: 'document',
            size: file.size_bytes ? `${(file.size_bytes / 1024).toFixed(1)} KB` : '--',
            modified: file.created_at || '',
            owner: '',
            visibility: file.visibility || 'department',
            parent_path: file.parent_path || '',
        };
        console.log('Setting movingFile to:', fileItem);
        console.log('Opening move modal, current state - isMoveModalOpen:', isMoveModalOpen);
        setMovingFile(fileItem);
        setIsMoveModalOpen(true);
    };

    const handleAddToGroup = async (file: FileItem, groupId: string) => {
        try {
            const response = await authFetch(`/api/files/${companyId}/${file.id}/group`, {
                method: 'POST',
                body: JSON.stringify({ group_id: groupId }),
            });
            if (response.ok) {
                fetchFiles();
            }
        } catch (error) {
            console.error('Error adding file to group:', error);
        }
        setActiveMenu(null);
    };

    const handleCreateFileRequest = async (data: FileRequestData) => {
        try {
            const payload = {
                name: data.name,
                destination_path: data.destination_path,
                expires_in_days: Number(data.expires_in_days),
                visibility: data.visibility,
                ...(data.department_id ? { department_id: data.department_id } : {}),
                ...(data.max_uploads ? { max_uploads: Number(data.max_uploads) } : {}),
            };
            console.log('File request payload:', JSON.stringify(payload, null, 2));
            const response = await authFetch('/api/file-requests', {
                method: 'POST',
                body: JSON.stringify(payload),
            });

            if (response.ok) {
                setIsRequestModalOpen(false);
            } else {
                const errorData = await response.json();
                throw new Error(errorData.error || 'Failed to create file request');
            }
        } catch (error) {
            console.error('Error creating file request:', error);
            // throw error; // Don't throw, just log so modal doesn't crash app
        }
    };
    const handleRename = async (newName: string) => {
        if (!fileToRename) return;
        
        // Check if file is locked
        if (fileToRename.is_locked) {
            alert('Cannot rename a locked file. Please unlock it first.');
            return;
        }
        
        try {
            const response = await authFetch(`/api/files/${companyId}/rename`, {
                method: 'POST',
                body: JSON.stringify({ 
                    old_name: fileToRename.name, 
                    new_name: newName,
                    parent_path: currentPath.slice(1).join('/') // Remove "Home", join rest
                }),
            });

            if (response.ok) {
                fetchFiles();
            }
        } catch (error) {
            console.error('Error renaming file:', error);
        }
    };

    const handleDelete = async (file: FileItem) => {
        if (!confirm(`Are you sure you want to move "${file.name}" to the Recycle Bin?`)) return;

        // Construct full path
        const currentPathStr = currentPath.slice(1).join('/');
        const fullPath = currentPathStr ? `${currentPathStr}/${file.name}` : file.name;

        try {
            const response = await authFetch(`/api/files/${companyId}/delete`, {
                method: 'POST',
                body: JSON.stringify({ path: fullPath })
            });

            if (response.ok) {
                fetchFiles();
            }
        } catch (error) {
            console.error('Error deleting file:', error);
        }
    };

    const toggleStar = async (file: FileItem) => {
        // Optimistic update
        const wasStarred = starredFiles.includes(file.id);
        const newStarred = wasStarred
            ? starredFiles.filter(id => id !== file.id)
            : [...starredFiles, file.id];

        setStarredFiles(newStarred);
        setFiles(files.map(f => f.id === file.id ? { ...f, is_starred: !f.is_starred } : f));

        try {
            // Groups use a different endpoint since they're not in files_metadata
            const endpoint = file.type === 'group' 
                ? `/api/groups/${companyId}/${file.id}/star`
                : `/api/files/${companyId}/${file.id}/star`;
            
            const response = await authFetch(endpoint, {
                method: 'POST',
            });
            
            if (!response.ok) {
                // Revert optimistic update on failure
                setStarredFiles(wasStarred ? [...starredFiles] : starredFiles.filter(id => id !== file.id));
                setFiles(files.map(f => f.id === file.id ? { ...f, is_starred: wasStarred } : f));
                console.error('Failed to toggle star - access denied');
            }
        } catch (error) {
            console.error('Error updating star:', error);
            fetchFiles(); // Revert on error
        }
    };

    // Check if user can access a locked file
    const canAccessLockedFile = (file: FileItem) => {
        if (!file.is_locked) return true;
        const isOwner = user?.id === file.owner_id;
        const isLocker = user?.id === file.locked_by;
        
        // Role hierarchy check - must meet or exceed the required role level
        const roleLevel = (role: string) => {
            switch (role) {
                case 'SuperAdmin': return 100;
                case 'Admin': return 80;
                case 'Manager': return 60;
                case 'Employee': return 40;
                default: return 20;
            }
        };
        
        const userLevel = roleLevel(user?.role || '');
        const requiredLevel = file.lock_requires_role ? roleLevel(file.lock_requires_role) : 100;
        const hasRequiredRole = userLevel >= requiredLevel;
        
        return isLocker || isOwner || hasRequiredRole;
    };

    // Check if user can access a locked group
    const canAccessLockedGroup = (group: FileItem) => {
        if (!group.is_locked) return true;
        const isOwner = user?.id === group.owner_id;
        const isLocker = user?.id === group.locked_by;
        
        // Role hierarchy check - must meet or exceed the required role level
        const roleLevel = (role: string) => {
            switch (role) {
                case 'SuperAdmin': return 100;
                case 'Admin': return 80;
                case 'Manager': return 60;
                case 'Employee': return 40;
                default: return 20;
            }
        };
        
        const userLevel = roleLevel(user?.role || '');
        const requiredLevel = group.lock_requires_role ? roleLevel(group.lock_requires_role) : 100;
        const hasRequiredRole = userLevel >= requiredLevel;
        
        return isLocker || isOwner || hasRequiredRole;
    };

    const handlePreview = (file: FileItem) => {
        // SECURITY: Check if user can access locked file before preview
        if (!canAccessLockedFile(file)) {
            // Show a toast or alert that file is locked
            console.warn('Cannot preview locked file - access denied');
            return;
        }
        setPreviewFile({
            name: file.name,
            url: `/api/download/${companyId}/${file.id}`,
            type: file.type
        });
    };

    const handleShare = (file: FileItem) => {
        setShareFile(file);
        setIsShareModalOpen(true);
        setActiveMenu(null);
    };
    
    // AI handlers
    const handleAiSummarize = (file: FileItem) => {
        setAiFile(file);
        setIsAiSummaryModalOpen(true);
        setActiveMenu(null);
    };
    
    const handleAiQuestion = (file: FileItem) => {
        setAiFile(file);
        setIsAiQuestionModalOpen(true);
        setActiveMenu(null);
    };

    const handleDownload = async (file: FileItem) => {
        if (!companyId) return;
        
        // SECURITY: Check if user can access locked file before download
        if (!canAccessLockedFile(file)) {
            console.warn('Cannot download locked file - access denied');
            return;
        }
        
        try {
            // SECURITY: Use header-based auth instead of token-in-URL
            // Folders are automatically downloaded as zip archives
            const response = await authFetch(`/api/download/${companyId}/${file.id}`);
            if (!response.ok) {
                throw new Error('Download failed');
            }
            const blob = await response.blob();
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            // For folders, add .zip extension
            const downloadName = file.type === 'folder' ? `${file.name}.zip` : file.name;
            a.download = downloadName;
            document.body.appendChild(a);
            a.click();
            window.URL.revokeObjectURL(url);
            document.body.removeChild(a);
        } catch (error) {
            console.error('Download error:', error);
            alert('Failed to download ' + (file.type === 'folder' ? 'folder' : 'file'));
        }
        setActiveMenu(null);
    };

    const handleLockToggle = (file: FileItem) => {
        setLockingFile(file);
        setActiveMenu(null);
        if (file.is_locked) {
            setIsUnlockModalOpen(true);
        } else {
            setIsLockModalOpen(true);
        }
    };

    const handleLockFile = async (password: string | null, requiredRole: string | null) => {
        if (!lockingFile || !companyId) return;
        setIsLocking(true);
        try {
            // Use group API for groups, file API for files
            const apiPath = lockingFile.type === 'group'
                ? `/api/groups/${companyId}/${lockingFile.id}/lock`
                : `/api/files/${companyId}/${lockingFile.id}/lock`;

            const response = await authFetch(apiPath, {
                method: 'POST',
                body: JSON.stringify({
                    password: password,
                    required_role: requiredRole
                })
            });

            if (response.ok) {
                fetchFiles();
                setIsLockModalOpen(false);
                setLockingFile(null);
            } else {
                const error = await response.json();
                throw new Error(error.error || `Failed to lock ${lockingFile.type === 'group' ? 'group' : 'file'}`);
            }
        } finally {
            setIsLocking(false);
        }
    };

    const handleUnlockFile = async (password: string | null): Promise<{ error?: string; requires_password?: boolean }> => {
        if (!lockingFile || !companyId) return { error: 'No file selected' };
        setIsLocking(true);
        try {
            // Use group API for groups, file API for files
            const apiPath = lockingFile.type === 'group'
                ? `/api/groups/${companyId}/${lockingFile.id}/unlock`
                : `/api/files/${companyId}/${lockingFile.id}/unlock`;

            const response = await authFetch(apiPath, {
                method: 'POST',
                body: JSON.stringify({ password })
            });

            const result = await response.json();
            
            if (response.ok && !result.error) {
                fetchFiles();
                setIsUnlockModalOpen(false);
                setLockingFile(null);
                return {};
            } else {
                return { 
                    error: result.error || `Failed to unlock ${lockingFile.type === 'group' ? 'group' : 'file'}`,
                    requires_password: result.requires_password
                };
            }
        } finally {
            setIsLocking(false);
        }
    };

    const handleMoveFile = async (targetParentId: string | null, targetDepartmentId: string | null, targetVisibility: string = 'department', newName?: string) => {
        if (!movingFile || !companyId) return { success: false, error: 'No file selected' };
        setIsMoving(true);
        try {
            const response = await authFetch(`/api/files/${companyId}/${movingFile.id}/move`, {
                method: 'PUT',
                body: JSON.stringify({
                    target_parent_id: targetParentId,
                    target_department_id: targetDepartmentId,
                    target_visibility: targetVisibility,
                    new_name: newName || null
                })
            });

            const result = await response.json();
            
            if (!response.ok || result.error) {
                // Return structured result for duplicate handling
                return {
                    success: false,
                    error: result.error || 'Failed to move file',
                    duplicate: result.duplicate || false,
                    conflicting_name: result.conflicting_name,
                    suggested_name: result.suggested_name
                };
            }
            
            fetchFiles();
            setIsMoveModalOpen(false);
            setMovingFile(null);
            
            // If group viewer is open, refresh group files
            if (isGroupViewerOpen && viewingGroup) {
                try {
                    const res = await authFetch(`/api/groups/${companyId}/${viewingGroup.id}/files`);
                    if (res.ok) {
                        const data = await res.json();
                        setGroupFiles(data.files || []);
                    }
                } catch (e) {
                    console.error('Error refreshing group files:', e);
                }
            }
            
            return { success: true };
        } catch (err) {
            return { success: false, error: 'Failed to move file' };
        } finally {
            setIsMoving(false);
        }
    };

    const openMoveModal = (file: FileItem) => {
        setMovingFile(file);
        setIsMoveModalOpen(true);
        setActiveMenu(null);
    };

    // Drag and drop handlers
    const handleDragStart = (e: React.DragEvent, file: FileItem) => {
        if (file.is_locked) {
            e.preventDefault();
            return;
        }
        setDraggedFile(file);
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('text/plain', file.id);
    };

    const handleDragOver = (e: React.DragEvent, targetId: string | null) => {
        e.preventDefault();
        if (draggedFile && draggedFile.id !== targetId) {
            setDropTargetId(targetId);
            e.dataTransfer.dropEffect = 'move';
        }
    };

    const handleDragLeave = () => {
        setDropTargetId(null);
    };

    const handleMoveFileDrop = async (e: React.DragEvent, targetParentId: string | null) => {
        e.preventDefault();
        setDropTargetId(null);
        
        if (!draggedFile || !companyId) {
            setDraggedFile(null);
            return;
        }

        // Don't drop on itself or if locked
        if (draggedFile.id === targetParentId || draggedFile.is_locked) {
            setDraggedFile(null);
            return;
        }

        const draggedFileBackup = draggedFile;
        setDraggedFile(null);

        try {
            // Check if dragging a group - use group move endpoint
            if (draggedFileBackup.type === 'group') {
                console.log('Moving group to root:', { groupId: draggedFileBackup.id });
                const response = await authFetch(`/api/groups/${companyId}/${draggedFileBackup.id}/move`, {
                    method: 'PUT',
                    body: JSON.stringify({ 
                        target_path: '', // Empty string = move to root
                    }),
                });

                const result = await response.json();
                console.log('Group move to root response:', response.status, result);
                
                if (!response.ok || result.error) {
                    alert(result.error || result.message || 'Failed to move group');
                } else {
                    fetchFiles();
                }
            } else {
                // Regular file move
                const response = await authFetch(`/api/files/${companyId}/${draggedFileBackup.id}/move`, {
                    method: 'PUT',
                    body: JSON.stringify({
                        target_parent_id: targetParentId,
                        target_department_id: null
                    })
                });

                const result = await response.json();
                
                if (!response.ok || result.error) {
                    alert(result.error || result.message || 'Failed to move file');
                } else {
                    fetchFiles();
                }
            }
        } catch (error) {
            console.error('Error moving file/group:', error);
        }
    };

    const handleDragEnd = () => {
        setDraggedFile(null);
        setDropTargetId(null);
    };

    const handleViewActivity = (file: FileItem) => {
        setActivityFile(file);
        setIsActivityModalOpen(true);
        setActiveMenu(null);
    };

    const handleViewProperties = (file: FileItem) => {
        setPropertiesFile(file);
        setIsPropertiesModalOpen(true);
        setActiveMenu(null);
    };

    // Copy file to clipboard
    const handleCopy = (file: FileItem) => {
        if (file.type === 'folder' || file.type === 'group') return; // Can't copy folders or groups
        setClipboardFile(file);
        setActiveMenu(null);
    };

    // Paste file from clipboard to current location
    const handlePaste = async () => {
        if (!companyId || !clipboardFile) return;
        
        setIsPasting(true);
        try {
            // Get current path (excluding "Home")
            const currentParentPath = currentPath.length > 1 ? currentPath.slice(1).join('/') : null;
            
            const response = await authFetch(`/api/files/${companyId}/${clipboardFile.id}/copy`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    target_parent_path: currentParentPath,
                    target_department_id: selectedDepartment || null,
                    target_visibility: fileViewMode,
                }),
            });
            
            if (response.ok) {
                // Clear clipboard after successful paste
                setClipboardFile(null);
                // Refresh file list
                fetchFiles();
            } else {
                const errorData = await response.json().catch(() => ({}));
                console.error('Failed to paste file:', errorData.error || 'Unknown error');
            }
        } catch (error) {
            console.error('Error pasting file:', error);
        } finally {
            setIsPasting(false);
        }
    };

    const handleToggleCompanyFolder = async (file: FileItem) => {
        if (!companyId || file.type !== 'folder') return;
        
        try {
            const response = await authFetch(`/api/files/${companyId}/${file.id}/company-folder`, {
                method: 'PUT',
            });
            
            if (response.ok) {
                // Refresh file list to get updated state
                fetchFiles();
            } else {
                console.error('Failed to toggle company folder status');
            }
        } catch (error) {
            console.error('Error toggling company folder:', error);
        }
        setActiveMenu(null);
    };

    // Permission checks
    const canLockFiles = user?.role === 'SuperAdmin' || user?.role === 'Admin' || user?.role === 'Manager';
    const canViewActivity = user?.role === 'SuperAdmin' || user?.role === 'Admin';
    const isAdminOrHigher = user?.role === 'SuperAdmin' || user?.role === 'Admin';
    
    // File-level permission checks
    const canDeleteFile = (file: FileItem) => {
        if (file.is_locked) return false;
        if (isAdminOrHigher) return true;
        return file.owner_id === user?.id;
    };
    
    // Share permission: owner, manager, or admin can share
    const canShareFile = (file: FileItem) => {
        if (isAdminOrHigher) return true;
        if (user?.role === 'Manager') return true;
        return file.owner_id === user?.id;
    };
    
    const canMoveFile = (file: FileItem) => {
        if (file.is_locked) return false;
        // All users can move unlocked files within their access scope
        return true;
    };
    
    // Get files that can be deleted/moved from selection
    const getSelectedFilesForAction = (action: 'delete' | 'move') => {
        const selectedFilesList = files.filter(f => selectedFiles.has(f.id));
        if (action === 'delete') {
            return selectedFilesList.filter(canDeleteFile);
        }
        return selectedFilesList.filter(canMoveFile);
    };
    
    const deletableSelectedFiles = getSelectedFilesForAction('delete');
    const movableSelectedFiles = getSelectedFilesForAction('move');

    const [sortBy, setSortBy] = useState<'name' | 'size' | 'modified'>('modified');
    const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc');
    const [searchQuery, setSearchQuery] = useState('');
    
    // Pagination
    const [currentPage, setCurrentPage] = useState(1);

    // Check if any modal is open (disable shortcuts when modals are open)
    const isAnyModalOpen = isRenameOpen || isNewFolderOpen || isActivityModalOpen || 
        isLockModalOpen || isUnlockModalOpen || isMoveModalOpen || 
        isPropertiesModalOpen || isShareModalOpen || isRequestModalOpen || 
        isUploadModalOpen || isBulkMoveModalOpen || previewFile !== null;

    // Keyboard shortcuts for file operations - read from preset context
    const { getResolvedBinding } = useKeyboardShortcutsContext();
    
    // Helper to get binding from current preset
    const getBinding = useCallback((actionId: ShortcutActionId) => {
        const binding = getResolvedBinding(actionId);
        return binding ? { keys: binding.keys, isSequence: binding.isSequence } : null;
    }, [getResolvedBinding]);

    const fileShortcuts: Shortcut[] = useMemo(() => {
        const shortcuts: Shortcut[] = [];
        
        // Upload files
        const uploadBinding = getBinding('file.upload');
        if (uploadBinding) {
            shortcuts.push({
                id: 'file.upload',
                keys: uploadBinding.keys,
                description: 'Upload files',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen) {
                        fileInputRef.current?.click();
                    }
                },
                enabled: !isAnyModalOpen,
                isSequence: uploadBinding.isSequence,
            });
        }
        
        // New folder
        const newFolderBinding = getBinding('file.newFolder');
        if (newFolderBinding) {
            shortcuts.push({
                id: 'file.newFolder',
                keys: newFolderBinding.keys,
                description: 'Create new folder',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen) {
                        setIsNewFolderOpen(true);
                    }
                },
                enabled: !isAnyModalOpen,
                isSequence: newFolderBinding.isSequence,
            });
        }
        
        // Delete selected
        const deleteBinding = getBinding('file.delete');
        if (deleteBinding) {
            shortcuts.push({
                id: 'file.delete',
                keys: deleteBinding.keys,
                description: 'Delete selected files',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen && selectedFiles.size > 0) {
                        handleBulkDelete();
                    }
                },
                enabled: !isAnyModalOpen && selectedFiles.size > 0,
                isSequence: deleteBinding.isSequence,
            });
        }
        
        // Rename selected file
        const renameBinding = getBinding('file.rename');
        if (renameBinding) {
            shortcuts.push({
                id: 'file.rename',
                keys: renameBinding.keys,
                description: 'Rename selected file',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen && selectedFiles.size === 1) {
                        const fileId = Array.from(selectedFiles)[0];
                        const file = files.find(f => f.id === fileId);
                        if (file && !file.is_locked && !file.is_company_folder) {
                            setFileToRename(file);
                            setIsRenameOpen(true);
                        }
                    }
                },
                enabled: !isAnyModalOpen && selectedFiles.size === 1,
                isSequence: renameBinding.isSequence,
            });
        }
        
        // Move selected files
        const moveBinding = getBinding('file.move');
        if (moveBinding) {
            shortcuts.push({
                id: 'file.move',
                keys: moveBinding.keys,
                description: 'Move selected files',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen && selectedFiles.size > 0 && movableSelectedFiles.length > 0) {
                        setIsBulkMoveModalOpen(true);
                    }
                },
                enabled: !isAnyModalOpen && movableSelectedFiles.length > 0,
                isSequence: moveBinding.isSequence,
            });
        }
        
        // Open/enter
        const openBinding = getBinding('file.open');
        if (openBinding) {
            shortcuts.push({
                id: 'file.open',
                keys: openBinding.keys,
                description: 'Open file or enter folder',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen && focusedFileIndex >= 0 && focusedFileIndex < files.length) {
                        const file = files[focusedFileIndex];
                        if (file) {
                            if (file.type === 'group') {
                                handleGroupClick(file);
                            } else if (file.type === 'folder') {
                                setCurrentPath([...currentPath, file.name]);
                            } else {
                                handlePreview(file);
                            }
                        }
                    }
                },
                enabled: !isAnyModalOpen && focusedFileIndex >= 0,
                isSequence: openBinding.isSequence,
            });
        }
        
        // Download
        const downloadBinding = getBinding('file.download');
        if (downloadBinding) {
            shortcuts.push({
                id: 'file.download',
                keys: downloadBinding.keys,
                description: 'Download focused file',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen && focusedFileIndex >= 0 && focusedFileIndex < files.length) {
                        const file = files[focusedFileIndex];
                        if (file) {
                            handleDownload(file);
                        }
                    }
                },
                enabled: !isAnyModalOpen && focusedFileIndex >= 0,
                isSequence: downloadBinding.isSequence,
            });
        }
        
        // Preview
        const previewBinding = getBinding('file.preview');
        if (previewBinding) {
            shortcuts.push({
                id: 'file.preview',
                keys: previewBinding.keys,
                description: 'Preview file',
                category: 'files',
                action: () => {
                    if (!isAnyModalOpen && focusedFileIndex >= 0 && focusedFileIndex < files.length) {
                        const file = files[focusedFileIndex];
                        if (file && file.type !== 'folder') {
                            handlePreview(file);
                        }
                    }
                },
                enabled: !isAnyModalOpen && focusedFileIndex >= 0,
                isSequence: previewBinding.isSequence,
            });
        }
        
        // Select all
        const selectAllBinding = getBinding('select.all');
        if (selectAllBinding) {
            shortcuts.push({
                id: 'select.all',
                keys: selectAllBinding.keys,
                description: 'Select all files',
                category: 'selection',
                action: () => {
                    if (!isAnyModalOpen) {
                        selectAllFiles();
                        setIsSelectionMode(true);
                    }
                },
                enabled: !isAnyModalOpen,
                isSequence: selectAllBinding.isSequence,
            });
        }
        
        // Toggle selection
        const toggleBinding = getBinding('select.toggle');
        if (toggleBinding) {
            shortcuts.push({
                id: 'select.toggle',
                keys: toggleBinding.keys,
                description: 'Toggle selection on focused item',
                category: 'selection',
                action: () => {
                    if (!isAnyModalOpen && focusedFileIndex >= 0 && focusedFileIndex < files.length) {
                        const file = files[focusedFileIndex];
                        if (file) {
                            toggleFileSelection(file.id);
                            setIsSelectionMode(true);
                        }
                    }
                },
                enabled: !isAnyModalOpen && focusedFileIndex >= 0,
                isSequence: toggleBinding.isSequence,
            });
        }
        
        // Navigate up
        const upBinding = getBinding('select.up');
        if (upBinding) {
            shortcuts.push({
                id: 'select.up',
                keys: upBinding.keys,
                description: 'Move focus up',
                category: 'selection',
                action: () => {
                    if (!isAnyModalOpen) {
                        setFocusedFileIndex(prev => {
                            if (prev > 0) return prev - 1;
                            if (prev === -1 && files.length > 0) return 0;
                            return prev;
                        });
                    }
                },
                enabled: !isAnyModalOpen,
                isSequence: upBinding.isSequence,
            });
        }
        
        // Navigate down
        const downBinding = getBinding('select.down');
        if (downBinding) {
            shortcuts.push({
                id: 'select.down',
                keys: downBinding.keys,
                description: 'Move focus down',
                category: 'selection',
                action: () => {
                    if (!isAnyModalOpen) {
                        setFocusedFileIndex(prev => {
                            if (prev < files.length - 1) return prev + 1;
                            return prev;
                        });
                    }
                },
                enabled: !isAnyModalOpen,
                isSequence: downBinding.isSequence,
            });
        }
        
        // Navigate left
        const leftBinding = getBinding('select.left');
        if (leftBinding) {
            shortcuts.push({
                id: 'select.left',
                keys: leftBinding.keys,
                description: 'Move focus left',
                category: 'selection',
                action: () => {
                    if (!isAnyModalOpen && viewMode === 'grid') {
                        setFocusedFileIndex(prev => {
                            if (prev > 0) return prev - 1;
                            if (prev === -1 && files.length > 0) return 0;
                            return prev;
                        });
                    }
                },
                enabled: !isAnyModalOpen && viewMode === 'grid',
                isSequence: leftBinding.isSequence,
            });
        }
        
        // Navigate right
        const rightBinding = getBinding('select.right');
        if (rightBinding) {
            shortcuts.push({
                id: 'select.right',
                keys: rightBinding.keys,
                description: 'Move focus right',
                category: 'selection',
                action: () => {
                    if (!isAnyModalOpen && viewMode === 'grid') {
                        setFocusedFileIndex(prev => {
                            if (prev < files.length - 1) return prev + 1;
                            return prev;
                        });
                    }
                },
                enabled: !isAnyModalOpen && viewMode === 'grid',
                isSequence: rightBinding.isSequence,
            });
        }
        
        return shortcuts;
    }, [isAnyModalOpen, selectedFiles, files, focusedFileIndex, movableSelectedFiles, viewMode, currentPath, getBinding]);

    useKeyboardShortcuts(fileShortcuts, { enabled: !isAnyModalOpen });

    // Reset focused index when files change
    useEffect(() => {
        setFocusedFileIndex(-1);
    }, [files.length, currentPath]);

    const getIcon = (file: FileItem) => {
        switch (file.type) {
            case 'group': return <Layers className="w-16 h-16" style={{ color: file.color || '#3B82F6' }} />;
            case 'folder': return <Folder className="w-16 h-16 text-blue-500" />;
            case 'image': return <ImageIcon className="w-16 h-16 text-purple-500" />;
            case 'video': return <ImageIcon className="w-16 h-16 text-red-500" />;
            case 'audio': return <ImageIcon className="w-16 h-16 text-yellow-500" />;
            default: return <FileText className="w-16 h-16 text-gray-500" />;
        }
    };

    // Smaller icons for Quick Access section
    const getSmallIcon = (file: FileItem) => {
        switch (file.type) {
            case 'group': return <Layers className="w-8 h-8" style={{ color: file.color || '#3B82F6' }} />;
            case 'folder': return <Folder className="w-8 h-8 text-blue-500" />;
            case 'image': return <ImageIcon className="w-8 h-8 text-purple-500" />;
            case 'video': return <ImageIcon className="w-8 h-8 text-red-500" />;
            case 'audio': return <ImageIcon className="w-8 h-8 text-yellow-500" />;
            default: return <FileText className="w-8 h-8 text-gray-500" />;
        }
    };

    const handleDrag = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();

        // Ignore if we are dragging an internal file
        if (draggedFile) return;

        if (e.type === "dragenter" || e.type === "dragover") {
            setIsDragging(true);
        } else if (e.type === "dragleave") {
            setIsDragging(false);
        }
    };

    const handleDrop = async (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();

        // Ignore if we are dragging an internal file
        if (draggedFile) return;

        setIsDragging(false);

        const files = Array.from(e.dataTransfer.files);
        if (files.length > 0) {
            await startUpload(files);
        }
    };

    const handleFileInput = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (e.target.files && e.target.files.length > 0) {
            await startUpload(Array.from(e.target.files));
        }
    };

    const startUpload = async (files: File[]) => {
        const newUploads: UploadFile[] = files.map(f => ({
            file: f,
            progress: 0,
            status: 'pending'
        }));

        setUploadFilesList(newUploads);
        setIsUploadModalOpen(true);

        for (let i = 0; i < newUploads.length; i++) {
            const uploadItem = newUploads[i];
            setUploadFilesList(prev => prev.map((item, idx) => idx === i ? { ...item, status: 'uploading' } : item));

            const formData = new FormData();
            formData.append('file', uploadItem.file);

            try {
                // Mock progress
                for (let p = 0; p <= 90; p += 30) {
                    await new Promise(r => setTimeout(r, 200));
                    setUploadFilesList(prev => prev.map((item, idx) => idx === i ? { ...item, progress: p } : item));
                }

                // Get current path (strip "Home")
                const parentPath = currentPath.slice(1).join('/');
                const queryParams = new URLSearchParams();
                if (parentPath) queryParams.set('parent_path', parentPath);
                // Auto-set visibility based on current view mode
                if (fileViewMode === 'private') queryParams.set('visibility', 'private');
                const queryString = queryParams.toString();
                const uploadUrl = `${import.meta.env.VITE_API_URL || ''}/api/upload/${companyId}${queryString ? `?${queryString}` : ''}`;

                // Note: authFetch sets Content-Type to application/json by default, but for FormData we need to let browser set it
                // So we override headers to remove Content-Type
                const response = await authFetch(uploadUrl, {
                    method: 'POST',
                    headers: {
                        // Remove Content-Type to let browser set boundary for FormData
                        'Content-Type': undefined as any
                    },
                    body: formData,
                });

                if (response.ok) {
                    const data = await response.json();
                    // Check for blocked extension error (returned as 200 with error field)
                    if (data.error === 'blocked_extension') {
                        const errorMsg = data.message || `File type .${data.extension} is not allowed`;
                        setUploadFilesList(prev => prev.map((item, idx) => idx === i ? { ...item, status: 'error', error: errorMsg } : item));
                        alert(errorMsg);
                    } else {
                        setUploadFilesList(prev => prev.map((item, idx) => idx === i ? { ...item, status: 'completed', progress: 100 } : item));
                        fetchFiles();
                    }
                } else {
                    throw new Error('Upload failed');
                }
            } catch (error) {
                console.error(error);
                setUploadFilesList(prev => prev.map((item, idx) => idx === i ? { ...item, status: 'error', error: 'Failed' } : item));
            }
        }
    };

    const filteredFiles = files
        .filter(f => f.name.toLowerCase().includes(searchQuery.toLowerCase()))
        .sort((a, b) => {
            // Always show folders first
            const aIsFolder = a.type === 'folder';
            const bIsFolder = b.type === 'folder';
            if (aIsFolder && !bIsFolder) return -1;
            if (!aIsFolder && bIsFolder) return 1;
            
            // Then apply normal sort criteria
            let comparison = 0;
            if (sortBy === 'name') {
                comparison = a.name.localeCompare(b.name);
            } else if (sortBy === 'size') {
                comparison = (a.size_bytes || 0) - (b.size_bytes || 0);
            } else if (sortBy === 'modified') {
                comparison = new Date(a.modified).getTime() - new Date(b.modified).getTime();
            }
            return sortOrder === 'asc' ? comparison : -comparison;
        });

    // Pagination calculations
    const totalPages = Math.ceil(filteredFiles.length / itemsPerPage);
    const paginatedFiles = filteredFiles.slice(
        (currentPage - 1) * itemsPerPage,
        currentPage * itemsPerPage
    );

    // Reset to page 1 when search query or path changes
    useEffect(() => {
        setCurrentPage(1);
    }, [searchQuery, currentPath]);

    const handleSort = (key: 'name' | 'size' | 'modified') => {
        if (sortBy === key) {
            setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc');
        } else {
            setSortBy(key);
            setSortOrder('asc');
        }
    };

    const [draggedFile, setDraggedFile] = useState<FileItem | null>(null);

    const handleFileDragStart = (e: React.DragEvent, file: FileItem) => {
        e.stopPropagation();
        setDraggedFile(file);
        e.dataTransfer.setData('application/json', JSON.stringify(file));
        e.dataTransfer.effectAllowed = 'move';
    };

    const handleFolderDragOver = (e: React.DragEvent, folder: FileItem) => {
        e.preventDefault();
        e.stopPropagation();
        if (draggedFile && draggedFile.id !== folder.id && folder.type === 'folder') {
            e.dataTransfer.dropEffect = 'move';
            e.currentTarget.classList.add('bg-primary-100', 'border-primary-500');
        }
    };

    const handleFolderDragLeave = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        e.currentTarget.classList.remove('bg-primary-100', 'border-primary-500');
    };

    const handleFolderDrop = async (e: React.DragEvent, folder: FileItem) => {
        e.preventDefault();
        e.stopPropagation();
        e.currentTarget.classList.remove('bg-primary-100', 'border-primary-500');

        console.log('Folder drop triggered:', { 
            draggedFile: draggedFile?.name, 
            draggedType: draggedFile?.type,
            targetFolder: folder.name 
        });

        if (!draggedFile || draggedFile.id === folder.id || draggedFile.is_locked) {
            console.log('Drop rejected:', { noDraggedFile: !draggedFile, sameId: draggedFile?.id === folder.id, isLocked: draggedFile?.is_locked });
            return;
        }

        // Optimistic update
        setFiles(prev => prev.filter(f => f.id !== draggedFile.id));
        const draggedFileBackup = draggedFile;
        setDraggedFile(null);

        try {
            // Check if dragging a group - use group move endpoint
            if (draggedFileBackup.type === 'group') {
                console.log('Moving group to folder:', { groupId: draggedFileBackup.id, folderId: folder.id });
                const response = await authFetch(`/api/groups/${companyId}/${draggedFileBackup.id}/move`, {
                    method: 'PUT',
                    body: JSON.stringify({ 
                        target_folder_id: folder.id,
                    }),
                });

                const result = await response.json();
                console.log('Group move response:', response.status, result);
                
                if (!response.ok || result.error) {
                    alert(result.error || result.message || 'Failed to move group');
                    fetchFiles(); // Revert
                    return;
                }

                // Refresh to ensure everything is synced
                fetchFiles();
            } else {
                // Use the move API endpoint for regular files
                const response = await authFetch(`/api/files/${companyId}/${draggedFileBackup.id}/move`, {
                    method: 'PUT',
                    body: JSON.stringify({ 
                        target_parent_id: folder.id,
                        target_department_id: null
                    }),
                });

                const result = await response.json();
                
                if (!response.ok || result.error) {
                    alert(result.error || result.message || 'Failed to move file');
                    fetchFiles(); // Revert
                    return;
                }

                // Refresh to ensure everything is synced
                fetchFiles();
            }
        } catch (error) {
            console.error('Error moving file/group:', error);
            fetchFiles(); // Revert
        }
    };

    return (
        <div className="h-full flex flex-col space-y-3 sm:space-y-4">
            {/* Header & Actions */}
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 sm:gap-4 mb-1 sm:mb-2">
                <div>
                    <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white">Files</h1>
                    <p className="text-xs sm:text-sm text-gray-500 dark:text-gray-400 mt-0.5 sm:mt-1">Manage your company documents and assets</p>
                </div>
                <div className="flex space-x-3">
                    {/* View Mode Switcher */}
                    <div className="relative" ref={viewModeRef}>
                        <button
                            onClick={(e) => { e.stopPropagation(); setIsViewModeOpen(!isViewModeOpen); }}
                            className="flex items-center px-4 py-2 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm"
                        >
                            {fileViewMode === 'private' ? (
                                <><EyeOff className="w-4 h-4 mr-2 text-purple-500" />My Private Files</>
                            ) : selectedDepartment ? (
                                <><Building2 className="w-4 h-4 mr-2 text-green-500" />{departments.find(d => d.id === selectedDepartment)?.name || 'Department'}</>
                            ) : (
                                <><Users className="w-4 h-4 mr-2 text-blue-500" />All Departments</>
                            )}
                            <ChevronDown className="w-4 h-4 ml-2 text-gray-400" />
                        </button>
                        {isViewModeOpen && (
                            <div className="absolute left-0 mt-2 w-56 bg-white dark:bg-gray-800 rounded-lg shadow-lg py-1 ring-1 ring-black ring-opacity-5 z-50 border border-gray-200 dark:border-gray-700 max-h-80 overflow-y-auto">
                                {/* All Departments option */}
                                <button
                                    onClick={() => { setFileViewMode('department'); setSelectedDepartment(null); setIsViewModeOpen(false); }}
                                    className={clsx(
                                        "flex items-center w-full px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700",
                                        fileViewMode === 'department' && !selectedDepartment && "bg-gray-50 dark:bg-gray-700 font-medium"
                                    )}
                                >
                                    <Users className="w-4 h-4 mr-3 text-blue-500" />
                                    All Departments
                                    {fileViewMode === 'department' && !selectedDepartment && <span className="ml-auto text-primary-500">✓</span>}
                                </button>
                                
                                {/* Individual department options for admins */}
                                {departments.length > 0 && (user?.role === 'Admin' || user?.role === 'SuperAdmin') && (
                                    <>
                                        <div className="border-t border-gray-200 dark:border-gray-700 my-1" />
                                        <div className="px-4 py-1.5 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                            Filter by Department
                                        </div>
                                        {departments.map((dept) => (
                                            <button
                                                key={dept.id}
                                                onClick={() => { setFileViewMode('department'); setSelectedDepartment(dept.id); setIsViewModeOpen(false); }}
                                                className={clsx(
                                                    "flex items-center w-full px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700",
                                                    fileViewMode === 'department' && selectedDepartment === dept.id && "bg-gray-50 dark:bg-gray-700 font-medium"
                                                )}
                                            >
                                                <Building2 className="w-4 h-4 mr-3 text-green-500" />
                                                {dept.name}
                                                {fileViewMode === 'department' && selectedDepartment === dept.id && <span className="ml-auto text-primary-500">✓</span>}
                                            </button>
                                        ))}
                                    </>
                                )}
                                
                                <div className="border-t border-gray-200 dark:border-gray-700 my-1" />
                                <button
                                    onClick={() => { setFileViewMode('private'); setSelectedDepartment(null); setIsViewModeOpen(false); }}
                                    className={clsx(
                                        "flex items-center w-full px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700",
                                        fileViewMode === 'private' && "bg-gray-50 dark:bg-gray-700 font-medium"
                                    )}
                                >
                                    <EyeOff className="w-4 h-4 mr-3 text-purple-500" />
                                    My Private Files
                                    {fileViewMode === 'private' && <span className="ml-auto text-primary-500">✓</span>}
                                </button>
                            </div>
                        )}
                    </div>
                    {/* Desktop: Individual icon buttons */}
                    <div className="hidden sm:flex items-center space-x-2">
                        {/* Select Mode Toggle */}
                        <button
                            onClick={() => {
                                if (isSelectionMode) {
                                    clearSelection();
                                } else {
                                    setIsSelectionMode(true);
                                }
                            }}
                            title={isSelectionMode ? 'Cancel selection' : 'Select files'}
                            className={clsx(
                                "p-2.5 border rounded-lg shadow-sm transition-colors",
                                isSelectionMode 
                                    ? "bg-primary-50 dark:bg-primary-900/20 border-primary-300 dark:border-primary-700 text-primary-700 dark:text-primary-300" 
                                    : "bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700"
                            )}
                        >
                            {isSelectionMode ? <X className="w-5 h-5" /> : <CheckSquare className="w-5 h-5" />}
                        </button>
                        <Link
                            to="/recycle-bin"
                            title="Recycle Bin"
                            className="p-2.5 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm transition-colors"
                        >
                            <Trash2 className="w-5 h-5" />
                        </Link>
                        <button
                            onClick={() => setIsRequestModalOpen(true)}
                            title="Request Files"
                            className="p-2.5 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm transition-colors"
                        >
                            <LinkIcon className="w-5 h-5" />
                        </button>
                        {!currentGroup && (
                            <button
                                onClick={() => setIsNewFolderOpen(true)}
                                title="New Folder"
                                className="p-2.5 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm transition-colors"
                            >
                                <FolderPlus className="w-5 h-5" />
                            </button>
                        )}
                        {!currentGroup && fileViewMode === 'department' && (
                            <button
                                onClick={() => setIsCreateGroupOpen(true)}
                                title="New Group"
                                className="p-2.5 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm transition-colors"
                            >
                                <Layers className="w-5 h-5" />
                            </button>
                        )}
                        {clipboardFile && (
                            <button
                                onClick={handlePaste}
                                disabled={isPasting}
                                title={`Paste "${clipboardFile.name}" here`}
                                className={clsx(
                                    "px-3 py-2 border rounded-lg shadow-sm transition-colors flex items-center gap-1.5",
                                    isPasting
                                        ? "bg-gray-100 dark:bg-gray-700 border-gray-300 dark:border-gray-600 text-gray-400 cursor-not-allowed"
                                        : "bg-green-50 dark:bg-green-900/20 border-green-300 dark:border-green-700 text-green-700 dark:text-green-300 hover:bg-green-100 dark:hover:bg-green-900/40"
                                )}
                            >
                                <Clipboard className="w-4 h-4" />
                                <span className="text-sm font-medium">Paste</span>
                            </button>
                        )}
                    </div>
                    
                    {/* Mobile: Overflow menu */}
                    <div className="sm:hidden relative" ref={mobileMenuRef}>
                        <button
                            onClick={(e) => { e.stopPropagation(); setIsMobileMenuOpen(!isMobileMenuOpen); }}
                            className="p-2.5 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm transition-colors"
                        >
                            <MoreHorizontal className="w-5 h-5" />
                        </button>
                        {isMobileMenuOpen && (
                            <div className="absolute right-0 mt-2 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-lg py-1 ring-1 ring-black ring-opacity-5 z-50 border border-gray-200 dark:border-gray-700">
                                <button
                                    onClick={() => {
                                        if (isSelectionMode) {
                                            clearSelection();
                                        } else {
                                            setIsSelectionMode(true);
                                        }
                                        setIsMobileMenuOpen(false);
                                    }}
                                    className="flex items-center w-full px-4 py-3 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                >
                                    {isSelectionMode ? <X className="w-5 h-5 mr-3" /> : <CheckSquare className="w-5 h-5 mr-3" />}
                                    {isSelectionMode ? 'Cancel Selection' : 'Select Files'}
                                </button>
                                <Link
                                    to="/recycle-bin"
                                    onClick={() => setIsMobileMenuOpen(false)}
                                    className="flex items-center w-full px-4 py-3 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                >
                                    <Trash2 className="w-5 h-5 mr-3" />
                                    Recycle Bin
                                </Link>
                                <button
                                    onClick={() => { setIsRequestModalOpen(true); setIsMobileMenuOpen(false); }}
                                    className="flex items-center w-full px-4 py-3 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                >
                                    <LinkIcon className="w-5 h-5 mr-3" />
                                    Request Files
                                </button>
                                {!currentGroup && (
                                    <button
                                        onClick={() => { setIsNewFolderOpen(true); setIsMobileMenuOpen(false); }}
                                        className="flex items-center w-full px-4 py-3 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                    >
                                        <FolderPlus className="w-5 h-5 mr-3" />
                                        New Folder
                                    </button>
                                )}
                                {!currentGroup && fileViewMode === 'department' && (
                                    <button
                                        onClick={() => { setIsCreateGroupOpen(true); setIsMobileMenuOpen(false); }}
                                        className="flex items-center w-full px-4 py-3 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                    >
                                        <Layers className="w-5 h-5 mr-3" />
                                        New Group
                                    </button>
                                )}
                                {clipboardFile && (
                                    <button
                                        onClick={() => { handlePaste(); setIsMobileMenuOpen(false); }}
                                        disabled={isPasting}
                                        className="flex items-center w-full px-4 py-3 text-sm text-green-700 dark:text-green-300 hover:bg-green-50 dark:hover:bg-green-900/20"
                                    >
                                        <Clipboard className="w-5 h-5 mr-3" />
                                        Paste
                                        <span className="ml-1 text-xs text-green-500 truncate max-w-32">({clipboardFile.name})</span>
                                    </button>
                                )}
                            </div>
                        )}
                    </div>
                    
                    <input
                        type="file"
                        ref={fileInputRef}
                        className="hidden"
                        multiple
                        onChange={handleFileInput}
                    />
                    <button
                        onClick={() => fileInputRef.current?.click()}
                        className="flex items-center px-3 sm:px-4 py-2 bg-primary-600 rounded-lg text-sm font-medium text-white hover:bg-primary-700 shadow-sm"
                    >
                        <Upload className="w-4 h-4 sm:mr-2" />
                        <span className="hidden sm:inline">Upload File</span>
                    </button>
                </div>
            </div>

            {/* Bulk Action Bar */}
            {selectedFiles.size > 0 && (
                <div className="flex items-center justify-between px-4 py-3 bg-primary-50 dark:bg-primary-900/20 border border-primary-200 dark:border-primary-800 rounded-lg">
                    <div className="flex items-center gap-4">
                        <span className="text-sm font-medium text-primary-700 dark:text-primary-300">
                            {selectedFiles.size} item{selectedFiles.size !== 1 ? 's' : ''} selected
                        </span>
                        {/* Show permission info if some files can't be acted on */}
                        {(movableSelectedFiles.length < selectedFiles.size || deletableSelectedFiles.length < selectedFiles.size) && (
                            <span className="text-xs text-amber-600 dark:text-amber-400">
                                ({selectedFiles.size - movableSelectedFiles.length} locked)
                            </span>
                        )}
                        <button
                            onClick={selectAllFiles}
                            className="text-sm text-primary-600 dark:text-primary-400 hover:underline"
                        >
                            Select all
                        </button>
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={() => {
                                if (movableSelectedFiles.length === 0) {
                                    alert('Cannot move any of the selected files. Locked files cannot be moved.');
                                    return;
                                }
                                setIsBulkMoveModalOpen(true);
                            }}
                            disabled={movableSelectedFiles.length === 0}
                            className={clsx(
                                "flex items-center px-3 py-1.5 border rounded-lg text-sm font-medium",
                                movableSelectedFiles.length > 0
                                    ? "bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700"
                                    : "bg-gray-100 dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-400 dark:text-gray-500 cursor-not-allowed"
                            )}
                            title={movableSelectedFiles.length === 0 ? 'No movable files selected' : `Move ${movableSelectedFiles.length} file(s)`}
                        >
                            <Move className="w-4 h-4 mr-1.5" />
                            Move{movableSelectedFiles.length < selectedFiles.size && movableSelectedFiles.length > 0 && ` (${movableSelectedFiles.length})`}
                        </button>
                        <button
                            onClick={handleBulkDelete}
                            disabled={deletableSelectedFiles.length === 0}
                            className={clsx(
                                "flex items-center px-3 py-1.5 border rounded-lg text-sm font-medium",
                                deletableSelectedFiles.length > 0
                                    ? "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 hover:bg-red-100 dark:hover:bg-red-900/30"
                                    : "bg-gray-100 dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-400 dark:text-gray-500 cursor-not-allowed"
                            )}
                            title={deletableSelectedFiles.length === 0 ? 'No deletable files selected (locked or no permission)' : `Delete ${deletableSelectedFiles.length} file(s)`}
                        >
                            <Trash2 className="w-4 h-4 mr-1.5" />
                            Delete{deletableSelectedFiles.length < selectedFiles.size && deletableSelectedFiles.length > 0 && ` (${deletableSelectedFiles.length})`}
                        </button>
                        <button
                            onClick={clearSelection}
                            className="p-1.5 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                            <X className="w-4 h-4" />
                        </button>
                    </div>
                </div>
            )}

            {/* Quick Access */}
            {(() => {
                const starredItems = files.filter(f => f.is_starred);
                const visibleStarred = starredItems.slice(0, 4);
                const overflowStarred = starredItems.slice(4);
                const hasOverflow = overflowStarred.length > 0;

                if (starredItems.length === 0) return null;

                return (
                    <div className="mb-4">
                        <div className="flex items-center justify-between mb-3">
                            <h2 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">Quick Access</h2>
                            {hasOverflow && (
                                <div className="relative" ref={starredDropdownRef}>
                                    <button
                                        onClick={() => setShowMoreStarred(!showMoreStarred)}
                                        className="flex items-center text-xs font-medium text-primary-600 dark:text-primary-400 hover:text-primary-700 dark:hover:text-primary-300 transition-colors"
                                    >
                                        +{overflowStarred.length} more
                                        <ChevronDown className={clsx("w-4 h-4 ml-1 transition-transform", showMoreStarred && "rotate-180")} />
                                    </button>
                                    {showMoreStarred && (
                                        <div className="absolute right-0 top-full mt-2 w-72 bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 shadow-xl z-30 max-h-80 overflow-y-auto">
                                            <div className="p-2">
                                                <p className="text-xs text-gray-500 dark:text-gray-400 px-2 py-1 mb-1">More starred items</p>
                                                {overflowStarred.map(file => (
                                                    <div 
                                                        key={`overflow-${file.id}`} 
                                                        className="flex items-center space-x-3 p-2 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700/50 cursor-pointer transition-colors"
                                                        onClick={() => {
                                                            setShowMoreStarred(false);
                                                            if (file.type === 'group') {
                                                                handleGroupClick(file);
                                                            } else if (file.type === 'folder') {
                                                                setCurrentPath([...currentPath, file.name]);
                                                            } else {
                                                                handlePreview(file);
                                                            }
                                                        }}
                                                    >
                                                        <div className="p-1.5 bg-primary-50 dark:bg-primary-900/30 rounded flex-shrink-0">
                                                            {getSmallIcon(file)}
                                                        </div>
                                                        <div className="flex-1 min-w-0">
                                                            <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{file.name}</p>
                                                            <p className="text-xs text-gray-500 dark:text-gray-400">{file.size}</p>
                                                        </div>
                                                        <Star className="w-3.5 h-3.5 text-yellow-400 fill-current flex-shrink-0" />
                                                    </div>
                                                ))}
                                            </div>
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                            {visibleStarred.map(file => (
                                <div 
                                    key={`quick-${file.id}`} 
                                    className="bg-white dark:bg-gray-800 p-3 rounded-lg border border-gray-200 dark:border-gray-700 shadow-sm hover:shadow-md transition-shadow cursor-pointer flex items-center space-x-3" 
                                    onClick={() => {
                                        if (file.type === 'group') {
                                            handleGroupClick(file);
                                        } else if (file.type === 'folder') {
                                            setCurrentPath([...currentPath, file.name]);
                                        } else {
                                            handlePreview(file);
                                        }
                                    }}
                                >
                                    <div className="p-2 bg-primary-50 dark:bg-primary-900/30 rounded-lg flex-shrink-0">
                                        {getSmallIcon(file)}
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{file.name}</p>
                                        <p className="text-xs text-gray-500 dark:text-gray-400">{file.size}</p>
                                    </div>
                                    <Star className="w-4 h-4 text-yellow-400 fill-current flex-shrink-0" />
                                </div>
                            ))}
                        </div>
                    </div>
                );
            })()}

            <UploadProgressModal
                isOpen={isUploadModalOpen}
                onClose={() => setIsUploadModalOpen(false)}
                files={uploadFilesList}
            />

            {previewFile && (
                <FilePreviewModal
                    isOpen={!!previewFile}
                    onClose={handlePreviewClose}
                    file={previewFile}
                />
            )}

            <RenameModal
                isOpen={isRenameOpen}
                onClose={() => setIsRenameOpen(false)}
                onRename={handleRename}
                currentName={fileToRename?.name || ''}
            />

            <NewFolderModal
                isOpen={isNewFolderOpen}
                onClose={() => setIsNewFolderOpen(false)}
                onCreate={handleCreateFolder}
            />

            <FileActivityModal
                isOpen={isActivityModalOpen}
                onClose={() => {
                    setIsActivityModalOpen(false);
                    setActivityFile(null);
                }}
                fileId={activityFile?.id || ''}
                fileName={activityFile?.name || ''}
            />

            {/* File Properties Modal */}
            <FilePropertiesModal
                isOpen={isPropertiesModalOpen}
                onClose={() => {
                    setIsPropertiesModalOpen(false);
                    setPropertiesFile(null);
                    restoreGroupViewerIfMinimized();
                }}
                file={propertiesFile}
                companyId={companyId}
            />

            {/* Share File Modal */}
            {shareFile && (
                <ShareFileModal
                    isOpen={isShareModalOpen}
                    onClose={() => {
                        setIsShareModalOpen(false);
                        setShareFile(null);
                        restoreGroupViewerIfMinimized();
                    }}
                    file={shareFile}
                    companyId={companyId}
                    complianceMode={currentCompany?.compliance_mode}
                />
            )}

            {/* AI Summary Modal */}
            {aiFile && (
                <AiSummaryModal
                    isOpen={isAiSummaryModalOpen}
                    onClose={() => {
                        setIsAiSummaryModalOpen(false);
                        setAiFile(null);
                        restoreGroupViewerIfMinimized();
                    }}
                    file={aiFile}
                />
            )}

            {/* AI Question Modal */}
            {aiFile && (
                <AiQuestionModal
                    isOpen={isAiQuestionModalOpen}
                    onClose={() => {
                        setIsAiQuestionModalOpen(false);
                        setAiFile(null);
                        restoreGroupViewerIfMinimized();
                    }}
                    file={aiFile}
                />
            )}

            {/* Lock File Modal */}
            <LockFileModal
                isOpen={isLockModalOpen}
                onClose={() => {
                    setIsLockModalOpen(false);
                    setLockingFile(null);
                }}
                onLock={handleLockFile}
                fileName={lockingFile?.name || ''}
                isLocking={isLocking}
            />

            {/* Unlock File Modal */}
            <UnlockFileModal
                isOpen={isUnlockModalOpen}
                onClose={() => {
                    setIsUnlockModalOpen(false);
                    setLockingFile(null);
                }}
                onUnlock={handleUnlockFile}
                fileName={lockingFile?.name || ''}
                isUnlocking={isLocking}
                requiresPassword={lockingFile?.has_lock_password || false}
                requiredRole={lockingFile?.lock_requires_role}
            />

            {/* Move File Modal */}
            <MoveFileModal
                isOpen={isMoveModalOpen}
                onClose={() => {
                    setIsMoveModalOpen(false);
                    setMovingFile(null);
                    restoreGroupViewerIfMinimized();
                }}
                onMove={handleMoveFile}
                fileName={movingFile?.name || ''}
                isMoving={isMoving}
                currentPath={currentPath.length > 1 ? currentPath.slice(1).join('/') : null}
                currentVisibility={fileViewMode}
                canCrossDepartment={user?.role === 'SuperAdmin' || user?.role === 'Admin'}
            />

            {/* Bulk Move Modal */}
            <MoveFileModal
                isOpen={isBulkMoveModalOpen}
                onClose={() => {
                    setIsBulkMoveModalOpen(false);
                }}
                onMove={handleBulkMove}
                fileName=""
                fileCount={selectedFiles.size}
                isMoving={isBulkMoving}
                currentPath={currentPath.length > 1 ? currentPath.slice(1).join('/') : null}
                currentVisibility={fileViewMode}
                canCrossDepartment={user?.role === 'SuperAdmin' || user?.role === 'Admin'}
            />

            {/* Move Group Modal */}
            <MoveFileModal
                isOpen={isMoveGroupModalOpen}
                onClose={() => {
                    setIsMoveGroupModalOpen(false);
                    setGroupToMove(null);
                }}
                onMove={handleMoveGroupConfirm}
                fileName={groupToMove?.name || ''}
                isMoving={false}
                currentPath={currentPath.length > 1 ? currentPath.slice(1).join('/') : null}
                currentVisibility={fileViewMode}
                canCrossDepartment={user?.role === 'SuperAdmin' || user?.role === 'Admin'}
            />

            <div
                className={clsx(
                    "bg-white dark:bg-gray-800 border rounded-lg shadow-sm flex-1 flex flex-col transition-colors mt-4",
                    isDragging ? "border-primary-500 bg-primary-50 dark:bg-primary-900/20 border-2 border-dashed" : "border-gray-200 dark:border-gray-700"
                )}
                onDragEnter={handleDrag}
                onDragLeave={handleDrag}
                onDragOver={handleDrag}
                onDrop={handleDrop}
            >
                {/* Toolbar */}
                <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex flex-col sm:flex-row sm:items-center justify-between bg-gray-50 dark:bg-gray-900/50 bg-opacity-50 gap-3">
                    {/* Breadcrumbs */}
                    <div className="flex items-center space-x-2 text-sm text-gray-600 overflow-x-auto pb-2 sm:pb-0 scrollbar-hide">
                        <Clock className="w-4 h-4 flex-shrink-0 text-gray-400" />
                        {currentPath.map((folder, index) => (
                            <div key={index} className="flex items-center flex-shrink-0">
                                {index > 0 && <span className="mx-1 text-gray-400">/</span>}
                                <span
                                    className={clsx(
                                        "hover:text-primary-600 dark:hover:text-primary-400 cursor-pointer px-1 py-0.5 rounded transition-colors", 
                                        index === currentPath.length - 1 && "font-semibold text-gray-900 dark:text-white",
                                        index === 0 && dropTargetId === 'home' && "bg-primary-100 dark:bg-primary-900/30 ring-2 ring-primary-400"
                                    )}
                                    onClick={() => {
                                        // Navigate to this path
                                        if (index === 0 && currentGroup) {
                                            // Exiting from a group - go back to root
                                            handleExitGroup();
                                        } else {
                                            const newPath = currentPath.slice(0, index + 1);
                                            setCurrentPath(newPath);
                                        }
                                    }}
                                    onDragOver={(e) => {
                                        if (index === 0 && draggedFile) {
                                            e.preventDefault();
                                            handleDragOver(e, 'home');
                                        }
                                    }}
                                    onDragLeave={handleDragLeave}
                                    onDrop={(e) => {
                                        if (index === 0 && draggedFile) {
                                            handleMoveFileDrop(e, null); // null = move to root
                                        }
                                    }}
                                >
                                    {index === 0 && <Home className="w-3 h-3 inline mr-1" />}
                                    {folder}
                                </span>
                            </div>
                        ))}
                    </div>

                    {/* Search & View Toggle */}
                    <div className="flex items-center space-x-3 w-full sm:w-auto">
                        <div className="relative flex-1 sm:flex-none group">
                            <Search className="w-4 h-4 absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 group-hover:text-primary-500 transition-colors" />
                            <input
                                type="text"
                                placeholder="Search files..."
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                                className="w-full sm:w-64 pl-9 pr-4 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-1 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 transition-shadow"
                            />
                        </div>
                        <div className="border-l border-gray-300 dark:border-gray-600 h-6 hidden sm:block" />
                        <div className="flex space-x-1 bg-gray-100 dark:bg-gray-700 p-1 rounded-lg">
                            <button
                                onClick={() => setViewMode('grid')}
                                className={clsx("p-1.5 rounded-md transition-all", viewMode === 'grid' ? "bg-white dark:bg-gray-600 shadow-sm text-primary-600 dark:text-primary-400" : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200")}
                            >
                                <Grid className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => setViewMode('list')}
                                className={clsx("p-1.5 rounded-md transition-all", viewMode === 'list' ? "bg-white dark:bg-gray-600 shadow-sm text-primary-600 dark:text-primary-400" : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200")}
                            >
                                <List className="w-4 h-4" />
                            </button>
                        </div>
                    </div>
                </div>

                {/* File Area */}
                <div className="p-4 relative flex flex-col flex-1 min-h-[400px]">
                    {isDragging && !draggedFile && (
                        <div className="absolute inset-0 flex items-center justify-center bg-white dark:bg-gray-800 bg-opacity-90 dark:bg-opacity-90 z-50 backdrop-blur-sm">
                            <div className="text-center p-8 border-4 border-dashed border-primary-400 rounded-xl bg-primary-50 dark:bg-primary-900/30">
                                <Upload className="w-16 h-16 text-primary-500 mx-auto mb-4 animate-bounce" />
                                <p className="text-xl font-bold text-primary-700 dark:text-primary-300">Drop files to upload</p>
                            </div>
                        </div>
                    )}

                    {/* Empty State */}
                    {!isLoading && filteredFiles.length === 0 && !isDragging && (
                        <div className="flex flex-col items-center justify-center flex-1 py-16 text-center">
                            <div className="p-6 rounded-full bg-gray-100 dark:bg-gray-800 mb-6">
                                <Folder className="w-16 h-16 text-gray-400 dark:text-gray-500" />
                            </div>
                            <h3 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
                                {searchQuery ? 'No files found' : 'No files yet'}
                            </h3>
                            <p className="text-gray-500 dark:text-gray-400 mb-6 max-w-md">
                                {searchQuery 
                                    ? `No files match "${searchQuery}". Try a different search term.`
                                    : 'Get started by uploading your first file or creating a folder. You can also drag and drop files here.'}
                            </p>
                            {!searchQuery && (
                                <div className="flex gap-3">
                                    <button
                                        onClick={() => setIsNewFolderOpen(true)}
                                        className="flex items-center px-4 py-2 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 shadow-sm"
                                    >
                                        <FolderPlus className="w-4 h-4 mr-2" />
                                        New Folder
                                    </button>
                                    <button
                                        onClick={() => fileInputRef.current?.click()}
                                        className="flex items-center px-4 py-2 bg-primary-600 rounded-lg text-sm font-medium text-white hover:bg-primary-700 shadow-sm"
                                    >
                                        <Upload className="w-4 h-4 mr-2" />
                                        Upload Files
                                    </button>
                                </div>
                            )}
                        </div>
                    )}

                    {paginatedFiles.length > 0 && (viewMode === 'grid' ? (
                        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-8 min-[1800px]:grid-cols-10 min-[2200px]:grid-cols-12 min-[2800px]:grid-cols-14 min-[3200px]:grid-cols-16 gap-4 justify-items-center content-start">
                            {paginatedFiles.map((file, index) => (
                                file.type === 'group' ? (
                                    // Render FileGroupStack for group items
                                    <div key={file.id} className="w-full max-w-[180px] relative">
                                        <FileGroupStack
                                            id={file.id}
                                            name={file.name}
                                            color={file.color}
                                            fileCount={file.file_count || 0}
                                            totalSize={file.total_size}
                                            owner={file.owner}
                                            onClick={() => handleGroupClick(file)}
                                            onMenuClick={() => setActiveGroupMenu(activeGroupMenu === file.id ? null : file.id)}
                                            isSelected={selectedFiles.has(file.id)}
                                            isDraggable={!isSelectionMode}
                                            onDragStart={(e) => handleGroupDragStart(e, file)}
                                            isLocked={file.is_locked}
                                            lockRequiresRole={file.lock_requires_role}
                                        />
                                        {/* Group context menu */}
                                        {activeGroupMenu === file.id && (
                                            <div className="absolute top-12 right-2 z-50 bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 py-1 min-w-[160px]">
                                                {/* If locked and user can't access, show access denied message */}
                                                {file.is_locked && !canAccessLockedGroup(file) ? (
                                                    <>
                                                        {/* Show locked/access denied message */}
                                                        <div className="px-4 py-3 text-sm text-red-500 flex items-center">
                                                            <Lock className="w-4 h-4 mr-2" />
                                                            <span>Access denied{file.lock_requires_role ? ` - requires ${file.lock_requires_role}` : ''}</span>
                                                        </div>
                                                    </>
                                                ) : (
                                                    <>
                                                        <button
                                                            className="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center"
                                                            onClick={(e) => { e.stopPropagation(); handleGroupClick(file); setActiveGroupMenu(null); }}
                                                        >
                                                            <Layers className="w-4 h-4 mr-2 text-gray-400" /> Open Group
                                                        </button>
                                                        <button
                                                            className="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center"
                                                            onClick={(e) => { e.stopPropagation(); toggleStar(file); setActiveGroupMenu(null); }}
                                                        >
                                                            <Star className={clsx("w-4 h-4 mr-2", file.is_starred ? "text-yellow-400 fill-yellow-400" : "text-gray-400")} />
                                                            {file.is_starred ? 'Unstar' : 'Star'}
                                                        </button>
                                                        <button
                                                            className="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center"
                                                            onClick={(e) => { e.stopPropagation(); handleRenameGroup(file); }}
                                                        >
                                                            <Edit3 className="w-4 h-4 mr-2 text-gray-400" /> Rename
                                                        </button>
                                                        <button
                                                            className="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center"
                                                            onClick={(e) => { e.stopPropagation(); handleMoveGroup(file); }}
                                                        >
                                                            <Move className="w-4 h-4 mr-2 text-gray-400" /> Move Group
                                                        </button>
                                                        {/* Lock/Unlock Group */}
                                                        {file.is_locked ? (
                                                            <button
                                                                className="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center"
                                                                onClick={(e) => { e.stopPropagation(); handleUnlockGroup(file); }}
                                                            >
                                                                <Unlock className="w-4 h-4 mr-2 text-gray-400" /> Unlock Group
                                                            </button>
                                                        ) : (
                                                            <button
                                                                className="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center"
                                                                onClick={(e) => { e.stopPropagation(); handleLockGroup(file); }}
                                                            >
                                                                <Lock className="w-4 h-4 mr-2 text-gray-400" /> Lock Group
                                                            </button>
                                                        )}
                                                        <button
                                                            className="w-full px-4 py-2 text-left text-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 flex items-center"
                                                            onClick={(e) => { e.stopPropagation(); handleDeleteGroup(file); }}
                                                        >
                                                            <Trash2 className="w-4 h-4 mr-2" /> Delete Group
                                                        </button>
                                                    </>
                                                )}
                                            </div>
                                        )}
                                    </div>
                                ) : (
                                <div
                                    key={file.id}
                                    className={clsx(
                                        "group relative bg-white dark:bg-gray-800 border rounded-xl p-4 hover:shadow-lg transition-all cursor-pointer flex flex-col items-center text-center h-44 w-full max-w-[180px]",
                                        selectedFiles.has(file.id)
                                            ? "border-primary-400 dark:border-primary-500 ring-2 ring-primary-200 dark:ring-primary-800"
                                            : focusedFileIndex === (currentPage - 1) * itemsPerPage + index
                                                ? "border-primary-300 dark:border-primary-600 ring-2 ring-primary-100 dark:ring-primary-900/50"
                                                : "border-gray-200 dark:border-gray-700 hover:border-primary-300 dark:hover:border-primary-500"
                                    )}
                                    draggable={!isSelectionMode}
                                    onDragStart={(e) => {
                                        if (isSelectionMode) return;
                                        // Use correct handler for groups vs files
                                        if (file.type === 'group') {
                                            handleGroupDragStart(e, file);
                                        } else {
                                            handleFileDragStart(e, file);
                                        }
                                    }}
                                    onDragOver={(e) => file.type === 'folder' && handleFolderDragOver(e, file)}
                                    onDragLeave={handleFolderDragLeave}
                                    onDrop={(e) => file.type === 'folder' && handleFolderDrop(e, file)}
                                    onClick={(e) => {
                                        if (isSelectionMode) {
                                            e.stopPropagation();
                                            toggleFileSelection(file.id);
                                        }
                                    }}
                                >
                                    {/* Selection Checkbox */}
                                    {isSelectionMode && (
                                        <div 
                                            className="absolute top-2 left-2 z-10"
                                            onClick={(e) => { e.stopPropagation(); toggleFileSelection(file.id); }}
                                        >
                                            {selectedFiles.has(file.id) ? (
                                                <CheckSquare className="w-5 h-5 text-primary-600" />
                                            ) : (
                                                <Square className="w-5 h-5 text-gray-400 hover:text-primary-500" />
                                            )}
                                        </div>
                                    )}
                                    <div
                                        className="flex-1 flex items-center justify-center w-full mb-3 cursor-pointer"
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            if (isSelectionMode) return;
                                            if (file.type === 'group') {
                                                handleGroupClick(file);
                                            } else if (file.type === 'folder') {
                                                setCurrentPath([...currentPath, file.name]);
                                            } else {
                                                handlePreview(file);
                                            }
                                        }}
                                    >
                                        {getIcon(file)}
                                    </div>
                                    <div className="w-full">
                                        <p className="text-sm font-medium text-gray-900 dark:text-white truncate w-full" title={file.name}>{file.name}</p>
                                        <div className="flex items-center justify-between mt-1">
                                            <p className="text-xs text-gray-500 dark:text-gray-400">{file.size}</p>
                                            {/* Owner avatar or company icon with styled hover tooltip */}
                                            <div className="relative group/avatar">
                                                {file.type === 'folder' && file.is_company_folder ? (
                                                    <div className="h-10 w-10 rounded-full bg-gray-200 dark:bg-gray-600 flex items-center justify-center ring-2 ring-white dark:ring-gray-800 shadow-sm">
                                                        <Building2 className="w-5 h-5 text-gray-600 dark:text-gray-300" />
                                                    </div>
                                                ) : (
                                                    <Avatar 
                                                        src={file.owner_avatar} 
                                                        name={file.owner || 'Unknown'} 
                                                        size="md"
                                                        className="ring-2 ring-white dark:ring-gray-800 shadow-sm hover:ring-primary-300 dark:hover:ring-primary-600 transition-all cursor-default"
                                                    />
                                                )}
                                                {/* Styled tooltip */}
                                                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-1.5 bg-gray-900 dark:bg-gray-700 text-white text-xs rounded-lg opacity-0 group-hover/avatar:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-lg z-10">
                                                    <div className="font-medium">{file.type === 'folder' && file.is_company_folder ? 'Company Folder' : (file.owner || 'Unknown')}</div>
                                                    <div className="text-gray-400 text-[10px]">{file.type === 'folder' && file.is_company_folder ? 'Shared' : 'Owner'}</div>
                                                    {/* Tooltip arrow */}
                                                    <div className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-gray-900 dark:border-t-gray-700"></div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity flex space-x-1">
                                        {file.visibility === 'private' && <span title="Private - only visible to you"><EyeOff className="w-4 h-4 text-purple-500" /></span>}
                                        {file.is_locked && <span title="Locked"><Lock className="w-4 h-4 text-orange-500" /></span>}
                                        {file.is_starred && <Star className="w-4 h-4 text-yellow-400 fill-current" />}
                                        <button
                                            ref={(el) => { if (el) menuButtonRefs.current.set(`grid-${file.id}`, el); }}
                                            className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-full bg-white dark:bg-gray-800 shadow-sm border border-gray-100 dark:border-gray-700"
                                            onClick={(e) => { e.stopPropagation(); setActiveMenu(activeMenu === file.id ? null : file.id); }}
                                        >
                                            <MoreVertical className="w-4 h-4 text-gray-500" />
                                        </button>
                                        {activeMenu === file.id && (
                                            <FileActionMenu
                                                file={file}
                                                companyId={companyId || ''}
                                                complianceMode={currentCompany?.compliance_mode}
                                                canLockFiles={canLockFiles}
                                                canViewActivity={canViewActivity}
                                                canDelete={canDeleteFile(file)}
                                                canShare={canShareFile(file)}
                                                currentUserId={user?.id}
                                                currentUserRole={user?.role}
                                                canUseAi={aiStatus.hasAccess}
                                                aiEnabled={aiStatus.enabled}
                                                onPreview={handlePreview}
                                                onShare={handleShare}
                                                onDownload={handleDownload}
                                                onStar={toggleStar}
                                                onRename={(f) => { setFileToRename(f); setIsRenameOpen(true); }}
                                                onLock={handleLockToggle}
                                                onActivity={handleViewActivity}
                                                onMove={openMoveModal}
                                                onCopy={handleCopy}
                                                onDelete={handleDelete}
                                                onProperties={handleViewProperties}
                                                onToggleCompanyFolder={handleToggleCompanyFolder}
                                                onAiSummarize={handleAiSummarize}
                                                onAiQuestion={handleAiQuestion}
                                                groups={groups}
                                                isInsideGroup={!!currentGroup}
                                                onAddToGroup={handleAddToGroup}
                                                onRemoveFromGroup={handleRemoveFromGroup}
                                                onCreateGroupFromFile={handleCreateGroupFromFile}
                                                buttonRef={{ current: menuButtonRefs.current.get(`grid-${file.id}`) || null }}
                                            />
                                        )}
                                    </div>
                                </div>
                                )
                            ))}
                        </div>
                    ) : (
                        <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-gray-700">
                            <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                                <thead className="bg-gray-50 dark:bg-gray-900/50">
                                    <tr>
                                        {isSelectionMode && (
                                            <th className="px-4 py-3 w-10">
                                                <button
                                                    onClick={() => {
                                                        if (selectedFiles.size === paginatedFiles.length) {
                                                            setSelectedFiles(new Set());
                                                        } else {
                                                            selectAllFiles();
                                                        }
                                                    }}
                                                    className="text-gray-400 hover:text-primary-500"
                                                >
                                                    {selectedFiles.size === paginatedFiles.length && paginatedFiles.length > 0 ? (
                                                        <CheckSquare className="w-5 h-5 text-primary-600" />
                                                    ) : (
                                                        <Square className="w-5 h-5" />
                                                    )}
                                                </button>
                                            </th>
                                        )}
                                        <th
                                            className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
                                            onClick={() => handleSort('name')}
                                        >
                                            <div className="flex items-center">
                                                Name
                                                {sortBy === 'name' && (sortOrder === 'asc' ? <span className="ml-1">↑</span> : <span className="ml-1">↓</span>)}
                                            </div>
                                        </th>
                                        <th
                                            className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden sm:table-cell cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
                                            onClick={() => handleSort('size')}
                                        >
                                            <div className="flex items-center">
                                                Size
                                                {sortBy === 'size' && (sortOrder === 'asc' ? <span className="ml-1">↑</span> : <span className="ml-1">↓</span>)}
                                            </div>
                                        </th>
                                        <th
                                            className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden md:table-cell cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
                                            onClick={() => handleSort('modified')}
                                        >
                                            <div className="flex items-center">
                                                Modified
                                                {sortBy === 'modified' && (sortOrder === 'asc' ? <span className="ml-1">↑</span> : <span className="ml-1">↓</span>)}
                                            </div>
                                        </th>
                                        <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden lg:table-cell">Owner</th>
                                        <th className="relative px-6 py-3"><span className="sr-only">Actions</span></th>
                                    </tr>
                                </thead>
                                <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                                    {paginatedFiles.map((file, index) => (
                                        <tr
                                            key={file.id}
                                            className={clsx(
                                                "transition-colors cursor-pointer group",
                                                selectedFiles.has(file.id)
                                                    ? "bg-primary-50 dark:bg-primary-900/20"
                                                    : focusedFileIndex === (currentPage - 1) * itemsPerPage + index
                                                        ? "bg-primary-50/50 dark:bg-primary-900/10"
                                                        : "hover:bg-gray-50 dark:hover:bg-gray-700/50"
                                            )}
                                            draggable={!isSelectionMode}
                                            onDragStart={(e) => {
                                                if (isSelectionMode) return;
                                                // Use correct handler for groups vs files
                                                if (file.type === 'group') {
                                                    handleGroupDragStart(e, file);
                                                } else {
                                                    handleFileDragStart(e, file);
                                                }
                                            }}
                                            onDragOver={(e) => file.type === 'folder' && handleFolderDragOver(e, file)}
                                            onDragLeave={handleFolderDragLeave}
                                            onDrop={(e) => file.type === 'folder' && handleFolderDrop(e, file)}
                                            onClick={() => {
                                                if (isSelectionMode) {
                                                    toggleFileSelection(file.id);
                                                }
                                            }}
                                        >
                                            {isSelectionMode && (
                                                <td className="px-4 py-4 w-10">
                                                    <button
                                                        onClick={(e) => { e.stopPropagation(); toggleFileSelection(file.id); }}
                                                        className="text-gray-400 hover:text-primary-500"
                                                    >
                                                        {selectedFiles.has(file.id) ? (
                                                            <CheckSquare className="w-5 h-5 text-primary-600" />
                                                        ) : (
                                                            <Square className="w-5 h-5" />
                                                        )}
                                                    </button>
                                                </td>
                                            )}
                                            <td className="px-6 py-4 whitespace-nowrap cursor-pointer" onClick={(e) => {
                                                e.stopPropagation();
                                                if (isSelectionMode) return;
                                                if (file.type === 'group') {
                                                    handleGroupClick(file);
                                                } else if (file.type === 'folder') {
                                                    setCurrentPath([...currentPath, file.name]);
                                                } else {
                                                    handlePreview(file);
                                                }
                                            }}>
                                                <div className="flex items-center">
                                                    <div className="flex-shrink-0 h-8 w-8 flex items-center justify-center">
                                                        {getIcon(file)}
                                                    </div>
                                                    <div className="ml-4">
                                                        <div className="text-sm font-medium text-gray-900 dark:text-white flex items-center">
                                                            {file.name}
                                                            {file.visibility === 'private' && <span title="Private"><EyeOff className="w-3.5 h-3.5 ml-2 text-purple-500" /></span>}
                                                            {file.is_locked && <span title="Locked"><Lock className="w-3.5 h-3.5 ml-1 text-orange-500" /></span>}
                                                        </div>
                                                        <div className="sm:hidden text-xs text-gray-500 dark:text-gray-400">{file.size} • {file.modified}</div>
                                                    </div>
                                                </div>
                                            </td>
                                            <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden sm:table-cell">
                                                {file.size}
                                            </td>
                                            <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden md:table-cell">
                                                {file.modified}
                                            </td>
                                            <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden lg:table-cell">
                                                <div className="flex items-center" title={file.type === 'folder' && file.is_company_folder ? 'Company Folder' : (file.owner || 'Unknown')}>
                                                    {file.type === 'folder' && file.is_company_folder ? (
                                                        <div className="h-6 w-6 rounded-full bg-gray-200 dark:bg-gray-600 flex items-center justify-center mr-2">
                                                            <Building2 className="w-3 h-3 text-gray-600 dark:text-gray-300" />
                                                        </div>
                                                    ) : (
                                                        <Avatar 
                                                            src={file.owner_avatar} 
                                                            name={file.owner || 'Unknown'} 
                                                            size="xs"
                                                            className="mr-2"
                                                        />
                                                    )}
                                                    <span>{file.type === 'folder' && file.is_company_folder ? 'Company' : (file.owner || 'Unknown')}</span>
                                                </div>
                                            </td>
                                            <td className="px-6 py-4 whitespace-nowrap text-right text-sm font-medium relative">
                                                <div className="opacity-0 group-hover:opacity-100 transition-opacity">
                                                    <button
                                                        ref={(el) => { if (el) menuButtonRefs.current.set(`list-${file.id}`, el); }}
                                                        className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 p-1 rounded-full hover:bg-gray-100 dark:hover:bg-gray-700"
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            setActiveMenu(activeMenu === file.id ? null : file.id);
                                                        }}
                                                    >
                                                        <MoreVertical className="w-5 h-5" />
                                                    </button>
                                                </div>
                                                {activeMenu === file.id && (
                                                    <FileActionMenu
                                                        file={file}
                                                        companyId={companyId || ''}
                                                        complianceMode={currentCompany?.compliance_mode}
                                                        canLockFiles={canLockFiles}
                                                        canViewActivity={canViewActivity}
                                                        canDelete={canDeleteFile(file)}
                                                        canShare={canShareFile(file)}
                                                        currentUserId={user?.id}
                                                        currentUserRole={user?.role}
                                                        canUseAi={aiStatus.hasAccess}
                                                        aiEnabled={aiStatus.enabled}
                                                        onPreview={handlePreview}
                                                        onShare={handleShare}
                                                        onDownload={handleDownload}
                                                        onStar={toggleStar}
                                                        onRename={(f) => { setFileToRename(f); setIsRenameOpen(true); }}
                                                        onLock={handleLockToggle}
                                                        onActivity={handleViewActivity}
                                                        onMove={openMoveModal}
                                                        onCopy={handleCopy}
                                                        onDelete={handleDelete}
                                                        onProperties={handleViewProperties}
                                                        onToggleCompanyFolder={handleToggleCompanyFolder}
                                                        onAiSummarize={handleAiSummarize}
                                                        onAiQuestion={handleAiQuestion}
                                                        groups={groups}
                                                        isInsideGroup={!!currentGroup}
                                                        onAddToGroup={handleAddToGroup}
                                                        onRemoveFromGroup={handleRemoveFromGroup}
                                                        onCreateGroupFromFile={handleCreateGroupFromFile}
                                                        buttonRef={{ current: menuButtonRefs.current.get(`list-${file.id}`) || null }}
                                                    />
                                                )}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    ))}

                    {/* Pagination Controls */}
                    {totalPages > 1 && (
                        <div className="flex items-center justify-between mt-6 pt-4 border-t border-gray-200 dark:border-gray-700">
                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                Showing {((currentPage - 1) * itemsPerPage) + 1} - {Math.min(currentPage * itemsPerPage, filteredFiles.length)} of {filteredFiles.length} items
                            </p>
                            <div className="flex items-center space-x-2">
                                <button
                                    onClick={() => setCurrentPage(prev => Math.max(1, prev - 1))}
                                    disabled={currentPage === 1}
                                    className={clsx(
                                        "flex items-center px-3 py-1.5 text-sm font-medium rounded-lg border transition-colors",
                                        currentPage === 1
                                            ? "text-gray-400 dark:text-gray-600 border-gray-200 dark:border-gray-700 cursor-not-allowed"
                                            : "text-gray-700 dark:text-gray-200 border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                                    )}
                                >
                                    <ChevronLeft className="w-4 h-4 mr-1" />
                                    Previous
                                </button>
                                <div className="flex items-center space-x-1">
                                    {Array.from({ length: totalPages }, (_, i) => i + 1)
                                        .filter(page => {
                                            // Show first, last, current, and pages around current
                                            return page === 1 || page === totalPages || Math.abs(page - currentPage) <= 1;
                                        })
                                        .map((page, index, array) => (
                                            <React.Fragment key={page}>
                                                {index > 0 && array[index - 1] !== page - 1 && (
                                                    <span className="px-2 text-gray-400">...</span>
                                                )}
                                                <button
                                                    onClick={() => setCurrentPage(page)}
                                                    className={clsx(
                                                        "w-8 h-8 text-sm font-medium rounded-lg transition-colors",
                                                        currentPage === page
                                                            ? "bg-primary-600 text-white"
                                                            : "text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                                    )}
                                                >
                                                    {page}
                                                </button>
                                            </React.Fragment>
                                        ))}
                                </div>
                                <button
                                    onClick={() => setCurrentPage(prev => Math.min(totalPages, prev + 1))}
                                    disabled={currentPage === totalPages}
                                    className={clsx(
                                        "flex items-center px-3 py-1.5 text-sm font-medium rounded-lg border transition-colors",
                                        currentPage === totalPages
                                            ? "text-gray-400 dark:text-gray-600 border-gray-200 dark:border-gray-700 cursor-not-allowed"
                                            : "text-gray-700 dark:text-gray-200 border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
                                    )}
                                >
                                    Next
                                    <ChevronRight className="w-4 h-4 ml-1" />
                                </button>
                            </div>
                        </div>
                    )}
                </div>
            </div>

            {/* Create Group Modal */}
            <CreateGroupModal
                isOpen={isCreateGroupOpen}
                onClose={() => { setIsCreateGroupOpen(false); setPendingGroupFile(null); }}
                onCreate={handleCreateGroup}
                initialFileName={pendingGroupFile?.name}
            />

            {/* File Group Viewer Modal */}
            <FileGroupViewer
                isOpen={isGroupViewerOpen}
                isMinimized={isGroupViewerMinimized}
                group={viewingGroup}
                files={groupFiles}
                isLoadingFiles={isLoadingGroupFiles}
                onClose={() => { setIsGroupViewerOpen(false); setIsGroupViewerMinimized(false); setViewingGroup(null); setGroupFiles([]); setIsLoadingGroupFiles(false); }}
                onMinimize={() => setIsGroupViewerMinimized(true)}
                onExpand={() => setIsGroupViewerMinimized(false)}
                onPreview={handleGroupViewerPreview}
                onDownload={handleGroupViewerDownload}
                onRemoveFromGroup={handleGroupViewerRemove}
                onMoveToFolder={handleGroupViewerMoveToFolder}
                onCopy={handleGroupViewerCopy}
                onShare={handleGroupViewerShare}
                onProperties={handleGroupViewerProperties}
                onAiSummarize={handleGroupViewerAiSummarize}
                onAiQuestion={handleGroupViewerAiQuestion}
                aiEnabled={aiStatus.enabled}
                canUseAi={aiStatus.hasAccess}
                companyId={companyId || ''}
                authFetch={authFetch}
            />
        </div>
    );
}
