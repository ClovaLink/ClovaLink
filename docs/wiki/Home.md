# ClovaLink Documentation

Welcome to ClovaLink - an open-source, multi-tenant file management and compliance platform built with Rust and React.

## Overview

ClovaLink provides secure file storage, sharing, and compliance features for organizations of all sizes. It supports multiple compliance frameworks (HIPAA, SOX, GDPR) and offers a flexible extension system for customization.

## Key Features

### File Management
- **Secure Upload/Download** - End-to-end encrypted file transfers
- **File Requests** - Create secure upload portals for external users
- **Version Control** - Automatic file versioning for compliance
- **Folder Structure** - Hierarchical organization with department isolation
- **Deduplication** - Content-addressed storage reduces redundant data
- **S3 Replication** - Async backup/mirror to secondary bucket for DR

### Multi-Tenancy
- **Isolated Data** - Complete data separation between organizations
- **Custom Branding** - Per-tenant email templates and settings
- **Department-based Access** - Fine-grained access control within tenants
- **Storage Quotas** - Configurable limits per organization

### Security & Compliance
- **Virus Scanning** - ClamAV integration scans all uploads for malware
- **Password Policies** - Configurable per-tenant requirements
- **IP Restrictions** - Allowlist/blocklist IP access controls
- **Session Fingerprinting** - Detect and prevent token theft
- **MFA Support** - TOTP-based two-factor authentication
- **Audit Logging** - Complete activity trails for compliance
- **GDPR Tools** - Data export and deletion request handling

### User Management
- **Role-Based Access Control (RBAC)** - Four base roles with customization
- **Custom Roles** - Create organization-specific permission sets
- **User Suspension** - Temporary access restrictions with reason tracking
- **Session Management** - View and revoke active sessions

### Notifications
- **Email Notifications** - Customizable email templates
- **In-App Notifications** - Real-time activity alerts
- **Per-User Preferences** - Users control their notification settings

### Extensions
- **UI Extensions** - Add custom interface components
- **File Processors** - Automate file handling workflows
- **Webhooks** - Integrate with external systems
- **Automation Jobs** - Scheduled background tasks

## Quick Start

### Prerequisites
- Docker and Docker Compose (or Podman)
- 4GB RAM minimum
- PostgreSQL 16+ (included in Docker setup)
- Redis 7+ (included in Docker setup)

### 1. Clone the Repository
```bash
git clone https://github.com/your-org/clovalink.git
cd clovalink
```

### 2. Configure Environment
```bash
cd infra
cp .env.example .env
# Edit .env with your settings (database, S3, JWT secret, etc.)
```

### 3. Start Services
```bash
docker compose up -d
```

### 4. Access the Application
- **Frontend**: http://localhost:8080
- **API**: http://localhost:3000
- **Health Check**: http://localhost:8080/health

### 5. Default Login
```
Email: superadmin@clovalink.com
Password: password123
```

> **Important**: Change the default password immediately in production!

## Documentation Index

| Section | Description |
|---------|-------------|
| [API Reference](API-Reference) | Complete REST API documentation |
| [Architecture](Architecture) | System design and data flows |
| [Deployment Guide](Deployment-Guide) | Production deployment instructions |
| [Extensions SDK](Extensions-SDK) | Build custom extensions |
| [Admin Guide](Admin-Guide) | Tenant and user management |
| [Security](Security) | Security features and configuration |
| [Virus Scanning](Virus-Scanning) | ClamAV integration and malware protection |
| [Discord Integration](Discord-Integration) | Discord DM notifications setup |

## Tech Stack

### Backend
- **Language**: Rust 1.75+
- **Framework**: Axum (async web framework)
- **Database**: PostgreSQL 16 with SQLx
- **Cache**: Redis 7
- **Storage**: Local filesystem or S3-compatible (AWS, Backblaze B2, MinIO) with optional replication
- **Authentication**: JWT with session fingerprinting

### Frontend
- **Framework**: React 18 with TypeScript
- **Styling**: Tailwind CSS
- **State Management**: React Query (TanStack Query)
- **Build Tool**: Vite
- **UI Components**: Headless UI, Lucide icons

### Infrastructure
- **Containerization**: Docker with multi-stage builds
- **Web Server**: Nginx (frontend proxy)
- **Orchestration**: Docker Compose / Podman

## License

ClovaLink is open source software licensed under the MIT License.

## Support

- **GitHub Issues**: Report bugs and feature requests
- **Discussions**: Community support and questions
- **Wiki**: This documentation

---

*ClovaLink v1.0 - An open source project*

