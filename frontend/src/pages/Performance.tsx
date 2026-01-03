import { useState, useEffect, useCallback } from 'react';
import {
  Activity,
  Server,
  Database,
  Zap,
  Clock,
  AlertTriangle,
  TrendingUp,
  Users,
  Building2,
  ArrowUpRight,
  ArrowDownRight,
  RefreshCw,
  HardDrive,
  Wifi,
  CheckCircle,
  XCircle,
  BarChart3,
  Filter,
  Globe,
  HelpCircle,
  Shield,
  ShieldAlert,
  ShieldCheck,
} from 'lucide-react';
import { useAuthFetch } from '../context/AuthContext';
import clsx from 'clsx';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Line,
  ComposedChart,
} from 'recharts';

interface HealthCheck {
  name: string;
  status: string;
  latency_ms?: number;
  details?: Record<string, unknown>;
}

interface DetailedHealth {
  status: string;
  uptime_seconds: number;
  uptime_formatted: string;
  version: string;
  timestamp: number;
  checks: HealthCheck[];
  database: {
    connected: boolean;
    pool_size: number;
    pool_idle: number;
    pool_in_use: number;
    latency_ms: number;
  };
  redis: {
    connected: boolean;
    latency_ms?: number;
  };
  storage: {
    backend: string;
    connected: boolean;
    latency_ms?: number;
    bucket?: string;
    replication_enabled: boolean;
    replication_mode?: string;
    replication_bucket?: string;
  };
  memory: {
    rss_mb?: number;
    heap_mb?: number;
  };
  virus_scan: {
    enabled: boolean;
    connected: boolean;
    version?: string;
    latency_ms?: number;
  };
}

interface UsageSummary {
  total_requests: number;
  total_errors: number;
  error_rate: number;
  avg_response_time_ms: number;
  total_request_bytes: number;
  total_response_bytes: number;
  unique_users: number;
  unique_tenants: number;
  requests_per_minute: number;
  from: string;
  to: string;
}

interface TenantUsage {
  tenant_id: string | null;
  tenant_name: string | null;
  category: 'tenant' | 'unauthenticated' | 'unknown';
  request_count: number;
  error_count: number;
  avg_response_time_ms: number;
  total_bytes: number;
}

interface EndpointUsage {
  endpoint: string;
  method: string;
  request_count: number;
  error_count: number;
  avg_response_time_ms: number;
  p95_response_time_ms?: number;
}

interface SlowRequest {
  endpoint: string;
  method: string;
  avg_response_time_ms: number;
  max_response_time_ms: number;
  request_count: number;
  error_rate: number;
}

interface TimeSeriesPoint {
  time_bucket: string;
  request_count: number;
  error_count: number;
  avg_response_time_ms: number;
}

interface ErrorDetail {
  id: string;
  endpoint: string;
  method: string;
  status_code: number;
  error_message: string | null;
  tenant_id: string | null;
  tenant_name: string | null;
  user_id: string | null;
  user_email: string | null;
  ip_address: string | null;
  created_at: string;
  response_time_ms: number;
}

interface PaginatedErrors {
  errors: ErrorDetail[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

interface VirusScanMetrics {
  enabled: boolean;
  clamd_connected: boolean;
  clamd_version?: string;
  pending_jobs: number;
  scanning_jobs: number;
  failed_jobs: number;
  scans_last_hour: number;
  infections_last_hour: number;
  avg_scan_duration_ms?: number;
  total_bytes_scanned_last_hour: number;
}

type TimeRange = '1h' | '24h' | '7d' | '30d';

const timeRangeOptions: { value: TimeRange; label: string }[] = [
  { value: '1h', label: 'Last Hour' },
  { value: '24h', label: 'Last 24 Hours' },
  { value: '7d', label: 'Last 7 Days' },
  { value: '30d', label: 'Last 30 Days' },
];

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function formatNumber(num: number): string {
  if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M';
  if (num >= 1000) return (num / 1000).toFixed(1) + 'K';
  return num.toString();
}

export default function Performance() {
  const authFetch = useAuthFetch();
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [timeRange, setTimeRange] = useState<TimeRange>('24h');
  const [health, setHealth] = useState<DetailedHealth | null>(null);
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [tenantUsage, setTenantUsage] = useState<TenantUsage[]>([]);
  const [endpointUsage, setEndpointUsage] = useState<EndpointUsage[]>([]);
  const [slowRequests, setSlowRequests] = useState<SlowRequest[]>([]);
  const [timeseries, setTimeseries] = useState<TimeSeriesPoint[]>([]);
  const [recentErrors, setRecentErrors] = useState<ErrorDetail[]>([]);
  const [virusScanMetrics, setVirusScanMetrics] = useState<VirusScanMetrics | null>(null);
  const [errorsTotal, setErrorsTotal] = useState(0);
  const [errorsTotalPages, setErrorsTotalPages] = useState(0);
  const [errorsPage, setErrorsPage] = useState(1);
  const [errorsPerPage] = useState(20);
  const [errorsLoading, setErrorsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'overview' | 'endpoints' | 'tenants' | 'errors'>('overview');

  const getTimeRangeParams = useCallback(() => {
    const now = new Date();
    let from: Date;
    
    switch (timeRange) {
      case '1h':
        from = new Date(now.getTime() - 60 * 60 * 1000);
        break;
      case '24h':
        from = new Date(now.getTime() - 24 * 60 * 60 * 1000);
        break;
      case '7d':
        from = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
        break;
      case '30d':
        from = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);
        break;
    }
    
    return `from=${from.toISOString()}&to=${now.toISOString()}`;
  }, [timeRange]);

  const fetchErrors = useCallback(async (page: number = 1) => {
    setErrorsLoading(true);
    const params = getTimeRangeParams();
    
    try {
      const errorsRes = await authFetch(`/api/admin/usage/errors?${params}&page=${page}&per_page=${errorsPerPage}`);
      if (errorsRes.ok) {
        const data: PaginatedErrors = await errorsRes.json();
        setRecentErrors(data.errors);
        setErrorsTotal(data.total);
        setErrorsTotalPages(data.total_pages);
        setErrorsPage(data.page);
      }
    } catch (err) {
      console.error('Failed to fetch errors:', err);
    } finally {
      setErrorsLoading(false);
    }
  }, [authFetch, getTimeRangeParams, errorsPerPage]);

  const fetchData = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    
    const params = getTimeRangeParams();
    const granularity = timeRange === '1h' ? 'minute' : timeRange === '24h' ? 'hour' : 'day';
    
    try {
      const [healthRes, summaryRes, tenantsRes, endpointsRes, slowRes, timeseriesRes, virusScanRes] = await Promise.all([
        authFetch('/api/admin/health'),
        authFetch(`/api/admin/usage/summary?${params}`),
        authFetch(`/api/admin/usage/by-tenant?${params}`),
        authFetch(`/api/admin/usage/by-endpoint?${params}`),
        authFetch(`/api/admin/usage/slow-requests?${params}`),
        authFetch(`/api/admin/usage/timeseries?${params}&granularity=${granularity}`),
        authFetch('/api/admin/virus-scan/metrics'),
      ]);

      if (healthRes.ok) {
        setHealth(await healthRes.json());
      }
      
      if (virusScanRes.ok) {
        setVirusScanMetrics(await virusScanRes.json());
      }
      
      if (summaryRes.ok) {
        setSummary(await summaryRes.json());
      }
      
      if (tenantsRes.ok) {
        setTenantUsage(await tenantsRes.json());
      }
      
      if (endpointsRes.ok) {
        setEndpointUsage(await endpointsRes.json());
      }
      
      if (slowRes.ok) {
        setSlowRequests(await slowRes.json());
      }
      
      if (timeseriesRes.ok) {
        setTimeseries(await timeseriesRes.json());
      }
    } catch (err) {
      console.error('Failed to fetch performance data:', err);
      setError('Failed to load performance data');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [authFetch, getTimeRangeParams, timeRange]);

  useEffect(() => {
    fetchData();
    
    // Auto-refresh every 30 seconds
    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, [fetchData]);

  // Fetch errors when switching to errors tab or when page changes
  useEffect(() => {
    if (activeTab === 'errors') {
      fetchErrors(errorsPage);
    }
  }, [activeTab, errorsPage, fetchErrors]);

  // Reset page when time range changes
  useEffect(() => {
    setErrorsPage(1);
  }, [timeRange]);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="space-y-6 p-4 md:p-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
            <Activity className="w-7 h-7 text-primary-600" />
            Performance Dashboard
          </h1>
          <p className="text-gray-500 dark:text-gray-400 mt-1">
            System health and API usage monitoring
          </p>
        </div>
        
        <div className="flex items-center gap-3">
          <select
            value={timeRange}
            onChange={(e) => setTimeRange(e.target.value as TimeRange)}
            className="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white text-sm"
          >
            {timeRangeOptions.map((opt) => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
          
          <button
            onClick={fetchData}
            disabled={refreshing}
            className="p-2 rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
          >
            <RefreshCw className={clsx("w-5 h-5 text-gray-600 dark:text-gray-300", refreshing && "animate-spin")} />
          </button>
        </div>
      </div>

      {error && (
        <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-300">
          {error}
        </div>
      )}

      {/* System Health */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-4">
        {/* Database Status */}
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className={clsx(
                "p-2 rounded-lg",
                health?.database.connected ? "bg-green-100 dark:bg-green-900/30" : "bg-red-100 dark:bg-red-900/30"
              )}>
                <Database className={clsx(
                  "w-5 h-5",
                  health?.database.connected ? "text-green-600 dark:text-green-400" : "text-red-600 dark:text-red-400"
                )} />
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">Database</p>
                <p className="font-semibold text-gray-900 dark:text-white">
                  {health?.database.connected ? 'Connected' : 'Disconnected'}
                </p>
              </div>
            </div>
            {health?.database.connected ? (
              <CheckCircle className="w-5 h-5 text-green-500" />
            ) : (
              <XCircle className="w-5 h-5 text-red-500" />
            )}
          </div>
          {health?.database && (
            <div className="mt-3 text-xs text-gray-500 dark:text-gray-400 space-y-1">
              <div className="flex justify-between">
                <span>Pool Size:</span>
                <span className="font-medium">{health.database.pool_size}</span>
              </div>
              <div className="flex justify-between">
                <span>In Use:</span>
                <span className="font-medium">{health.database.pool_in_use}</span>
              </div>
              <div className="flex justify-between">
                <span>Latency:</span>
                <span className="font-medium">{health.database.latency_ms}ms</span>
              </div>
            </div>
          )}
        </div>

        {/* Redis Status */}
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className={clsx(
                "p-2 rounded-lg",
                health?.redis.connected ? "bg-green-100 dark:bg-green-900/30" : "bg-yellow-100 dark:bg-yellow-900/30"
              )}>
                <Zap className={clsx(
                  "w-5 h-5",
                  health?.redis.connected ? "text-green-600 dark:text-green-400" : "text-yellow-600 dark:text-yellow-400"
                )} />
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">Redis Cache</p>
                <p className="font-semibold text-gray-900 dark:text-white">
                  {health?.redis.connected ? 'Connected' : 'Disconnected'}
                </p>
              </div>
            </div>
            {health?.redis.connected ? (
              <CheckCircle className="w-5 h-5 text-green-500" />
            ) : (
              <AlertTriangle className="w-5 h-5 text-yellow-500" />
            )}
          </div>
          {health?.redis.latency_ms !== undefined && (
            <div className="mt-3 text-xs text-gray-500 dark:text-gray-400">
              <div className="flex justify-between">
                <span>Latency:</span>
                <span className="font-medium">{health.redis.latency_ms}ms</span>
              </div>
            </div>
          )}
        </div>

        {/* Storage Status */}
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-blue-100 dark:bg-blue-900/30">
                <HardDrive className="w-5 h-5 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">Storage</p>
                <p className="font-semibold text-gray-900 dark:text-white capitalize">
                  {health?.storage.connected ? 'Connected' : 'Disconnected'}
                </p>
              </div>
            </div>
            {health?.storage.connected ? (
              <CheckCircle className="w-5 h-5 text-green-500" />
            ) : (
              <XCircle className="w-5 h-5 text-red-500" />
            )}
          </div>
          <div className="mt-3 text-xs text-gray-500 dark:text-gray-400 space-y-1">
            <div className="flex justify-between">
              <span>Backend:</span>
              <span className="font-medium uppercase">{health?.storage.backend || 'Unknown'}</span>
            </div>
            {health?.storage.latency_ms !== undefined && (
              <div className="flex justify-between">
                <span>Latency:</span>
                <span className="font-medium">{health.storage.latency_ms}ms</span>
              </div>
            )}
            {health?.storage.bucket && (
              <div className="flex justify-between">
                <span>Bucket:</span>
                <span className="font-medium truncate max-w-[120px]">{health.storage.bucket}</span>
              </div>
            )}
            <div className="flex justify-between items-center">
              <span>Replication:</span>
              {health?.storage.replication_enabled ? (
                <span className="font-medium text-green-600 dark:text-green-400 capitalize">
                  {health.storage.replication_mode || 'Enabled'}
                </span>
              ) : (
                <span className="font-medium text-gray-400 dark:text-gray-500">Disabled</span>
              )}
            </div>
            {health?.storage.replication_enabled && health?.storage.replication_bucket && (
              <div className="flex justify-between">
                <span>Backup Bucket:</span>
                <span className="font-medium truncate max-w-[100px]">{health.storage.replication_bucket}</span>
              </div>
            )}
          </div>
        </div>

        {/* Uptime */}
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-purple-100 dark:bg-purple-900/30">
              <Server className="w-5 h-5 text-purple-600 dark:text-purple-400" />
            </div>
            <div>
              <p className="text-sm text-gray-500 dark:text-gray-400">Server Uptime</p>
              <p className="font-semibold text-gray-900 dark:text-white">
                {health?.uptime_formatted || '—'}
              </p>
            </div>
          </div>
          <div className="mt-3 text-xs text-gray-500 dark:text-gray-400">
            <div className="flex justify-between">
              <span>Version:</span>
              <span className="font-medium">v{health?.version || '—'}</span>
            </div>
            {health?.memory.rss_mb && (
              <div className="flex justify-between mt-1">
                <span>Memory:</span>
                <span className="font-medium">{health.memory.rss_mb.toFixed(1)} MB</span>
              </div>
            )}
          </div>
        </div>

        {/* Virus Scanning */}
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className={clsx(
                "p-2 rounded-lg",
                !virusScanMetrics?.enabled ? "bg-gray-100 dark:bg-gray-700" :
                virusScanMetrics?.clamd_connected ? "bg-green-100 dark:bg-green-900/30" : "bg-yellow-100 dark:bg-yellow-900/30"
              )}>
                {!virusScanMetrics?.enabled ? (
                  <Shield className="w-5 h-5 text-gray-400 dark:text-gray-500" />
                ) : virusScanMetrics?.infections_last_hour > 0 ? (
                  <ShieldAlert className="w-5 h-5 text-red-600 dark:text-red-400" />
                ) : (
                  <ShieldCheck className="w-5 h-5 text-green-600 dark:text-green-400" />
                )}
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">Virus Scanning</p>
                <p className="font-semibold text-gray-900 dark:text-white">
                  {!virusScanMetrics?.enabled ? 'Disabled' :
                   virusScanMetrics?.clamd_connected ? 'Active' : 'Disconnected'}
                </p>
              </div>
            </div>
            {virusScanMetrics?.enabled && (
              virusScanMetrics?.clamd_connected ? (
                <CheckCircle className="w-5 h-5 text-green-500" />
              ) : (
                <AlertTriangle className="w-5 h-5 text-yellow-500" />
              )
            )}
          </div>
          {virusScanMetrics?.enabled && (
            <div className="mt-3 text-xs text-gray-500 dark:text-gray-400 space-y-1">
              <div className="flex justify-between">
                <span>Scans (1h):</span>
                <span className="font-medium">{virusScanMetrics.scans_last_hour}</span>
              </div>
              {virusScanMetrics.infections_last_hour > 0 && (
                <div className="flex justify-between">
                  <span>Threats:</span>
                  <span className="font-medium text-red-600 dark:text-red-400">
                    {virusScanMetrics.infections_last_hour}
                  </span>
                </div>
              )}
              <div className="flex justify-between">
                <span>Pending:</span>
                <span className="font-medium">{virusScanMetrics.pending_jobs}</span>
              </div>
              {virusScanMetrics.avg_scan_duration_ms && (
                <div className="flex justify-between">
                  <span>Avg Time:</span>
                  <span className="font-medium">{virusScanMetrics.avg_scan_duration_ms.toFixed(0)}ms</span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Usage Stats Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 text-sm mb-1">
            <BarChart3 className="w-4 h-4" />
            Total Requests
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {formatNumber(summary?.total_requests || 0)}
          </p>
        </div>
        
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 text-sm mb-1">
            <AlertTriangle className="w-4 h-4" />
            Errors
          </div>
          <p className="text-2xl font-bold text-red-600 dark:text-red-400">
            {formatNumber(summary?.total_errors || 0)}
          </p>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            {(summary?.error_rate || 0).toFixed(2)}% error rate
          </p>
        </div>
        
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 text-sm mb-1">
            <Clock className="w-4 h-4" />
            Avg Response
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {(summary?.avg_response_time_ms || 0).toFixed(0)}ms
          </p>
        </div>
        
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 text-sm mb-1">
            <TrendingUp className="w-4 h-4" />
            Req/min
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {(summary?.requests_per_minute || 0).toFixed(1)}
          </p>
        </div>
        
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 text-sm mb-1">
            <Users className="w-4 h-4" />
            Active Users
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {summary?.unique_users || 0}
          </p>
        </div>
        
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 text-sm mb-1">
            <Wifi className="w-4 h-4" />
            Data Transfer
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {formatBytes((summary?.total_request_bytes || 0) + (summary?.total_response_bytes || 0))}
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-gray-200 dark:border-gray-700">
        <nav className="flex gap-4 overflow-x-auto">
          {[
            { id: 'overview', label: 'Overview', icon: BarChart3 },
            { id: 'endpoints', label: 'Endpoints', icon: Activity },
            { id: 'tenants', label: 'Tenants', icon: Building2 },
            { id: 'errors', label: 'Errors', icon: AlertTriangle, count: summary?.total_errors || 0 },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as typeof activeTab)}
              className={clsx(
                "flex items-center gap-2 px-4 py-3 border-b-2 text-sm font-medium transition-colors whitespace-nowrap",
                activeTab === tab.id
                  ? "border-primary-600 text-primary-600"
                  : "border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300"
              )}
            >
              <tab.icon className="w-4 h-4" />
              {tab.label}
              {'count' in tab && (tab as { count?: number }).count && (tab as { count?: number }).count! > 0 && (
                <span className="px-1.5 py-0.5 text-xs rounded-full bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400">
                  {(tab as { count?: number }).count}
                </span>
              )}
            </button>
          ))}
        </nav>
      </div>

      {/* Tab Content */}
      {activeTab === 'overview' && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Request Timeline */}
          <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="font-semibold text-gray-900 dark:text-white">Request Volume</h3>
              {timeseries.length > 0 && (
                <div className="flex items-center gap-4 text-xs">
                  <div className="flex items-center gap-1.5">
                    <div className="w-3 h-3 rounded-sm bg-primary-500" />
                    <span className="text-gray-500 dark:text-gray-400">Requests</span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <div className="w-3 h-3 rounded-sm bg-red-500" />
                    <span className="text-gray-500 dark:text-gray-400">Errors</span>
                  </div>
                </div>
              )}
            </div>
            <div className="h-48">
              {timeseries.length > 0 ? (
                <ResponsiveContainer width="100%" height="100%">
                  <ComposedChart
                    data={timeseries.slice(-24).map(point => ({
                      ...point,
                      time: new Date(point.time_bucket).toLocaleTimeString([], { 
                        hour: 'numeric', 
                        minute: '2-digit',
                        hour12: true 
                      }),
                    }))}
                    margin={{ top: 5, right: 5, left: -20, bottom: 5 }}
                  >
                    <defs>
                      <linearGradient id="requestGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.4}/>
                        <stop offset="95%" stopColor="#3b82f6" stopOpacity={0.05}/>
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.3} />
                    <XAxis 
                      dataKey="time" 
                      tick={{ fill: '#9ca3af', fontSize: 10 }}
                      axisLine={{ stroke: '#4b5563' }}
                      tickLine={{ stroke: '#4b5563' }}
                      interval="preserveStartEnd"
                    />
                    <YAxis 
                      tick={{ fill: '#9ca3af', fontSize: 10 }}
                      axisLine={{ stroke: '#4b5563' }}
                      tickLine={{ stroke: '#4b5563' }}
                      allowDecimals={false}
                    />
                    <Tooltip
                      contentStyle={{
                        backgroundColor: '#1f2937',
                        border: '1px solid #374151',
                        borderRadius: '8px',
                        fontSize: '12px',
                      }}
                      labelStyle={{ color: '#f3f4f6', fontWeight: 600, marginBottom: '4px' }}
                      itemStyle={{ color: '#d1d5db' }}
                      formatter={(value: number, name: string) => [
                        value,
                        name === 'request_count' ? 'Requests' : 'Errors'
                      ]}
                    />
                    <Area
                      type="monotone"
                      dataKey="request_count"
                      stroke="#3b82f6"
                      strokeWidth={2}
                      fill="url(#requestGradient)"
                      dot={false}
                      activeDot={{ r: 4, fill: '#3b82f6', stroke: '#fff', strokeWidth: 2 }}
                    />
                    <Line
                      type="monotone"
                      dataKey="error_count"
                      stroke="#ef4444"
                      strokeWidth={2}
                      dot={{ r: 3, fill: '#ef4444', stroke: '#fff', strokeWidth: 1 }}
                      activeDot={{ r: 5, fill: '#ef4444', stroke: '#fff', strokeWidth: 2 }}
                    />
                  </ComposedChart>
                </ResponsiveContainer>
              ) : (
                <div className="h-full flex items-center justify-center text-gray-500 dark:text-gray-400">
                  No data available
                </div>
              )}
            </div>
          </div>

          {/* Slow Requests */}
          <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
            <h3 className="font-semibold text-gray-900 dark:text-white mb-4">Slowest Endpoints</h3>
            <div className="space-y-3 max-h-48 overflow-y-auto">
              {slowRequests.length > 0 ? slowRequests.slice(0, 5).map((req, idx) => (
                <div key={idx} className="flex items-center justify-between text-sm">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className={clsx(
                      "px-1.5 py-0.5 rounded text-xs font-medium",
                      req.method === 'GET' ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400" :
                      req.method === 'POST' ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400" :
                      "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300"
                    )}>
                      {req.method}
                    </span>
                    <span className="truncate text-gray-700 dark:text-gray-300 font-mono text-xs">
                      {req.endpoint}
                    </span>
                  </div>
                  <span className={clsx(
                    "font-medium whitespace-nowrap ml-2",
                    req.avg_response_time_ms > 1000 ? "text-red-600 dark:text-red-400" :
                    req.avg_response_time_ms > 500 ? "text-yellow-600 dark:text-yellow-400" :
                    "text-gray-600 dark:text-gray-400"
                  )}>
                    {req.avg_response_time_ms.toFixed(0)}ms
                  </span>
                </div>
              )) : (
                <div className="text-gray-500 dark:text-gray-400 text-center py-4">
                  No slow requests detected
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {activeTab === 'endpoints' && (
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-gray-50 dark:bg-gray-700/50">
                <tr>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Endpoint</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Method</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Requests</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Errors</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Avg Time</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">P95 Time</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {endpointUsage.length > 0 ? endpointUsage.map((ep, idx) => (
                  <tr key={idx} className="hover:bg-gray-50 dark:hover:bg-gray-700/30">
                    <td className="px-4 py-3 font-mono text-sm text-gray-900 dark:text-white">{ep.endpoint}</td>
                    <td className="px-4 py-3">
                      <span className={clsx(
                        "px-2 py-0.5 rounded text-xs font-medium",
                        ep.method === 'GET' ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400" :
                        ep.method === 'POST' ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400" :
                        ep.method === 'PUT' ? "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400" :
                        ep.method === 'DELETE' ? "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400" :
                        "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300"
                      )}>
                        {ep.method}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-gray-900 dark:text-white">{formatNumber(ep.request_count)}</td>
                    <td className="px-4 py-3 text-right text-sm">
                      <span className={ep.error_count > 0 ? "text-red-600 dark:text-red-400" : "text-gray-500 dark:text-gray-400"}>
                        {ep.error_count}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-gray-900 dark:text-white">{ep.avg_response_time_ms.toFixed(0)}ms</td>
                    <td className="px-4 py-3 text-right text-sm text-gray-500 dark:text-gray-400">
                      {ep.p95_response_time_ms ? `${ep.p95_response_time_ms.toFixed(0)}ms` : '—'}
                    </td>
                  </tr>
                )) : (
                  <tr>
                    <td colSpan={6} className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
                      No endpoint data available
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {activeTab === 'tenants' && (
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-gray-50 dark:bg-gray-700/50">
                <tr>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Tenant</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Requests</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Errors</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Error Rate</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Avg Time</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Data Transfer</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {tenantUsage.length > 0 ? tenantUsage.map((tenant, idx) => {
                  // Determine icon and styling based on category
                  const IconComponent = tenant.category === 'tenant' 
                    ? Building2 
                    : tenant.category === 'unauthenticated' 
                      ? Globe 
                      : HelpCircle;
                  const iconColor = tenant.category === 'tenant'
                    ? 'text-primary-500'
                    : tenant.category === 'unauthenticated'
                      ? 'text-blue-500'
                      : 'text-gray-400';
                  const nameColor = tenant.category === 'tenant'
                    ? 'text-gray-900 dark:text-white'
                    : tenant.category === 'unauthenticated'
                      ? 'text-blue-600 dark:text-blue-400'
                      : 'text-gray-500 dark:text-gray-400 italic';
                  const displayName = tenant.tenant_name || (tenant.category === 'unauthenticated' ? 'Unauthenticated' : 'Unknown');
                  
                  return (
                    <tr key={idx} className="hover:bg-gray-50 dark:hover:bg-gray-700/30">
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <IconComponent className={`w-4 h-4 ${iconColor}`} />
                          <span className={`text-sm ${nameColor}`}>{displayName}</span>
                          {tenant.category !== 'tenant' && (
                            <span className={clsx(
                              "px-1.5 py-0.5 rounded text-xs",
                              tenant.category === 'unauthenticated' 
                                ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                                : "bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400"
                            )}>
                              {tenant.category === 'unauthenticated' ? 'Public' : 'Untracked'}
                            </span>
                          )}
                        </div>
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-gray-900 dark:text-white">{formatNumber(tenant.request_count)}</td>
                      <td className="px-4 py-3 text-right text-sm">
                        <span className={tenant.error_count > 0 ? "text-red-600 dark:text-red-400" : "text-gray-500 dark:text-gray-400"}>
                          {tenant.error_count}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right text-sm">
                        <span className={clsx(
                          tenant.request_count > 0 && (tenant.error_count / tenant.request_count) > 0.05
                            ? "text-red-600 dark:text-red-400"
                            : "text-gray-500 dark:text-gray-400"
                        )}>
                          {tenant.request_count > 0 
                            ? ((tenant.error_count / tenant.request_count) * 100).toFixed(2) 
                            : 0}%
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-gray-900 dark:text-white">
                        {tenant.avg_response_time_ms.toFixed(0)}ms
                      </td>
                      <td className="px-4 py-3 text-right text-sm text-gray-500 dark:text-gray-400">
                        {formatBytes(tenant.total_bytes)}
                      </td>
                    </tr>
                  );
                }) : (
                  <tr>
                    <td colSpan={6} className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
                      No tenant data available
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {activeTab === 'errors' && (
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden">
          <div className="p-4 border-b border-gray-200 dark:border-gray-700 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
            <div>
              <h3 className="font-semibold text-gray-900 dark:text-white">Recent Errors</h3>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                {errorsTotal > 0 
                  ? `Showing ${((errorsPage - 1) * errorsPerPage) + 1}-${Math.min(errorsPage * errorsPerPage, errorsTotal)} of ${errorsTotal} errors`
                  : 'No errors in the selected time range'}
              </p>
            </div>
            {errorsLoading && (
              <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-primary-600"></div>
            )}
          </div>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-gray-50 dark:bg-gray-700/50">
                <tr>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Time</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Endpoint</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Status</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Error</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">User/Tenant</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">IP</th>
                  <th className="px-4 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Time</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {recentErrors.length > 0 ? recentErrors.map((err) => (
                  <tr key={err.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/30">
                    <td className="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">
                      {new Date(err.created_at).toLocaleString(undefined, {
                        month: 'short',
                        day: 'numeric',
                        hour: '2-digit',
                        minute: '2-digit',
                        second: '2-digit'
                      })}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <span className={clsx(
                          "px-2 py-0.5 rounded text-xs font-medium",
                          err.method === 'GET' ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400" :
                          err.method === 'POST' ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400" :
                          err.method === 'PUT' ? "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400" :
                          err.method === 'DELETE' ? "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400" :
                          "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300"
                        )}>
                          {err.method}
                        </span>
                        <span className="font-mono text-xs text-gray-900 dark:text-white truncate max-w-[200px]">
                          {err.endpoint}
                        </span>
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span className={clsx(
                        "px-2 py-1 rounded text-xs font-medium",
                        err.status_code >= 500 ? "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400" :
                        err.status_code >= 400 ? "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400" :
                        "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300"
                      )}>
                        {err.status_code}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-sm text-gray-700 dark:text-gray-300 max-w-[200px] truncate">
                      {err.error_message || '—'}
                    </td>
                    <td className="px-4 py-3 text-sm">
                      <div className="text-gray-900 dark:text-white text-xs">
                        {err.user_email || 'Anonymous'}
                      </div>
                      <div className="text-gray-500 dark:text-gray-400 text-xs">
                        {err.tenant_name || 'Unknown tenant'}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 font-mono text-xs">
                      {err.ip_address || '—'}
                    </td>
                    <td className="px-4 py-3 text-right text-sm text-gray-500 dark:text-gray-400">
                      {err.response_time_ms}ms
                    </td>
                  </tr>
                )) : (
                  <tr>
                    <td colSpan={7} className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
                      <div className="flex flex-col items-center gap-2">
                        <CheckCircle className="w-8 h-8 text-green-500" />
                        <span>No errors in the selected time range</span>
                      </div>
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          
          {/* Pagination Controls */}
          {errorsTotalPages > 1 && (
            <div className="px-4 py-3 border-t border-gray-200 dark:border-gray-700 flex flex-col sm:flex-row items-center justify-between gap-3">
              <div className="text-sm text-gray-500 dark:text-gray-400">
                Page {errorsPage} of {errorsTotalPages}
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setErrorsPage(1)}
                  disabled={errorsPage === 1 || errorsLoading}
                  className="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  First
                </button>
                <button
                  onClick={() => setErrorsPage(p => Math.max(1, p - 1))}
                  disabled={errorsPage === 1 || errorsLoading}
                  className="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  Previous
                </button>
                
                {/* Page number buttons */}
                <div className="hidden sm:flex items-center gap-1">
                  {Array.from({ length: Math.min(5, errorsTotalPages) }, (_, i) => {
                    let pageNum: number;
                    if (errorsTotalPages <= 5) {
                      pageNum = i + 1;
                    } else if (errorsPage <= 3) {
                      pageNum = i + 1;
                    } else if (errorsPage >= errorsTotalPages - 2) {
                      pageNum = errorsTotalPages - 4 + i;
                    } else {
                      pageNum = errorsPage - 2 + i;
                    }
                    return (
                      <button
                        key={pageNum}
                        onClick={() => setErrorsPage(pageNum)}
                        disabled={errorsLoading}
                        className={clsx(
                          "w-8 h-8 text-sm font-medium rounded-lg transition-colors",
                          pageNum === errorsPage
                            ? "bg-primary-600 text-white"
                            : "border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
                        )}
                      >
                        {pageNum}
                      </button>
                    );
                  })}
                </div>
                
                <button
                  onClick={() => setErrorsPage(p => Math.min(errorsTotalPages, p + 1))}
                  disabled={errorsPage === errorsTotalPages || errorsLoading}
                  className="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  Next
                </button>
                <button
                  onClick={() => setErrorsPage(errorsTotalPages)}
                  disabled={errorsPage === errorsTotalPages || errorsLoading}
                  className="px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  Last
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

