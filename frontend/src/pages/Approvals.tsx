import { useState, useEffect } from 'react';
import {
    CheckCircle, XCircle, Clock, FileText, Search, RefreshCw,
    Filter, User, HardDrive, ChevronLeft, ChevronRight,
    CheckSquare, Square, MinusSquare, X, ShieldCheck
} from 'lucide-react';
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

const statusConfig = {
    pending: {
        color: 'text-amber-600 dark:text-amber-400',
        bg: 'bg-amber-100 dark:bg-amber-900/30',
        border: 'border-amber-200 dark:border-amber-800',
        icon: Clock,
        label: 'Pending',
    },
    approved: {
        color: 'text-green-600 dark:text-green-400',
        bg: 'bg-green-100 dark:bg-green-900/30',
        border: 'border-green-200 dark:border-green-800',
        icon: CheckCircle,
        label: 'Approved',
    },
    rejected: {
        color: 'text-red-600 dark:text-red-400',
        bg: 'bg-red-100 dark:bg-red-900/30',
        border: 'border-red-200 dark:border-red-800',
        icon: XCircle,
        label: 'Rejected',
    },
};

function formatFileSize(bytes: number): string {
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
}

const ITEMS_PER_PAGE = 15;

export function Approvals() {
    const [tab, setTab] = useState<'pending' | 'history'>('pending');
    const [pendingItems, setPendingItems] = useState<ApprovalItem[]>([]);
    const [historyItems, setHistoryItems] = useState<ApprovalItem[]>([]);
    const [stats, setStats] = useState<ApprovalStats>({ pending: 0, approved: 0, rejected: 0 });
    const [isLoading, setIsLoading] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');
    const [rejectItem, setRejectItem] = useState<ApprovalItem | null>(null);
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const [bulkLoading, setBulkLoading] = useState(false);
    const [selectedDetail, setSelectedDetail] = useState<ApprovalItem | null>(null);
    const [currentPage, setCurrentPage] = useState(1);
    const [filterStatus, setFilterStatus] = useState('');
    const [filterDate, setFilterDate] = useState('');
    const authFetch = useAuthFetch();
    const { currentCompany } = useTenant();
    const { formatDate, formatDateTime } = useGlobalSettings();
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

    // Reset page and selection when switching tabs
    useEffect(() => {
        setCurrentPage(1);
        setSelectedIds(new Set());
    }, [tab]);

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

    // Bulk actions
    const handleBulkApprove = async () => {
        if (selectedIds.size === 0 || !companyId) return;
        if (!confirm(`Approve ${selectedIds.size} file(s)?`)) return;
        setBulkLoading(true);
        try {
            const promises = Array.from(selectedIds).map(id =>
                authFetch(`/api/approvals/${companyId}/${id}/approve`, { method: 'POST' })
            );
            await Promise.all(promises);
            setSelectedIds(new Set());
            fetchData();
        } catch (error) {
            console.error('Bulk approve failed:', error);
        } finally {
            setBulkLoading(false);
        }
    };

    const handleBulkReject = async () => {
        if (selectedIds.size === 0 || !companyId) return;
        const reason = prompt(`Reject ${selectedIds.size} file(s)? Enter reason:`);
        if (!reason) return;
        setBulkLoading(true);
        try {
            const promises = Array.from(selectedIds).map(id =>
                authFetch(`/api/approvals/${companyId}/${id}/reject`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ reason }),
                })
            );
            await Promise.all(promises);
            setSelectedIds(new Set());
            fetchData();
        } catch (error) {
            console.error('Bulk reject failed:', error);
        } finally {
            setBulkLoading(false);
        }
    };

    const toggleSelect = (id: string, event: React.MouseEvent) => {
        event.stopPropagation();
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) {
                next.delete(id);
            } else {
                next.add(id);
            }
            return next;
        });
    };

    const toggleSelectAll = () => {
        if (selectedIds.size === paginatedItems.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(paginatedItems.map(a => a.id)));
        }
    };

    // Filtering
    const filterByDate = (item: ApprovalItem) => {
        if (!filterDate) return true;
        const created = new Date(item.created_at);
        const now = new Date();
        const days = parseInt(filterDate);
        const cutoff = new Date(now.getTime() - days * 24 * 60 * 60 * 1000);
        return created >= cutoff;
    };

    const filteredPending = pendingItems.filter(item =>
        (item.file_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.uploader_email.toLowerCase().includes(searchTerm.toLowerCase())) &&
        filterByDate(item)
    );

    const filteredHistory = historyItems.filter(item =>
        (item.file_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.uploader_email.toLowerCase().includes(searchTerm.toLowerCase())) &&
        filterByDate(item) &&
        (!filterStatus || item.status === filterStatus)
    );

    const items = tab === 'pending' ? filteredPending : filteredHistory;
    const totalPages = Math.ceil(items.length / ITEMS_PER_PAGE);
    const paginatedItems = items.slice((currentPage - 1) * ITEMS_PER_PAGE, currentPage * ITEMS_PER_PAGE);

    const hasActiveFilters = filterStatus || filterDate || searchTerm;

    // Pagination helpers
    const goToPage = (page: number) => {
        if (page >= 1 && page <= totalPages) {
            setCurrentPage(page);
            setSelectedIds(new Set());
        }
    };

    const getPageNumbers = () => {
        const pages: (number | string)[] = [];
        if (totalPages <= 7) {
            for (let i = 1; i <= totalPages; i++) pages.push(i);
        } else {
            if (currentPage <= 3) {
                pages.push(1, 2, 3, 4, '...', totalPages);
            } else if (currentPage >= totalPages - 2) {
                pages.push(1, '...', totalPages - 3, totalPages - 2, totalPages - 1, totalPages);
            } else {
                pages.push(1, '...', currentPage - 1, currentPage, currentPage + 1, '...', totalPages);
            }
        }
        return pages;
    };

    const getStatusInfo = (status: string) => {
        return statusConfig[status as keyof typeof statusConfig] || statusConfig.pending;
    };

    if (isLoading) {
        return (
            <div className="flex items-center justify-center min-h-[60vh]">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
            </div>
        );
    }

    return (
        <div className="space-y-4 sm:space-y-6">
            {/* Header */}
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 sm:gap-4">
                <div className="flex items-center gap-2 sm:gap-3">
                    <ShieldCheck className="w-6 h-6 sm:w-8 sm:h-8 text-primary-600" />
                    <div>
                        <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white">
                            Document Approvals
                        </h1>
                        <p className="text-xs sm:text-sm text-gray-500 dark:text-gray-400">
                            Review and approve uploaded documents
                        </p>
                    </div>
                </div>
                <button
                    onClick={fetchData}
                    disabled={isLoading}
                    className="flex items-center justify-center gap-2 px-3 sm:px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50 self-start sm:self-auto"
                >
                    <RefreshCw className={clsx('w-4 h-4', isLoading && 'animate-spin')} />
                    <span className="hidden sm:inline">Refresh</span>
                </button>
            </div>

            {/* Stats Cards */}
            <div className="grid grid-cols-3 gap-4">
                <div className={clsx('rounded-xl p-4 border', statusConfig.pending.bg, statusConfig.pending.border)}>
                    <div className="flex items-center justify-between">
                        <span className={clsx('text-sm', statusConfig.pending.color)}>Pending</span>
                        <Clock className={clsx('w-5 h-5', statusConfig.pending.color)} />
                    </div>
                    <div className={clsx('mt-2 text-2xl font-bold', statusConfig.pending.color)}>
                        {stats.pending}
                    </div>
                </div>
                <div className={clsx('rounded-xl p-4 border', statusConfig.approved.bg, statusConfig.approved.border)}>
                    <div className="flex items-center justify-between">
                        <span className={clsx('text-sm', statusConfig.approved.color)}>Approved</span>
                        <CheckCircle className={clsx('w-5 h-5', statusConfig.approved.color)} />
                    </div>
                    <div className={clsx('mt-2 text-2xl font-bold', statusConfig.approved.color)}>
                        {stats.approved}
                    </div>
                </div>
                <div className={clsx('rounded-xl p-4 border', statusConfig.rejected.bg, statusConfig.rejected.border)}>
                    <div className="flex items-center justify-between">
                        <span className={clsx('text-sm', statusConfig.rejected.color)}>Rejected</span>
                        <XCircle className={clsx('w-5 h-5', statusConfig.rejected.color)} />
                    </div>
                    <div className={clsx('mt-2 text-2xl font-bold', statusConfig.rejected.color)}>
                        {stats.rejected}
                    </div>
                </div>
            </div>

            {/* Filters */}
            <div className="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
                <div className="flex items-center gap-2 mb-3">
                    <Filter className="w-4 h-4 text-gray-400" />
                    <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Filters</span>
                </div>
                <div className="flex flex-wrap items-center gap-3">
                    {/* Tabs */}
                    <div className="flex space-x-1 bg-gray-100 dark:bg-gray-900/50 rounded-lg p-1">
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

                    {/* Status filter (history only) */}
                    {tab === 'history' && (
                        <select
                            value={filterStatus}
                            onChange={(e) => { setFilterStatus(e.target.value); setCurrentPage(1); }}
                            className="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                        >
                            <option value="">All Status</option>
                            <option value="approved">Approved</option>
                            <option value="rejected">Rejected</option>
                        </select>
                    )}

                    {/* Date filter */}
                    <select
                        value={filterDate}
                        onChange={(e) => { setFilterDate(e.target.value); setCurrentPage(1); }}
                        className="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white text-sm"
                    >
                        <option value="">All Time</option>
                        <option value="7">Last 7 days</option>
                        <option value="30">Last 30 days</option>
                        <option value="90">Last 90 days</option>
                    </select>

                    {/* Search */}
                    <div className="relative flex-1 min-w-[200px]">
                        <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 w-4 h-4" />
                        <input
                            type="text"
                            placeholder="Search files or uploaders..."
                            value={searchTerm}
                            onChange={(e) => { setSearchTerm(e.target.value); setCurrentPage(1); }}
                            className="w-full pl-9 pr-4 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                        />
                    </div>

                    {hasActiveFilters && (
                        <button
                            onClick={() => { setSearchTerm(''); setFilterStatus(''); setFilterDate(''); setCurrentPage(1); }}
                            className="flex items-center gap-1 px-3 py-2 text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                        >
                            <X className="w-3 h-3" /> Clear
                        </button>
                    )}
                </div>
            </div>

            {/* Bulk Actions Bar */}
            {tab === 'pending' && selectedIds.size > 0 && (
                <div className="bg-primary-50 dark:bg-primary-900/20 border border-primary-200 dark:border-primary-800 rounded-xl p-3 flex items-center justify-between">
                    <span className="text-sm font-medium text-primary-700 dark:text-primary-300">
                        {selectedIds.size} item{selectedIds.size !== 1 ? 's' : ''} selected
                    </span>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={handleBulkApprove}
                            disabled={bulkLoading}
                            className="px-3 py-1.5 text-sm font-medium text-white bg-green-600 hover:bg-green-700 rounded-lg transition-colors disabled:opacity-50"
                        >
                            Approve Selected
                        </button>
                        <button
                            onClick={handleBulkReject}
                            disabled={bulkLoading}
                            className="px-3 py-1.5 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors disabled:opacity-50"
                        >
                            Reject Selected
                        </button>
                        <button
                            onClick={() => setSelectedIds(new Set())}
                            className="p-1.5 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                        >
                            <X className="w-4 h-4" />
                        </button>
                    </div>
                </div>
            )}

            {/* Item List */}
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
                {/* Select All Header (pending tab) */}
                {tab === 'pending' && paginatedItems.length > 0 && (
                    <div className="flex items-center gap-3 px-4 sm:px-6 py-3 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-700">
                        <button onClick={toggleSelectAll} className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
                            {selectedIds.size === 0 ? (
                                <Square className="w-5 h-5" />
                            ) : selectedIds.size === paginatedItems.length ? (
                                <CheckSquare className="w-5 h-5 text-primary-600" />
                            ) : (
                                <MinusSquare className="w-5 h-5 text-primary-600" />
                            )}
                        </button>
                        <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                            Select All
                        </span>
                        <span className="text-xs text-gray-400 dark:text-gray-500 ml-auto">
                            Showing {((currentPage - 1) * ITEMS_PER_PAGE) + 1}–{Math.min(currentPage * ITEMS_PER_PAGE, items.length)} of {items.length}
                        </span>
                    </div>
                )}

                {/* History header */}
                {tab === 'history' && paginatedItems.length > 0 && (
                    <div className="flex items-center px-4 sm:px-6 py-3 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-700">
                        <span className="text-xs text-gray-400 dark:text-gray-500 ml-auto">
                            Showing {((currentPage - 1) * ITEMS_PER_PAGE) + 1}–{Math.min(currentPage * ITEMS_PER_PAGE, items.length)} of {items.length}
                        </span>
                    </div>
                )}

                {paginatedItems.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400">
                        <FileText className="w-12 h-12 mb-3 text-gray-300 dark:text-gray-600" />
                        <p className="text-sm">
                            {tab === 'pending' ? 'No files pending approval' : 'No approval history yet'}
                        </p>
                    </div>
                ) : (
                    <div className="divide-y divide-gray-200 dark:divide-gray-700">
                        {paginatedItems.map((item) => {
                            const info = getStatusInfo(item.status);
                            const StatusIcon = info.icon;
                            const isSelected = selectedIds.has(item.id);

                            return (
                                <div
                                    key={item.id}
                                    onClick={() => setSelectedDetail(item)}
                                    className={clsx(
                                        "flex items-start sm:items-center gap-3 sm:gap-4 px-4 sm:px-6 py-4 cursor-pointer transition-colors",
                                        isSelected
                                            ? "bg-primary-50 dark:bg-primary-900/10"
                                            : "hover:bg-gray-50 dark:hover:bg-gray-700/30"
                                    )}
                                >
                                    {/* Checkbox (pending only) */}
                                    {tab === 'pending' && (
                                        <button
                                            onClick={(e) => toggleSelect(item.id, e)}
                                            className="flex-shrink-0 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 mt-0.5"
                                        >
                                            {isSelected ? (
                                                <CheckSquare className="w-5 h-5 text-primary-600" />
                                            ) : (
                                                <Square className="w-5 h-5" />
                                            )}
                                        </button>
                                    )}

                                    {/* Status Icon */}
                                    <div className={clsx("flex-shrink-0 w-10 h-10 rounded-lg flex items-center justify-center", info.bg)}>
                                        <StatusIcon className={clsx("w-5 h-5", info.color)} />
                                    </div>

                                    {/* Content */}
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2">
                                            <p className="text-sm font-medium text-gray-900 dark:text-white truncate">
                                                {item.file_name}
                                            </p>
                                            {tab === 'history' && (
                                                <span className={clsx("px-2 py-0.5 text-xs font-medium rounded-full flex-shrink-0", info.bg, info.color)}>
                                                    {info.label}
                                                </span>
                                            )}
                                        </div>
                                        <p className="text-sm text-gray-500 dark:text-gray-400 truncate">
                                            {item.uploader_name || item.uploader_email}
                                        </p>

                                        {/* Metadata row */}
                                        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 mt-1">
                                            <span className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
                                                <HardDrive className="w-3 h-3" />
                                                {formatFileSize(item.file_size)}
                                            </span>
                                            <span className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
                                                <Clock className="w-3 h-3" />
                                                {formatDateTime ? formatDateTime(item.created_at) : new Date(item.created_at).toLocaleString()}
                                            </span>
                                            <span className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
                                                <User className="w-3 h-3" />
                                                {item.uploader_email}
                                            </span>
                                        </div>

                                        {/* Rejection reason */}
                                        {item.rejection_reason && (
                                            <p className="mt-1.5 text-xs text-red-500 dark:text-red-400 italic">
                                                Reason: {item.rejection_reason}
                                            </p>
                                        )}
                                    </div>

                                    {/* Actions */}
                                    <div className="flex-shrink-0 flex items-center gap-2">
                                        {tab === 'pending' ? (
                                            <>
                                                <button
                                                    onClick={(e) => { e.stopPropagation(); handleApprove(item); }}
                                                    className="px-3 py-1.5 text-xs font-medium text-white bg-green-600 hover:bg-green-700 rounded-lg transition-colors"
                                                >
                                                    Approve
                                                </button>
                                                <button
                                                    onClick={(e) => { e.stopPropagation(); setRejectItem(item); }}
                                                    className="px-3 py-1.5 text-xs font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors"
                                                >
                                                    Reject
                                                </button>
                                            </>
                                        ) : (
                                            <div className="text-right">
                                                {item.decider_email && (
                                                    <p className="text-xs text-gray-500 dark:text-gray-400">
                                                        by {item.decider_email}
                                                    </p>
                                                )}
                                                {item.decided_at && (
                                                    <p className="text-xs text-gray-400 dark:text-gray-500">
                                                        {formatDate ? formatDate(item.decided_at) : new Date(item.decided_at).toLocaleDateString()}
                                                    </p>
                                                )}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>

            {/* Pagination */}
            {totalPages > 1 && (
                <div className="flex items-center justify-between">
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                        Showing {((currentPage - 1) * ITEMS_PER_PAGE) + 1}–{Math.min(currentPage * ITEMS_PER_PAGE, items.length)} of {items.length}
                    </p>
                    <div className="flex items-center gap-1">
                        <button
                            onClick={() => goToPage(currentPage - 1)}
                            disabled={currentPage === 1}
                            className="p-2 rounded-lg text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-30"
                        >
                            <ChevronLeft className="w-4 h-4" />
                        </button>
                        {getPageNumbers().map((page, i) =>
                            typeof page === 'string' ? (
                                <span key={`ellipsis-${i}`} className="px-2 text-gray-400">…</span>
                            ) : (
                                <button
                                    key={page}
                                    onClick={() => goToPage(page)}
                                    className={clsx(
                                        "px-3 py-1.5 text-sm rounded-lg transition-colors",
                                        page === currentPage
                                            ? "bg-primary-600 text-white"
                                            : "text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                                    )}
                                >
                                    {page}
                                </button>
                            )
                        )}
                        <button
                            onClick={() => goToPage(currentPage + 1)}
                            disabled={currentPage === totalPages}
                            className="p-2 rounded-lg text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-30"
                        >
                            <ChevronRight className="w-4 h-4" />
                        </button>
                    </div>
                </div>
            )}

            {/* Detail Modal */}
            {selectedDetail && (
                <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50" onClick={() => setSelectedDetail(null)}>
                    <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden" onClick={(e) => e.stopPropagation()}>
                        {/* Modal Header */}
                        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                            <h3 className="text-lg font-semibold text-gray-900 dark:text-white">Approval Details</h3>
                            <button onClick={() => setSelectedDetail(null)} className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700">
                                <X className="w-5 h-5" />
                            </button>
                        </div>

                        {/* Modal Body */}
                        <div className="px-6 py-5 space-y-4">
                            {/* Status badge */}
                            {(() => {
                                const info = getStatusInfo(selectedDetail.status);
                                const StatusIcon = info.icon;
                                return (
                                    <div className={clsx("inline-flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium", info.bg, info.color)}>
                                        <StatusIcon className="w-4 h-4" />
                                        {info.label}
                                    </div>
                                );
                            })()}

                            {/* File info */}
                            <div className="space-y-3">
                                <div>
                                    <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">File</label>
                                    <div className="flex items-center gap-2 mt-1">
                                        <FileText className="w-5 h-5 text-gray-400" />
                                        <span className="text-sm font-medium text-gray-900 dark:text-white">{selectedDetail.file_name}</span>
                                    </div>
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 ml-7">
                                        {formatFileSize(selectedDetail.file_size)} • {selectedDetail.content_type || 'Unknown type'}
                                    </p>
                                </div>

                                <div>
                                    <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Uploaded by</label>
                                    <p className="text-sm text-gray-900 dark:text-white mt-1">{selectedDetail.uploader_name || selectedDetail.uploader_email}</p>
                                    {selectedDetail.uploader_name && <p className="text-xs text-gray-500 dark:text-gray-400">{selectedDetail.uploader_email}</p>}
                                </div>

                                <div>
                                    <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Submitted</label>
                                    <p className="text-sm text-gray-900 dark:text-white mt-1">
                                        {formatDateTime ? formatDateTime(selectedDetail.created_at) : new Date(selectedDetail.created_at).toLocaleString()}
                                    </p>
                                </div>

                                {selectedDetail.decided_at && (
                                    <div>
                                        <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Decided</label>
                                        <p className="text-sm text-gray-900 dark:text-white mt-1">
                                            {formatDateTime ? formatDateTime(selectedDetail.decided_at) : new Date(selectedDetail.decided_at).toLocaleString()}
                                            {selectedDetail.decider_email && (
                                                <span className="text-gray-500 dark:text-gray-400"> by {selectedDetail.decider_email}</span>
                                            )}
                                        </p>
                                    </div>
                                )}

                                {selectedDetail.rejection_reason && (
                                    <div>
                                        <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Rejection Reason</label>
                                        <p className="text-sm text-red-600 dark:text-red-400 mt-1 bg-red-50 dark:bg-red-900/20 rounded-lg p-3">
                                            {selectedDetail.rejection_reason}
                                        </p>
                                    </div>
                                )}
                            </div>
                        </div>

                        {/* Modal Footer (actions for pending) */}
                        {selectedDetail.status === 'pending' && (
                            <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50">
                                <button
                                    onClick={() => { handleApprove(selectedDetail); setSelectedDetail(null); }}
                                    className="px-4 py-2 text-sm font-medium text-white bg-green-600 hover:bg-green-700 rounded-lg transition-colors"
                                >
                                    Approve
                                </button>
                                <button
                                    onClick={() => { setRejectItem(selectedDetail); setSelectedDetail(null); }}
                                    className="px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors"
                                >
                                    Reject
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            )}

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
