import React, { useState, useEffect } from 'react';
import { X, Download, Loader2 } from 'lucide-react';

interface FilePreviewModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: {
        name: string;
        url: string; // We'll need to generate a presigned URL or serve via proxy
        type: 'image' | 'document' | 'video' | 'audio' | 'folder';
    } | null;
}

export function FilePreviewModal({ isOpen, onClose, file }: FilePreviewModalProps) {
    const [csvContent, setCsvContent] = useState<string[][]>([]);
    const [textContent, setTextContent] = useState<string>('');
    const [loading, setLoading] = useState(false);
    const [mediaBlobUrl, setMediaBlobUrl] = useState<string | null>(null);
    const [mediaError, setMediaError] = useState<string | null>(null);

    useEffect(() => {
        if (!file) {
            // Cleanup blob URL when file is cleared
            if (mediaBlobUrl) {
                URL.revokeObjectURL(mediaBlobUrl);
                setMediaBlobUrl(null);
            }
            return;
        }

        const isCSV = file.name.toLowerCase().endsWith('.csv');
        const isText = file.name.toLowerCase().endsWith('.txt') ||
            file.name.toLowerCase().endsWith('.md') ||
            file.name.toLowerCase().endsWith('.json') ||
            file.name.toLowerCase().endsWith('.xml') ||
            file.name.toLowerCase().endsWith('.log');
        const isImage = file.type === 'image';
        const isVideo = file.type === 'video';
        const isAudio = file.type === 'audio';
        const isPDF = file.name.toLowerCase().endsWith('.pdf');

        // Reset states
        setMediaError(null);
        setCsvContent([]);
        setTextContent('');

        // For media files (image, video, audio, PDF), fetch as blob with auth header
        if (isImage || isVideo || isAudio || isPDF) {
            setLoading(true);
            if (mediaBlobUrl) {
                URL.revokeObjectURL(mediaBlobUrl);
                setMediaBlobUrl(null);
            }
            
            // Add ?preview=true to differentiate preview from download in audit logs
            const previewUrl = file.url.includes('?') ? `${file.url}&preview=true` : `${file.url}?preview=true`;
            fetch(previewUrl, {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}`
                }
            })
                .then(res => {
                    if (!res.ok) throw new Error('Failed to fetch file');
                    return res.blob();
                })
                .then(blob => {
                    // Create a File object with the correct name so PDF viewers can use it
                    const namedFile = new File([blob], file.name, { type: blob.type });
                    const url = URL.createObjectURL(namedFile);
                    setMediaBlobUrl(url);
                })
                .catch(err => {
                    console.error('Failed to load media', err);
                    setMediaError('Failed to load file. Please try downloading instead.');
                })
                .finally(() => setLoading(false));
        } else if (isCSV || isText) {
            setLoading(true);
            // Add ?preview=true to differentiate preview from download in audit logs
            const previewUrl = file.url.includes('?') ? `${file.url}&preview=true` : `${file.url}?preview=true`;
            fetch(previewUrl, {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}`
                }
            })
                .then(res => {
                    if (!res.ok) throw new Error('Failed to fetch file');
                    return res.text();
                })
                .then(text => {
                    if (isCSV) {
                        const rows = text.split('\n').map(row => row.split(','));
                        setCsvContent(rows);
                    } else {
                        setTextContent(text);
                    }
                })
                .catch(err => console.error('Failed to load content', err))
                .finally(() => setLoading(false));
        }

        // Cleanup on unmount
        return () => {
            if (mediaBlobUrl) {
                URL.revokeObjectURL(mediaBlobUrl);
            }
        };
    }, [file]);

    if (!isOpen || !file) return null;

    const isImage = file.type === 'image';
    const isVideo = file.type === 'video';
    const isAudio = file.type === 'audio';
    const isPDF = file.name.toLowerCase().endsWith('.pdf');
    const isCSV = file.name.toLowerCase().endsWith('.csv');
    const isText = file.name.toLowerCase().endsWith('.txt') ||
        file.name.toLowerCase().endsWith('.md') ||
        file.name.toLowerCase().endsWith('.json') ||
        file.name.toLowerCase().endsWith('.xml') ||
        file.name.toLowerCase().endsWith('.log');

    return (
        <div className="fixed inset-0 z-[60] overflow-y-auto bg-black bg-opacity-90 flex items-center justify-center p-4">
            <div className="relative w-full max-w-5xl h-[80vh] bg-white dark:bg-gray-800 rounded-lg shadow-2xl flex flex-col">
                {/* Header */}
                <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
                    <h3 className="text-lg font-medium text-gray-900 dark:text-white truncate">{file.name}</h3>
                    <div className="flex items-center space-x-2">
                        <button
                            onClick={async () => {
                                // Download with auth header
                                const response = await fetch(file.url, {
                                    headers: { 'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}` }
                                });
                                const blob = await response.blob();
                                const url = URL.createObjectURL(blob);
                                const a = document.createElement('a');
                                a.href = url;
                                a.download = file.name;
                                a.click();
                                URL.revokeObjectURL(url);
                            }}
                            className="p-2 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-full"
                            title="Download"
                        >
                            <Download className="w-5 h-5" />
                        </button>
                        <button
                            onClick={onClose}
                            className="p-2 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-full"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>
                </div>

                {/* Content */}
                <div className="flex-1 overflow-auto bg-gray-100 dark:bg-gray-900 flex items-center justify-center p-4">
                    {loading ? (
                        <div className="flex flex-col items-center justify-center text-gray-500 dark:text-gray-400">
                            <Loader2 className="w-8 h-8 animate-spin mb-2" />
                            <p>Loading preview...</p>
                        </div>
                    ) : mediaError ? (
                        <div className="text-center">
                            <p className="text-red-500 dark:text-red-400 mb-4">{mediaError}</p>
                            <button
                                onClick={async () => {
                                    // Download with auth header
                                    const response = await fetch(file.url, {
                                        headers: { 'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}` }
                                    });
                                    const blob = await response.blob();
                                    const url = URL.createObjectURL(blob);
                                    const a = document.createElement('a');
                                    a.href = url;
                                    a.download = file.name;
                                    a.click();
                                    URL.revokeObjectURL(url);
                                }}
                                className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white bg-primary-600 hover:bg-primary-700"
                            >
                                <Download className="w-4 h-4 mr-2" />
                                Download File
                            </button>
                        </div>
                    ) : isImage && mediaBlobUrl ? (
                        <img
                            src={mediaBlobUrl}
                            alt={file.name}
                            className="max-w-full max-h-full object-contain shadow-lg"
                        />
                    ) : isVideo && mediaBlobUrl ? (
                        <video
                            src={mediaBlobUrl}
                            controls
                            autoPlay={false}
                            className="max-w-full max-h-full shadow-lg rounded-lg"
                            style={{ maxHeight: '70vh' }}
                        >
                            Your browser does not support the video tag.
                        </video>
                    ) : isAudio && mediaBlobUrl ? (
                        <div className="w-full max-w-lg bg-white dark:bg-gray-800 rounded-lg shadow-lg p-6">
                            <p className="text-gray-700 dark:text-gray-300 mb-4 text-center font-medium">{file.name}</p>
                            <audio
                                src={mediaBlobUrl}
                                controls
                                className="w-full"
                            >
                                Your browser does not support the audio tag.
                            </audio>
                        </div>
                    ) : isPDF && mediaBlobUrl ? (
                        <iframe
                            src={mediaBlobUrl}
                            className="w-full h-full border-none shadow-lg bg-white rounded-lg"
                            title="PDF Preview"
                        />
                    ) : isCSV ? (
                        <div className="w-full h-full overflow-auto bg-white dark:bg-gray-800 shadow-lg p-4">
                            {loading ? (
                                <div className="flex items-center justify-center h-full">Loading...</div>
                            ) : (
                                <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                                    <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                                        {csvContent.map((row, i) => (
                                            <tr key={i} className={i === 0 ? "bg-gray-50 dark:bg-gray-700 font-medium" : ""}>
                                                {row.map((cell, j) => (
                                                    <td key={j} className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400 border-r border-gray-100 dark:border-gray-700 last:border-none">
                                                        {cell}
                                                    </td>
                                                ))}
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            )}
                        </div>
                    ) : isText ? (
                        <div className="w-full h-full overflow-auto bg-white dark:bg-gray-800 shadow-lg p-6">
                            {loading ? (
                                <div className="flex items-center justify-center h-full">Loading...</div>
                            ) : (
                                <pre className="whitespace-pre-wrap font-mono text-sm text-gray-800 dark:text-gray-200">
                                    {textContent}
                                </pre>
                            )}
                        </div>
                    ) : (
                        <div className="text-center">
                            <p className="text-gray-500 dark:text-gray-400 mb-4">Preview not available for this file type.</p>
                            <button
                                onClick={async () => {
                                    // Download with auth header
                                    const response = await fetch(file.url, {
                                        headers: { 'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}` }
                                    });
                                    const blob = await response.blob();
                                    const url = URL.createObjectURL(blob);
                                    const a = document.createElement('a');
                                    a.href = url;
                                    a.download = file.name;
                                    a.click();
                                    URL.revokeObjectURL(url);
                                }}
                                className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white bg-primary-600 hover:bg-primary-700"
                            >
                                <Download className="w-4 h-4 mr-2" />
                                Download to View
                            </button>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
