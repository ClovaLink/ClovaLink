import { useState, useEffect } from 'react';
import { X, Folder, FolderOpen, Home, ChevronRight, ChevronDown, Building2, Users, EyeOff } from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch } from '../context/AuthContext';
import { useTenant } from '../context/TenantContext';

interface FolderNode {
    id: string;
    name: string;
    parent_path: string | null;
    children: FolderNode[];
    isExpanded: boolean;
}

interface MoveResult {
    success: boolean;
    error?: string;
    duplicate?: boolean;
    conflicting_name?: string;
    suggested_name?: string;
}

interface MoveFileModalProps {
    isOpen: boolean;
    onClose: () => void;
    onMove: (targetParentId: string | null, targetDepartmentId: string | null, targetVisibility: string, newName?: string) => Promise<MoveResult>;
    fileName: string;
    fileCount?: number;  // For bulk moves
    isMoving: boolean;
    currentPath: string | null;
    currentVisibility?: 'department' | 'private';
    canCrossDepartment: boolean;
}

export function MoveFileModal({ 
    isOpen, 
    onClose, 
    onMove, 
    fileName,
    fileCount = 1,
    isMoving,
    currentPath,
    currentVisibility = 'department',
    canCrossDepartment
}: MoveFileModalProps) {
    const authFetch = useAuthFetch();
    const { currentCompany } = useTenant();
    const [folders, setFolders] = useState<FolderNode[]>([]);
    const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);
    const [selectedPath, setSelectedPath] = useState<string | null>(null);
    const [departments, setDepartments] = useState<any[]>([]);
    const [selectedDepartment, setSelectedDepartment] = useState<string | null>(null);
    const [selectedVisibility, setSelectedVisibility] = useState<'department' | 'private'>(currentVisibility);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState('');
    const [isDuplicate, setIsDuplicate] = useState(false);
    const [newFileName, setNewFileName] = useState('');
    
    // Reset state when modal opens
    useEffect(() => {
        if (isOpen) {
            setSelectedVisibility(currentVisibility);
            setError('');
            setIsDuplicate(false);
            setNewFileName('');
        }
    }, [isOpen, currentVisibility]);

    useEffect(() => {
        if (isOpen && currentCompany?.id) {
            fetchFolders();
            if (canCrossDepartment) {
                fetchDepartments();
            }
        }
    }, [isOpen, currentCompany?.id, selectedDepartment, selectedVisibility]);

    const fetchFolders = async () => {
        if (!currentCompany?.id) return;
        setIsLoading(true);
        try {
            const params = new URLSearchParams();
            if (selectedDepartment) params.set('department_id', selectedDepartment);
            params.set('visibility', selectedVisibility);
            const queryString = params.toString();
            const res = await authFetch(`/api/files/${currentCompany.id}?${queryString}`);
            if (res.ok) {
                const files = await res.json();
                // Filter only folders and build tree
                const folderList = files.filter((f: any) => f.type === 'folder');
                const tree = buildFolderTree(folderList);
                setFolders(tree);
            }
        } catch (err) {
            console.error('Failed to fetch folders', err);
        } finally {
            setIsLoading(false);
        }
    };

    const fetchDepartments = async () => {
        if (!currentCompany?.id) return;
        try {
            const res = await authFetch(`/api/departments?tenant_id=${currentCompany.id}`);
            if (res.ok) {
                const depts = await res.json();
                setDepartments(depts);
            }
        } catch (err) {
            console.error('Failed to fetch departments', err);
        }
    };

    const buildFolderTree = (folderList: any[]): FolderNode[] => {
        // Create nodes
        const nodes: { [key: string]: FolderNode } = {};
        folderList.forEach(f => {
            nodes[f.id] = {
                id: f.id,
                name: f.name,
                parent_path: f.parent_path || null,
                children: [],
                isExpanded: false
            };
        });

        // Build tree
        const rootFolders: FolderNode[] = [];
        
        folderList.forEach(f => {
            const node = nodes[f.id];
            // Find parent by matching parent_path
            if (!f.parent_path || f.parent_path === '') {
                rootFolders.push(node);
            } else {
                // Find the parent folder
                const parentName = f.parent_path.split('/').pop();
                const parent = folderList.find(p => 
                    p.name === parentName && 
                    (f.parent_path === p.name || f.parent_path.endsWith('/' + p.name))
                );
                if (parent && nodes[parent.id]) {
                    nodes[parent.id].children.push(node);
                } else {
                    rootFolders.push(node);
                }
            }
        });

        return rootFolders;
    };

    const toggleExpand = (folderId: string) => {
        setFolders(prev => {
            const toggle = (nodes: FolderNode[]): FolderNode[] => {
                return nodes.map(node => {
                    if (node.id === folderId) {
                        return { ...node, isExpanded: !node.isExpanded };
                    }
                    return { ...node, children: toggle(node.children) };
                });
            };
            return toggle(prev);
        });
    };

    const selectFolder = (folderId: string | null, path: string | null) => {
        setSelectedFolderId(folderId);
        setSelectedPath(path);
    };

    const handleMove = async (withNewName?: boolean) => {
        setError('');
        setIsDuplicate(false);
        try {
            const nameToUse = withNewName && newFileName.trim() ? newFileName.trim() : undefined;
            const result = await onMove(selectedFolderId, selectedDepartment, selectedVisibility, nameToUse);
            
            if (result.success) {
                onClose();
            } else if (result.duplicate) {
                setIsDuplicate(true);
                setNewFileName(result.suggested_name || fileName);
                setError(result.error || `A file with this name already exists in the target location`);
            } else {
                setError(result.error || 'Failed to move file');
            }
        } catch (err) {
            setError('Failed to move file');
        }
    };
    
    const handleMoveWithRename = async () => {
        if (!newFileName.trim()) {
            setError('Please enter a new file name');
            return;
        }
        await handleMove(true);
    };

    const renderFolderNode = (node: FolderNode, depth: number = 0): React.ReactElement => {
        const hasChildren = node.children.length > 0;
        const path = node.parent_path ? `${node.parent_path}/${node.name}` : node.name;
        const isSelected = selectedFolderId === node.id;
        const isCurrentLocation = currentPath === path;

        return (
            <div key={node.id}>
                <div 
                    className={clsx(
                        "flex items-center gap-2 px-3 py-2 cursor-pointer rounded-lg transition-colors",
                        isSelected && "bg-primary-100 dark:bg-primary-900/30 border border-primary-300 dark:border-primary-700",
                        !isSelected && !isCurrentLocation && "hover:bg-gray-100 dark:hover:bg-gray-700",
                        isCurrentLocation && "opacity-50 cursor-not-allowed"
                    )}
                    style={{ paddingLeft: `${12 + depth * 20}px` }}
                    onClick={() => !isCurrentLocation && selectFolder(node.id, path)}
                >
                    {hasChildren ? (
                        <button 
                            onClick={(e) => { e.stopPropagation(); toggleExpand(node.id); }}
                            className="p-0.5 hover:bg-gray-200 dark:hover:bg-gray-600 rounded"
                        >
                            {node.isExpanded ? 
                                <ChevronDown className="w-4 h-4 text-gray-500" /> : 
                                <ChevronRight className="w-4 h-4 text-gray-500" />
                            }
                        </button>
                    ) : (
                        <span className="w-5" />
                    )}
                    {node.isExpanded ? (
                        <FolderOpen className="w-5 h-5 text-yellow-500" />
                    ) : (
                        <Folder className="w-5 h-5 text-yellow-500" />
                    )}
                    <span className={clsx(
                        "text-sm truncate",
                        isSelected ? "text-primary-700 dark:text-primary-300 font-medium" : "text-gray-700 dark:text-gray-200"
                    )}>
                        {node.name}
                    </span>
                    {isCurrentLocation && (
                        <span className="text-xs text-gray-400">(current)</span>
                    )}
                </div>
                {node.isExpanded && node.children.map(child => renderFolderNode(child, depth + 1))}
            </div>
        );
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-[60] overflow-y-auto">
            <div className="flex items-center justify-center min-h-screen px-4">
                <div className="fixed inset-0 bg-black/50 transition-opacity" onClick={onClose} />
                
                <div className="relative bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-lg w-full max-h-[80vh] flex flex-col">
                    {/* Header */}
                    <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
                        <div>
                            <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                                Move to...
                            </h3>
                            <p className="text-sm text-gray-500 dark:text-gray-400 truncate max-w-[350px]">
                                {fileCount > 1 ? `${fileCount} items selected` : fileName}
                            </p>
                        </div>
                        <button onClick={onClose} className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded">
                            <X className="w-5 h-5 text-gray-500" />
                        </button>
                    </div>

                    {/* Visibility Selector */}
                    <div className="p-4 border-b border-gray-200 dark:border-gray-700">
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            Move to
                        </label>
                        <div className="flex gap-2">
                            <button
                                onClick={() => {
                                    setSelectedVisibility('department');
                                    setSelectedFolderId(null);
                                    setSelectedPath(null);
                                }}
                                className={clsx(
                                    "flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg border-2 transition-colors",
                                    selectedVisibility === 'department'
                                        ? "border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300"
                                        : "border-gray-200 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
                                )}
                            >
                                <Users className="w-4 h-4" />
                                <span className="font-medium">Department Files</span>
                            </button>
                            <button
                                onClick={() => {
                                    setSelectedVisibility('private');
                                    setSelectedFolderId(null);
                                    setSelectedPath(null);
                                }}
                                className={clsx(
                                    "flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg border-2 transition-colors",
                                    selectedVisibility === 'private'
                                        ? "border-purple-500 bg-purple-50 dark:bg-purple-900/20 text-purple-700 dark:text-purple-300"
                                        : "border-gray-200 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
                                )}
                            >
                                <EyeOff className="w-4 h-4" />
                                <span className="font-medium">My Private Files</span>
                            </button>
                        </div>
                    </div>

                    {/* Department Selector */}
                    {canCrossDepartment && departments.length > 0 && (
                        <div className="p-4 border-b border-gray-200 dark:border-gray-700">
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                <Building2 className="w-4 h-4 inline mr-1" />
                                Department
                            </label>
                            <select
                                value={selectedDepartment || ''}
                                onChange={(e) => {
                                    setSelectedDepartment(e.target.value || null);
                                    setSelectedFolderId(null);
                                    setSelectedPath(null);
                                }}
                                className="w-full px-3 py-2 border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                            >
                                <option value="">All Departments</option>
                                {departments.map(dept => (
                                    <option key={dept.id} value={dept.id}>{dept.name}</option>
                                ))}
                            </select>
                        </div>
                    )}

                    {/* Folder Tree */}
                    <div className="flex-1 overflow-y-auto p-4">
                        {isLoading ? (
                            <div className="flex items-center justify-center py-8">
                                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
                            </div>
                        ) : (
                            <div className="space-y-1">
                                {/* Root/Home option */}
                                <div 
                                    className={clsx(
                                        "flex items-center gap-2 px-3 py-2 cursor-pointer rounded-lg transition-colors",
                                        selectedFolderId === null && selectedPath === null && "bg-primary-100 dark:bg-primary-900/30 border border-primary-300 dark:border-primary-700",
                                        !(selectedFolderId === null && selectedPath === null) && "hover:bg-gray-100 dark:hover:bg-gray-700"
                                    )}
                                    onClick={() => selectFolder(null, null)}
                                >
                                    <Home className="w-5 h-5 text-blue-500" />
                                    <span className={clsx(
                                        "text-sm font-medium",
                                        selectedFolderId === null && selectedPath === null 
                                            ? "text-primary-700 dark:text-primary-300" 
                                            : "text-gray-700 dark:text-gray-200"
                                    )}>
                                        Home (Root)
                                    </span>
                                </div>

                                {/* Folder list */}
                                {folders.map(folder => renderFolderNode(folder))}

                                {folders.length === 0 && (
                                    <p className="text-center text-gray-500 dark:text-gray-400 py-4 text-sm">
                                        No folders found
                                    </p>
                                )}
                            </div>
                        )}
                    </div>

                    {/* Error / Duplicate Handling */}
                    {error && (
                        <div className="px-4 pb-2">
                            <div className={clsx(
                                "p-3 rounded-lg text-sm",
                                isDuplicate 
                                    ? "bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300 border border-amber-200 dark:border-amber-700"
                                    : "bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300"
                            )}>
                                {error}
                            </div>
                            
                            {isDuplicate && (
                                <div className="mt-3 space-y-2">
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                        Rename file to:
                                    </label>
                                    <input
                                        type="text"
                                        value={newFileName}
                                        onChange={(e) => setNewFileName(e.target.value)}
                                        className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-primary-500"
                                        placeholder="Enter new file name"
                                    />
                                </div>
                            )}
                        </div>
                    )}

                    {/* Footer */}
                    <div className="flex gap-3 p-4 border-t border-gray-200 dark:border-gray-700">
                        <button
                            onClick={onClose}
                            className="flex-1 px-4 py-2 border border-gray-200 dark:border-gray-600 text-gray-700 dark:text-gray-200 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700"
                        >
                            Cancel
                        </button>
                        {isDuplicate ? (
                            <button
                                onClick={handleMoveWithRename}
                                disabled={isMoving || !newFileName.trim()}
                                className={clsx(
                                    "flex-1 px-4 py-2 bg-primary-600 text-white rounded-lg font-medium",
                                    (isMoving || !newFileName.trim()) ? "opacity-50 cursor-not-allowed" : "hover:bg-primary-700"
                                )}
                            >
                                {isMoving ? 'Moving...' : 'Move with New Name'}
                            </button>
                        ) : (
                            <button
                                onClick={() => handleMove()}
                                disabled={isMoving}
                                className={clsx(
                                    "flex-1 px-4 py-2 bg-primary-600 text-white rounded-lg font-medium",
                                    isMoving ? "opacity-50 cursor-not-allowed" : "hover:bg-primary-700"
                                )}
                            >
                                {isMoving ? 'Moving...' : 'Move Here'}
                            </button>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
