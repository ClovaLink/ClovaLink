import React, { useState, useEffect, useMemo } from 'react';
import {
    X, Layers, Eye, Download, FolderOutput, Trash2, 
    FileText, Image, Film, Music, File, MoreHorizontal,
    Search, ChevronLeft, ChevronRight, Maximize2,
    Copy, Share2, Sparkles, MessageSquare, Info,
    ChevronDown, ArrowUpDown
} from 'lucide-react';
import clsx from 'clsx';

type SortOption = 'newest' | 'oldest' | 'name-asc' | 'name-desc' | 'size-largest' | 'size-smallest' | 'type';

const SORT_OPTIONS: { value: SortOption; label: string }[] = [
    { value: 'newest', label: 'Newest first' },
    { value: 'oldest', label: 'Oldest first' },
    { value: 'name-asc', label: 'Name (A-Z)' },
    { value: 'name-desc', label: 'Name (Z-A)' },
    { value: 'size-largest', label: 'Size (largest)' },
    { value: 'size-smallest', label: 'Size (smallest)' },
    { value: 'type', label: 'File type' },
];

interface GroupFile {
    id: string;
    name: string;
    content_type?: string;
    size_bytes?: number;
    parent_path?: string;
    created_at?: string;
    owner_id?: string;
    is_starred?: boolean;
}

interface FileGroup {
    id: string;
    name: string;
    description?: string;
    color?: string;
    file_count: number;
}

interface FileGroupViewerProps {
    isOpen: boolean;
    isMinimized?: boolean;
    group: FileGroup | null;
    files: GroupFile[];
    isLoadingFiles?: boolean;
    onClose: () => void;
    onMinimize?: () => void;
    onExpand?: () => void;
    onPreview: (file: GroupFile) => void;
    onDownload: (file: GroupFile) => void;
    onRemoveFromGroup: (file: GroupFile) => void;
    onMoveToFolder: (file: GroupFile) => void;
    // New action handlers (Star removed - star the group instead)
    onCopy?: (file: GroupFile) => void;
    onShare?: (file: GroupFile) => void;
    onProperties?: (file: GroupFile) => void;
    onAiSummarize?: (file: GroupFile) => void;
    onAiQuestion?: (file: GroupFile) => void;
    // AI status
    aiEnabled?: boolean;
    canUseAi?: boolean;
    companyId: string;
    authFetch: (url: string, options?: RequestInit) => Promise<Response>;
}

const ITEMS_PER_PAGE = 5;

// Format file size
const formatSize = (bytes?: number) => {
    if (!bytes) return '--';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
};

// Get file extension
const getExtension = (name: string) => {
    const parts = name.split('.');
    return parts.length > 1 ? parts.pop()?.toUpperCase() : '';
};

// Get icon based on content type
const getFileIcon = (contentType?: string, name?: string) => {
    const iconClass = "w-8 h-8";
    if (!contentType) {
        const ext = name?.split('.').pop()?.toLowerCase();
        if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext || '')) {
            return <Image className={clsx(iconClass, 'text-purple-500')} />;
        }
        if (['mp4', 'webm', 'mov', 'avi'].includes(ext || '')) {
            return <Film className={clsx(iconClass, 'text-pink-500')} />;
        }
        if (['mp3', 'wav', 'ogg', 'flac'].includes(ext || '')) {
            return <Music className={clsx(iconClass, 'text-green-500')} />;
        }
        if (['pdf'].includes(ext || '')) {
            return <FileText className={clsx(iconClass, 'text-red-500')} />;
        }
        return <File className={iconClass} />;
    }
    if (contentType.startsWith('image/')) return <Image className={clsx(iconClass, 'text-purple-500')} />;
    if (contentType.startsWith('video/')) return <Film className={clsx(iconClass, 'text-pink-500')} />;
    if (contentType.startsWith('audio/')) return <Music className={clsx(iconClass, 'text-green-500')} />;
    if (contentType.includes('pdf')) return <FileText className={clsx(iconClass, 'text-red-500')} />;
    return <FileText className={clsx(iconClass, 'text-blue-500')} />;
};

// Check if file is previewable
const isPreviewable = (contentType?: string, name?: string) => {
    if (contentType) {
        return contentType.startsWith('image/') || 
               contentType.startsWith('video/') || 
               contentType.startsWith('audio/') ||
               contentType.includes('pdf') ||
               contentType.startsWith('text/');
    }
    const ext = name?.split('.').pop()?.toLowerCase();
    return ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'pdf', 'mp4', 'webm', 'mp3', 'wav', 'txt', 'md'].includes(ext || '');
};

// Check if file can use AI features (text-based documents only)
const canUseAiFeatures = (contentType?: string, name?: string) => {
    if (contentType) {
        return contentType.includes('pdf') || 
               contentType.includes('text') ||
               contentType.includes('word') ||
               contentType.includes('document') ||
               contentType.includes('spreadsheet') ||
               contentType.includes('presentation');
    }
    const ext = name?.split('.').pop()?.toLowerCase();
    return ['pdf', 'txt', 'md', 'doc', 'docx', 'xlsx', 'pptx'].includes(ext || '');
};

// Format date to relative time
const formatDate = (dateString?: string) => {
    if (!dateString) return '--';
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);
    
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
};

export function FileGroupViewer({
    isOpen,
    isMinimized = false,
    group,
    files,
    isLoadingFiles = false,
    onClose,
    onMinimize,
    onExpand,
    onPreview,
    onDownload,
    onRemoveFromGroup,
    onMoveToFolder,
    onCopy,
    onShare,
    onProperties,
    onAiSummarize,
    onAiQuestion,
    aiEnabled = false,
    canUseAi = false,
    companyId,
    authFetch,
}: FileGroupViewerProps) {
    const [searchQuery, setSearchQuery] = useState('');
    const [activeMenu, setActiveMenu] = useState<string | null>(null);
    const [currentPage, setCurrentPage] = useState(1);
    const [sortOption, setSortOption] = useState<SortOption>('newest');
    const [isSortDropdownOpen, setIsSortDropdownOpen] = useState(false);

    // Reset state when modal opens/closes
    useEffect(() => {
        if (!isOpen) {
            setSearchQuery('');
            setActiveMenu(null);
            setCurrentPage(1);
            setSortOption('newest');
            setIsSortDropdownOpen(false);
        }
    }, [isOpen]);

    // Close menus when clicking outside
    useEffect(() => {
        const handleClick = () => {
            setActiveMenu(null);
            setIsSortDropdownOpen(false);
        };
        if (activeMenu || isSortDropdownOpen) {
            document.addEventListener('click', handleClick);
            return () => document.removeEventListener('click', handleClick);
        }
    }, [activeMenu, isSortDropdownOpen]);

    // Keyboard navigation
    useEffect(() => {
        if (!isOpen || isMinimized) return;
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isOpen, isMinimized, onClose]);

    // Sort files based on selected option
    const sortedFiles = useMemo(() => {
        return [...files].sort((a, b) => {
            switch (sortOption) {
                case 'newest': {
                    const dateA = a.created_at ? new Date(a.created_at).getTime() : 0;
                    const dateB = b.created_at ? new Date(b.created_at).getTime() : 0;
                    return dateB - dateA;
                }
                case 'oldest': {
                    const dateA = a.created_at ? new Date(a.created_at).getTime() : 0;
                    const dateB = b.created_at ? new Date(b.created_at).getTime() : 0;
                    return dateA - dateB;
                }
                case 'name-asc':
                    return a.name.localeCompare(b.name);
                case 'name-desc':
                    return b.name.localeCompare(a.name);
                case 'size-largest':
                    return (b.size_bytes || 0) - (a.size_bytes || 0);
                case 'size-smallest':
                    return (a.size_bytes || 0) - (b.size_bytes || 0);
                case 'type': {
                    const extA = a.name.split('.').pop()?.toLowerCase() || '';
                    const extB = b.name.split('.').pop()?.toLowerCase() || '';
                    return extA.localeCompare(extB);
                }
                default:
                    return 0;
            }
        });
    }, [files, sortOption]);

    // Filter files by search
    const filteredFiles = useMemo(() => {
        return sortedFiles.filter(f => 
            f.name.toLowerCase().includes(searchQuery.toLowerCase())
        );
    }, [sortedFiles, searchQuery]);

    // Pagination
    const totalPages = Math.ceil(filteredFiles.length / ITEMS_PER_PAGE);
    const paginatedFiles = useMemo(() => {
        const start = (currentPage - 1) * ITEMS_PER_PAGE;
        return filteredFiles.slice(start, start + ITEMS_PER_PAGE);
    }, [filteredFiles, currentPage]);

    // Reset to page 1 when search changes
    useEffect(() => {
        setCurrentPage(1);
    }, [searchQuery]);

    if (!isOpen || !group) return null;

    // Minimized pill view
    if (isMinimized) {
        return (
            <div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-50">
                <button
                    onClick={onExpand}
                    className="flex items-center gap-2 px-4 py-2 bg-white dark:bg-gray-800 rounded-full shadow-xl border border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
                >
                    <div 
                        className="p-1.5 rounded-md"
                        style={{ backgroundColor: `${group.color || '#3B82F6'}20` }}
                    >
                        <Layers className="w-4 h-4" style={{ color: group.color || '#3B82F6' }} />
                    </div>
                    <span className="text-sm font-medium text-gray-900 dark:text-white">
                        {group.name}
                    </span>
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                        {files.length} files
                    </span>
                    <Maximize2 className="w-4 h-4 text-gray-400" />
                </button>
            </div>
        );
    }

    const menuItemClass = "flex items-center w-full px-3 py-2 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700";

    return (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow-2xl w-full max-w-4xl min-h-[70vh] max-h-[90vh] flex flex-col overflow-hidden">
                {/* Header */}
                <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
                    <div className="flex items-center gap-3">
                        <div 
                            className="p-2.5 rounded-lg"
                            style={{ backgroundColor: `${group.color || '#3B82F6'}20` }}
                        >
                            <Layers className="w-6 h-6" style={{ color: group.color || '#3B82F6' }} />
                        </div>
                        <div>
                            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                                {group.name}
                            </h2>
                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                {files.length} {files.length === 1 ? 'file' : 'files'}
                                {group.description && ` • ${group.description}`}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={onClose}
                        className="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Toolbar */}
                <div className="flex items-center gap-3 p-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50">
                    {/* Search */}
                    <div className="flex-1 relative">
                        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
                        <input
                            type="text"
                            placeholder="Search files..."
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                            className="w-full pl-9 pr-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                        />
                    </div>
                    {/* Sort dropdown */}
                    <div className="relative">
                        <button
                            onClick={(e) => { e.stopPropagation(); setIsSortDropdownOpen(!isSortDropdownOpen); }}
                            className="flex items-center gap-1.5 px-3 py-2 text-sm text-gray-600 dark:text-gray-300 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
                        >
                            <ArrowUpDown className="w-3.5 h-3.5" />
                            <span className="hidden sm:inline">{SORT_OPTIONS.find(o => o.value === sortOption)?.label}</span>
                            <ChevronDown className={clsx("w-3.5 h-3.5 transition-transform", isSortDropdownOpen && "rotate-180")} />
                        </button>
                        
                        {isSortDropdownOpen && (
                            <div className="absolute right-0 top-full mt-1 w-44 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 py-1 z-30">
                                {SORT_OPTIONS.map((option) => (
                                    <button
                                        key={option.value}
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            setSortOption(option.value);
                                            setIsSortDropdownOpen(false);
                                            setCurrentPage(1); // Reset to page 1 when sort changes
                                        }}
                                        className={clsx(
                                            "flex items-center w-full px-3 py-2 text-sm transition-colors",
                                            sortOption === option.value
                                                ? "bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400"
                                                : "text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                        )}
                                    >
                                        {option.label}
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>
                </div>

                {/* Content */}
                <div className="flex-1 overflow-y-auto">
                    {isLoadingFiles ? (
                        <div className="flex flex-col items-center justify-center py-16 text-gray-400">
                            <div className="w-10 h-10 border-4 border-gray-300 dark:border-gray-600 border-t-blue-500 rounded-full animate-spin mb-4" />
                            <p>Loading files...</p>
                        </div>
                    ) : filteredFiles.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-gray-400">
                            <Layers className="w-12 h-12 mb-4 opacity-50" />
                            <p className="text-lg font-medium">
                                {searchQuery ? 'No files match your search' : 'This group is empty'}
                            </p>
                            <p className="text-sm mt-1">
                                {searchQuery ? 'Try a different search term' : 'Add files to this group from the file browser'}
                            </p>
                        </div>
                    ) : (
                        <div className="divide-y divide-gray-100 dark:divide-gray-700">
                            {paginatedFiles.map((file) => {
                                const canAi = aiEnabled && canUseAi && canUseAiFeatures(file.content_type, file.name);
                                
                                return (
                                    <div
                                        key={file.id}
                                        className="group flex items-center gap-4 px-4 py-3 hover:bg-gray-50 dark:hover:bg-gray-700/30 cursor-pointer transition-colors"
                                        onClick={() => isPreviewable(file.content_type, file.name) && onPreview(file)}
                                    >
                                        {/* File icon */}
                                        <div className="flex-shrink-0">
                                            {getFileIcon(file.content_type, file.name)}
                                        </div>
                                        
                                        {/* File info */}
                                        <div className="flex-1 min-w-0">
                                            <div className="flex items-center gap-2">
                                                <p className="text-sm font-medium text-gray-900 dark:text-white truncate">
                                                    {file.name}
                                                </p>
                                            </div>
                                            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                                {formatSize(file.size_bytes)}
                                                <span className="mx-1.5">•</span>
                                                {getExtension(file.name)}
                                                <span className="mx-1.5">•</span>
                                                {formatDate(file.created_at)}
                                            </p>
                                        </div>

                                        {/* Quick actions */}
                                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                                            {isPreviewable(file.content_type, file.name) && (
                                                <button
                                                    onClick={(e) => { e.stopPropagation(); onPreview(file); }}
                                                    className="p-2 text-gray-400 hover:text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/30 rounded-lg transition-colors"
                                                    title="Preview"
                                                >
                                                    <Eye className="w-4 h-4" />
                                                </button>
                                            )}
                                            <button
                                                onClick={(e) => { e.stopPropagation(); onDownload(file); }}
                                                className="p-2 text-gray-400 hover:text-green-600 hover:bg-green-50 dark:hover:bg-green-900/30 rounded-lg transition-colors"
                                                title="Download"
                                            >
                                                <Download className="w-4 h-4" />
                                            </button>
                                            
                                            {/* More actions menu */}
                                            <div className="relative">
                                                <button
                                                    onClick={(e) => { 
                                                        e.stopPropagation(); 
                                                        setActiveMenu(activeMenu === file.id ? null : file.id); 
                                                    }}
                                                    className="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                                                    title="More actions"
                                                >
                                                    <MoreHorizontal className="w-4 h-4" />
                                                </button>
                                                
                                                {activeMenu === file.id && (
                                                    <div className="absolute right-0 top-full mt-1 w-56 bg-white dark:bg-gray-800 rounded-xl shadow-xl border border-gray-200 dark:border-gray-700 py-1 z-30 overflow-hidden">
                                                        {/* AI Actions */}
                                                        {canAi && (
                                                            <>
                                                                <div className="px-3 py-1.5 text-xs font-medium text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-900/50">
                                                                    AI Actions
                                                                </div>
                                                                {onAiSummarize && (
                                                                    <button
                                                                        onClick={(e) => { e.stopPropagation(); onAiSummarize(file); setActiveMenu(null); }}
                                                                        className={menuItemClass}
                                                                    >
                                                                        <Sparkles className="w-4 h-4 mr-2 text-purple-500" />
                                                                        Summarize
                                                                    </button>
                                                                )}
                                                                {onAiQuestion && (
                                                                    <button
                                                                        onClick={(e) => { e.stopPropagation(); onAiQuestion(file); setActiveMenu(null); }}
                                                                        className={menuItemClass}
                                                                    >
                                                                        <MessageSquare className="w-4 h-4 mr-2 text-blue-500" />
                                                                        Ask AI
                                                                    </button>
                                                                )}
                                                                <div className="border-t border-gray-100 dark:border-gray-700 my-1" />
                                                            </>
                                                        )}

                                                        {/* Standard Actions */}
                                                        {/* Note: Star is intentionally removed for grouped files - star the group instead */}
                                                        {onCopy && (
                                                            <button
                                                                onClick={(e) => { e.stopPropagation(); onCopy(file); setActiveMenu(null); }}
                                                                className={menuItemClass}
                                                            >
                                                                <Copy className="w-4 h-4 mr-2 text-gray-400" />
                                                                Copy
                                                            </button>
                                                        )}
                                                        {onShare && (
                                                            <button
                                                                onClick={(e) => { e.stopPropagation(); onShare(file); setActiveMenu(null); }}
                                                                className={menuItemClass}
                                                            >
                                                                <Share2 className="w-4 h-4 mr-2 text-gray-400" />
                                                                Share
                                                            </button>
                                                        )}
                                                        
                                                        <div className="border-t border-gray-100 dark:border-gray-700 my-1" />
                                                        
                                                        <button
                                                            onClick={(e) => { e.stopPropagation(); onMoveToFolder(file); setActiveMenu(null); }}
                                                            className={menuItemClass}
                                                        >
                                                            <FolderOutput className="w-4 h-4 mr-2 text-gray-400" />
                                                            Move to Folder
                                                        </button>
                                                        
                                                        {onProperties && (
                                                            <button
                                                                onClick={(e) => { e.stopPropagation(); onProperties(file); setActiveMenu(null); }}
                                                                className={menuItemClass}
                                                            >
                                                                <Info className="w-4 h-4 mr-2 text-gray-400" />
                                                                Properties
                                                            </button>
                                                        )}
                                                        
                                                        <div className="border-t border-gray-100 dark:border-gray-700 my-1" />
                                                        
                                                        <button
                                                            onClick={(e) => { e.stopPropagation(); onRemoveFromGroup(file); setActiveMenu(null); }}
                                                            className={clsx(menuItemClass, "text-red-600 dark:text-red-400")}
                                                        >
                                                            <Trash2 className="w-4 h-4 mr-2" />
                                                            Remove from Group
                                                        </button>
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>

                {/* Footer with pagination */}
                {filteredFiles.length > 0 && (
                    <div className="px-4 py-3 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 flex items-center justify-between">
                        <div className="text-sm text-gray-500 dark:text-gray-400">
                            {filteredFiles.length} {filteredFiles.length === 1 ? 'file' : 'files'}
                            {searchQuery && ` matching "${searchQuery}"`}
                        </div>
                        
                        {totalPages > 1 && (
                            <div className="flex items-center gap-2">
                                <button
                                    onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                                    disabled={currentPage === 1}
                                    className={clsx(
                                        "p-1.5 rounded-lg transition-colors",
                                        currentPage === 1 
                                            ? "text-gray-300 dark:text-gray-600 cursor-not-allowed"
                                            : "text-gray-500 hover:text-gray-700 hover:bg-gray-200 dark:hover:bg-gray-700"
                                    )}
                                >
                                    <ChevronLeft className="w-4 h-4" />
                                </button>
                                
                                <span className="text-sm text-gray-600 dark:text-gray-300 min-w-[80px] text-center">
                                    Page {currentPage} of {totalPages}
                                </span>
                                
                                <button
                                    onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                                    disabled={currentPage === totalPages}
                                    className={clsx(
                                        "p-1.5 rounded-lg transition-colors",
                                        currentPage === totalPages 
                                            ? "text-gray-300 dark:text-gray-600 cursor-not-allowed"
                                            : "text-gray-500 hover:text-gray-700 hover:bg-gray-200 dark:hover:bg-gray-700"
                                    )}
                                >
                                    <ChevronRight className="w-4 h-4" />
                                </button>
                            </div>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}

export default FileGroupViewer;
