import React from 'react';
import { Layers, MoreVertical, Lock } from 'lucide-react';
import clsx from 'clsx';

// Generate initials from owner name (e.g., "Manager User" -> "MU")
const getInitials = (name?: string): string => {
    if (!name) return '?';
    const words = name.trim().split(/\s+/);
    if (words.length === 1) {
        return words[0].substring(0, 2).toUpperCase();
    }
    return (words[0][0] + words[words.length - 1][0]).toUpperCase();
};

// Format bytes to human-readable size (like regular files)
const formatFileSize = (bytes?: number): string => {
    if (!bytes || bytes === 0) return '0 KB';
    
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const k = 1024;
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    const size = bytes / Math.pow(k, i);
    
    return `${size.toFixed(1)} ${units[i]}`;
};

// Vibrant colors for group icons - matches the aesthetic of folders/files
const VIBRANT_COLORS = [
    '#F472B6', // Pink
    '#FBBF24', // Amber/Yellow
    '#A78BFA', // Purple
    '#34D399', // Emerald
    '#60A5FA', // Blue
    '#F87171', // Red
    '#2DD4BF', // Teal
];

// Get a consistent vibrant color based on group name
const getVibrantColor = (name: string): string => {
    let hash = 0;
    for (let i = 0; i < name.length; i++) {
        hash = name.charCodeAt(i) + ((hash << 5) - hash);
    }
    return VIBRANT_COLORS[Math.abs(hash) % VIBRANT_COLORS.length];
};

interface FileGroupStackProps {
    id: string;
    name: string;
    color?: string;
    fileCount: number;
    totalSize?: number; // Total size in bytes
    owner?: string;
    onClick: () => void;
    onMenuClick?: (e: React.MouseEvent) => void;
    showMenu?: boolean;
    isSelected?: boolean;
    isDraggable?: boolean;
    onDragStart?: (e: React.DragEvent) => void;
    className?: string;
    // Locking
    isLocked?: boolean;
    lockRequiresRole?: string;
}

export function FileGroupStack({
    id,
    name,
    color,
    fileCount,
    totalSize,
    owner,
    onClick,
    onMenuClick,
    showMenu,
    isSelected,
    isDraggable = false,
    onDragStart,
    className,
    isLocked,
    lockRequiresRole,
}: FileGroupStackProps) {
    // Calculate how many "cards" to show in stack (max 3)
    const stackCards = Math.min(Math.max(fileCount, 1), 3);
    
    // Use provided color or generate a vibrant one based on name
    const iconColor = color || getVibrantColor(name);

    return (
        <div
            className={clsx(
                'group relative cursor-pointer transition-all duration-200',
                'hover:-translate-y-0.5 hover:shadow-lg',
                className
            )}
            onClick={onClick}
            draggable={isDraggable}
            onDragStart={onDragStart}
        >
            {/* Background stacked cards - subtle effect */}
            {stackCards >= 3 && (
                <div
                    className="absolute inset-0 rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 h-44"
                    style={{ 
                        transform: 'rotate(3deg) translateX(3px)',
                    }}
                />
            )}
            {stackCards >= 2 && (
                <div
                    className="absolute inset-0 rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 h-44"
                    style={{ 
                        transform: 'rotate(1.5deg) translateX(1.5px)',
                    }}
                />
            )}
            
            {/* Main card - matches regular file card structure exactly */}
            {/* Note: removed overflow-hidden to allow tooltip to escape the card bounds */}
            <div
                className={clsx(
                    "relative bg-white dark:bg-gray-800 border rounded-xl p-4 transition-all flex flex-col items-center text-center h-44 w-full",
                    isSelected
                        ? "border-primary-400 dark:border-primary-500 ring-2 ring-primary-200 dark:ring-primary-800"
                        : "border-gray-200 dark:border-gray-700 hover:border-primary-300 dark:hover:border-primary-500"
                )}
                style={{
                    // Subtle colored border tint on hover
                    borderColor: isSelected ? undefined : undefined,
                }}
            >
                {/* Enhanced glow background - more visible */}
                <div 
                    className="absolute inset-0 pointer-events-none"
                    style={{
                        background: `radial-gradient(circle at center, ${iconColor}50 0%, ${iconColor}20 40%, transparent 70%)`,
                        opacity: 0.5,
                    }}
                />

                {/* Lock indicator - top left */}
                {isLocked && (
                    <div 
                        className="absolute top-2 left-2 z-20 group/lock"
                        title={lockRequiresRole ? `Locked (${lockRequiresRole} or higher)` : 'Locked'}
                    >
                        <div className="p-1.5 bg-amber-100 dark:bg-amber-900/30 rounded-full border border-amber-200 dark:border-amber-800">
                            <Lock className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
                        </div>
                        {/* Tooltip */}
                        <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 bg-gray-900 dark:bg-gray-700 text-white text-xs rounded opacity-0 group-hover/lock:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-lg z-[100]">
                            {lockRequiresRole ? `${lockRequiresRole} or higher` : 'Locked'}
                            <div className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-gray-900 dark:border-t-gray-700"></div>
                        </div>
                    </div>
                )}

                {/* Menu button - top right, visible on hover */}
                {onMenuClick && (
                    <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity flex space-x-1 z-20">
                        <button
                            className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-full bg-white dark:bg-gray-800 shadow-sm border border-gray-100 dark:border-gray-700"
                            onClick={(e) => {
                                e.stopPropagation();
                                onMenuClick(e);
                            }}
                        >
                            <MoreVertical className="w-4 h-4 text-gray-500" />
                        </button>
                    </div>
                )}

                {/* Icon area - flex-1 to take remaining space, centered */}
                <div className="flex-1 flex items-center justify-center w-full mb-3 relative">
                    <Layers 
                        className="w-14 h-14" 
                        style={{ 
                            color: iconColor,
                            filter: `drop-shadow(0 0 8px ${iconColor}80)`,
                        }}
                        strokeWidth={1.5}
                    />
                </div>

                {/* Bottom section - name, size, and avatar (matches regular files) */}
                <div className="w-full relative z-10">
                    <p className="text-sm font-medium text-gray-900 dark:text-white truncate w-full text-left" title={name}>
                        {name}
                    </p>
                    <div className="flex items-center justify-between mt-1">
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                            {formatFileSize(totalSize)}
                        </p>
                        {/* Owner avatar with tooltip */}
                        {owner && (
                            <div className="relative group/avatar">
                                <div 
                                    className="h-10 w-10 rounded-full flex items-center justify-center text-white text-xs font-bold bg-primary-600 ring-2 ring-white dark:ring-gray-800 shadow-sm hover:ring-primary-300 dark:hover:ring-primary-600 transition-all cursor-default"
                                    title={owner}
                                >
                                    {getInitials(owner)}
                                </div>
                                {/* Styled tooltip - high z-index to escape card bounds */}
                                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-1.5 bg-gray-900 dark:bg-gray-700 text-white text-xs rounded-lg opacity-0 group-hover/avatar:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-lg z-[100]">
                                    <div className="font-medium">{owner}</div>
                                    <div className="text-gray-400 text-[10px]">Owner</div>
                                    {/* Tooltip arrow */}
                                    <div className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-gray-900 dark:border-t-gray-700"></div>
                                </div>
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}

export default FileGroupStack;
