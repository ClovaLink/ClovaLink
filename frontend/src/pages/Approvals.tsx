import { useState, useEffect } from 'react';
import { CheckCircle, XCircle, Clock, FileText, Search, RefreshCw } from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch } from '../context/AuthContext';
import { useTenant } from '../context/TenantContext';
import { useGlobalSettings } from '../context/GlobalSettingsContext';
import { RejectFileModal } from '../components/RejectFileModal';

interface ApprovalItem {
    id: string;
    file_id: string;
    tenant_id: string;
    policy_id?: string;
    requested_by: string;
    status: string;
    decided_by?: string;
    decided_at?: string;
    rejection_reason?: string;
    created_at: string;
    file_name: string;
    file_size: number;
    content_type: string;
    department_id?: string;
    uploader_email: string;
    uploader_name?: string;
    decider_email?: string;
}

interface ApprovalStats {
    pending: number;
    approved: number;
    rejected: number;
}

function formatFileSize(bytes: number): string {
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
}

export function Approvals() {
    const [tab, setTab] = useState<'pending' | 'history'>('pending');
    const [pendingItems, setPendingItems] = useState<ApprovalItem[]>([]);
    const [historyItems, setHistoryItems] = useState<ApprovalItem[]>([]);
    const [stats, setStats] = useState<ApprovalStats>({ pending: 0, approved: 0, rejected: 0 });
    const [isLoading, setIsLoading] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');
    const [rejectItem, setRejectItem] = useState<ApprovalItem | null>(null);
    const authFetch = useAuthFetch();
    const { currentCompany } = useTenant();
    const { formatDate } = useGlobalSettings();
    const companyId = currentCompany?.id;

    const fetchData = async () => {
        if (!companyId) return;
        setIsLoading(true);
        try {
            const [pendingRes, historyRes, statsRes] = await Promise.all([
                authFetch(`/api/approvals/${companyId}/pending`),
                authFetch(`/api/approvals/${companyId}/history`),
                authFetch(`/api/approvals/${companyId}/stats`),
            ]);
            if (pendingRes.ok) {
                const data = await pendingRes.json();
                setPendingItems(data.approvals || []);
            }
            if (historyRes.ok) {
                const data = await historyRes.json();
                setHistoryItems(data.history || []);
            }
            if (statsRes.ok) {
                const data = await statsRes.json();
                setStats(data);
            }
        } catch (error) {
            console.error('Failed to fetch approvals:', error);
        } finally {
            setIsLoading(false);
        }
    };

    useEffect(() => {
        fetchData();
    }, [companyId]);

    const handleApprove = async (item: ApprovalItem) => {
        if (!companyId) return;
        try {
            const response = await authFetch(`/api/approvals/${companyId}/${item.id}/approve`, {
                method: 'POST',
            });
            if (response.ok) {
                fetchData();
            }
        } catch (error) {
            console.error('Failed to approve:', error);
        }
    };

    const handleReject = async (reason: string) => {
        if (!companyId || !rejectItem) return;
        try {
            const response = await authFetch(`/api/approvals/${companyId}/${rejectItem.id}/reject`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ reason }),
            });
            if (response.ok) {
                setRejectItem(null);
                fetchData();
            }
        } catch (error) {
            console.error('Failed to reject:', error);
        }
    };

    const filteredPending = pendingItems.filter(item =>
        item.file_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.uploader_email.toLowerCase().includes(searchTerm.toLowerCase())
    );

    const filteredHistory = historyItems.filter(item =>
        item.file_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.uploader_email.toLowerCase().includes(searchTerm.toLowerCase())
    );

    const items = tab === 'pending' ? filteredPending : filteredHistory;

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Document Approvals</h1>
                    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                        Review and approve uploaded documents
                    </p>
                </div>
                <button
                    onClick={fetchData}
                    className="p-2 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                    title="Refresh"
                >
                    <RefreshCw className={clsx("w-5 h-5", isLoading && "animate-spin")} />
                </button>
            </div>

            {/* Stats */}
            <div className="grid grid-cols-3 gap-4">
                <div className="bg-amber-50 dark:bg-amber-900/20 rounded-lg p-4">
                    <div className="flex items-center space-x-2">
                        <Clock className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                        <span className="text-sm font-medium text-amber-700 dark:text-amber-300">Pending</span>
                    </div>
                    <p className="text-2xl font-bold text-amber-900 dark:text-amber-100 mt-1">{stats.pending}</p>
                </div>
                <div className="bg-green-50 dark:bg-green-900/20 rounded-lg p-4">
                    <div className="flex items-center space-x-2">
                        <CheckCircle className="w-5 h-5 text-green-600 dark:text-green-400" />
                        <span className="text-sm font-medium text-green-700 dark:text-green-300">Approved</span>
                    </div>
                    <p className="text-2xl font-bold text-green-900 dark:text-green-100 mt-1">{stats.approved}</p>
                </div>
                <div className="bg-red-50 dark:bg-red-900/20 rounded-lg p-4">
                    <div className="flex items-center space-x-2">
                        <XCircle className="w-5 h-5 text-red-600 dark:text-red-400" />
                        <span className="text-sm font-medium text-red-700 dark:text-red-300">Rejected</span>
                    </div>
                    <p className="text-2xl font-bold text-red-900 dark:text-red-100 mt-1">{stats.rejected}</p>
                </div>
            </div>

            {/* Tabs + Search */}
            <div className="flex items-center justify-between">
                <div className="flex space-x-1 bg-gray-100 dark:bg-gray-800 rounded-lg p-1">
                    <button
                        onClick={() => setTab('pending')}
                        className={clsx(
                            "px-4 py-2 text-sm font-medium rounded-md transition-colors",
                            tab === 'pending'
                                ? "bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm"
                                : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
                        )}
                    >
                        Pending {stats.pending > 0 && <span className="ml-1 px-1.5 py-0.5 text-xs bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300 rounded-full">{stats.pending}</span>}
                    </button>
                    <button
                        onClick={() => setTab('history')}
                        className={clsx(
                            "px-4 py-2 text-sm font-medium rounded-md transition-colors",
                            tab === 'history'
                                ? "bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm"
                                : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
                        )}
                    >
                        History
                    </button>
                </div>
                <div className="relative">
                    <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 w-4 h-4" />
                    <input
                        type="text"
                        placeholder="Search files or uploaders..."
                        value={searchTerm}
                        onChange={(e) => setSearchTerm(e.target.value)}
                        className="pl-9 pr-4 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                    />
                </div>
            </div>

            {/* Table */}
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
                {isLoading ? (
                    <div className="flex items-center justify-center py-12">
                        <RefreshCw className="w-6 h-6 text-gray-400 animate-spin" />
                    </div>
                ) : items.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400">
                        <FileText className="w-12 h-12 mb-3 text-gray-300 dark:text-gray-600" />
                        <p className="text-sm">
                            {tab === 'pending' ? 'No files pending approval' : 'No approval history yet'}
                        </p>
                    </div>
                ) : (
                    <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                        <thead className="bg-gray-50 dark:bg-gray-900/50">
                            <tr>
                                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">File</th>
                                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden sm:table-cell">Uploader</th>
                                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden md:table-cell">Date</th>
                                {tab === 'history' && (
                                    <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider hidden lg:table-cell">Status</th>
                                )}
                                <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Actions</th>
                            </tr>
                        </thead>
                        <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                            {items.map((item) => (
                                <tr key={item.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/50">
                                    <td className="px-6 py-4 whitespace-nowrap">
                                        <div className="flex items-center">
                                            <FileText className="w-5 h-5 text-gray-400 mr-3 flex-shrink-0" />
                                            <div>
                                                <p className="text-sm font-medium text-gray-900 dark:text-white truncate max-w-[200px]">{item.file_name}</p>
                                                <p className="text-xs text-gray-500 dark:text-gray-400">{formatFileSize(item.file_size)}</p>
                                            </div>
                                        </div>
                                    </td>
                                    <td className="px-6 py-4 whitespace-nowrap hidden sm:table-cell">
                                        <p className="text-sm text-gray-900 dark:text-white">{item.uploader_name || item.uploader_email}</p>
                                        {item.uploader_name && <p className="text-xs text-gray-500 dark:text-gray-400">{item.uploader_email}</p>}
                                    </td>
                                    <td className="px-6 py-4 whitespace-nowrap hidden md:table-cell">
                                        <p className="text-sm text-gray-500 dark:text-gray-400">
                                            {formatDate ? formatDate(item.created_at) : new Date(item.created_at).toLocaleDateString()}
                                        </p>
                                    </td>
                                    {tab === 'history' && (
                                        <td className="px-6 py-4 whitespace-nowrap hidden lg:table-cell">
                                            <span className={clsx(
                                                "px-2 py-1 text-xs font-medium rounded-full",
                                                item.status === 'approved' && "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
                                                item.status === 'rejected' && "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400"
                                            )}>
                                                {item.status === 'approved' ? 'Approved' : 'Rejected'}
                                            </span>
                                            {item.rejection_reason && (
                                                <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 max-w-[200px] truncate" title={item.rejection_reason}>
                                                    {item.rejection_reason}
                                                </p>
                                            )}
                                        </td>
                                    )}
                                    <td className="px-6 py-4 whitespace-nowrap text-right">
                                        {tab === 'pending' ? (
                                            <div className="flex items-center justify-end space-x-2">
                                                <button
                                                    onClick={() => handleApprove(item)}
                                                    className="px-3 py-1.5 text-xs font-medium text-white bg-green-600 hover:bg-green-700 rounded-lg transition-colors"
                                                >
                                                    Approve
                                                </button>
                                                <button
                                                    onClick={() => setRejectItem(item)}
                                                    className="px-3 py-1.5 text-xs font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors"
                                                >
                                                    Reject
                                                </button>
                                            </div>
                                        ) : (
                                            <span className="text-xs text-gray-500 dark:text-gray-400">
                                                {item.decider_email && `by ${item.decider_email}`}
                                            </span>
                                        )}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </div>

            {/* Reject Modal */}
            {rejectItem && (
                <RejectFileModal
                    isOpen={!!rejectItem}
                    onClose={() => setRejectItem(null)}
                    onReject={handleReject}
                    fileName={rejectItem.file_name}
                />
            )}
        </div>
    );
}
