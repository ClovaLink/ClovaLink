use aws_sdk_s3::{Client, config::Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use std::pin::Pin;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use tokio_util::io::ReaderStream;

// Encryption imports
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

/// A pinned, boxed stream of bytes for zero-copy streaming downloads
pub type StorageByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub name: String,
    pub size: u64,
    pub modified: String, // ISO 8601 string
    pub is_dir: bool,
}

/// Callback for tracking upload progress
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

#[async_trait]
pub trait Storage: Send + Sync {
    /// Upload data to storage (in-memory)
    async fn upload(&self, key: &str, data: Vec<u8>) -> Result<String, Box<dyn Error + Send + Sync>>;
    
    /// Upload from a file path (streaming) - more efficient for large files
    async fn upload_from_path(&self, key: &str, path: &Path) -> Result<String, Box<dyn Error + Send + Sync>>;
    
    async fn download(&self, key: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>>;
    async fn delete(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn list(&self, prefix: &str) -> Result<Vec<FileMetadata>, Box<dyn Error + Send + Sync>>;
    async fn create_folder(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn rename(&self, from: &str, to: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn exists(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;
    
    /// Generate a presigned URL for direct download (S3-compatible storage only)
    /// Returns Ok(None) if not supported (e.g., local storage)
    /// Works with any S3-compatible provider (AWS, Wasabi, MinIO, Backblaze B2, etc.)
    async fn presigned_download_url(
        &self,
        key: &str,
        expires_in_secs: u64,
    ) -> Result<Option<String>, Box<dyn Error + Send + Sync>>;
    
    /// Check if this storage backend supports presigned URLs
    fn supports_presigned_urls(&self) -> bool;
    
    /// Stream download - returns chunks without loading entire file into memory
    /// Returns a tuple of (stream, file_size_bytes) for setting Content-Length header
    /// This is the preferred method for large file downloads when presigned URLs aren't available
    async fn download_stream(&self, key: &str) -> Result<(StorageByteStream, u64), Box<dyn Error + Send + Sync>>;
    
    /// Health check - tests connectivity and returns latency in milliseconds
    async fn health_check(&self) -> Result<u64, Box<dyn Error + Send + Sync>>;
}

pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    pub async fn new(bucket: String) -> Self {
        let region_provider = RegionProviderChain::default_provider().or_else(Region::new("us-east-1"));
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let client = Client::new(&config);
        Self { client, bucket }
    }
}

#[async_trait]
impl Storage for S3Storage {
    async fn upload(&self, key: &str, data: Vec<u8>) -> Result<String, Box<dyn Error + Send + Sync>> {
        let body = ByteStream::from(data);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await?;
        Ok(format!("s3://{}/{}", self.bucket, key))
    }
    
    async fn upload_from_path(&self, key: &str, path: &Path) -> Result<String, Box<dyn Error + Send + Sync>> {
        // Stream from file to S3
        let body = ByteStream::from_path(path).await?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await?;
        Ok(format!("s3://{}/{}", self.bucket, key))
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        let data = resp.body.collect().await?;
        Ok(data.into_bytes().to_vec())
    }

    async fn delete(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<FileMetadata>, Box<dyn Error + Send + Sync>> {
        let prefix = if prefix.is_empty() {
            "".to_string()
        } else if prefix.ends_with('/') {
            prefix.to_string()
        } else {
            format!("{}/", prefix)
        };

        let mut response = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&prefix)
            .delimiter("/")
            .send()
            .await?;
        
        let mut files = Vec::new();
        
        // Process directories (CommonPrefixes)
        if let Some(prefixes) = response.common_prefixes {
            for prefix in prefixes {
                if let Some(p) = prefix.prefix {
                    files.push(FileMetadata {
                        name: p,
                        size: 0,
                        modified: String::new(),
                        is_dir: true,
                    });
                }
            }
        }

        // Process files (Contents)
        if let Some(objects) = response.contents {
            for obj in objects {
                if let Some(key) = obj.key {
                    // Skip the prefix itself if it appears in contents (e.g. the folder placeholder)
                    if key == prefix {
                        continue;
                    }
                    
                    let size = obj.size.unwrap_or(0) as u64;
                    let modified = obj.last_modified.map(|d| d.to_string()).unwrap_or_default();
                    
                    files.push(FileMetadata {
                        name: key,
                        size,
                        modified,
                        is_dir: false,
                    });
                }
            }
        }
        
        while response.is_truncated.unwrap_or(false) {
            let next_token = response.next_continuation_token.clone();
            response = self.client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&prefix)
                .delimiter("/")
                .continuation_token(next_token.unwrap())
                .send()
                .await?;
                
            if let Some(prefixes) = response.common_prefixes {
                for prefix in prefixes {
                    if let Some(p) = prefix.prefix {
                        files.push(FileMetadata {
                            name: p,
                            size: 0,
                            modified: String::new(),
                            is_dir: true,
                        });
                    }
                }
            }

            if let Some(objects) = response.contents {
                for obj in objects {
                    if let Some(key) = obj.key {
                        if key == prefix {
                            continue;
                        }
                        let size = obj.size.unwrap_or(0) as u64;
                        let modified = obj.last_modified.map(|d| d.to_string()).unwrap_or_default();
                        
                        files.push(FileMetadata {
                            name: key,
                            size,
                            modified,
                            is_dir: false,
                        });
                    }
                }
            }
        }
        
        Ok(files)
    }

    async fn create_folder(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let key = if key.ends_with('/') { key.to_string() } else { format!("{}/", key) };
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(vec![]))
            .send()
            .await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Copy object
        self.client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(format!("{}/{}", self.bucket, from))
            .key(to)
            .send()
            .await?;

        // Delete original
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(from)
            .send()
            .await?;
            
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match self.client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await 
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    /// Generate a presigned URL for direct download from S3-compatible storage
    /// Works with AWS S3, Wasabi, MinIO, Backblaze B2, etc. - uses configured endpoint
    async fn presigned_download_url(
        &self,
        key: &str,
        expires_in_secs: u64,
    ) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        let presigning_config = PresigningConfig::expires_in(Duration::from_secs(expires_in_secs))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        
        let presigned_request = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(presigning_config)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        
        Ok(Some(presigned_request.uri().to_string()))
    }
    
    fn supports_presigned_urls(&self) -> bool {
        true
    }
    
    async fn download_stream(&self, key: &str) -> Result<(StorageByteStream, u64), Box<dyn Error + Send + Sync>> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        
        // Get content length for Content-Length header
        let size = resp.content_length().unwrap_or(0) as u64;
        
        // Convert S3 ByteStream to async reader, then to a Stream of Bytes
        let async_reader = resp.body.into_async_read();
        let reader_stream = ReaderStream::new(async_reader);
        
        Ok((Box::pin(reader_stream), size))
    }
    
    async fn health_check(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let start = std::time::Instant::now();
        // Use list_objects_v2 with max_keys=1 as a lightweight connectivity test
        self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .max_keys(1)
            .send()
            .await?;
        Ok(start.elapsed().as_millis() as u64)
    }
}

pub struct LocalStorage {
    base_path: std::path::PathBuf,
}

impl LocalStorage {
    pub fn new(base_path: &str) -> Self {
        std::fs::create_dir_all(base_path).unwrap_or_default();
        Self {
            base_path: std::path::PathBuf::from(base_path),
        }
    }
}

#[async_trait]
impl Storage for LocalStorage {
    async fn upload(&self, key: &str, data: Vec<u8>) -> Result<String, Box<dyn Error + Send + Sync>> {
        let path = self.base_path.join(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, data).await?;
        Ok(format!("local://{}", path.display()))
    }
    
    async fn upload_from_path(&self, key: &str, source_path: &Path) -> Result<String, Box<dyn Error + Send + Sync>> {
        let dest_path = self.base_path.join(key);
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // Copy file from source to destination (streaming)
        tokio::fs::copy(source_path, &dest_path).await?;
        Ok(format!("local://{}", dest_path.display()))
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let path = self.base_path.join(key);
        let data = tokio::fs::read(path).await?;
        Ok(data)
    }

    async fn delete(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = self.base_path.join(key);
        if path.is_dir() {
            tokio::fs::remove_dir_all(path).await?;
        } else {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<FileMetadata>, Box<dyn Error + Send + Sync>> {
        let mut files = Vec::new();
        let path = self.base_path.join(prefix);
        
        if !path.exists() {
            return Ok(files);
        }

        let mut entries = tokio::fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = metadata.is_dir();
            let size = metadata.len();
            let modified = metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string()) // Simple timestamp for now
                .unwrap_or_default();

            files.push(FileMetadata {
                name,
                size,
                modified,
                is_dir,
            });
        }
        Ok(files)
    }

    async fn create_folder(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = self.base_path.join(key);
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let from_path = self.base_path.join(from);
        let to_path = self.base_path.join(to);
        
        // Ensure the parent directory of the target exists
        if let Some(parent) = to_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        tokio::fs::rename(from_path, to_path).await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let path = self.base_path.join(key);
        Ok(path.exists())
    }
    
    /// Local storage doesn't support presigned URLs - returns None to trigger proxy fallback
    async fn presigned_download_url(
        &self,
        _key: &str,
        _expires_in_secs: u64,
    ) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        Ok(None) // Not supported, download handler will fallback to proxy
    }
    
    fn supports_presigned_urls(&self) -> bool {
        false
    }
    
    async fn download_stream(&self, key: &str) -> Result<(StorageByteStream, u64), Box<dyn Error + Send + Sync>> {
        let path = self.base_path.join(key);
        let file = tokio::fs::File::open(&path).await?;
        let metadata = file.metadata().await?;
        let size = metadata.len();
        
        // Create a ReaderStream from the tokio file (streams in ~8KB chunks by default)
        let stream = ReaderStream::new(file);
        
        Ok((Box::pin(stream), size))
    }
    
    async fn health_check(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let start = std::time::Instant::now();
        // Check if base path exists and is accessible
        if self.base_path.exists() && self.base_path.is_dir() {
            Ok(start.elapsed().as_millis() as u64)
        } else {
            Err("Local storage path does not exist or is not a directory".into())
        }
    }
}

// ============================================================================
// ENCRYPTED LOCAL STORAGE
// ============================================================================
// 
// Provides transparent ChaCha20-Poly1305 encryption for local file storage.
// Files are stored as: [12-byte nonce][ciphertext][16-byte auth tag]
//
// Features:
// - Encrypt-on-upload, decrypt-on-download
// - Backwards compatible: auto-detects and returns plaintext files
// - Each file uses a unique random nonce (no nonce reuse)
// - AEAD provides both confidentiality and integrity
// ============================================================================

/// Nonce size for ChaCha20-Poly1305 (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Encrypted local storage wrapper
/// 
/// Wraps LocalStorage and transparently encrypts/decrypts file content using
/// ChaCha20-Poly1305. The encryption key is provided at construction time.
pub struct EncryptedLocalStorage {
    inner: LocalStorage,
    cipher: ChaCha20Poly1305,
}

impl EncryptedLocalStorage {
    /// Create a new encrypted local storage with the given base path and encryption key.
    /// 
    /// # Arguments
    /// * `base_path` - Directory path for file storage
    /// * `key` - 32-byte encryption key (256 bits)
    /// 
    /// # Panics
    /// Panics if the key is not exactly 32 bytes.
    pub fn new(base_path: &str, key: &[u8; 32]) -> Self {
        tracing::info!("Initializing encrypted local storage at: {}", base_path);
        Self {
            inner: LocalStorage::new(base_path),
            cipher: ChaCha20Poly1305::new(key.into()),
        }
    }
    
    /// Create from a base64-encoded key string.
    /// 
    /// # Arguments
    /// * `base_path` - Directory path for file storage
    /// * `key_base64` - Base64-encoded 32-byte encryption key
    /// 
    /// # Returns
    /// `Ok(Self)` if the key is valid, `Err` if decoding fails or key size is wrong.
    pub fn from_base64_key(base_path: &str, key_base64: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use base64::Engine;
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(key_base64)
            .map_err(|e| format!("Invalid base64 encryption key: {}", e))?;
        
        if key_bytes.len() != 32 {
            return Err(format!(
                "Encryption key must be exactly 32 bytes, got {} bytes", 
                key_bytes.len()
            ).into());
        }
        
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        
        Ok(Self::new(base_path, &key))
    }
    
    /// Encrypt data with a random nonce.
    /// Returns [nonce || ciphertext] where nonce is 12 bytes.
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encryption failed: {}", e))?;
        
        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);
        
        Ok(result)
    }
    
    /// Decrypt data that has nonce prepended.
    /// Expects [nonce || ciphertext] format.
    /// 
    /// Returns the plaintext, or the original data if decryption fails
    /// (backwards compatibility with unencrypted files).
    fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        // Need at least nonce + auth tag (12 + 16 = 28 bytes)
        if data.len() < NONCE_SIZE + 16 {
            // Too short to be encrypted, return as-is (plaintext)
            tracing::debug!("File too short to be encrypted, returning as plaintext");
            return data.to_vec();
        }
        
        // Extract nonce and ciphertext
        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
        let ciphertext = &data[NONCE_SIZE..];
        
        // Try to decrypt
        match self.cipher.decrypt(nonce, ciphertext) {
            Ok(plaintext) => {
                tracing::debug!("Successfully decrypted file ({} bytes)", plaintext.len());
                plaintext
            }
            Err(_) => {
                // Decryption failed - probably a plaintext file from before encryption was enabled
                tracing::debug!("Decryption failed, assuming plaintext file (backwards compatibility)");
                data.to_vec()
            }
        }
    }
}

#[async_trait]
impl Storage for EncryptedLocalStorage {
    async fn upload(&self, key: &str, data: Vec<u8>) -> Result<String, Box<dyn Error + Send + Sync>> {
        let encrypted = self.encrypt(&data)?;
        tracing::debug!(
            "Encrypting upload: {} -> {} bytes (key: {})",
            data.len(),
            encrypted.len(),
            key
        );
        self.inner.upload(key, encrypted).await
    }
    
    async fn upload_from_path(&self, key: &str, path: &Path) -> Result<String, Box<dyn Error + Send + Sync>> {
        // Read file, encrypt, then write
        // (We can't stream-encrypt with AEAD since we need the whole message for auth tag)
        let data = tokio::fs::read(path).await?;
        let encrypted = self.encrypt(&data)?;
        tracing::debug!(
            "Encrypting upload from path: {} -> {} bytes (key: {})",
            data.len(),
            encrypted.len(),
            key
        );
        self.inner.upload(key, encrypted).await
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let encrypted = self.inner.download(key).await?;
        Ok(self.decrypt(&encrypted))
    }

    async fn delete(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.inner.delete(key).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<FileMetadata>, Box<dyn Error + Send + Sync>> {
        // List returns metadata only, no decryption needed
        // Note: file sizes will be encrypted sizes (slightly larger than original)
        self.inner.list(prefix).await
    }

    async fn create_folder(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.inner.create_folder(key).await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Rename is just a filesystem operation, encryption is preserved
        self.inner.rename(from, to).await
    }

    async fn exists(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        self.inner.exists(key).await
    }
    
    async fn presigned_download_url(
        &self,
        _key: &str,
        _expires_in_secs: u64,
    ) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        // Local storage doesn't support presigned URLs
        Ok(None)
    }
    
    fn supports_presigned_urls(&self) -> bool {
        false
    }
    
    async fn download_stream(&self, key: &str) -> Result<(StorageByteStream, u64), Box<dyn Error + Send + Sync>> {
        // For encrypted storage, we must decrypt the entire file first
        // (ChaCha20-Poly1305 is an AEAD cipher that needs the full ciphertext for auth)
        let encrypted = self.inner.download(key).await?;
        let decrypted = self.decrypt(&encrypted);
        let size = decrypted.len() as u64;
        
        // Create a stream from the decrypted bytes
        let stream = futures::stream::once(async move {
            Ok::<Bytes, std::io::Error>(Bytes::from(decrypted))
        });
        
        Ok((Box::pin(stream), size))
    }
    
    async fn health_check(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        self.inner.health_check().await
    }
}
