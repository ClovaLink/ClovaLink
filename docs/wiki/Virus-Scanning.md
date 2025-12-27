# Virus Scanning

ClovaLink integrates with ClamAV to automatically scan uploaded files for viruses, malware, and other threats. This document covers configuration, administration, and troubleshooting.

## Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        File Upload Flow                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   User Upload  ──▶  Store File  ──▶  Queue Scan Job  ──▶  ClamAV Scan   │
│                         │                                      │         │
│                         ▼                                      ▼         │
│                   scan_status:              ┌──────────────────────────┐ │
│                   "pending"                 │  Clean?                  │ │
│                                             │  ├─ Yes: status="clean"  │ │
│                                             │  └─ No:  Quarantine/     │ │
│                                             │         Delete/Flag      │ │
│                                             └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### Why ClamAV?

- **Open Source**: No licensing fees, actively maintained
- **Industry Standard**: Used by major email providers and enterprises
- **Automatic Updates**: Virus definitions update automatically
- **Low Latency**: Daemon mode provides fast scanning (< 100ms for small files)
- **Scalable**: Multiple workers can scan in parallel

---

## Architecture

### Components

| Component | Description |
|-----------|-------------|
| **ClamAV Daemon (clamd)** | Background service that performs scans |
| **Freshclam** | Automatically updates virus definitions |
| **Scan Queue** | PostgreSQL-backed job queue for async scanning |
| **Scan Workers** | Backend threads that send files to ClamAV |

### Scan Flow

1. **Upload**: File is uploaded and stored
2. **Queue**: Scan job is created with `status: pending`
3. **Pick Up**: Worker claims the job
4. **Scan**: File sent to ClamAV daemon via TCP
5. **Result**: 
   - Clean → `scan_status: clean`
   - Infected → Action taken based on tenant settings

### Database Tables

| Table | Purpose |
|-------|---------|
| `virus_scan_settings` | Per-tenant configuration |
| `virus_scan_jobs` | Scan job queue |
| `virus_scan_results` | Scan history and metrics |
| `quarantined_files` | Files moved to quarantine |
| `user_malware_counts` | Track repeat offenders |

---

## Configuration

### Environment Variables

Configure in `backend/.env` or `infra/.env`:

```bash
# Enable/disable virus scanning
CLAMAV_ENABLED=true

# ClamAV daemon connection
CLAMAV_HOST=localhost        # Or 'clamav' in Docker
CLAMAV_PORT=3310

# Scan timeout (milliseconds)
CLAMAV_TIMEOUT_MS=30000      # 30 seconds

# Concurrent scan workers
CLAMAV_WORKERS=4

# Skip files larger than this (MB)
CLAMAV_MAX_FILE_SIZE_MB=100

# Backpressure: max pending jobs (0 = unlimited)
CLAMAV_MAX_QUEUE_SIZE=10000
```

### Per-Tenant Settings

Each tenant can customize their scanning behavior:

| Setting | Description | Default |
|---------|-------------|---------|
| `enabled` | Enable scanning for this tenant | `true` |
| `file_types` | File extensions to scan (empty = all) | `[]` |
| `max_file_size_mb` | Skip files larger than this | `100` |
| `action_on_detect` | What to do when malware found | `quarantine` |
| `notify_admin` | Email admin on detection | `true` |
| `notify_uploader` | Email uploader on detection | `false` |
| `auto_suspend_uploader` | Auto-suspend repeat offenders | `false` |
| `suspend_threshold` | Malware count before suspension | `1` |

#### Configure via UI

1. Go to **Settings** → **Security** → **Virus Scanning**
2. Adjust settings
3. Save

#### Configure via API

```bash
PUT /api/settings/virus-scan
Authorization: Bearer <token>
Content-Type: application/json

{
  "enabled": true,
  "action_on_detect": "quarantine",
  "notify_admin": true,
  "notify_uploader": true,
  "auto_suspend_uploader": true,
  "suspend_threshold": 3,
  "max_file_size_mb": 50
}
```

---

## Scan Actions

When malware is detected, one of three actions is taken:

### Quarantine (Recommended)

```
action_on_detect: "quarantine"
```

- File is moved to quarantine storage
- Original file reference is updated
- Admin can review and restore if false positive
- **Best for**: Most use cases

### Delete

```
action_on_detect: "delete"
```

- File is permanently deleted
- No recovery possible
- **Best for**: High-security environments

### Flag

```
action_on_detect: "flag"
```

- File remains accessible
- `scan_status` set to `infected`
- Admin must manually handle
- **Best for**: Testing, low-risk environments

---

## Quarantine Management

### Viewing Quarantined Files

1. Navigate to **Security** → **Quarantine**
2. View list of quarantined files with:
   - Original filename
   - Threat name
   - Uploader
   - Quarantine date
   - File size

### Admin Actions

#### Release (False Positive)

If a file was incorrectly flagged:

1. Select the file
2. Click **Release**
3. File is restored to original location
4. `scan_status` set to `clean`

#### Permanently Delete

To remove a quarantined file forever:

1. Select the file
2. Click **Delete Permanently**
3. Confirm deletion
4. File is removed from quarantine storage

### Quarantine API

```bash
# List quarantined files
GET /api/quarantine

# Release a file
POST /api/quarantine/{id}/release

# Permanently delete
DELETE /api/quarantine/{id}
```

---

## Auto-Suspend Feature

Automatically suspend users who repeatedly upload malware.

### Enable Auto-Suspend

```bash
PUT /api/settings/virus-scan
{
  "auto_suspend_uploader": true,
  "suspend_threshold": 3
}
```

### How It Works

1. User uploads malware → count incremented
2. Count reaches threshold → user suspended
3. Suspension reason: "Automatic suspension: uploaded malware"
4. Admin notified via security alert
5. Admin must manually unsuspend user

### View Malware Counts

Admins can see malware upload history:

1. Go to **Users** → Select user
2. View **Security** tab
3. See malware upload count and history

---

## Email Notifications

### Admin Notification

When malware is detected, admins receive:

- **Subject**: 🛡️ Security Alert: Malware Detected in {filename}
- **Content**: 
  - File name
  - Threat name (e.g., "Win.Trojan.Generic")
  - Uploader email
  - Action taken
  - Link to security dashboard

### Uploader Notification

If `notify_uploader` is enabled:

- **Subject**: Security Notice: Your uploaded file was flagged
- **Content**: 
  - File name
  - Reason flagged
  - Action taken
  - What to do next

### Configure Email Templates

Customize notification content:

1. Go to **Settings** → **Email Templates**
2. Edit `malware_detected` or `malware_detected_uploader`
3. Save

---

## Performance Tuning

### Worker Count

```bash
CLAMAV_WORKERS=4
```

- **Low volume** (< 100 uploads/day): 2 workers
- **Medium volume** (100-1000/day): 4 workers
- **High volume** (1000+/day): 8+ workers

### Queue Backpressure

```bash
CLAMAV_MAX_QUEUE_SIZE=10000
```

When queue is full:
- New scan jobs are rejected
- Upload still succeeds
- File marked as `scan_status: skipped`
- Prevents memory exhaustion

### Timeout

```bash
CLAMAV_TIMEOUT_MS=30000
```

- Increase for slow storage or large files
- Decrease if ClamAV is local and fast
- Jobs that timeout are retried with exponential backoff

### File Size Limit

```bash
CLAMAV_MAX_FILE_SIZE_MB=100
```

Large files:
- Take longer to scan
- May timeout
- Consider skipping files > 100MB
- Marked as `scan_status: skipped`

---

## Monitoring

### Scan Metrics

View in **Dashboard** → **Security**:

- Total scans today/week/month
- Infected files detected
- Average scan time
- Queue depth

### Scan Statuses

| Status | Meaning |
|--------|---------|
| `pending` | Waiting to be scanned |
| `clean` | Scanned, no threats found |
| `infected` | Malware detected |
| `skipped` | Too large or scanning disabled |
| `error` | Scan failed (will retry) |

### Health Check

The `/health` endpoint includes ClamAV status:

```json
{
  "status": "healthy",
  "components": {
    "database": "ok",
    "redis": "ok",
    "clamav": "ok"
  }
}
```

---

## Docker / Podman Setup

### Using compose.yml

ClamAV is included in the default compose configuration:

```yaml
clamav:
  image: clamav/clamav-debian:latest
  container_name: clovalink-clamav
  ports:
    - "3310:3310"
  volumes:
    - clamav_data:/var/lib/clamav
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "clamdscan", "--ping", "1"]
    interval: 30s
    timeout: 10s
    retries: 5
    start_period: 120s  # Allow time for definition download
```

### First Start

On first start, ClamAV downloads virus definitions:

1. Container starts
2. Freshclam downloads ~300MB of definitions
3. Clamd loads definitions into memory
4. Health check passes after ~2 minutes
5. Ready to scan

### Definition Updates

Freshclam automatically updates definitions:
- Checks every 2 hours by default
- Updates are atomic (no downtime)
- New definitions loaded on next scan

---

## Troubleshooting

### ClamAV Not Responding

**Symptoms**: Scans timeout or fail

**Solutions**:
1. Check ClamAV is running:
   ```bash
   podman ps | grep clamav
   ```
2. Check logs:
   ```bash
   podman logs clovalink-clamav
   ```
3. Verify port is accessible:
   ```bash
   nc -zv localhost 3310
   ```

### Slow Scans

**Symptoms**: Scans taking > 10 seconds

**Solutions**:
1. Check ClamAV resource usage
2. Increase workers if CPU available
3. Consider skipping large files
4. Ensure ClamAV has sufficient memory (needs ~1GB)

### High Queue Depth

**Symptoms**: Many pending scan jobs

**Solutions**:
1. Increase `CLAMAV_WORKERS`
2. Check for slow storage
3. Increase `CLAMAV_TIMEOUT_MS`
4. Scale horizontally (multiple backend instances)

### False Positives

**Symptoms**: Legitimate files flagged as malware

**Solutions**:
1. Use Quarantine (not Delete) to allow recovery
2. Check VirusTotal for second opinion
3. Release from quarantine if false positive
4. Consider adding to file type exclusions

### ClamAV Out of Memory

**Symptoms**: Container crashes, "Out of memory" in logs

**Solutions**:
1. Increase container memory limit to 2GB+
2. ClamAV needs ~1GB for definitions
3. Check for memory leaks in other containers

### Definitions Not Updating

**Symptoms**: Old signature version in health check

**Solutions**:
1. Check freshclam logs:
   ```bash
   podman exec clovalink-clamav cat /var/log/clamav/freshclam.log
   ```
2. Verify network access to clamav.net
3. Restart container to trigger update

---

## Security Considerations

### Network Isolation

ClamAV should only be accessible from the backend:

```yaml
networks:
  clovalink-net:
    driver: bridge
```

Never expose port 3310 to the public internet.

### File Access

ClamAV scans files via TCP streaming:
- Files are sent over network to daemon
- No shared filesystem needed
- Secure even in multi-tenant environments

### Threat Intelligence

ClamAV signatures detect:
- Viruses and worms
- Trojans and ransomware
- Malicious scripts
- Known exploit files
- Phishing documents

For enterprise environments, consider:
- Additional threat feeds
- Sandboxing integration
- Machine learning detection

---

## API Reference

### Get Scan Settings

```bash
GET /api/settings/virus-scan
```

### Update Scan Settings

```bash
PUT /api/settings/virus-scan
{
  "enabled": true,
  "action_on_detect": "quarantine",
  "notify_admin": true
}
```

### Get Scan Status for File

```bash
GET /api/files/{id}
# Response includes: "scan_status": "clean"
```

### List Quarantined Files

```bash
GET /api/quarantine
```

### Release from Quarantine

```bash
POST /api/quarantine/{id}/release
```

### Permanently Delete Quarantined File

```bash
DELETE /api/quarantine/{id}
```

### Get Scan Metrics

```bash
GET /api/security/scan-metrics
```

Response:
```json
{
  "total_scans_today": 150,
  "infected_today": 2,
  "avg_scan_time_ms": 85,
  "queue_depth": 3,
  "scanner_version": "ClamAV 1.2.0",
  "signature_version": "27150"
}
```

