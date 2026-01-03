import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Share2, Download, FileText, Image, Film, Music, Folder, ChevronLeft, ChevronRight, Loader2, Eye, FolderPlus, Check } from 'lucide-react';
import { format } from 'date-fns';
import clsx from 'clsx';
import { useAuthFetch } from '../context/AuthContext';
import { FilePreviewModal } from '../components/FilePreviewModal';

interface SharedFile {
    id: string;
    name: string;
    size: number;
    content_type: string | null;
    folder_path: string | null;
    shared_by_id: string;
    shared_by_name: string;
    shared_at: string;
    share_token: string;
    expires_at: string | null;
}

const getFileIcon = (contentType: string | null, name: string) => {
    if (!contentType) {
        const ext = name.split('.').pop()?.toLowerCase();
        if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext || '')) {
            return <Image className="w-8 h-8 text-green-500" />;
        }
        if (['mp4', 'mov', 'avi', 'webm'].includes(ext || '')) {
            return <Film className="w-8 h-8 text-purple-500" />;
        }
        if (['mp3', 'wav', 'flac', 'aac'].includes(ext || '')) {
            return <Music className="w-8 h-8 text-pink-500" />;
        }
        return <FileText className="w-8 h-8 text-blue-500" />;
    }
    
    if (contentType.startsWith('image/')) return <Image className="w-8 h-8 text-green-500" />;
    if (contentType.startsWith('video/')) return <Film className="w-8 h-8 text-purple-500" />;
    if (contentType.startsWith('audio/')) return <Music className="w-8 h-8 text-pink-500" />;
    return <FileText className="w-8 h-8 text-blue-500" />;
};

const getFileType = (contentType: string | null, name: string): 'image' | 'document' | 'video' | 'audio' | 'folder' => {
    if (!contentType) {
        const ext = name.split('.').pop()?.toLowerCase();
        if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp'].includes(ext || '')) return 'image';
        if (['mp4', 'mov', 'avi', 'webm', 'mkv'].includes(ext || '')) return 'video';
        if (['mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a'].includes(ext || '')) return 'audio';
        return 'document';
    }
    
    if (contentType.startsWith('image/')) return 'image';
    if (contentType.startsWith('video/')) return 'video';
    if (contentType.startsWith('audio/')) return 'audio';
    return 'document';
};

const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

export function SharedWithMe() {
    const authFetch = useAuthFetch();
    const navigate = useNavigate();
    
    const [files, setFiles] = useState<SharedFile[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [page, setPage] = useState(1);
    const [totalPages, setTotalPages] = useState(1);
    const [total, setTotal] = useState(0);
    const perPage = 20;
    
    // Preview modal state
    const [previewFile, setPreviewFile] = useState<{ name: string; url: string; type: 'image' | 'document' | 'video' | 'audio' | 'folder' } | null>(null);
    const [isPreviewOpen, setIsPreviewOpen] = useState(false);
    
    // Save to My Files state
    const [savingFileId, setSavingFileId] = useState<string | null>(null);
    const [savedFileIds, setSavedFileIds] = useState<Set<string>>(new Set());
    const [saveMessage, setSaveMessage] = useState<string | null>(null);

    const fetchSharedFiles = useCallback(async () => {
        setLoading(true);
        setError(null);
        
        try {
            const res = await authFetch(`/api/shared-with-me?page=${page}&per_page=${perPage}`);
            if (res.ok) {
                const data = await res.json();
                setFiles(data.files || []);
                setTotalPages(data.total_pages || 1);
                setTotal(data.total || 0);
            } else {
                setError('Failed to load shared files');
            }
        } catch {
            setError('Failed to load shared files');
        } finally {
            setLoading(false);
        }
    }, [authFetch, page, perPage]);

    useEffect(() => {
        fetchSharedFiles();
    }, [fetchSharedFiles]);

    const handleDownload = async (file: SharedFile, e?: React.MouseEvent) => {
        e?.stopPropagation();
        try {
            const response = await authFetch(`/api/share/${file.share_token}`, {
                headers: { 'Accept': '*/*' }
            });
            
            if (response.ok) {
                const blob = await response.blob();
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = file.name;
                a.click();
                URL.revokeObjectURL(url);
            }
        } catch {
            // Silent fail for downloads
        }
    };
    
    const handlePreview = (file: SharedFile, e?: React.MouseEvent) => {
        e?.stopPropagation();
        const fileType = getFileType(file.content_type, file.name);
        setPreviewFile({
            name: file.name,
            url: `/api/share/${file.share_token}`,
            type: fileType,
        });
        setIsPreviewOpen(true);
    };
    
    const handleMyFilesClick = () => {
        navigate('/files');
    };
    
    const handleSaveToMyFiles = async (file: SharedFile, e?: React.MouseEvent) => {
        e?.stopPropagation();
        
        if (savingFileId || savedFileIds.has(file.id)) return;
        
        setSavingFileId(file.id);
        setSaveMessage(null);
        
        try {
            const res = await authFetch('/api/shared-with-me/copy', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    file_id: file.id,
                    share_token: file.share_token,
                }),
            });
            
            if (res.ok) {
                const data = await res.json();
                setSavedFileIds(prev => new Set(prev).add(file.id));
                setSaveMessage(data.message || `"${file.name}" saved to your files`);
                setTimeout(() => setSaveMessage(null), 3000);
            } else {
                const errorData = await res.json().catch(() => ({}));
                setSaveMessage(errorData.error || 'Failed to save file');
                setTimeout(() => setSaveMessage(null), 3000);
            }
        } catch {
            setSaveMessage('Failed to save file');
            setTimeout(() => setSaveMessage(null), 3000);
        } finally {
            setSavingFileId(null);
        }
    };

    return (
        <div className="min-h-screen bg-gray-50 dark:bg-gray-900">
            {/* Header */}
            <div className="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
                <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-4">
                            <div className="p-3 bg-primary-100 dark:bg-primary-900/30 rounded-xl">
                                <Share2 className="w-6 h-6 text-primary-600 dark:text-primary-400" />
                            </div>
                            <div>
                                <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Shared with Me</h1>
                                <p className="text-sm text-gray-500 dark:text-gray-400">
                                    Files and folders others have shared with you
                                </p>
                            </div>
                        </div>
                        <button
                            onClick={handleMyFilesClick}
                            className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-200 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors"
                        >
                            <Folder className="w-4 h-4" />
                            My Files
                        </button>
                    </div>
                </div>
            </div>

            {/* Content */}
            <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                {/* Save Message Toast */}
                {saveMessage && (
                    <div className="mb-4 p-4 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg text-green-700 dark:text-green-300 text-sm flex items-center gap-2">
                        <Check className="w-4 h-4" />
                        {saveMessage}
                    </div>
                )}
                
                {/* Loading */}
                {loading && (
                    <div className="flex items-center justify-center py-12">
                        <Loader2 className="w-8 h-8 text-primary-600 animate-spin" />
                    </div>
                )}

                {/* Error */}
                {error && (
                    <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-300 text-sm">
                        {error}
                    </div>
                )}

                {/* Empty State */}
                {!loading && !error && files.length === 0 && (
                    <div className="text-center py-12">
                        <Share2 className="w-12 h-12 mx-auto text-gray-400 mb-4" />
                        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">No shared files</h3>
                        <p className="text-gray-500 dark:text-gray-400">
                            When someone shares a file with you, it will appear here.
                        </p>
                    </div>
                )}

                {/* Files List */}
                {!loading && files.length > 0 && (
                    <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 shadow-sm overflow-hidden">
                        {/* Table Header */}
                        <div className="hidden md:grid grid-cols-12 gap-4 px-6 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                            <div className="col-span-5">Name</div>
                            <div className="col-span-2">Shared By</div>
                            <div className="col-span-2">Shared On</div>
                            <div className="col-span-1">Size</div>
                            <div className="col-span-2 text-right">Actions</div>
                        </div>

                        {/* File Rows */}
                        <div className="divide-y divide-gray-100 dark:divide-gray-700">
                            {files.map((file) => (
                                <div
                                    key={file.id}
                                    onClick={() => handlePreview(file)}
                                    className="flex flex-col md:grid md:grid-cols-12 gap-3 md:gap-4 px-6 py-4 hover:bg-gray-50 dark:hover:bg-gray-700/30 transition-colors cursor-pointer"
                                >
                                    {/* File Info */}
                                    <div className="col-span-5 flex items-center gap-3">
                                        <div className="flex-shrink-0 p-2 bg-gray-50 dark:bg-gray-700 rounded-lg">
                                            {getFileIcon(file.content_type, file.name)}
                                        </div>
                                        <div className="min-w-0">
                                            <p className="text-sm font-medium text-gray-900 dark:text-white truncate">
                                                {file.name}
                                            </p>
                                            {file.folder_path && (
                                                <p className="text-xs text-gray-500 dark:text-gray-400 truncate">
                                                    {file.folder_path}
                                                </p>
                                            )}
                                        </div>
                                    </div>

                                    {/* Shared By */}
                                    <div className="col-span-2 flex items-center gap-2">
                                        <div className="w-6 h-6 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-xs font-medium text-primary-700 dark:text-primary-300">
                                            {file.shared_by_name?.charAt(0)?.toUpperCase() || '?'}
                                        </div>
                                        <span className="text-sm text-gray-600 dark:text-gray-300 truncate">
                                            {file.shared_by_name}
                                        </span>
                                    </div>

                                    {/* Shared Date */}
                                    <div className="col-span-2 flex items-center">
                                        <span className="text-sm text-gray-500 dark:text-gray-400">
                                            {format(new Date(file.shared_at), 'MMM d, yyyy')}
                                        </span>
                                    </div>

                                    {/* Size */}
                                    <div className="col-span-1 flex items-center">
                                        <span className="text-sm text-gray-500 dark:text-gray-400">
                                            {formatBytes(file.size)}
                                        </span>
                                    </div>

                                    {/* Actions */}
                                    <div className="col-span-2 flex items-center justify-end gap-1">
                                        <button
                                            onClick={(e) => handlePreview(file, e)}
                                            className="p-2 text-gray-500 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-lg transition-colors"
                                            title="Preview"
                                        >
                                            <Eye className="w-4 h-4" />
                                        </button>
                                        <button
                                            onClick={(e) => handleSaveToMyFiles(file, e)}
                                            disabled={savingFileId === file.id || savedFileIds.has(file.id)}
                                            className={clsx(
                                                "p-2 rounded-lg transition-colors",
                                                savedFileIds.has(file.id)
                                                    ? "text-green-600 bg-green-50 dark:bg-green-900/20"
                                                    : "text-gray-500 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20",
                                                savingFileId === file.id && "opacity-50 cursor-wait"
                                            )}
                                            title={savedFileIds.has(file.id) ? "Saved to your files" : "Save to My Files"}
                                        >
                                            {savingFileId === file.id ? (
                                                <Loader2 className="w-4 h-4 animate-spin" />
                                            ) : savedFileIds.has(file.id) ? (
                                                <Check className="w-4 h-4" />
                                            ) : (
                                                <FolderPlus className="w-4 h-4" />
                                            )}
                                        </button>
                                        <button
                                            onClick={(e) => handleDownload(file, e)}
                                            className="p-2 text-gray-500 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-lg transition-colors"
                                            title="Download"
                                        >
                                            <Download className="w-4 h-4" />
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>

                        {/* Pagination */}
                        {totalPages > 1 && (
                            <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
                                <p className="text-sm text-gray-500 dark:text-gray-400">
                                    Showing {((page - 1) * perPage) + 1} - {Math.min(page * perPage, total)} of {total} files
                                </p>
                                <div className="flex items-center gap-2">
                                    <button
                                        onClick={() => setPage((p) => Math.max(1, p - 1))}
                                        disabled={page === 1}
                                        className="p-2 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        <ChevronLeft className="w-4 h-4" />
                                    </button>
                                    <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                                        Page {page} of {totalPages}
                                    </span>
                                    <button
                                        onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                                        disabled={page === totalPages}
                                        className="p-2 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        <ChevronRight className="w-4 h-4" />
                                    </button>
                                </div>
                            </div>
                        )}
                    </div>
                )}
            </div>
            
            {/* File Preview Modal */}
            <FilePreviewModal
                isOpen={isPreviewOpen}
                onClose={() => {
                    setIsPreviewOpen(false);
                    setPreviewFile(null);
                }}
                file={previewFile}
            />
        </div>
    );
}

export default SharedWithMe;
