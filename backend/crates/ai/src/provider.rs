//! AI Provider Trait
//! 
//! Abstract interface for AI providers (OpenAI, Anthropic, Azure, etc.)

use async_trait::async_trait;
use crate::error::AiError;

/// Response from an AI operation including token usage
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub content: String,
    pub tokens_used: u32,
    pub model: String,
}

/// Embedding response
#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
    pub tokens_used: u32,
    pub model: String,
}

/// Abstract trait for AI providers
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider identifier (e.g., "openai", "anthropic")
    fn name(&self) -> &str;
    
    /// Whether this provider is approved for HIPAA-compliant usage
    fn is_hipaa_approved(&self) -> bool;
    
    /// Summarize the given text
    async fn summarize(&self, text: &str, max_tokens: u32) -> Result<AiResponse, AiError>;
    
    /// Answer a question given context
    async fn answer(&self, question: &str, context: &str) -> Result<AiResponse, AiError>;
    
    /// Generate embeddings for semantic search
    async fn embed(&self, text: &str) -> Result<EmbeddingResponse, AiError>;
    
    /// Test the API connection (used for settings validation)
    async fn test_connection(&self) -> Result<bool, AiError>;
}

/// Provider registry for resolving provider by name
pub struct ProviderRegistry;

impl ProviderRegistry {
    /// Get a provider instance by name
    pub fn get(name: &str, api_key: &str) -> Option<Box<dyn AiProvider>> {
        match name.to_lowercase().as_str() {
            "openai" => Some(Box::new(crate::openai::OpenAiProvider::new(api_key.to_string()))),
            // Future providers can be added here:
            // "anthropic" => Some(Box::new(crate::anthropic::AnthropicProvider::new(api_key))),
            // "azure" => Some(Box::new(crate::azure::AzureProvider::new(api_key))),
            _ => None,
        }
    }
    
    /// List available provider names
    pub fn available_providers() -> Vec<&'static str> {
        vec!["openai"]
    }
    
    /// Check if a provider is HIPAA approved
    pub fn is_hipaa_approved(name: &str) -> bool {
        match name.to_lowercase().as_str() {
            // OpenAI has a BAA available for enterprise
            "openai" => true,
            // Add other approved providers here
            _ => false,
        }
    }
}

