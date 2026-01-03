import { X, Folder, FileText, Image, Film, Music, Lock, Eye, EyeOff, Calendar, User, HardDrive, Building } from 'lucide-react';
import { format } from 'date-fns';
import { FileCommentsPanel } from './FileCommentsPanel';

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
    is_locked?: boolean;
    locked_by?: string;
    locked_at?: string;
    lock_requires_role?: string;
    visibility?: 'department' | 'private';
    department_id?: string;
    content_type?: string;
    storage_path?: string;
    color?: string;
    file_count?: number;
}

interface FilePropertiesModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: FileItem | null;
    departmentName?: string;
    companyId?: string;
}

const getFileIcon = (type: string) => {
    switch (type) {
        case 'folder':
            return <Folder className="w-12 h-12 text-yellow-500" />;
        case 'image':
            return <Image className="w-12 h-12 text-green-500" />;
        case 'video':
            return <Film className="w-12 h-12 text-purple-500" />;
        case 'audio':
            return <Music className="w-12 h-12 text-pink-500" />;
        default:
            return <FileText className="w-12 h-12 text-blue-500" />;
    }
};

export function FilePropertiesModal({ isOpen, onClose, file, departmentName, companyId }: FilePropertiesModalProps) {
    if (!isOpen || !file) return null;

    const formatDate = (dateStr?: string) => {
        if (!dateStr) return 'Unknown';
        try {
            return format(new Date(dateStr), 'PPpp');
        } catch {
            return dateStr;
        }
    };

    const PropertyRow = ({ icon: Icon, label, value, className = '' }: { icon: any; label: string; value: React.ReactNode; className?: string }) => (
        <div className="flex items-start py-3 border-b border-gray-100 dark:border-gray-700 last:border-0">
            <Icon className="w-4 h-4 text-gray-400 mt-0.5 mr-3 flex-shrink-0" />
            <div className="flex-1 min-w-0">
                <p className="text-xs text-gray-500 dark:text-gray-400 mb-0.5">{label}</p>
                <p className={`text-sm text-gray-900 dark:text-white ${className}`}>{value}</p>
            </div>
        </div>
    );

    return (
        <div className="fixed inset-0 z-[60] overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
                {/* Backdrop */}
                <div 
                    className="fixed inset-0 bg-black/50 transition-opacity" 
                    onClick={onClose}
                />

                {/* Modal */}
                <div className="relative bg-white dark:bg-gray-800 rounded-xl shadow-xl w-full max-w-md transform transition-all">
                    {/* Header */}
                    <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                            Properties
                        </h2>
                        <button
                            onClick={onClose}
                            className="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    {/* Content */}
                    <div className="px-6 py-4">
                        {/* File Icon and Name */}
                        <div className="flex items-center space-x-4 pb-4 border-b border-gray-200 dark:border-gray-700 mb-4">
                            <div className="flex-shrink-0 p-3 bg-gray-50 dark:bg-gray-700/50 rounded-xl">
                                {getFileIcon(file.type)}
                            </div>
                            <div className="flex-1 min-w-0">
                                <h3 className="text-base font-medium text-gray-900 dark:text-white truncate" title={file.name}>
                                    {file.name}
                                </h3>
                                <p className="text-sm text-gray-500 dark:text-gray-400 capitalize">
                                    {file.type === 'folder' ? 'Folder' : file.content_type || `${file.type} file`}
                                </p>
                            </div>
                        </div>

                        {/* Properties List */}
                        <div className="space-y-0">
                            {/* Owner */}
                            <div className="flex items-start py-3 border-b border-gray-100 dark:border-gray-700">
                                <User className="w-4 h-4 text-gray-400 mt-0.5 mr-3 flex-shrink-0" />
                                <div className="flex-1 min-w-0">
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mb-0.5">Owner</p>
                                    <div className="flex items-center">
                                        {file.owner_avatar ? (
                                            <img 
                                                src={file.owner_avatar} 
                                                alt={file.owner}
                                                className="w-6 h-6 rounded-full object-cover mr-2"
                                            />
                                        ) : (
                                            <div className="w-6 h-6 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-xs font-medium text-primary-700 dark:text-primary-300 mr-2">
                                                {file.owner?.charAt(0)?.toUpperCase() || '?'}
                                            </div>
                                        )}
                                        <span className="text-sm text-gray-900 dark:text-white">{file.owner || 'Unknown'}</span>
                                    </div>
                                </div>
                            </div>

                            {/* Size */}
                            <PropertyRow 
                                icon={HardDrive} 
                                label="Size" 
                                value={file.size || (file.type === 'folder' ? 'Calculating...' : 'Unknown')} 
                            />

                            {/* Visibility */}
                            <div className="flex items-start py-3 border-b border-gray-100 dark:border-gray-700">
                                {file.visibility === 'private' ? (
                                    <EyeOff className="w-4 h-4 text-purple-500 mt-0.5 mr-3 flex-shrink-0" />
                                ) : (
                                    <Eye className="w-4 h-4 text-gray-400 mt-0.5 mr-3 flex-shrink-0" />
                                )}
                                <div className="flex-1 min-w-0">
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mb-0.5">Visibility</p>
                                    <p className="text-sm text-gray-900 dark:text-white">
                                        {file.visibility === 'private' ? (
                                            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-purple-100 dark:bg-purple-900/30 text-purple-800 dark:text-purple-300">
                                                Private
                                            </span>
                                        ) : (
                                            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300">
                                                Department
                                            </span>
                                        )}
                                    </p>
                                </div>
                            </div>

                            {/* Department (if department visibility) */}
                            {file.visibility === 'department' && departmentName && (
                                <PropertyRow 
                                    icon={Building} 
                                    label="Department" 
                                    value={departmentName} 
                                />
                            )}

                            {/* Lock Status */}
                            {file.is_locked && (
                                <div className="flex items-start py-3 border-b border-gray-100 dark:border-gray-700">
                                    <Lock className="w-4 h-4 text-orange-500 mt-0.5 mr-3 flex-shrink-0" />
                                    <div className="flex-1 min-w-0">
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mb-0.5">Lock Status</p>
                                        <p className="text-sm text-gray-900 dark:text-white">
                                            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-orange-100 dark:bg-orange-900/30 text-orange-800 dark:text-orange-300">
                                                Locked
                                            </span>
                                            {file.lock_requires_role && (
                                                <span className="ml-2 text-xs text-gray-500">
                                                    ({file.lock_requires_role}+ required)
                                                </span>
                                            )}
                                        </p>
                                        {file.locked_at && (
                                            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                                                Locked on {formatDate(file.locked_at)}
                                            </p>
                                        )}
                                    </div>
                                </div>
                            )}

                            {/* Created Date */}
                            <PropertyRow 
                                icon={Calendar} 
                                label="Created" 
                                value={formatDate(file.created_at)} 
                            />

                            {/* Modified Date */}
                            <PropertyRow 
                                icon={Calendar} 
                                label="Modified" 
                                value={formatDate(file.modified)} 
                            />
                        </div>
                    </div>

                    {/* Comments Section - only for files, not folders */}
                    {file.type !== 'folder' && companyId && (
                        <FileCommentsPanel fileId={file.id} companyId={companyId} />
                    )}

                    {/* Footer */}
                    <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
                        <button
                            onClick={onClose}
                            className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-200 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors"
                        >
                            Close
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
