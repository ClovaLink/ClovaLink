import { useState } from 'react';
import { X, AlertTriangle } from 'lucide-react';

interface RejectFileModalProps {
    isOpen: boolean;
    onClose: () => void;
    onReject: (reason: string) => void;
    fileName: string;
}

export function RejectFileModal({ isOpen, onClose, onReject, fileName }: RejectFileModalProps) {
    const [reason, setReason] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);

    if (!isOpen) return null;

    const handleSubmit = async () => {
        if (reason.trim().length < 5) return;
        setIsSubmitting(true);
        await onReject(reason.trim());
        setIsSubmitting(false);
        setReason('');
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
            <div className="absolute inset-0 bg-black/50" onClick={onClose} />
            <div className="relative bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-md w-full mx-4">
                <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
                    <div className="flex items-center space-x-2">
                        <AlertTriangle className="w-5 h-5 text-red-500" />
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">Reject File</h3>
                    </div>
                    <button onClick={onClose} className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
                        <X className="w-5 h-5" />
                    </button>
                </div>
                <div className="p-4 space-y-4">
                    <p className="text-sm text-gray-600 dark:text-gray-400">
                        You are about to reject <span className="font-medium text-gray-900 dark:text-white">"{fileName}"</span>.
                        Please provide a reason for the rejection.
                    </p>
                    <textarea
                        value={reason}
                        onChange={(e) => setReason(e.target.value)}
                        placeholder="Reason for rejection (required, minimum 5 characters)..."
                        rows={4}
                        className="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-900 text-gray-900 dark:text-white focus:ring-2 focus:ring-red-500 focus:border-transparent resize-none"
                        autoFocus
                    />
                    {reason.trim().length > 0 && reason.trim().length < 5 && (
                        <p className="text-xs text-red-500">Reason must be at least 5 characters</p>
                    )}
                </div>
                <div className="flex justify-end space-x-3 p-4 border-t border-gray-200 dark:border-gray-700">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 rounded-lg transition-colors"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleSubmit}
                        disabled={reason.trim().length < 5 || isSubmitting}
                        className="px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed rounded-lg transition-colors"
                    >
                        {isSubmitting ? 'Rejecting...' : 'Reject'}
                    </button>
                </div>
            </div>
        </div>
    );
}
