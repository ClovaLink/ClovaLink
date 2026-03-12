import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { useAuth, useAuthFetch } from './AuthContext';

export interface Company {
    id: string;
    name: string;
    role: 'Owner' | 'Admin' | 'Member' | 'Viewer';
    status?: string;
    compliance_mode?: string;
    retention_policy_days?: number;
    data_export_enabled?: boolean;
    approval_workflow_enabled?: boolean;
    backup_enabled?: boolean;
}

interface TenantContextType {
    currentCompany: Company;
    companies: Company[];
    setCurrentCompany: (company: Company) => Promise<void>;
}

const TenantContext = createContext<TenantContextType | undefined>(undefined);

export function TenantProvider({ children }: { children: ReactNode }) {
    const [companies, setCompanies] = useState<Company[]>([]);
    const [currentCompany, setCurrentCompanyState] = useState<Company>({ id: '', name: 'Loading...', role: 'Viewer' });
    const { tenant, switchTenant, refreshUser } = useAuth();
    const authFetch = useAuthFetch();

    // Update currentCompany when tenant changes from AuthContext
    useEffect(() => {
        if (tenant) {
            setCurrentCompanyState({
                id: tenant.id,
                name: tenant.name,
                role: 'Viewer', // Default, actual role is in User object
                compliance_mode: tenant.compliance_mode,
                retention_policy_days: tenant.retention_policy_days,
                data_export_enabled: tenant.data_export_enabled,
                approval_workflow_enabled: tenant.approval_workflow_enabled,
                backup_enabled: tenant.backup_enabled,
            });
        }
    }, [tenant]);

    const handleSetCurrentCompany = async (company: Company) => {
        // Optimistic update
        setCurrentCompanyState(company);
        try {
            await switchTenant(company.id);
            // Force page reload to ensure all data refreshes with new tenant context
            window.location.reload();
        } catch (error) {
            console.error("Failed to switch company", error);
            alert("Failed to switch company. The company may be suspended or you may not have access.");
            // Revert on failure
            if (tenant) {
                setCurrentCompanyState({
                    id: tenant.id,
                    name: tenant.name,
                    role: 'Viewer',
                });
            }
        }
    };

    useEffect(() => {
        // Only fetch tenants if user is authenticated (tenant exists)
        if (!tenant) {
            return;
        }

        const fetchTenants = async () => {
            try {
                // Use /api/tenants/accessible - works for all users (not just SuperAdmin)
                // Returns user's primary tenant + any tenants from allowed_tenant_ids
                const response = await authFetch('/api/tenants/accessible');
                if (response.ok) {
                    const data = await response.json();
                    setCompanies(data);
                    // Current company is handled by AuthContext
                }
            } catch (error) {
                console.error("Failed to fetch tenants", error);
            }
        };
        fetchTenants();
    }, [authFetch, tenant]); // Re-fetch when tenant changes

    return (
        <TenantContext.Provider value={{ currentCompany, companies, setCurrentCompany: handleSetCurrentCompany }}>
            {children}
        </TenantContext.Provider>
    );
}

export function useTenant() {
    const context = useContext(TenantContext);
    if (context === undefined) {
        throw new Error('useTenant must be used within a TenantProvider');
    }
    return context;
}
