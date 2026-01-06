import React, { useState, useEffect } from 'react';
import { X, Download, Loader2, FileSpreadsheet, Presentation, ChevronLeft, ChevronRight } from 'lucide-react';

// Helper functions to extract IDs from download URLs
// URL format: /api/files/{companyId}/download/{fileId}
function extractFileIdFromUrl(url: string): string | null {
    const match = url.match(/\/download\/([a-f0-9-]+)/i);
    return match ? match[1] : null;
}

function extractCompanyIdFromUrl(url: string): string | null {
    const match = url.match(/\/api\/files\/([a-f0-9-]+)\//i);
    return match ? match[1] : null;
}

interface FilePreviewModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: {
        id?: string; // File ID for backend API calls
        companyId?: string; // Company/tenant ID for backend API calls
        name: string;
        url: string; // We'll need to generate a presigned URL or serve via proxy
        type: 'image' | 'document' | 'video' | 'audio' | 'folder';
        size?: number; // File size in bytes for determining client vs server processing
    } | null;
}

interface ExcelSheet {
    name: string;
    data: (string | number | boolean | null)[][];
}

interface PPTXInfo {
    slideCount: number;
    title?: string;
    author?: string;
}

export function FilePreviewModal({ isOpen, onClose, file }: FilePreviewModalProps) {
    const [csvContent, setCsvContent] = useState<string[][]>([]);
    const [textContent, setTextContent] = useState<string>('');
    const [loading, setLoading] = useState(false);
    const [mediaBlobUrl, setMediaBlobUrl] = useState<string | null>(null);
    const [mediaError, setMediaError] = useState<string | null>(null);
    
    // Excel preview state
    const [excelSheets, setExcelSheets] = useState<ExcelSheet[]>([]);
    const [selectedSheetIndex, setSelectedSheetIndex] = useState(0);
    
    // PPTX preview state
    const [pptxInfo, setPptxInfo] = useState<PPTXInfo | null>(null);

    useEffect(() => {
        if (!file) {
            // Cleanup blob URL when file is cleared
            if (mediaBlobUrl) {
                URL.revokeObjectURL(mediaBlobUrl);
                setMediaBlobUrl(null);
            }
            return;
        }

        const fileName = file.name.toLowerCase();
        const isCSV = fileName.endsWith('.csv');
        const isText = fileName.endsWith('.txt') ||
            fileName.endsWith('.md') ||
            fileName.endsWith('.json') ||
            fileName.endsWith('.xml') ||
            fileName.endsWith('.log');
        const isImage = file.type === 'image';
        const isVideo = file.type === 'video';
        const isAudio = file.type === 'audio';
        const isPDF = fileName.endsWith('.pdf');
        const isExcel = fileName.endsWith('.xlsx') || fileName.endsWith('.xls');
        const isPPTX = fileName.endsWith('.pptx') || fileName.endsWith('.ppt');

        // Reset states
        setMediaError(null);
        setCsvContent([]);
        setTextContent('');
        setExcelSheets([]);
        setSelectedSheetIndex(0);
        setPptxInfo(null);

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
        } else if (isExcel) {
            // Excel file preview using backend API (calamine)
            setLoading(true);
            
            // Try to extract file ID and company ID from the URL or use provided props
            const fileId = file.id || extractFileIdFromUrl(file.url);
            const companyId = file.companyId || extractCompanyIdFromUrl(file.url);
            
            if (fileId && companyId) {
                // Use backend API for secure, server-side Excel parsing
                fetch(`/api/preview/${companyId}/${fileId}`, {
                    headers: {
                        'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}`
                    }
                })
                    .then(res => {
                        if (!res.ok) throw new Error('Failed to fetch preview');
                        return res.json();
                    })
                    .then((data: { type: string; sheets: { name: string; rows: (string | number | boolean | null)[][] }[] }) => {
                        if (data.type === 'excel' && data.sheets) {
                            const sheets: ExcelSheet[] = data.sheets.map(sheet => ({
                                name: sheet.name,
                                data: sheet.rows
                            }));
                            setExcelSheets(sheets);
                        } else {
                            setMediaError('Failed to parse Excel file. Try downloading instead.');
                        }
                    })
                    .catch(err => {
                        console.error('Failed to load Excel preview:', err);
                        setMediaError('Failed to load file preview. Please try downloading instead.');
                    })
                    .finally(() => setLoading(false));
            } else {
                // Fallback: if no file ID, show download prompt
                setMediaError('Excel preview requires file metadata. Please download to view.');
                setLoading(false);
            }
        } else if (isPPTX) {
            // PowerPoint file - show metadata and download option
            // Full PPTX rendering requires complex libraries, so we show basic info
            setLoading(true);
            const previewUrl = file.url.includes('?') ? `${file.url}&preview=true` : `${file.url}?preview=true`;
            fetch(previewUrl, {
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token')}`
                }
            })
                .then(res => {
                    if (!res.ok) throw new Error('Failed to fetch file');
                    return res.arrayBuffer();
                })
                .then(async buffer => {
                    try {
                        // Use JSZip-like approach to read PPTX (which is a ZIP file)
                        const JSZip = (await import('jszip')).default;
                        const zip = await JSZip.loadAsync(buffer);
                        
                        // Count slides by looking at ppt/slides/slide*.xml files
                        let slideCount = 0;
                        zip.forEach((relativePath) => {
                            if (relativePath.match(/ppt\/slides\/slide\d+\.xml/)) {
                                slideCount++;
                            }
                        });
                        
                        // Try to get title and author from core.xml
                        let title: string | undefined;
                        let author: string | undefined;
                        const coreXml = zip.file('docProps/core.xml');
                        if (coreXml) {
                            const coreContent = await coreXml.async('text');
                            const titleMatch = coreContent.match(/<dc:title>([^<]*)<\/dc:title>/);
                            const authorMatch = coreContent.match(/<dc:creator>([^<]*)<\/dc:creator>/);
                            title = titleMatch?.[1];
                            author = authorMatch?.[1];
                        }
                        
                        setPptxInfo({ slideCount, title, author });
                    } catch (err) {
                        console.error('Failed to parse PPTX file:', err);
                        // Still show something useful
                        setPptxInfo({ slideCount: 0, title: file.name });
                    }
                })
                .catch(err => {
                    console.error('Failed to load PPTX file', err);
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

    const fileName = file.name.toLowerCase();
    const isImage = file.type === 'image';
    const isVideo = file.type === 'video';
    const isAudio = file.type === 'audio';
    const isPDF = fileName.endsWith('.pdf');
    const isCSV = fileName.endsWith('.csv');
    const isText = fileName.endsWith('.txt') ||
        fileName.endsWith('.md') ||
        fileName.endsWith('.json') ||
        fileName.endsWith('.xml') ||
        fileName.endsWith('.log');
    const isExcel = fileName.endsWith('.xlsx') || fileName.endsWith('.xls');
    const isPPTX = fileName.endsWith('.pptx') || fileName.endsWith('.ppt');

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
                    ) : isExcel && excelSheets.length > 0 ? (
                        <div className="w-full h-full flex flex-col bg-white dark:bg-gray-800 shadow-lg rounded-lg overflow-hidden">
                            {/* Sheet tabs */}
                            {excelSheets.length > 1 && (
                                <div className="flex items-center gap-1 px-4 py-2 bg-gray-100 dark:bg-gray-700 border-b border-gray-200 dark:border-gray-600 overflow-x-auto">
                                    <FileSpreadsheet className="w-4 h-4 text-green-600 mr-2 flex-shrink-0" />
                                    {excelSheets.map((sheet, idx) => (
                                        <button
                                            key={sheet.name}
                                            onClick={() => setSelectedSheetIndex(idx)}
                                            className={`px-3 py-1.5 text-sm font-medium rounded-t whitespace-nowrap transition-colors ${
                                                selectedSheetIndex === idx
                                                    ? 'bg-white dark:bg-gray-800 text-primary-600 dark:text-primary-400 border-t border-x border-gray-200 dark:border-gray-600'
                                                    : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
                                            }`}
                                        >
                                            {sheet.name}
                                        </button>
                                    ))}
                                </div>
                            )}
                            {/* Spreadsheet content */}
                            <div className="flex-1 overflow-auto p-4">
                                <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700 border border-gray-200 dark:border-gray-700">
                                    <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                                        {excelSheets[selectedSheetIndex]?.data.map((row, i) => (
                                            <tr key={i} className={i === 0 ? "bg-gray-50 dark:bg-gray-700 font-semibold" : "hover:bg-gray-50 dark:hover:bg-gray-700/50"}>
                                                {/* Row number */}
                                                <td className="px-2 py-2 text-xs text-gray-400 dark:text-gray-500 bg-gray-50 dark:bg-gray-700/50 border-r border-gray-200 dark:border-gray-600 text-center w-10">
                                                    {i + 1}
                                                </td>
                                                {row.map((cell, j) => (
                                                    <td 
                                                        key={j} 
                                                        className="px-3 py-2 text-sm text-gray-700 dark:text-gray-300 border-r border-gray-100 dark:border-gray-700 last:border-none whitespace-nowrap max-w-xs truncate"
                                                        title={String(cell ?? '')}
                                                    >
                                                        {cell !== null && cell !== undefined ? String(cell) : ''}
                                                    </td>
                                                ))}
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                                {excelSheets[selectedSheetIndex]?.data.length >= 1000 && (
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-2 text-center">
                                        Showing first 1,000 rows. Download for full content.
                                    </p>
                                )}
                            </div>
                        </div>
                    ) : isPPTX && pptxInfo ? (
                        <div className="w-full max-w-md bg-white dark:bg-gray-800 rounded-xl shadow-lg p-8 text-center">
                            <div className="mb-6">
                                <div className="w-20 h-20 mx-auto bg-orange-100 dark:bg-orange-900/30 rounded-2xl flex items-center justify-center mb-4">
                                    <Presentation className="w-10 h-10 text-orange-600 dark:text-orange-400" />
                                </div>
                                <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
                                    {pptxInfo.title || file.name}
                                </h3>
                                {pptxInfo.author && (
                                    <p className="text-sm text-gray-500 dark:text-gray-400">
                                        by {pptxInfo.author}
                                    </p>
                                )}
                            </div>
                            
                            <div className="flex items-center justify-center gap-6 mb-6 text-gray-600 dark:text-gray-400">
                                <div className="flex items-center gap-2">
                                    <div className="flex items-center justify-center w-8 h-8 bg-gray-100 dark:bg-gray-700 rounded-lg">
                                        <ChevronLeft className="w-4 h-4" />
                                        <ChevronRight className="w-4 h-4 -ml-2" />
                                    </div>
                                    <span className="text-2xl font-bold text-gray-900 dark:text-white">
                                        {pptxInfo.slideCount}
                                    </span>
                                    <span className="text-sm">slides</span>
                                </div>
                            </div>
                            
                            <p className="text-sm text-gray-500 dark:text-gray-400 mb-6">
                                PowerPoint preview requires download. Click below to open in your preferred application.
                            </p>
                            
                            <button
                                onClick={async () => {
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
                                className="inline-flex items-center px-6 py-3 border border-transparent text-sm font-medium rounded-lg shadow-sm text-white bg-orange-600 hover:bg-orange-700 transition-colors"
                            >
                                <Download className="w-4 h-4 mr-2" />
                                Download Presentation
                            </button>
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
