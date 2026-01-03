import { useState, useEffect } from 'react';
import { 
  X, Activity, User, CheckCircle, AlertTriangle, FolderOpen, Folder, FileText, 
  Image, EyeOff, File, Download, Trash2, Move, MoreVertical,
  Grid, List, Search, Home, Video, Music, Archive, Code, Table2,
  ChevronLeft, ChevronRight, RefreshCw, CheckSquare, Square, RotateCcw
} from 'lucide-react';
import { useAuthFetch, useAuth } from '../context/AuthContext';
import { useTenant } from '../context/TenantContext';
import { useGlobalSettings } from '../context/GlobalSettingsContext';
import { MoveFileModal } from './MoveFileModal';
import { Avatar } from './Avatar';
import clsx from 'clsx';

interface UserData {
  id: string;
  name: string;
  email: string;
  role: string;
  status: 'active' | 'inactive';
  avatar_url?: string | null;
}

interface ActivityLog {
  id: string;
  user: string;
  user_id?: string;
  action: string;
  resource: string;
  resource_type: string;
  timestamp: string;
  status: 'success' | 'warning';
  ip_address?: string;
}

interface FileItem {
  id: string;
  name: string;
  type: 'folder' | 'image' | 'document' | 'pdf' | 'spreadsheet' | 'video' | 'audio' | 'code' | 'archive' | 'other' | 'group';
  size: string;
  size_bytes: number;
  modified: string;
  is_directory: boolean;
  visibility?: 'department' | 'private';
  owner_id?: string;
}

interface TrashItem {
  id: string;
  file_id: string;
  name: string;
  path: string;
  size: string;
  size_bytes: number;
  is_directory: boolean;
  deleted_at: string;
  original_path: string;
  visibility?: string;
}

interface UserDetailsModalProps {
  isOpen: boolean;
  onClose: () => void;
  user: UserData | null;
}

export function UserDetailsModal({ isOpen, onClose, user }: UserDetailsModalProps) {
  const [activeTab, setActiveTab] = useState<'profile' | 'activity' | 'files'>('profile');
  const [activityLogs, setActivityLogs] = useState<ActivityLog[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const limit = 20;
  const authFetch = useAuthFetch();
  const { user: currentUser } = useAuth();
  const { currentCompany } = useTenant();
  const { formatDate: globalFormatDate } = useGlobalSettings();
  
  // Files tab state
  const [files, setFiles] = useState<FileItem[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const [filesPath, setFilesPath] = useState<string[]>([]);
  const [fileViewMode, setFileViewMode] = useState<'grid' | 'list'>('list');
  const [searchQuery, setSearchQuery] = useState('');
  const [activeMenu, setActiveMenu] = useState<string | null>(null);
  
  // Pagination
  const [filesPage, setFilesPage] = useState(0);
  const filesPerPage = 15;
  
  // Bulk selection
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  
  // File sub-view: files or trash
  const [fileSubView, setFileSubView] = useState<'files' | 'trash'>('files');
  const [trashItems, setTrashItems] = useState<TrashItem[]>([]);
  const [trashLoading, setTrashLoading] = useState(false);
  
  // Move modal state
  const [isMoveModalOpen, setIsMoveModalOpen] = useState(false);
  const [fileToMove, setFileToMove] = useState<FileItem | null>(null);
  const [isMoving, setIsMoving] = useState(false);
  
  // Check if current user can view private files
  const canViewPrivateFiles = currentUser?.role === 'SuperAdmin' || currentUser?.role === 'Admin';

  useEffect(() => {
    if (isOpen && user && activeTab === 'activity') {
      fetchUserActivity();
    }
  }, [isOpen, user, activeTab, offset]);
  
  useEffect(() => {
    if (isOpen && user && activeTab === 'files' && canViewPrivateFiles && currentCompany?.id) {
      if (fileSubView === 'files') {
        fetchUserFiles();
      } else {
        fetchUserTrash();
      }
    }
  }, [isOpen, user, activeTab, filesPath, currentCompany?.id, fileSubView]);
  
  // Reset state when modal closes or user changes
  useEffect(() => {
    if (!isOpen || !user) {
      setFilesPath([]);
      setFiles([]);
      setActiveMenu(null);
      setSearchQuery('');
      setFilesPage(0);
      setSelectedFiles(new Set());
      setIsSelectionMode(false);
      setFileSubView('files');
      setTrashItems([]);
    }
  }, [isOpen, user]);
  
  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = () => setActiveMenu(null);
    document.addEventListener('click', handleClickOutside);
    return () => document.removeEventListener('click', handleClickOutside);
  }, []);
  
  // Reset pagination when search changes
  useEffect(() => {
    setFilesPage(0);
  }, [searchQuery]);
  
  const fetchUserFiles = async () => {
    if (!user || !currentCompany?.id) return;
    
    try {
      setFilesLoading(true);
      const path = filesPath.join('/');
      const params = new URLSearchParams({
        visibility: 'private',
        owner_id: user.id,
      });
      if (path) params.set('path', path);
      
      const response = await authFetch(`/api/files/${currentCompany.id}?${params}`);
      if (!response.ok) throw new Error('Failed to fetch files');
      
      const data = await response.json();
      setFiles(data.map((f: any) => ({
        id: f.id,
        name: f.name,
        type: f.is_directory ? 'folder' : getFileType(f.name),
        size: formatFileSize(f.size_bytes),
        size_bytes: f.size_bytes,
        modified: f.updated_at,
        is_directory: f.is_directory,
        visibility: f.visibility,
        owner_id: f.owner_id,
      })));
    } catch (error) {
      console.error('Error fetching user files:', error);
      setFiles([]);
    } finally {
      setFilesLoading(false);
    }
  };
  
  const fetchUserTrash = async () => {
    if (!user || !currentCompany?.id) return;
    
    try {
      setTrashLoading(true);
      const response = await authFetch(`/api/trash/${currentCompany.id}?owner_id=${user.id}`);
      if (!response.ok) throw new Error('Failed to fetch trash');
      
      const data = await response.json();
      setTrashItems(data.map((f: any) => ({
        id: f.id,
        file_id: f.file_id,
        name: f.name,
        path: f.path,
        size: f.size,
        size_bytes: f.size_bytes || 0,
        is_directory: f.is_directory,
        deleted_at: f.deleted_at,
        original_path: f.original_path,
        visibility: f.visibility,
      })));
    } catch (error) {
      console.error('Error fetching user trash:', error);
      setTrashItems([]);
    } finally {
      setTrashLoading(false);
    }
  };
  
  const getFileType = (filename: string): FileItem['type'] => {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp'];
    const docExts = ['doc', 'docx', 'txt', 'rtf', 'odt'];
    const spreadsheetExts = ['xls', 'xlsx', 'csv', 'ods'];
    const videoExts = ['mp4', 'mov', 'avi', 'mkv', 'webm'];
    const audioExts = ['mp3', 'wav', 'ogg', 'flac', 'aac'];
    const codeExts = ['js', 'ts', 'py', 'java', 'c', 'cpp', 'rs', 'go', 'html', 'css', 'json'];
    const archiveExts = ['zip', 'rar', '7z', 'tar', 'gz'];
    
    if (imageExts.includes(ext)) return 'image';
    if (ext === 'pdf') return 'pdf';
    if (docExts.includes(ext)) return 'document';
    if (spreadsheetExts.includes(ext)) return 'spreadsheet';
    if (videoExts.includes(ext)) return 'video';
    if (audioExts.includes(ext)) return 'audio';
    if (codeExts.includes(ext)) return 'code';
    if (archiveExts.includes(ext)) return 'archive';
    return 'other';
  };
  
  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };
  
  const getFileIcon = (type: FileItem['type'], size: 'sm' | 'lg' = 'sm') => {
    const sizeClass = size === 'lg' ? 'w-10 h-10' : 'w-5 h-5';
    switch (type) {
      case 'folder': return <Folder className={clsx(sizeClass, "text-yellow-500")} />;
      case 'image': return <Image className={clsx(sizeClass, "text-purple-500")} />;
      case 'pdf': return <FileText className={clsx(sizeClass, "text-red-500")} />;
      case 'document': return <FileText className={clsx(sizeClass, "text-blue-500")} />;
      case 'spreadsheet': return <Table2 className={clsx(sizeClass, "text-green-500")} />;
      case 'video': return <Video className={clsx(sizeClass, "text-pink-500")} />;
      case 'audio': return <Music className={clsx(sizeClass, "text-orange-500")} />;
      case 'code': return <Code className={clsx(sizeClass, "text-cyan-500")} />;
      case 'archive': return <Archive className={clsx(sizeClass, "text-amber-600")} />;
      default: return <File className={clsx(sizeClass, "text-gray-500")} />;
    }
  };
  
  const handleDownload = async (file: FileItem) => {
    if (!currentCompany?.id) return;
    try {
      // SECURITY: Use header-based auth instead of token-in-URL
      const response = await authFetch(`/api/download/${currentCompany.id}/${file.id}`);
      if (!response.ok) {
        throw new Error('Download failed');
      }
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = file.name;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);
    } catch (error) {
      console.error('Download error:', error);
      alert('Failed to download file');
    }
  };
  
  const handleDelete = async (file: FileItem) => {
    if (!currentCompany?.id || !confirm(`Delete "${file.name}"? It will be moved to the user's Recycle Bin.`)) return;
    
    const currentPathStr = filesPath.join('/');
    const fullPath = currentPathStr ? `${currentPathStr}/${file.name}` : file.name;
    
    try {
      const response = await authFetch(`/api/files/${currentCompany.id}/delete`, {
        method: 'POST',
        body: JSON.stringify({ path: fullPath })
      });
      
      if (response.ok) {
        fetchUserFiles();
        setSelectedFiles(prev => {
          const newSet = new Set(prev);
          newSet.delete(file.id);
          return newSet;
        });
      } else {
        const data = await response.json().catch(() => ({}));
        alert(data.error || 'Failed to delete file');
      }
    } catch {
      alert('Failed to delete file');
    }
  };
  
  const handleBulkDelete = async () => {
    if (!currentCompany?.id || selectedFiles.size === 0) return;
    if (!confirm(`Delete ${selectedFiles.size} item(s)? They will be moved to the user's Recycle Bin.`)) return;
    
    const currentPathStr = filesPath.join('/');
    let successCount = 0;
    
    for (const fileId of selectedFiles) {
      const file = files.find(f => f.id === fileId);
      if (!file) continue;
      
      const fullPath = currentPathStr ? `${currentPathStr}/${file.name}` : file.name;
      
      try {
        const response = await authFetch(`/api/files/${currentCompany.id}/delete`, {
          method: 'POST',
          body: JSON.stringify({ path: fullPath })
        });
        if (response.ok) successCount++;
      } catch {
        // Continue with others
      }
    }
    
    setSelectedFiles(new Set());
    setIsSelectionMode(false);
    fetchUserFiles();
  };
  
  const openMoveModal = (file: FileItem) => {
    setFileToMove(file);
    setIsMoveModalOpen(true);
  };
  
  const handleMoveFile = async (targetParentId: string | null, targetDepartmentId: string | null, targetVisibility: string, newName?: string) => {
    if (!currentCompany?.id || !fileToMove) return { success: false, error: 'No file selected' };
    
    setIsMoving(true);
    try {
      const response = await authFetch(`/api/files/${currentCompany.id}/${fileToMove.id}/move`, {
        method: 'PUT',
        body: JSON.stringify({
          target_parent_id: targetParentId,
          target_department_id: targetDepartmentId,
          target_visibility: targetVisibility,
          new_name: newName || null
        })
      });
      
      const data = await response.json().catch(() => ({}));
      
      if (response.ok && !data.error) {
        fetchUserFiles();
        setSelectedFiles(prev => {
          const newSet = new Set(prev);
          newSet.delete(fileToMove.id);
          return newSet;
        });
        setIsMoveModalOpen(false);
        setFileToMove(null);
        return { success: true };
      } else {
        return {
          success: false,
          error: data.error || 'Failed to move file',
          duplicate: data.duplicate || false,
          conflicting_name: data.conflicting_name,
          suggested_name: data.suggested_name
        };
      }
    } finally {
      setIsMoving(false);
    }
  };
  
  const handleBulkMove = async (targetParentId: string | null, targetDepartmentId: string | null, targetVisibility: string, _newName?: string) => {
    if (!currentCompany?.id || selectedFiles.size === 0) return { success: false, error: 'No files selected' };
    
    setIsMoving(true);
    let successCount = 0;
    for (const fileId of selectedFiles) {
      try {
        const response = await authFetch(`/api/files/${currentCompany.id}/${fileId}/move`, {
          method: 'PUT',
          body: JSON.stringify({
            target_parent_id: targetParentId,
            target_department_id: targetDepartmentId,
            target_visibility: targetVisibility
          })
        });
        if (response.ok) successCount++;
      } catch {
        // Continue
      }
    }
    
    setSelectedFiles(new Set());
    setIsSelectionMode(false);
    setIsMoving(false);
    setIsMoveModalOpen(false);
    setFileToMove(null);
    fetchUserFiles();
    return { success: successCount > 0 };
  };
  
  const handleRestore = async (item: TrashItem) => {
    if (!currentCompany?.id) return;
    
    try {
      const response = await authFetch(`/api/trash/${currentCompany.id}/restore/${encodeURIComponent(item.path)}`, {
        method: 'POST'
      });
      
      if (response.ok) {
        fetchUserTrash();
      } else {
        alert('Failed to restore file');
      }
    } catch {
      alert('Failed to restore file');
    }
  };
  
  const handlePermanentDelete = async (item: TrashItem) => {
    if (!currentCompany?.id || !confirm(`Permanently delete "${item.name}"? This cannot be undone.`)) return;
    
    try {
      const response = await authFetch(`/api/trash/${currentCompany.id}/delete/${encodeURIComponent(item.path)}`, {
        method: 'POST'
      });
      
      if (response.ok) {
        fetchUserTrash();
      } else {
        alert('Failed to permanently delete file');
      }
    } catch {
      alert('Failed to permanently delete file');
    }
  };
  
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
    setSelectedFiles(new Set(paginatedFiles.map(f => f.id)));
  };
  
  const clearSelection = () => {
    setSelectedFiles(new Set());
    setIsSelectionMode(false);
  };

  const filteredFiles = files.filter(f => 
    f.name.toLowerCase().includes(searchQuery.toLowerCase())
  );
  
  // Pagination
  const totalFilesPages = Math.ceil(filteredFiles.length / filesPerPage);
  const paginatedFiles = filteredFiles.slice(
    filesPage * filesPerPage,
    (filesPage + 1) * filesPerPage
  );

  const fetchUserActivity = async () => {
    if (!user) return;
    
    try {
      setIsLoading(true);
      const params = new URLSearchParams({
        limit: limit.toString(),
        offset: offset.toString(),
      });
      
      const response = await authFetch(`/api/users/${user.id}/activity-logs?${params}`);
      if (!response.ok) throw new Error('Failed to fetch activity');
      
      const data = await response.json();
      setActivityLogs(data.logs || []);
      setTotal(data.total || 0);
    } catch (error) {
      console.error('Error fetching user activity:', error);
    } finally {
      setIsLoading(false);
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

  const formatAction = (action: string) => {
    return action.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
  };

  if (!isOpen || !user) return null;

  const getInitials = (name: string) => {
    return name.split(' ').map(n => n[0]).join('').toUpperCase();
  };

  const totalPages = Math.ceil(total / limit);
  const currentPage = Math.floor(offset / limit) + 1;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto">
      <div className="fixed inset-0 bg-black/50 backdrop-blur-sm" onClick={onClose} />
      
      <div className={clsx(
        "relative mx-4 bg-white dark:bg-gray-800 rounded-xl shadow-2xl flex flex-col",
        activeTab === 'files' ? "w-full max-w-5xl max-h-[90vh]" : "w-full max-w-2xl max-h-[90vh]"
      )}>
        {/* Header */}
        <div className="p-6 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Avatar 
              src={user.avatar_url} 
              name={user.name} 
              size="lg"
            />
            <div>
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white">{user.name}</h2>
              <p className="text-sm text-gray-500 dark:text-gray-400">{user.email}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-full hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            <X className="w-5 h-5 text-gray-500" />
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-gray-200 dark:border-gray-700">
          <button
            onClick={() => setActiveTab('profile')}
            className={clsx(
              "flex-1 px-6 py-3 text-sm font-medium transition-colors flex items-center justify-center gap-2",
              activeTab === 'profile'
                ? "text-primary-600 border-b-2 border-primary-600"
                : "text-gray-500 hover:text-gray-700 dark:text-gray-400"
            )}
          >
            <User className="w-4 h-4" />
            Profile
          </button>
          <button
            onClick={() => setActiveTab('activity')}
            className={clsx(
              "flex-1 px-6 py-3 text-sm font-medium transition-colors flex items-center justify-center gap-2",
              activeTab === 'activity'
                ? "text-primary-600 border-b-2 border-primary-600"
                : "text-gray-500 hover:text-gray-700 dark:text-gray-400"
            )}
          >
            <Activity className="w-4 h-4" />
            Activity
          </button>
          {canViewPrivateFiles && (
            <button
              onClick={() => setActiveTab('files')}
              className={clsx(
                "flex-1 px-6 py-3 text-sm font-medium transition-colors flex items-center justify-center gap-2",
                activeTab === 'files'
                  ? "text-primary-600 border-b-2 border-primary-600"
                  : "text-gray-500 hover:text-gray-700 dark:text-gray-400"
              )}
            >
              <FolderOpen className="w-4 h-4" />
              Files
            </button>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-hidden">
          {activeTab === 'files' && canViewPrivateFiles ? (
            <div className="h-full flex flex-col">
              {/* Admin notice */}
              <div className="flex items-center justify-between px-4 py-2 bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800">
                <div className="flex items-center gap-2 text-amber-700 dark:text-amber-300 text-sm">
                  <EyeOff className="w-4 h-4 flex-shrink-0" />
                  <span>Admin view of {user.name}'s private files. All access is logged.</span>
                </div>
                
                {/* Sub-view toggle */}
                <div className="flex bg-amber-100 dark:bg-amber-900/40 rounded-lg p-0.5">
                  <button
                    onClick={() => setFileSubView('files')}
                    className={clsx(
                      "px-3 py-1 text-xs font-medium rounded transition-colors",
                      fileSubView === 'files' 
                        ? "bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm" 
                        : "text-amber-700 dark:text-amber-300"
                    )}
                  >
                    Files
                  </button>
                  <button
                    onClick={() => setFileSubView('trash')}
                    className={clsx(
                      "px-3 py-1 text-xs font-medium rounded transition-colors flex items-center gap-1",
                      fileSubView === 'trash' 
                        ? "bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm" 
                        : "text-amber-700 dark:text-amber-300"
                    )}
                  >
                    <Trash2 className="w-3 h-3" />
                    Recycle Bin
                  </button>
                </div>
              </div>
              
              {fileSubView === 'files' ? (
                <>
                  {/* Toolbar */}
                  <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between bg-gray-50 dark:bg-gray-900/50">
                    {/* Breadcrumbs */}
                    <div className="flex items-center gap-2 text-sm">
                      <button
                        onClick={() => { setFilesPath([]); setFilesPage(0); }}
                        className={clsx(
                          "flex items-center gap-1 px-2 py-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700",
                          filesPath.length === 0 ? "text-gray-900 dark:text-white font-medium" : "text-primary-600"
                        )}
                      >
                        <Home className="w-4 h-4" />
                        Root
                      </button>
                      {filesPath.map((folder, index) => (
                        <div key={index} className="flex items-center gap-1">
                          <span className="text-gray-400">/</span>
                          <button
                            onClick={() => { setFilesPath(filesPath.slice(0, index + 1)); setFilesPage(0); }}
                            className={clsx(
                              "px-2 py-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700",
                              index === filesPath.length - 1
                                ? "text-gray-900 dark:text-white font-medium"
                                : "text-primary-600"
                            )}
                          >
                            {folder}
                          </button>
                        </div>
                      ))}
                    </div>
                    
                    {/* Right side controls */}
                    <div className="flex items-center gap-3">
                      {/* Select toggle */}
                      <button
                        onClick={() => {
                          if (isSelectionMode) {
                            clearSelection();
                          } else {
                            setIsSelectionMode(true);
                          }
                        }}
                        className={clsx(
                          "px-3 py-1.5 text-sm rounded-lg border transition-colors",
                          isSelectionMode
                            ? "bg-primary-50 dark:bg-primary-900/20 border-primary-300 dark:border-primary-700 text-primary-700 dark:text-primary-300"
                            : "bg-white dark:bg-gray-700 border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-600"
                        )}
                      >
                        <CheckSquare className="w-4 h-4 inline mr-1" />
                        {isSelectionMode ? 'Cancel' : 'Select'}
                      </button>
                      
                      <div className="relative">
                        <Search className="w-4 h-4 absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400" />
                        <input
                          type="text"
                          placeholder="Search files..."
                          value={searchQuery}
                          onChange={(e) => setSearchQuery(e.target.value)}
                          className="w-40 pl-9 pr-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                        />
                      </div>
                      <div className="flex bg-gray-200 dark:bg-gray-700 rounded-lg p-0.5">
                        <button
                          onClick={() => setFileViewMode('list')}
                          className={clsx(
                            "p-1.5 rounded",
                            fileViewMode === 'list' ? "bg-white dark:bg-gray-600 shadow-sm" : "text-gray-500"
                          )}
                        >
                          <List className="w-4 h-4" />
                        </button>
                        <button
                          onClick={() => setFileViewMode('grid')}
                          className={clsx(
                            "p-1.5 rounded",
                            fileViewMode === 'grid' ? "bg-white dark:bg-gray-600 shadow-sm" : "text-gray-500"
                          )}
                        >
                          <Grid className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  </div>
                  
                  {/* Bulk action bar */}
                  {selectedFiles.size > 0 && (
                    <div className="px-4 py-2 bg-primary-50 dark:bg-primary-900/20 border-b border-primary-200 dark:border-primary-800 flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <span className="text-sm font-medium text-primary-700 dark:text-primary-300">
                          {selectedFiles.size} selected
                        </span>
                        <button onClick={selectAllFiles} className="text-sm text-primary-600 hover:underline">
                          Select all
                        </button>
                      </div>
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => {
                            // For bulk move, we'll use the first selected file as reference
                            const firstFile = files.find(f => selectedFiles.has(f.id));
                            if (firstFile) {
                              setFileToMove(firstFile);
                              setIsMoveModalOpen(true);
                            }
                          }}
                          className="px-3 py-1 text-sm bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 flex items-center gap-1"
                        >
                          <Move className="w-3.5 h-3.5" />
                          Move
                        </button>
                        <button
                          onClick={handleBulkDelete}
                          className="px-3 py-1 text-sm bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 rounded-lg hover:bg-red-100 dark:hover:bg-red-900/30 flex items-center gap-1"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                          Delete
                        </button>
                        <button onClick={clearSelection} className="p-1 text-gray-500 hover:text-gray-700">
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  )}
                  
                  {/* File Area */}
                  <div className="flex-1 overflow-y-auto p-4">
                    {filesLoading ? (
                      <div className="flex items-center justify-center py-16">
                        <div className="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
                      </div>
                    ) : paginatedFiles.length === 0 ? (
                      <div className="flex flex-col items-center justify-center py-16 text-gray-500 dark:text-gray-400">
                        <FolderOpen className="w-16 h-16 mb-4 opacity-40" />
                        <p className="text-lg font-medium">No private files</p>
                        <p className="text-sm mt-1">
                          {searchQuery ? `No files match "${searchQuery}"` : 'This user has no private files'}
                        </p>
                      </div>
                    ) : fileViewMode === 'grid' ? (
                      /* Grid View */
                      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
                        {paginatedFiles.map((file) => (
                          <div
                            key={file.id}
                            className={clsx(
                              "group relative bg-white dark:bg-gray-800 border rounded-xl p-3 hover:shadow-lg transition-all cursor-pointer",
                              selectedFiles.has(file.id)
                                ? "border-primary-400 ring-2 ring-primary-200 dark:ring-primary-800"
                                : "border-gray-200 dark:border-gray-700 hover:border-primary-300"
                            )}
                            onClick={() => {
                              if (isSelectionMode) {
                                toggleFileSelection(file.id);
                              } else if (file.is_directory) {
                                setFilesPath([...filesPath, file.name]);
                                setFilesPage(0);
                              }
                            }}
                          >
                            {isSelectionMode && (
                              <div className="absolute top-2 left-2 z-10">
                                {selectedFiles.has(file.id) ? (
                                  <CheckSquare className="w-5 h-5 text-primary-600" />
                                ) : (
                                  <Square className="w-5 h-5 text-gray-400" />
                                )}
                              </div>
                            )}
                            <div className="flex flex-col items-center text-center">
                              <div className="mb-2">{getFileIcon(file.type, 'lg')}</div>
                              <p className="text-sm font-medium text-gray-900 dark:text-white truncate w-full" title={file.name}>
                                {file.name}
                              </p>
                              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                {file.size || '-'}
                              </p>
                            </div>
                            
                            {!isSelectionMode && (
                              <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setActiveMenu(activeMenu === file.id ? null : file.id);
                                  }}
                                  className="p-1.5 bg-white dark:bg-gray-700 rounded-lg shadow-sm hover:bg-gray-100 dark:hover:bg-gray-600"
                                >
                                  <MoreVertical className="w-4 h-4 text-gray-500" />
                                </button>
                                
                                {activeMenu === file.id && (
                                  <div className="absolute right-0 mt-1 w-44 bg-white dark:bg-gray-800 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 py-1 z-50">
                                    {!file.is_directory && (
                                      <button
                                        onClick={(e) => { e.stopPropagation(); handleDownload(file); setActiveMenu(null); }}
                                        className="flex items-center w-full px-3 py-2 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                      >
                                        <Download className="w-4 h-4 mr-2 text-gray-400" />
                                        Download
                                      </button>
                                    )}
                                    <button
                                      onClick={(e) => { e.stopPropagation(); openMoveModal(file); setActiveMenu(null); }}
                                      className="flex items-center w-full px-3 py-2 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                                    >
                                      <Move className="w-4 h-4 mr-2 text-gray-400" />
                                      Move to Dept
                                    </button>
                                    <div className="border-t border-gray-200 dark:border-gray-700 my-1" />
                                    <button
                                      onClick={(e) => { e.stopPropagation(); handleDelete(file); setActiveMenu(null); }}
                                      className="flex items-center w-full px-3 py-2 text-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20"
                                    >
                                      <Trash2 className="w-4 h-4 mr-2" />
                                      Delete
                                    </button>
                                  </div>
                                )}
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    ) : (
                      /* List View - Table */
                      <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-gray-700">
                        <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                          <thead className="bg-gray-50 dark:bg-gray-900/50">
                            <tr>
                              {isSelectionMode && (
                                <th className="px-3 py-3 w-10">
                                  <button onClick={() => {
                                    if (selectedFiles.size === paginatedFiles.length) {
                                      setSelectedFiles(new Set());
                                    } else {
                                      selectAllFiles();
                                    }
                                  }}>
                                    {selectedFiles.size === paginatedFiles.length && paginatedFiles.length > 0 ? (
                                      <CheckSquare className="w-5 h-5 text-primary-600" />
                                    ) : (
                                      <Square className="w-5 h-5 text-gray-400" />
                                    )}
                                  </button>
                                </th>
                              )}
                              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                Name
                              </th>
                              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden sm:table-cell">
                                Size
                              </th>
                              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden md:table-cell">
                                Modified
                              </th>
                              <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                Actions
                              </th>
                            </tr>
                          </thead>
                          <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                            {paginatedFiles.map((file) => (
                              <tr
                                key={file.id}
                                className={clsx(
                                  "transition-colors cursor-pointer group",
                                  selectedFiles.has(file.id)
                                    ? "bg-primary-50 dark:bg-primary-900/20"
                                    : "hover:bg-gray-50 dark:hover:bg-gray-700/50"
                                )}
                                onClick={() => {
                                  if (isSelectionMode) {
                                    toggleFileSelection(file.id);
                                  } else if (file.is_directory) {
                                    setFilesPath([...filesPath, file.name]);
                                    setFilesPage(0);
                                  }
                                }}
                              >
                                {isSelectionMode && (
                                  <td className="px-3 py-3 w-10">
                                    {selectedFiles.has(file.id) ? (
                                      <CheckSquare className="w-5 h-5 text-primary-600" />
                                    ) : (
                                      <Square className="w-5 h-5 text-gray-400" />
                                    )}
                                  </td>
                                )}
                                <td className="px-4 py-3 whitespace-nowrap">
                                  <div className="flex items-center gap-3">
                                    {getFileIcon(file.type)}
                                    <span className="text-sm font-medium text-gray-900 dark:text-white">
                                      {file.name}
                                    </span>
                                  </div>
                                </td>
                                <td className="px-4 py-3 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden sm:table-cell">
                                  {file.size || '-'}
                                </td>
                                <td className="px-4 py-3 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden md:table-cell">
                                  {globalFormatDate(file.modified)}
                                </td>
                                <td className="px-4 py-3 whitespace-nowrap text-right">
                                  {!isSelectionMode && (
                                    <div className="flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                                      {!file.is_directory && (
                                        <button
                                          onClick={(e) => { e.stopPropagation(); handleDownload(file); }}
                                          className="p-1.5 text-gray-400 hover:text-primary-600 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
                                          title="Download"
                                        >
                                          <Download className="w-4 h-4" />
                                        </button>
                                      )}
                                      <button
                                        onClick={(e) => { e.stopPropagation(); openMoveModal(file); }}
                                        className="p-1.5 text-gray-400 hover:text-blue-600 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
                                        title="Move to Department"
                                      >
                                        <Move className="w-4 h-4" />
                                      </button>
                                      <button
                                        onClick={(e) => { e.stopPropagation(); handleDelete(file); }}
                                        className="p-1.5 text-gray-400 hover:text-red-600 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
                                        title="Delete"
                                      >
                                        <Trash2 className="w-4 h-4" />
                                      </button>
                                    </div>
                                  )}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </div>
                  
                  {/* Footer with pagination */}
                  <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      {filteredFiles.length} item{filteredFiles.length !== 1 ? 's' : ''} • 
                      {' '}{formatFileSize(filteredFiles.reduce((acc, f) => acc + f.size_bytes, 0))} total
                    </span>
                    
                    {totalFilesPages > 1 && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-gray-500">
                          Page {filesPage + 1} of {totalFilesPages}
                        </span>
                        <button
                          onClick={() => setFilesPage(p => Math.max(0, p - 1))}
                          disabled={filesPage === 0}
                          className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50"
                        >
                          <ChevronLeft className="w-4 h-4" />
                        </button>
                        <button
                          onClick={() => setFilesPage(p => Math.min(totalFilesPages - 1, p + 1))}
                          disabled={filesPage >= totalFilesPages - 1}
                          className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50"
                        >
                          <ChevronRight className="w-4 h-4" />
                        </button>
                      </div>
                    )}
                  </div>
                </>
              ) : (
                /* Trash Sub-view */
                <>
                  <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 flex items-center justify-between">
                    <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
                      <Trash2 className="w-4 h-4" />
                      <span>{user.name}'s Recycle Bin</span>
                    </div>
                    <button
                      onClick={fetchUserTrash}
                      className="p-1.5 text-gray-400 hover:text-gray-600 rounded hover:bg-gray-200 dark:hover:bg-gray-700"
                      title="Refresh"
                    >
                      <RefreshCw className="w-4 h-4" />
                    </button>
                  </div>
                  
                  <div className="flex-1 overflow-y-auto p-4">
                    {trashLoading ? (
                      <div className="flex items-center justify-center py-16">
                        <div className="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
                      </div>
                    ) : trashItems.length === 0 ? (
                      <div className="flex flex-col items-center justify-center py-16 text-gray-500 dark:text-gray-400">
                        <Trash2 className="w-16 h-16 mb-4 opacity-40" />
                        <p className="text-lg font-medium">Recycle Bin is empty</p>
                        <p className="text-sm mt-1">No deleted files for this user</p>
                      </div>
                    ) : (
                      <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-gray-700">
                        <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                          <thead className="bg-gray-50 dark:bg-gray-900/50">
                            <tr>
                              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                Name
                              </th>
                              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden sm:table-cell">
                                Size
                              </th>
                              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden md:table-cell">
                                Deleted
                              </th>
                              <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                Actions
                              </th>
                            </tr>
                          </thead>
                          <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                            {trashItems.map((item) => (
                              <tr key={item.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/50 group">
                                <td className="px-4 py-3 whitespace-nowrap">
                                  <div className="flex items-center gap-3">
                                    {item.is_directory ? (
                                      <Folder className="w-5 h-5 text-yellow-500 opacity-50" />
                                    ) : (
                                      <File className="w-5 h-5 text-gray-400" />
                                    )}
                                    <div>
                                      <p className="text-sm font-medium text-gray-900 dark:text-white">{item.name}</p>
                                      <p className="text-xs text-gray-500 dark:text-gray-400">{item.path}</p>
                                    </div>
                                  </div>
                                </td>
                                <td className="px-4 py-3 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden sm:table-cell">
                                  {item.is_directory ? '--' : item.size}
                                </td>
                                <td className="px-4 py-3 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 hidden md:table-cell">
                                  {item.deleted_at ? globalFormatDate(item.deleted_at) : '--'}
                                </td>
                                <td className="px-4 py-3 whitespace-nowrap text-right">
                                  <div className="flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                                    <button
                                      onClick={() => handleRestore(item)}
                                      className="p-1.5 text-gray-400 hover:text-green-600 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
                                      title="Restore"
                                    >
                                      <RotateCcw className="w-4 h-4" />
                                    </button>
                                    <button
                                      onClick={() => handlePermanentDelete(item)}
                                      className="p-1.5 text-gray-400 hover:text-red-600 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
                                      title="Delete Permanently"
                                    >
                                      <Trash2 className="w-4 h-4" />
                                    </button>
                                  </div>
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </div>
                  
                  <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50">
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      {trashItems.length} deleted item{trashItems.length !== 1 ? 's' : ''}
                    </span>
                  </div>
                </>
              )}
            </div>
          ) : activeTab === 'profile' ? (
            <div className="p-6 space-y-6 overflow-y-auto">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Role</label>
                  <p className="mt-1 text-sm text-gray-900 dark:text-white">{user.role}</p>
                </div>
                <div>
                  <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Status</label>
                  <p className="mt-1">
                    <span className={clsx(
                      "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium",
                      user.status === 'active'
                        ? "bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-300"
                        : "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-300"
                    )}>
                      {user.status === 'active' ? (
                        <CheckCircle className="w-3 h-3 mr-1" />
                      ) : (
                        <AlertTriangle className="w-3 h-3 mr-1" />
                      )}
                      {user.status.charAt(0).toUpperCase() + user.status.slice(1)}
                    </span>
                  </p>
                </div>
              </div>
              
              <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
                <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-4">Quick Stats</h3>
                <div className="grid grid-cols-2 gap-4">
                  <div className="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
                    <p className="text-2xl font-semibold text-gray-900 dark:text-white">{total}</p>
                    <p className="text-xs text-gray-500 dark:text-gray-400">Total Activities</p>
                  </div>
                  <div className="bg-gray-50 dark:bg-gray-700/50 rounded-lg p-4">
                    <p className="text-2xl font-semibold text-gray-900 dark:text-white">
                      {activityLogs.length > 0 ? formatDate(activityLogs[0]?.timestamp).split(',')[0] : 'N/A'}
                    </p>
                    <p className="text-xs text-gray-500 dark:text-gray-400">Last Active</p>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="p-6 space-y-4 overflow-y-auto">
              {isLoading ? (
                <div className="flex items-center justify-center py-8">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
                </div>
              ) : activityLogs.length === 0 ? (
                <div className="text-center py-8 text-gray-500 dark:text-gray-400">
                  <Activity className="w-12 h-12 mx-auto mb-4 opacity-50" />
                  <p>No activity recorded</p>
                </div>
              ) : (
                <>
                  <div className="space-y-3">
                    {activityLogs.map((log) => (
                      <div
                        key={log.id}
                        className="flex items-start gap-4 p-4 rounded-lg bg-gray-50 dark:bg-gray-700/50 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                      >
                        <div className={clsx(
                          "p-2 rounded-full",
                          log.status === 'success'
                            ? "bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400"
                            : "bg-yellow-100 dark:bg-yellow-900/30 text-yellow-600 dark:text-yellow-400"
                        )}>
                          {log.status === 'success' ? (
                            <CheckCircle className="w-4 h-4" />
                          ) : (
                            <AlertTriangle className="w-4 h-4" />
                          )}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center justify-between gap-2">
                            <p className="text-sm font-medium text-gray-900 dark:text-white truncate">
                              {formatAction(log.action)}
                            </p>
                            <span className="text-xs text-gray-400 flex-shrink-0">
                              {formatDate(log.timestamp)}
                            </span>
                          </div>
                          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                            {log.resource_type}: {log.resource}
                          </p>
                          {log.ip_address && (
                            <p className="text-xs text-gray-400 mt-1">
                              IP: {log.ip_address}
                            </p>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>

                  {/* Pagination */}
                  {totalPages > 1 && (
                    <div className="flex items-center justify-between pt-4 border-t border-gray-200 dark:border-gray-700">
                      <p className="text-sm text-gray-500 dark:text-gray-400">
                        Page {currentPage} of {totalPages}
                      </p>
                      <div className="flex gap-2">
                        <button
                          onClick={() => setOffset(Math.max(0, offset - limit))}
                          disabled={offset === 0}
                          className="px-3 py-1 text-sm bg-gray-100 dark:bg-gray-700 rounded disabled:opacity-50 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                        >
                          Previous
                        </button>
                        <button
                          onClick={() => setOffset(offset + limit)}
                          disabled={currentPage >= totalPages}
                          className="px-3 py-1 text-sm bg-gray-100 dark:bg-gray-700 rounded disabled:opacity-50 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                        >
                          Next
                        </button>
                      </div>
                    </div>
                  )}
                </>
              )}
            </div>
          )}
        </div>
      </div>
      
      {/* Move File Modal */}
      <MoveFileModal
        isOpen={isMoveModalOpen}
        onClose={() => {
          setIsMoveModalOpen(false);
          setFileToMove(null);
        }}
        onMove={selectedFiles.size > 1 ? handleBulkMove : handleMoveFile}
        fileName={fileToMove?.name || ''}
        fileCount={selectedFiles.size > 1 ? selectedFiles.size : 1}
        isMoving={isMoving}
        currentPath={fileToMove ? (filesPath.length > 0 ? filesPath.join('/') : null) : null}
        currentVisibility="private"
        canCrossDepartment={true}
      />
    </div>
  );
}
