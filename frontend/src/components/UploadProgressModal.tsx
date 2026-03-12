import React, { useState, useEffect } from 'react';
import { X, FileText, CheckCircle, AlertCircle, Loader2, ChevronDown, ChevronUp, Upload } from 'lucide-react';
import clsx from 'clsx';

export interface UploadFile {
    file: File;
    progress: number;
    status: 'pending' | 'uploading' | 'completed' | 'error';
    error?: string;
}

interface UploadProgressModalProps {
    isOpen: boolean;
    onClose: () => void;
    files: UploadFile[];
}

export function UploadProgressModal({ isOpen, onClose, files }: UploadProgressModalProps) {
    const [expanded, setExpanded] = useState(true);

    const totalProgress = files.length > 0
        ? Math.round(files.reduce((acc, f) => acc + f.progress, 0) / files.length)
        : 0;
    const isAllCompleted = files.length > 0 && files.every(f => f.status === 'completed' || f.status === 'error');
    const completedCount = files.filter(f => f.status === 'completed').length;
    const errorCount = files.filter(f => f.status === 'error').length;
    const hasErrors = errorCount > 0;
    const isUploading = files.some(f => f.status === 'uploading' || f.status === 'pending');

    // Auto-dismiss 4 seconds after all complete (no errors)
    useEffect(() => {
        if (isAllCompleted && !hasErrors && isOpen) {
            const timer = setTimeout(() => {
                onClose();
            }, 4000);
            return () => clearTimeout(timer);
        }
    }, [isAllCompleted, hasErrors, isOpen, onClose]);

    // Expand when new uploads start
    useEffect(() => {
        if (isUploading) {
            setExpanded(true);
        }
    }, [isUploading]);

    if (!isOpen || files.length === 0) return null;

    const headerText = isAllCompleted
        ? hasErrors
            ? `${completedCount} uploaded, ${errorCount} failed`
            : `${completedCount} upload${completedCount !== 1 ? 's' : ''} complete`
        : `Uploading ${files.length} file${files.length !== 1 ? 's' : ''}`;

    return (
        <div
            className="fixed bottom-4 right-4 z-50 w-[380px] bg-white dark:bg-gray-800 rounded-lg shadow-2xl border border-gray-200 dark:border-gray-700 overflow-hidden"
            style={{ animation: 'upload-slide-up 0.3s ease-out' }}
        >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-2.5 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-700">
                <div className="flex items-center gap-2 min-w-0">
                    {isUploading ? (
                        <Loader2 className="h-4 w-4 text-primary-500 animate-spin flex-shrink-0" />
                    ) : isAllCompleted && !hasErrors ? (
                        <CheckCircle className="h-4 w-4 text-green-500 flex-shrink-0" />
                    ) : hasErrors ? (
                        <AlertCircle className="h-4 w-4 text-red-500 flex-shrink-0" />
                    ) : (
                        <Upload className="h-4 w-4 text-gray-400 flex-shrink-0" />
                    )}
                    <span className="text-sm font-medium text-gray-900 dark:text-white truncate">
                        {headerText}
                    </span>
                </div>
                <div className="flex items-center gap-0.5 flex-shrink-0">
                    <button
                        onClick={() => setExpanded(!expanded)}
                        className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 transition-colors"
                    >
                        {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
                    </button>
                    <button
                        onClick={onClose}
                        className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 transition-colors"
                    >
                        <X className="h-4 w-4" />
                    </button>
                </div>
            </div>

            {/* Overall progress bar */}
            {isUploading && (
                <div className="h-1 bg-gray-200 dark:bg-gray-700">
                    <div
                        className={clsx(
                            "h-full transition-all duration-500 ease-out",
                            hasErrors ? "bg-red-500" : "bg-primary-500"
                        )}
                        style={{ width: `${totalProgress}%` }}
                    />
                </div>
            )}

            {/* File list */}
            {expanded && (
                <div className="max-h-[240px] overflow-y-auto divide-y divide-gray-100 dark:divide-gray-700/50">
                    {files.map((f, idx) => (
                        <div key={idx} className="flex items-center gap-3 px-4 py-2 hover:bg-gray-50 dark:hover:bg-gray-700/30">
                            <FileText className="h-4 w-4 text-gray-400 flex-shrink-0" />
                            <span className="text-sm text-gray-700 dark:text-gray-300 truncate flex-1 min-w-0">
                                {f.file.name}
                            </span>
                            <div className="flex-shrink-0">
                                {f.status === 'uploading' && (
                                    <Loader2 className="h-4 w-4 text-primary-500 animate-spin" />
                                )}
                                {f.status === 'completed' && (
                                    <CheckCircle className="h-4 w-4 text-green-500" />
                                )}
                                {f.status === 'error' && (
                                    <AlertCircle className="h-4 w-4 text-red-500" />
                                )}
                                {f.status === 'pending' && (
                                    <span className="text-xs text-gray-400">Pending</span>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}
