//! PII Redaction Service
//! 
//! Redacts personally identifiable information before sending to AI providers.

use regex::Regex;
use std::sync::OnceLock;

/// Redaction patterns for common PII
struct RedactionPatterns {
    email: Regex,
    phone: Regex,
    ssn: Regex,
    credit_card: Regex,
    ip_address: Regex,
}

static PATTERNS: OnceLock<RedactionPatterns> = OnceLock::new();

fn get_patterns() -> &'static RedactionPatterns {
    PATTERNS.get_or_init(|| RedactionPatterns {
        // Email addresses
        email: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
        // Phone numbers (various formats)
        phone: Regex::new(r"(\+?1[-.\s]?)?(\(?\d{3}\)?[-.\s]?)?\d{3}[-.\s]?\d{4}").unwrap(),
        // Social Security Numbers
        ssn: Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b").unwrap(),
        // Credit card numbers (basic pattern)
        credit_card: Regex::new(r"\b(?:\d{4}[-\s]?){3}\d{4}\b").unwrap(),
        // IP addresses
        ip_address: Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
    })
}

/// Service for redacting PII from text
pub struct RedactionService;

impl RedactionService {
    /// Redact all known PII patterns from text
    pub fn redact(text: &str) -> String {
        let patterns = get_patterns();
        
        let mut result = text.to_string();
        
        // Order matters - more specific patterns first
        result = patterns.ssn.replace_all(&result, "[REDACTED_SSN]").to_string();
        result = patterns.credit_card.replace_all(&result, "[REDACTED_CC]").to_string();
        result = patterns.email.replace_all(&result, "[REDACTED_EMAIL]").to_string();
        result = patterns.phone.replace_all(&result, "[REDACTED_PHONE]").to_string();
        result = patterns.ip_address.replace_all(&result, "[REDACTED_IP]").to_string();
        
        result
    }
    
    /// Chunk text into smaller pieces for minimal context sending
    /// Returns chunks of approximately max_tokens size (rough estimate: 4 chars = 1 token)
    pub fn chunk_text(text: &str, max_tokens: usize) -> Vec<String> {
        let max_chars = max_tokens * 4; // Rough token-to-char ratio
        let mut chunks = Vec::new();
        
        if text.len() <= max_chars {
            chunks.push(text.to_string());
            return chunks;
        }
        
        // Split by paragraphs first
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let mut current_chunk = String::new();
        
        for para in paragraphs {
            if current_chunk.len() + para.len() + 2 > max_chars {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.trim().to_string());
                    current_chunk = String::new();
                }
                
                // If single paragraph is too long, split by sentences
                if para.len() > max_chars {
                    let sentences: Vec<&str> = para.split(". ").collect();
                    for sentence in sentences {
                        if current_chunk.len() + sentence.len() + 2 > max_chars {
                            if !current_chunk.is_empty() {
                                chunks.push(current_chunk.trim().to_string());
                                current_chunk = String::new();
                            }
                        }
                        current_chunk.push_str(sentence);
                        current_chunk.push_str(". ");
                    }
                } else {
                    current_chunk.push_str(para);
                    current_chunk.push_str("\n\n");
                }
            } else {
                current_chunk.push_str(para);
                current_chunk.push_str("\n\n");
            }
        }
        
        if !current_chunk.trim().is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }
        
        chunks
    }
    
    /// Get hash of text chunk for change detection
    pub fn hash_chunk(text: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_email_redaction() {
        let text = "Contact john@example.com for more info";
        let redacted = RedactionService::redact(text);
        assert!(redacted.contains("[REDACTED_EMAIL]"));
        assert!(!redacted.contains("john@example.com"));
    }
    
    #[test]
    fn test_phone_redaction() {
        let text = "Call 555-123-4567 or (555) 123-4567";
        let redacted = RedactionService::redact(text);
        assert!(redacted.contains("[REDACTED_PHONE]"));
    }
    
    #[test]
    fn test_ssn_redaction() {
        let text = "SSN: 123-45-6789";
        let redacted = RedactionService::redact(text);
        assert!(redacted.contains("[REDACTED_SSN]"));
    }
}

