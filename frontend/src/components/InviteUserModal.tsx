import { useState, useEffect, useMemo } from 'react';
import { X, Lock, RefreshCw, Eye, EyeOff, User, Shield, Building2, Key, ChevronDown, Check, Globe } from 'lucide-react';
import clsx from 'clsx';
import { useAuthFetch, useAuth } from '../context/AuthContext';
import { usePasswordPolicy, validatePassword, PasswordPolicy } from './PasswordInput';

// Generate a secure random password
const generatePassword = (length = 12): string => {
    const lowercase = 'abcdefghijklmnopqrstuvwxyz';
    const uppercase = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
    const numbers = '0123456789';
    const symbols = '!@#$%^&*';
    const allChars = lowercase + uppercase + numbers + symbols;
    
    let password = '';
    password += lowercase[Math.floor(Math.random() * lowercase.length)];
    password += uppercase[Math.floor(Math.random() * uppercase.length)];
    password += numbers[Math.floor(Math.random() * numbers.length)];
    password += symbols[Math.floor(Math.random() * symbols.length)];
    
    for (let i = 4; i < length; i++) {
        password += allChars[Math.floor(Math.random() * allChars.length)];
    }
    
    return password.split('').sort(() => Math.random() - 0.5).join('');
};

// Accordion Section Component
interface AccordionSectionProps {
    title: string;
    icon: React.ReactNode;
    isOpen: boolean;
    onToggle: () => void;
    children: React.ReactNode;
    badge?: string | number;
    required?: boolean;
    completed?: boolean;
}

function AccordionSection({ title, icon, isOpen, onToggle, children, badge, required, completed }: AccordionSectionProps) {
    return (
        <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
            <button
                type="button"
                onClick={onToggle}
                className={clsx(
                    "w-full px-4 py-3 flex items-center justify-between text-left transition-colors",
                    isOpen 
                        ? "bg-gray-50 dark:bg-gray-700/50" 
                        : "bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-700/30"
                )}
            >
                <div className="flex items-center gap-3">
                    <div className={clsx(
                        "p-1.5 rounded-md",
                        completed 
                            ? "bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400"
                            : "bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
                    )}>
                        {completed ? <Check className="w-4 h-4" /> : icon}
                    </div>
                    <span className="font-medium text-gray-900 dark:text-white">{title}</span>
                    {required && !completed && (
                        <span className="text-xs text-red-500">Required</span>
                    )}
                    {badge !== undefined && badge !== 0 && (
                        <span className="px-2 py-0.5 text-xs font-medium bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 rounded-full">
                            {badge}
                        </span>
                    )}
                </div>
                <ChevronDown className={clsx(
                    "w-5 h-5 text-gray-400 transition-transform duration-200",
                    isOpen && "rotate-180"
                )} />
            </button>
            <div className={clsx(
                "transition-all duration-200 ease-in-out overflow-hidden",
                isOpen ? "max-h-[500px] opacity-100" : "max-h-0 opacity-0"
            )}>
                <div className="px-4 py-4 bg-white dark:bg-gray-800 border-t border-gray-100 dark:border-gray-700/50">
                    {children}
                </div>
            </div>
        </div>
    );
}

interface InviteUserModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (data: UserData) => Promise<void>;
    initialData?: UserData | null;
    targetTenantId?: string;
}

export interface UserData {
    name: string;
    email: string;
    role: string;
    password?: string;
    department_id?: string;
    allowed_tenant_ids?: string[];
    allowed_department_ids?: string[];
    confirm_password?: string;
    identity_provider?: string;
}

interface Role {
    id: string;
    name: string;
    base_role: string;
    is_system: boolean;
    tenant_id: string | null;
}

const systemBaseRoles = [
    { value: 'Employee', label: 'Employee', base_role: 'Employee' },
    { value: 'Manager', label: 'Manager', base_role: 'Manager' },
    { value: 'Admin', label: 'Admin', base_role: 'Admin' },
    { value: 'SuperAdmin', label: 'Super Admin', base_role: 'SuperAdmin' },
];

const roleHierarchy: Record<string, number> = {
    'SuperAdmin': 4,
    'Admin': 3,
    'Manager': 2,
    'Employee': 1,
};

export function InviteUserModal({ isOpen, onClose, onSubmit, initialData, targetTenantId }: InviteUserModalProps) {
    const [formData, setFormData] = useState<UserData>({
        name: '',
        email: '',
        role: 'Employee',
        password: '',
        department_id: '',
    });
    const [departments, setDepartments] = useState<any[]>([]);
    const [tenants, setTenants] = useState<any[]>([]);
    const [roles, setRoles] = useState<Role[]>([]);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [originalRole, setOriginalRole] = useState<string | null>(null);
    const [confirmPassword, setConfirmPassword] = useState('');
    const [departmentsByTenant, setDepartmentsByTenant] = useState<Record<string, any[]>>({});
    const [showTempPassword, setShowTempPassword] = useState(false);
    const [passwordErrors, setPasswordErrors] = useState<string[]>([]);
    const [hasSsoProviders, setHasSsoProviders] = useState(false);
    const [authMethod, setAuthMethod] = useState<'local' | 'oidc' | 'hybrid'>('local');
    
    // Fetch password policy
    const { policy: passwordPolicy } = usePasswordPolicy();
    
    // Accordion states
    const [openSections, setOpenSections] = useState<Set<string>>(new Set(['basic', 'role']));
    
    const { user, tenant } = useAuth();
    const authFetch = useAuthFetch();
    const isSuperAdmin = user?.role === 'SuperAdmin';
    
    const roleChanged = initialData && originalRole && formData.role !== originalRole;
    const currentTenantId = targetTenantId || tenant?.id;

    const toggleSection = (section: string) => {
        setOpenSections(prev => {
            const next = new Set(prev);
            if (next.has(section)) {
                next.delete(section);
            } else {
                next.add(section);
            }
            return next;
        });
    };

    const getBaseRoleLevel = (roleName: string): number => {
        if (roleHierarchy[roleName]) {
            return roleHierarchy[roleName];
        }
        const role = roles.find(r => r.name === roleName);
        if (role && roleHierarchy[role.base_role]) {
            return roleHierarchy[role.base_role];
        }
        return 1;
    };

    const getAvailableRoles = () => {
        const currentUserLevel = user?.role ? getBaseRoleLevel(user.role) : 0;
        
        const baseRoles = systemBaseRoles.filter(role => {
            const roleLevel = roleHierarchy[role.base_role] || 1;
            if (currentUserLevel === 4) return true;
            if (currentUserLevel === 3) return roleLevel < 4;
            if (currentUserLevel === 2) return roleLevel === 1;
            return false;
        });

        const customRoles = roles
            .filter(role => !role.is_system && role.tenant_id !== null)
            .filter(role => role.tenant_id === currentTenantId)
            .filter(role => {
                const roleLevel = roleHierarchy[role.base_role] || 1;
                if (currentUserLevel === 4) return true;
                if (currentUserLevel === 3) return roleLevel < 4;
                if (currentUserLevel === 2) return roleLevel === 1;
                return false;
            })
            .map(role => ({
                value: role.name,
                label: role.name,
                base_role: role.base_role,
            }));

        return [...baseRoles, ...customRoles];
    };

    const availableRoles = getAvailableRoles();

    useEffect(() => {
        if (isOpen) {
            setConfirmPassword('');
            setDepartmentsByTenant({});
            setShowTempPassword(false);
            
            // Reset accordion states
            if (initialData) {
                setOpenSections(new Set(['basic', 'role']));
            } else {
                setOpenSections(new Set(['basic', 'role', 'credentials']));
            }
            
            if (initialData) {
                setFormData({
                    ...initialData,
                    password: '',
                    allowed_tenant_ids: initialData.allowed_tenant_ids || [],
                    allowed_department_ids: initialData.allowed_department_ids || []
                });
                setOriginalRole(initialData.role);
            } else {
                setFormData({
                    name: '',
                    email: '',
                    role: 'Employee',
                    department_id: '',
                    password: '',
                    allowed_tenant_ids: [],
                    allowed_department_ids: []
                });
                setOriginalRole(null);
            }
            setAuthMethod('local');
            fetchDepartments();
            fetchRoles();
            fetchSsoProviders();
            if (isSuperAdmin) {
                fetchTenants();
            }
        }
    }, [isOpen, initialData, targetTenantId, isSuperAdmin]);

    useEffect(() => {
        if (isOpen && isSuperAdmin && formData.allowed_tenant_ids && formData.allowed_tenant_ids.length > 0) {
            fetchAllSelectedTenantDepartments();
        }
    }, [isOpen, formData.allowed_tenant_ids?.length]);

    const fetchTenants = async () => {
        try {
            const response = await authFetch('/api/tenants?limit=100');
            if (response.ok) {
                const data = await response.json();
                setTenants(data);
            }
        } catch (error) {
            console.error('Failed to fetch tenants', error);
        }
    };

    const fetchDepartments = async () => {
        try {
            const url = targetTenantId
                ? `/api/departments?tenant_id=${targetTenantId}`
                : '/api/departments';
            const response = await authFetch(url);
            if (response.ok) {
                const data = await response.json();
                
                if (user?.role === 'Manager') {
                    const managerDepts: string[] = [];
                    if (user.department_id) {
                        managerDepts.push(user.department_id);
                    }
                    if (user.allowed_department_ids) {
                        for (const d of user.allowed_department_ids) {
                            if (!managerDepts.includes(d)) {
                                managerDepts.push(d);
                            }
                        }
                    }
                    const filtered = data.filter((d: any) => managerDepts.includes(d.id));
                    setDepartments(filtered);
                    if (!initialData && user.department_id && filtered.length > 0) {
                        setFormData(prev => ({ ...prev, department_id: user.department_id }));
                    }
                } else {
                    setDepartments(data);
                }
            }
        } catch (error) {
            console.error('Failed to fetch departments', error);
        }
    };

    const fetchDepartmentsForTenant = async (tenantId: string) => {
        try {
            const response = await authFetch(`/api/departments?tenant_id=${tenantId}`);
            if (response.ok) {
                const data = await response.json();
                setDepartmentsByTenant(prev => ({
                    ...prev,
                    [tenantId]: data
                }));
            }
        } catch (error) {
            console.error(`Failed to fetch departments for tenant ${tenantId}`, error);
        }
    };

    const fetchAllSelectedTenantDepartments = async () => {
        if (formData.allowed_tenant_ids && formData.allowed_tenant_ids.length > 0) {
            for (const tenantId of formData.allowed_tenant_ids) {
                await fetchDepartmentsForTenant(tenantId);
            }
        }
    };

    const fetchRoles = async () => {
        try {
            const response = await authFetch('/api/roles?include_global=true');
            if (response.ok) {
                const data = await response.json();
                setRoles(data);
            }
        } catch (error) {
            console.error('Failed to fetch roles', error);
        }
    };

    const fetchSsoProviders = async () => {
        try {
            const response = await authFetch('/api/oidc/providers');
            if (response.ok) {
                const data = await response.json();
                setHasSsoProviders((data.providers || []).length > 0);
            }
        } catch {
            // SSO not configured — that's fine
        }
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);
        setPasswordErrors([]);
        
        if (roleChanged && !confirmPassword) {
            setError('Please enter your password to confirm the role change.');
            setOpenSections(prev => new Set([...prev, 'role']));
            return;
        }
        
        // Validate password against policy for new users (skip for SSO-only)
        if (!initialData && authMethod !== 'oidc' && formData.password) {
            if (passwordPolicy) {
                const errors = validatePassword(formData.password, passwordPolicy);
                if (errors.length > 0) {
                    setPasswordErrors(errors);
                    setError('Password does not meet requirements.');
                    setOpenSections(prev => new Set([...prev, 'credentials']));
                    return;
                }
            } else if (formData.password.length < 8) {
                setError('Password must be at least 8 characters.');
                setOpenSections(prev => new Set([...prev, 'credentials']));
                return;
            }
        }
        
        setIsSubmitting(true);

        try {
            const baseData = { ...formData, identity_provider: authMethod };
            if (authMethod === 'oidc') {
                delete baseData.password;
            }
            const submitData = roleChanged
                ? { ...baseData, confirm_password: confirmPassword }
                : baseData;
            await onSubmit(submitData);
            setFormData({
                name: '',
                email: '',
                role: 'Employee',
                password: '',
                department_id: '',
            });
            setConfirmPassword('');
            setOriginalRole(null);
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to invite user');
        } finally {
            setIsSubmitting(false);
        }
    };

    if (!isOpen) return null;

    const roleInList = availableRoles.some(r => r.value === formData.role);
    const displayRoles = roleInList ? availableRoles : [
        ...availableRoles,
        { value: formData.role, label: formData.role, base_role: formData.role }
    ];

    // Calculate completion states
    const basicComplete = formData.name.length > 0 && formData.email.length > 0;
    const roleComplete = formData.role.length > 0;
    const credentialsComplete = initialData || authMethod === 'oidc' || (formData.password && formData.password.length >= 8);
    
    // Count additional access items
    const accessCount = (formData.allowed_tenant_ids?.length || 0) + (formData.allowed_department_ids?.length || 0);

    return (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow-2xl max-w-lg w-full max-h-[90vh] overflow-hidden flex flex-col">
                {/* Header */}
                <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
                    <div>
                        <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
                            {initialData ? 'Edit User' : 'Invite New User'}
                        </h2>
                        <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                            {initialData ? 'Update user information and access' : 'Add a new team member to your organization'}
                        </p>
                    </div>
                    <button
                        onClick={onClose}
                        className="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                    >
                        <X className="w-5 h-5 text-gray-500" />
                    </button>
                </div>

                {/* Scrollable Content */}
                <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto">
                    <div className="p-6 space-y-3">
                        {error && (
                            <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-sm text-red-600 dark:text-red-400">
                                {error}
                            </div>
                        )}

                        {/* Basic Information */}
                        <AccordionSection
                            title="Basic Information"
                            icon={<User className="w-4 h-4" />}
                            isOpen={openSections.has('basic')}
                            onToggle={() => toggleSection('basic')}
                            required
                            completed={basicComplete}
                        >
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                        Full Name
                                    </label>
                                    <input
                                        type="text"
                                        required
                                        value={formData.name}
                                        onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                                        placeholder="John Doe"
                                        className="w-full px-3 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400"
                                    />
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                        Email Address
                                        {initialData && <span className="text-xs text-gray-400 font-normal ml-2">(Cannot be changed)</span>}
                                    </label>
                                    <input
                                        type="email"
                                        required
                                        disabled={!!initialData}
                                        value={formData.email}
                                        onChange={(e) => setFormData({ ...formData, email: e.target.value })}
                                        placeholder="john@company.com"
                                        className="w-full px-3 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400 disabled:bg-gray-100 dark:disabled:bg-gray-900 disabled:text-gray-500"
                                    />
                                </div>
                            </div>
                        </AccordionSection>

                        {/* Role & Department */}
                        <AccordionSection
                            title="Role & Department"
                            icon={<Shield className="w-4 h-4" />}
                            isOpen={openSections.has('role')}
                            onToggle={() => toggleSection('role')}
                            required
                            completed={roleComplete}
                        >
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                        Role
                                    </label>
                                    <select
                                        required
                                        value={formData.role}
                                        onChange={(e) => setFormData({ ...formData, role: e.target.value })}
                                        className="w-full px-3 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                    >
                                        {displayRoles.map((role) => (
                                            <option key={role.value} value={role.value}>
                                                {role.label}
                                            </option>
                                        ))}
                                    </select>
                                </div>

                                {roleChanged && (
                                    <div className="p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg">
                                        <div className="flex items-center gap-2 mb-2">
                                            <Lock className="w-4 h-4 text-amber-600 dark:text-amber-400" />
                                            <span className="text-sm font-medium text-amber-800 dark:text-amber-200">
                                                Confirm Role Change
                                            </span>
                                        </div>
                                        <p className="text-xs text-amber-700 dark:text-amber-300 mb-2">
                                            Changing from "{originalRole}" to "{formData.role}"
                                        </p>
                                        <input
                                            type="password"
                                            required={roleChanged}
                                            value={confirmPassword}
                                            onChange={(e) => setConfirmPassword(e.target.value)}
                                            placeholder="Enter your password"
                                            className="w-full px-3 py-2 border border-amber-300 dark:border-amber-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-amber-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400"
                                        />
                                    </div>
                                )}

                                <div>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                        Primary Department
                                        <span className="text-xs text-gray-400 font-normal ml-2">(Optional)</span>
                                    </label>
                                    <select
                                        value={formData.department_id}
                                        onChange={(e) => setFormData({ ...formData, department_id: e.target.value })}
                                        className="w-full px-3 py-2.5 border border-gray-300 dark:border-gray-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                    >
                                        <option value="">No department</option>
                                        {departments.map((dept) => (
                                            <option key={dept.id} value={dept.id}>
                                                {dept.name}
                                            </option>
                                        ))}
                                    </select>
                                </div>
                            </div>
                        </AccordionSection>

                        {/* Extended Access (only show if there's something to configure) */}
                        {(departments.length > 1 || (isSuperAdmin && tenants.length > 1)) && (
                            <AccordionSection
                                title="Extended Access"
                                icon={<Building2 className="w-4 h-4" />}
                                isOpen={openSections.has('access')}
                                onToggle={() => toggleSection('access')}
                                badge={accessCount > 0 ? accessCount : undefined}
                            >
                                <div className="space-y-4">
                                    {/* Additional Departments */}
                                    {departments.filter(d => d.id !== formData.department_id).length > 0 && (
                                        <div>
                                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                Additional Departments
                                            </label>
                                            <div className="space-y-2 max-h-32 overflow-y-auto p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                                                {departments.filter(d => d.id !== formData.department_id).map((dept) => (
                                                    <label key={dept.id} className="flex items-center gap-2 cursor-pointer">
                                                        <input
                                                            type="checkbox"
                                                            checked={formData.allowed_department_ids?.includes(dept.id)}
                                                            onChange={(e) => {
                                                                const current = formData.allowed_department_ids || [];
                                                                if (e.target.checked) {
                                                                    setFormData({ ...formData, allowed_department_ids: [...current, dept.id] });
                                                                } else {
                                                                    setFormData({ ...formData, allowed_department_ids: current.filter(id => id !== dept.id) });
                                                                }
                                                            }}
                                                            className="w-4 h-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                                        />
                                                        <span className="text-sm text-gray-700 dark:text-gray-300">{dept.name}</span>
                                                    </label>
                                                ))}
                                            </div>
                                        </div>
                                    )}

                                    {/* Additional Companies (SuperAdmin only) */}
                                    {isSuperAdmin && tenants.filter(t => t.id !== currentTenantId).length > 0 && (
                                        <div>
                                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                Additional Company Access
                                            </label>
                                            <div className="space-y-2 max-h-32 overflow-y-auto p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                                                {tenants.filter(t => t.id !== currentTenantId).map((t) => (
                                                    <label key={t.id} className="flex items-center gap-2 cursor-pointer">
                                                        <input
                                                            type="checkbox"
                                                            checked={formData.allowed_tenant_ids?.includes(t.id)}
                                                            onChange={(e) => {
                                                                const current = formData.allowed_tenant_ids || [];
                                                                if (e.target.checked) {
                                                                    setFormData({ ...formData, allowed_tenant_ids: [...current, t.id] });
                                                                    fetchDepartmentsForTenant(t.id);
                                                                } else {
                                                                    setFormData({ ...formData, allowed_tenant_ids: current.filter(id => id !== t.id) });
                                                                    setDepartmentsByTenant(prev => {
                                                                        const updated = { ...prev };
                                                                        delete updated[t.id];
                                                                        return updated;
                                                                    });
                                                                }
                                                            }}
                                                            className="w-4 h-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                                        />
                                                        <span className="text-sm text-gray-700 dark:text-gray-300">{t.name}</span>
                                                    </label>
                                                ))}
                                            </div>
                                        </div>
                                    )}

                                    {/* Departments from selected companies */}
                                    {Object.entries(departmentsByTenant).map(([tenantId, depts]) => {
                                        const tenantInfo = tenants.find(t => t.id === tenantId);
                                        if (!tenantInfo || depts.length === 0) return null;
                                        return (
                                            <div key={tenantId}>
                                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                    Departments in {tenantInfo.name}
                                                </label>
                                                <div className="space-y-2 max-h-32 overflow-y-auto p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                                                    {depts.map((dept: any) => (
                                                        <label key={dept.id} className="flex items-center gap-2 cursor-pointer">
                                                            <input
                                                                type="checkbox"
                                                                checked={formData.allowed_department_ids?.includes(dept.id)}
                                                                onChange={(e) => {
                                                                    const current = formData.allowed_department_ids || [];
                                                                    if (e.target.checked) {
                                                                        setFormData({ ...formData, allowed_department_ids: [...current, dept.id] });
                                                                    } else {
                                                                        setFormData({ ...formData, allowed_department_ids: current.filter(id => id !== dept.id) });
                                                                    }
                                                                }}
                                                                className="w-4 h-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                                            />
                                                            <span className="text-sm text-gray-700 dark:text-gray-300">{dept.name}</span>
                                                        </label>
                                                    ))}
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            </AccordionSection>
                        )}

                        {/* Credentials (new users only) */}
                        {!initialData && (
                            <AccordionSection
                                title="Credentials"
                                icon={<Key className="w-4 h-4" />}
                                isOpen={openSections.has('credentials')}
                                onToggle={() => toggleSection('credentials')}
                                required
                                completed={credentialsComplete as boolean}
                            >
                                <div className="space-y-3">
                                    {/* Auth Method Selector (only show if SSO providers exist) */}
                                    {hasSsoProviders && (
                                        <div>
                                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                                Authentication Method
                                            </label>
                                            <div className="grid grid-cols-3 gap-2">
                                                {(['local', 'oidc', 'hybrid'] as const).map((method) => (
                                                    <button
                                                        key={method}
                                                        type="button"
                                                        onClick={() => setAuthMethod(method)}
                                                        className={clsx(
                                                            'px-3 py-2 text-xs font-medium rounded-lg border transition-colors',
                                                            authMethod === method
                                                                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                                                                : 'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-700'
                                                        )}
                                                    >
                                                        {method === 'local' && <><Key className="w-3 h-3 inline mr-1" />Password</>}
                                                        {method === 'oidc' && <><Globe className="w-3 h-3 inline mr-1" />SSO Only</>}
                                                        {method === 'hybrid' && <><Shield className="w-3 h-3 inline mr-1" />Both</>}
                                                    </button>
                                                ))}
                                            </div>
                                            {authMethod === 'oidc' && (
                                                <p className="mt-1.5 text-xs text-gray-500 dark:text-gray-400">
                                                    User will sign in via SSO only. No password needed.
                                                </p>
                                            )}
                                        </div>
                                    )}

                                    {authMethod !== 'oidc' && (
                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                            Temporary Password
                                        </label>
                                        <div className="flex gap-2">
                                            <div className="relative flex-1">
                                                <input
                                                    type={showTempPassword ? 'text' : 'password'}
                                                    required
                                                    value={formData.password}
                                                    onChange={(e) => {
                                                        setFormData({ ...formData, password: e.target.value });
                                                        setPasswordErrors([]);
                                                    }}
                                                    placeholder={`Min. ${passwordPolicy?.min_length || 8} characters`}
                                                    className={clsx(
                                                        "w-full px-3 py-2.5 pr-10 border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-400 font-mono",
                                                        passwordErrors.length > 0
                                                            ? "border-red-500"
                                                            : "border-gray-300 dark:border-gray-600"
                                                    )}
                                                />
                                                <button
                                                    type="button"
                                                    onClick={() => setShowTempPassword(!showTempPassword)}
                                                    className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                                                >
                                                    {showTempPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                                </button>
                                            </div>
                                            <button
                                                type="button"
                                                onClick={() => {
                                                    const length = Math.max(passwordPolicy?.min_length || 8, 12);
                                                    const newPassword = generatePassword(length);
                                                    setFormData({ ...formData, password: newPassword });
                                                    setShowTempPassword(true);
                                                    setPasswordErrors([]);
                                                }}
                                                className="px-4 py-2.5 text-sm font-medium text-white bg-primary-600 hover:bg-primary-700 rounded-lg flex items-center gap-2 transition-colors"
                                            >
                                                <RefreshCw className="w-4 h-4" />
                                                Generate
                                            </button>
                                        </div>
                                        
                                        {/* Password errors */}
                                        {passwordErrors.length > 0 && (
                                            <div className="mt-2 p-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                                                <ul className="text-xs text-red-600 dark:text-red-400 space-y-1">
                                                    {passwordErrors.map((err, i) => (
                                                        <li key={i}>• {err}</li>
                                                    ))}
                                                </ul>
                                            </div>
                                        )}
                                        
                                        {/* Password requirements */}
                                        {passwordPolicy && formData.password && (
                                            <div className="mt-2 p-2 bg-gray-50 dark:bg-gray-800/50 rounded-lg border border-gray-200 dark:border-gray-700">
                                                <p className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Requirements:</p>
                                                <ul className="text-xs space-y-0.5">
                                                    <li className={formData.password.length >= passwordPolicy.min_length ? "text-green-600 dark:text-green-400" : "text-gray-400"}>
                                                        {formData.password.length >= passwordPolicy.min_length ? "✓" : "○"} {passwordPolicy.min_length}+ characters
                                                    </li>
                                                    {passwordPolicy.require_uppercase && (
                                                        <li className={/[A-Z]/.test(formData.password) ? "text-green-600 dark:text-green-400" : "text-gray-400"}>
                                                            {/[A-Z]/.test(formData.password) ? "✓" : "○"} Uppercase letter
                                                        </li>
                                                    )}
                                                    {passwordPolicy.require_lowercase && (
                                                        <li className={/[a-z]/.test(formData.password) ? "text-green-600 dark:text-green-400" : "text-gray-400"}>
                                                            {/[a-z]/.test(formData.password) ? "✓" : "○"} Lowercase letter
                                                        </li>
                                                    )}
                                                    {passwordPolicy.require_number && (
                                                        <li className={/[0-9]/.test(formData.password) ? "text-green-600 dark:text-green-400" : "text-gray-400"}>
                                                            {/[0-9]/.test(formData.password) ? "✓" : "○"} Number
                                                        </li>
                                                    )}
                                                    {passwordPolicy.require_special && (
                                                        <li className={/[!@#$%^&*()_+\-=\[\]{};':"\\|,.<>\/?]/.test(formData.password) ? "text-green-600 dark:text-green-400" : "text-gray-400"}>
                                                            {/[!@#$%^&*()_+\-=\[\]{};':"\\|,.<>\/?]/.test(formData.password) ? "✓" : "○"} Special character
                                                        </li>
                                                    )}
                                                </ul>
                                            </div>
                                        )}
                                        
                                        <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                                            User will be prompted to change this on first login
                                        </p>
                                    </div>
                                    )}
                                </div>
                            </AccordionSection>
                        )}
                    </div>
                </form>

                {/* Footer */}
                <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3 flex-shrink-0 bg-gray-50 dark:bg-gray-800/50">
                    <button
                        type="button"
                        onClick={onClose}
                        className="px-4 py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleSubmit}
                        disabled={isSubmitting}
                        className="px-5 py-2.5 text-sm font-medium text-white bg-primary-600 rounded-lg hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-2"
                    >
                        {isSubmitting ? (
                            <>
                                <RefreshCw className="w-4 h-4 animate-spin" />
                                {initialData ? 'Saving...' : 'Inviting...'}
                            </>
                        ) : (
                            initialData ? 'Save Changes' : 'Invite User'
                        )}
                    </button>
                </div>
            </div>
        </div>
    );
}
