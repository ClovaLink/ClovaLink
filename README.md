<a id="top"></a>
<p align="center">
  <img src="docs/assets/logo.svg" alt="ClovaLink" width="280">
</p>

<p align="center">
  <strong>Enterprise file management without the enterprise price tag.</strong>
</p>

<p align="center">
  Multi-tenant file management and compliance platform built with Rust and React.<br>
  Self-host it. Own your data. Stop paying $25/user/month.
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> •
  <a href="#features">Features</a> •
  <a href="#screenshots">Screenshots</a> •
  <a href="#contributing">Contributing</a> •
  <a href="https://clovalink.com"><b>Hosted Version</b></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/rust-1.75+-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/react-19-61dafb.svg" alt="React">
  <img src="https://img.shields.io/badge/postgresql-16-336791.svg" alt="PostgreSQL">
  <img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg" alt="PRs Welcome">
</p>

---

## Screenshots

<table>
<tr>
<td width="50%">
<img src="docs/assets/screenshots/dashboard.png" alt="Dashboard" width="100%">
<p align="center"><b>Dashboard</b> — HIPAA compliance monitoring, activity charts, file analytics</p>
</td>
<td width="50%">
<img src="docs/assets/screenshots/filemanager.png" alt="File Browser" width="100%">
<p align="center"><b>File Browser</b> — Folders, file groups, quick access, department filtering</p>
</td>
</tr>
<tr>
<td width="50%">
<img src="docs/assets/screenshots/companiesettings.png" alt="Company Settings" width="100%">
<p align="center"><b>Company Settings</b> — Compliance modes, storage quotas, retention policies</p>
</td>
<td width="50%">
<img src="docs/assets/screenshots/Security.png" alt="Security Alerts" width="100%">
<p align="center"><b>Security Alerts</b> — Real-time monitoring for suspicious activity</p>
</td>
</tr>
<tr>
<td width="50%">
<img src="docs/assets/screenshots/userpage.png" alt="User Management" width="100%">
<p align="center"><b>User Management</b> — Roles, departments, status tracking</p>
</td>
<td width="50%">
<img src="docs/assets/screenshots/performance.png" alt="Performance" width="100%">
<p align="center"><b>Performance</b> — System metrics and API monitoring</p>
</td>
</tr>
</table>

---

## The Problem

Enterprise file management has become absurdly expensive:

| Provider | Cost | Annual (50 users) |
|----------|------|-------------------|
| Box Business | $20/user/month | **$12,000** |
| Dropbox Business | $18/user/month | **$10,800** |
| SharePoint | $12.50/user/month | **$7,500** + Microsoft tax |
| Egnyte | $20/user/month | **$12,000** |
| **ClovaLink** | ~$20/month VPS | **~$240** total |

Most small businesses need 80% of enterprise features at 10% of the cost. **ClovaLink delivers exactly that.**

---

## Who Is This For?

| You Should Use ClovaLink If... | You Might Not Need ClovaLink If... |
|--------------------------------|-----------------------------------|
| You need HIPAA/SOX/GDPR compliance | You just need basic cloud storage |
| You manage multiple clients/tenants | You have 5 or fewer users |
| You want full control over your data | You need native Office 365 integration |
| You're an MSP serving multiple companies | Budget isn't a concern |
| You want to avoid per-user pricing | |

> **Want the features without the infrastructure?** [ClovaLink.com](https://clovalink.com) offers a fully managed hosted version for enterprise teams (10+ users).

---

## Features

### Core Capabilities

<table>
<tr>
<td width="33%" valign="top">

#### File Management
- Upload/download with progress
- Folder hierarchy (unlimited depth)
- File locking & versioning
- Soft delete with recovery
- Bulk operations
- Content-addressed storage (deduplication)
- **File Groups** (virtual collections)
- **Company Folders** (org-wide sharing)

</td>
<td width="33%" valign="top">

#### Security & Compliance
- HIPAA, SOX, GDPR modes
- Role-based access control
- Two-factor authentication (TOTP)
- Immutable audit logs
- Real-time security alerts
- Session management
- ClamAV virus scanning

</td>
<td width="33%" valign="top">

#### Multi-Tenancy
- Complete data isolation
- Per-tenant settings & branding
- Department-based organization
- Cross-tenant user access
- Tenant switching UI

</td>
</tr>
<tr>
<td width="33%" valign="top">

#### Sharing
- Expiring share links
- Password protection
- Download limits
- Public upload portals (file requests)
- Organization-wide or private

</td>
<td width="33%" valign="top">

#### Extensions
- UI extensions (iframe/ES modules)
- File processors (webhooks)
- Scheduled automations (cron)
- Sandboxed with permission grants

</td>
<td width="33%" valign="top">

#### AI Features
- Document summarization
- Question & Answer on docs
- Multi-provider support
- Self-hosted LLM support
- Usage limits & monitoring

</td>
</tr>
<tr>
<td width="33%" valign="top">

#### Storage Backends
- Local filesystem
- AWS S3
- Wasabi (80% cheaper than S3)
- Backblaze B2
- MinIO (self-hosted)
- S3 replication for DR

</td>
</tr>
</table>

### Compliance Modes

| Mode | Enforcements |
|------|--------------|
| **HIPAA** | Mandatory MFA, 15-min timeout, audit logging locked, public sharing blocked, 7-year retention |
| **SOX** | MFA required, file versioning mandatory, audit trails locked, 7-year retention |
| **GDPR** | Consent tracking, export logging, deletion request support, 2-year retention |
| **Standard** | No restrictions — full flexibility |

### Security Alerts Dashboard

Real-time monitoring for unusual activity patterns:

| Alert Type | Trigger | Severity |
|------------|---------|----------|
| Failed Login Spike | 5+ failed logins in 5 min | High |
| New IP Login | Login from unknown IP | Medium |
| Permission Escalation | Role upgraded to Admin+ | High |
| Bulk Download | 20+ files in 10 min | High |
| Blocked Extension | Prohibited file upload | Low |
| Account Lockout | Too many failed attempts | Critical |
| Malware Detected | Virus found in upload | Critical |

**Email notifications** automatically sent for Critical and High severity alerts.

---

## Quick Start

### Prerequisites

- Docker or Podman
- Git

### One-Command Setup

```bash
# Clone and start
git clone https://github.com/your-org/clovalink.git
cd clovalink/infra
docker compose up -d
```

<details>
<summary><b>Using Podman?</b></summary>

```bash
podman compose up -d
```

</details>

### Access Points

| Service | URL |
|---------|-----|
| Web Interface | http://localhost:8080 |
| API | http://localhost:3000 |
| PostgreSQL | localhost:5433 |
| Redis | localhost:6379 |

### Demo Credentials

| Role | Email | Password |
|------|-------|----------|
| SuperAdmin | superadmin@clovalink.com | password123 |
| Admin | admin@acme.com | password123 |
| Manager | manager@acme.com | password123 |
| Employee | employee@acme.com | password123 |

> **Change these credentials immediately in production!**

---

## Architecture

```mermaid
flowchart TB
    subgraph Clients[" "]
        direction LR
        Browser[Browser] ~~~ Mobile[Mobile] ~~~ API_Client[API Client]
    end

    subgraph Frontend[Frontend - Nginx]
        direction LR
        Static[React SPA] ~~~ Proxy[Reverse Proxy]
    end

    subgraph Backend[Backend - Rust/Axum]
        direction LR
        API[REST API] --> Auth[Auth] --> RateLimit[Rate Limiter]
    end

    subgraph Persistence[" "]
        direction LR
        subgraph Data[Data Layer]
            PG[(PostgreSQL)]
            Redis[(Redis)]
        end
        subgraph Storage[Storage Layer]
            S3[S3/Wasabi/B2]
            Local[Local FS]
        end
    end

    Clients --> Frontend
    Frontend --> Backend
    Backend --> Data
    Backend --> Storage
```

**Extensions**: UI extensions, file processors (webhooks), and scheduled jobs run sandboxed with permission grants.

### Why These Choices?

| Technology | Reason |
|------------|--------|
| **Rust** | Memory safety, zero-cost abstractions, single binary deployment |
| **Axum** | Async-first, tower middleware, type-safe extractors |
| **SQLx** | Compile-time SQL verification against actual schema |
| **PostgreSQL** | ACID, JSON columns, row-level security, rock solid |
| **Redis** | Sub-ms sessions, rate limiting, job queues |
| **React** | Component ecosystem, TypeScript support, mature tooling |

---

## Configuration

Create `backend/.env` from the example:

```bash
cp backend/.env.example backend/.env
```

### Required Settings

```env
DATABASE_URL=postgres://user:pass@localhost:5432/clovalink
REDIS_URL=redis://localhost:6379
JWT_SECRET=generate-a-64-char-random-string-here
```

### Storage Configuration

<details>
<summary><b>Local Storage</b> (Development)</summary>

```env
STORAGE_TYPE=local
UPLOAD_DIR=./uploads
```

</details>

<details>
<summary><b>AWS S3</b></summary>

```env
STORAGE_TYPE=s3
S3_BUCKET=your-bucket-name
AWS_ACCESS_KEY_ID=your-key
AWS_SECRET_ACCESS_KEY=your-secret
AWS_REGION=us-east-1
USE_PRESIGNED_URLS=true
```

</details>

<details>
<summary><b>Wasabi</b> (S3-compatible, 80% cheaper)</summary>

```env
STORAGE_TYPE=s3
S3_BUCKET=your-bucket-name
AWS_ACCESS_KEY_ID=your-key
AWS_SECRET_ACCESS_KEY=your-secret
AWS_REGION=us-east-1
S3_ENDPOINT=https://s3.wasabisys.com
USE_PRESIGNED_URLS=true
```

</details>

<details>
<summary><b>MinIO</b> (Self-hosted)</summary>

```env
STORAGE_TYPE=s3
S3_BUCKET=clovalink
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin
AWS_REGION=us-east-1
S3_ENDPOINT=http://localhost:9000
S3_PATH_STYLE=true
```

</details>

### S3 Replication (Optional)

For enterprise durability, ClovaLink supports asynchronous replication of uploaded files to a secondary S3 bucket. This provides disaster recovery and geographic redundancy.

**Two Modes:**

| Mode | Behavior |
|------|----------|
| **backup** | Uploads are replicated; deletions are not (keeps historical data) |
| **mirror** | Both uploads and deletions are synchronized |

<details>
<summary><b>Replication Configuration</b></summary>

```env
REPLICATION_ENABLED=true
REPLICATION_ENDPOINT=https://s3.us-west-2.amazonaws.com
REPLICATION_BUCKET=clovalink-backup
REPLICATION_REGION=us-west-2
REPLICATION_ACCESS_KEY=your-access-key
REPLICATION_SECRET_KEY=your-secret-key
REPLICATION_MODE=backup
REPLICATION_RETRY_SECONDS=60
REPLICATION_WORKERS=4
```

</details>

Replication is fully async and non-blocking — uploads complete immediately while replication jobs are queued in the background with automatic retries.

### Virus Scanning (Optional)

ClamAV integration scans all uploads for malware:

<details>
<summary><b>ClamAV Configuration</b></summary>

```env
CLAMAV_ENABLED=true
CLAMAV_HOST=clamav
CLAMAV_PORT=3310
```

</details>

> See [Virus Scanning Documentation](docs/wiki/Virus-Scanning.md) for quarantine, auto-suspend, and monitoring details.

> See [Deployment Guide](docs/wiki/Deployment-Guide.md) for detailed setup instructions.

---

## Project Structure

```
clovalink/
├── backend/
│   ├── crates/
│   │   ├── api/            # HTTP handlers, routes, middleware
│   │   ├── auth/           # JWT, passwords, 2FA
│   │   ├── core/           # Models, DB, mailer, cache, security
│   │   ├── extensions/     # Extension runtime
│   │   └── storage/        # S3/local storage abstraction
│   └── migrations/         # SQL schema files
├── frontend/
│   └── src/
│       ├── components/     # Reusable UI components
│       ├── context/        # React contexts (Auth, Tenant, Theme)
│       └── pages/          # Route components
└── infra/
    ├── compose.yml         # Docker Compose config
    └── Dockerfile.*        # Container builds
```

---

## Deployment

### Minimum Requirements

| Resource | Minimum | Handles |
|----------|---------|---------|
| CPU | 1 core | ~100 concurrent users |
| RAM | 1 GB | Basic operations |
| Storage | 10 GB | App + OS (files separate) |
| PostgreSQL | 14+ | Required |
| Redis | 6+ | Required |

### Recommended Production

| Resource | Recommended |
|----------|-------------|
| CPU | 2+ cores |
| RAM | 4 GB |
| PostgreSQL | Managed (RDS, Supabase, Neon) |
| Redis | Managed (ElastiCache, Upstash) |
| Storage | S3 or Wasabi |
| Proxy | Nginx/Caddy with TLS |

### Production Environment

```env
RUST_LOG=warn
JWT_SECRET=<64-character-cryptographically-random-string>
DATABASE_URL=postgres://...
REDIS_URL=redis://...
STORAGE_TYPE=s3
USE_PRESIGNED_URLS=true
CORS_ALLOWED_ORIGINS=https://yourdomain.com
```

---

## API Reference

### Public Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/auth/login` | User login |
| POST | `/api/auth/register` | User registration |
| POST | `/api/public-upload/:token` | File request upload |
| GET | `/api/public-download/:token` | Shared file download |

### Protected Endpoints

All require `Authorization: Bearer <token>` header.

| Resource | Operations |
|----------|------------|
| `/api/files` | CRUD, upload, download, share, lock, versions |
| `/api/file-requests` | Create portals, list submissions |
| `/api/users` | CRUD, roles, departments, suspend |
| `/api/tenants` | List, create, update, suspend (SuperAdmin) |
| `/api/departments` | CRUD, member management |
| `/api/roles` | CRUD, permission management |
| `/api/settings` | Compliance, branding, SMTP, blocked extensions |
| `/api/security/alerts` | List, resolve, dismiss alerts |
| `/api/audit-logs` | Query with filters, export |
| `/api/groups` | File groups CRUD, add/remove files |
| `/api/ai` | Summarization, Q&A, usage stats |

> See [backend/README.md](backend/README.md) for complete API documentation.

---

## Security

Security is a core focus of ClovaLink. Key measures include:

- **Tenant Isolation**: Row-level `tenant_id` enforcement on every table
- **JWT Hardening**: Short expiry, issuer/audience validation, key rotation support
- **Rate Limiting**: Atomic Redis-based limiting on sensitive endpoints
- **SQL Safety**: Compile-time query validation with SQLx
- **Content-Disposition**: Filename sanitization prevents header injection
- **Zip Slip Prevention**: Path validation on archive extraction
- **CORS Lockdown**: Explicit origin allowlisting in production

> See [Security Documentation](docs/wiki/Security.md) for complete security documentation and hardening guide.

---

## Roadmap

- [x] Multi-tenant architecture
- [x] HIPAA/SOX/GDPR compliance modes
- [x] Role-based access control
- [x] Extension system
- [x] Security alerts dashboard
- [x] Email notifications for alerts
- [x] AI-powered document features (summarization, Q&A)
- [x] File Groups (virtual collections)
- [x] Company Folders (org-wide sharing)
- [x] Office document preview (Excel, PowerPoint)
- [ ] Mobile apps (iOS/Android)
- [ ] WebDAV support
- [ ] Real-time collaboration
- [ ] Slack/Teams integration

---

## FAQ

<details>
<summary><b>How is this different from Nextcloud?</b></summary>

Nextcloud is a general-purpose collaboration suite. ClovaLink is purpose-built for:
- **True multi-tenancy** (not just user groups)
- **Compliance-first design** (HIPAA/SOX/GDPR modes lock settings)
- **MSP-friendly architecture** (manage many clients from one instance)
- **Rust performance** (handles more users with less resources)

</details>

<details>
<summary><b>Can I migrate from Box/Dropbox/SharePoint?</b></summary>

Not yet via built-in tools, but the API supports bulk upload. Community migration scripts are welcome!

</details>

<details>
<summary><b>Is there a hosted/SaaS version?</b></summary>

Yes! **[ClovaLink.com](https://clovalink.com)** offers a fully managed enterprise version for teams of 10+ users. Get all the features without the infrastructure overhead — we handle updates, backups, and scaling.

Self-hosting remains free and open source for smaller teams or those who prefer full control.

</details>

<details>
<summary><b>How do I back up my data?</b></summary>

1. **Database**: Standard PostgreSQL backup (pg_dump or managed provider snapshots)
2. **Files**: Sync your storage backend (S3 versioning, rclone for local)
3. **Config**: Version control your `.env` file (secrets in a vault)

</details>

<details>
<summary><b>What happens if I hit storage limits?</b></summary>

Per-tenant storage quotas can be configured. When exceeded, uploads are blocked until space is freed or quota increased. Admins receive storage warning emails at 80% capacity.

</details>

---

## Troubleshooting

<details>
<summary><b>Backend won't start: "Database connection refused"</b></summary>

```bash
# Check if PostgreSQL is running
docker compose ps postgres

# View logs
docker compose logs postgres

# Verify DATABASE_URL in .env matches compose.yml
```

</details>

<details>
<summary><b>CORS errors in browser</b></summary>

For development, ensure `CORS_DEV_MODE=true` is set in compose.yml.

For production, set `CORS_ALLOWED_ORIGINS` to your exact frontend domain.

</details>

<details>
<summary><b>File uploads fail with "413 Entity Too Large"</b></summary>

Nginx has a default body size limit. Add to your nginx config:

```nginx
client_max_body_size 100M;
```

</details>

<details>
<summary><b>Redis connection errors</b></summary>

```bash
# Check Redis is running
docker compose ps redis

# Test connection
docker compose exec redis redis-cli ping
# Should return: PONG
```

</details>

---

## Contributing

Contributions are welcome! Here's how:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'Add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

### Development Setup

```bash
# Backend (Rust)
cd backend
cargo run

# Frontend (React)
cd frontend
npm install
npm run dev
```

### Code Style

- **Rust**: `cargo fmt` and `cargo clippy`
- **TypeScript**: ESLint + Prettier (configured in project)

---

## License

MIT License — use it however you want. See [LICENSE](LICENSE) for details.

---

<p align="center">
  <b>Stop renting your file management. Own it.</b>
</p>

<p align="center">
  <a href="#top">Back to Top ↑</a>
</p>
