# Changelog

All notable changes to ClovaLink will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3.1] - 2026-02-09

### Fixed

- **SMTP TLS Configuration**: Fixed conflicting TLS modes that caused email delivery failures on most SMTP providers (Gmail, SendGrid, Office365, etc.). Port 465 now correctly uses implicit TLS, port 587 uses STARTTLS, and non-secure connections are handled properly.
- **HTML Email Rendering**: Emails were being sent with `text/plain` content type despite containing HTML, causing recipients to see raw HTML tags. Emails now use `text/html` content type for proper rendering.

## [0.1.3] - 2026-01-23

### Added

- **Local Storage Encryption**: Files stored on local disk are now encrypted at rest using ChaCha20-Poly1305 authenticated encryption
  - Enable by setting `ENCRYPTION_KEY` environment variable (base64-encoded 32-byte key)
  - Generate a key with: `openssl rand -base64 32`
  - Each file uses a unique random nonce (no nonce reuse)
  - AEAD provides both confidentiality and integrity verification
  - Backwards compatible: existing unencrypted files remain readable when encryption is enabled

- **Install Script Encryption Prompt**: The installer now asks if you want to enable local file encryption and automatically generates a secure key

### Changed

- Docker Compose now includes commented `ENCRYPTION_KEY` configuration with documentation

### Security

- Local storage encryption uses ChaCha20-Poly1305 (RFC 8439), a modern AEAD cipher
- S3 storage continues to use provider-side encryption (AWS SSE, etc.)
- Encryption keys must be exactly 32 bytes (256 bits)

### Notes

- **Key Management**: Store your encryption key securely. Losing the key means losing access to all encrypted files.
- **S3 Users**: The `ENCRYPTION_KEY` setting only applies to local storage. S3 storage uses provider-side encryption.
- **Performance**: ChaCha20 is optimized for software implementations and adds minimal overhead.

## [0.1.2] - 2026-01-15

### Added

- File Groups feature for organizing files into collections
- File comments and threaded replies
- Discord OAuth integration for notifications
- AI-powered file summaries and semantic search

### Fixed

- Various bug fixes and performance improvements

## [0.1.1] - 2026-01-01

### Added

- Virus scanning with ClamAV integration
- S3 replication for enterprise durability
- Transfer scheduler for prioritized uploads/downloads
- Circuit breaker pattern for external service resilience

## [0.1.0] - 2025-12-15

### Added

- Initial release
- Multi-tenant file management
- Role-based access control
- File sharing with expiring links
- File requests for external uploads
- Audit logging and compliance modes (HIPAA, SOX, GDPR)
- Content-addressed storage with deduplication
- S3 and local storage backends
