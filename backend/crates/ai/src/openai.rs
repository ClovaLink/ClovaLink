//! OpenAI Provider Implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::error::AiError;
use crate::provider::{AiProvider, AiResponse, EmbeddingResponse};

const OPENAI_API_URL: &str = "https://api.openai.com/v1";
const DEFAULT_CHAT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-ada-002";

pub struct OpenAiProvider {
    api_key: String,
    client: Client,
    chat_model: String,
    embedding_model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            chat_model: DEFAULT_CHAT_MODEL.to_string(),
            embedding_model: DEFAULT_EMBEDDING_MODEL.to_string(),
        }
    }
    
    pub fn with_models(api_key: String, chat_model: String, embedding_model: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            chat_model,
            embedding_model,
        }
    }
}

// OpenAI API request/response types
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct Usage {
    total_tokens: u32,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Deserialize)]
struct EmbeddingApiResponse {
    data: Vec<EmbeddingData>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }
    
    fn is_hipaa_approved(&self) -> bool {
        // OpenAI offers BAA for enterprise customers
        true
    }
    
    async fn summarize(&self, text: &str, max_tokens: u32) -> Result<AiResponse, AiError> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant that summarizes documents concisely. \
                         Provide a clear, professional summary highlighting key points.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("Please summarize the following text:\n\n{}", text),
            },
        ];
        
        self.chat_completion(messages, max_tokens.min(1000)).await
    }
    
    async fn answer(&self, question: &str, context: &str) -> Result<AiResponse, AiError> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant that answers questions based on the provided context. \
                         Only answer based on the information given. If the answer is not in the context, say so.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("Context:\n{}\n\nQuestion: {}", context, question),
            },
        ];
        
        self.chat_completion(messages, 500).await
    }
    
    async fn embed(&self, text: &str) -> Result<EmbeddingResponse, AiError> {
        let request = EmbeddingRequest {
            model: self.embedding_model.clone(),
            input: text.to_string(),
        };
        
        let response = self.client
            .post(format!("{}/embeddings", OPENAI_API_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("OpenAI embedding error: {} - {}", status, error_text);
            return Err(AiError::ProviderError(format!("OpenAI API error: {}", status)));
        }
        
        let api_response: EmbeddingApiResponse = response
            .json()
            .await
            .map_err(|_| AiError::InvalidResponse)?;
        
        let embedding = api_response.data
            .into_iter()
            .next()
            .ok_or(AiError::InvalidResponse)?
            .embedding;
        
        Ok(EmbeddingResponse {
            embedding,
            tokens_used: api_response.usage.map(|u| u.total_tokens).unwrap_or(0),
            model: self.embedding_model.clone(),
        })
    }
    
    async fn test_connection(&self) -> Result<bool, AiError> {
        // Simple test: try to get models list
        let response = self.client
            .get(format!("{}/models", OPENAI_API_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;
        
        Ok(response.status().is_success())
    }
}

impl OpenAiProvider {
    async fn chat_completion(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> Result<AiResponse, AiError> {
        let request = ChatRequest {
            model: self.chat_model.clone(),
            messages,
            max_tokens,
            temperature: 0.3, // Lower temperature for more consistent output
        };
        
        let response = self.client
            .post(format!("{}/chat/completions", OPENAI_API_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AiError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("OpenAI chat error: {} - {}", status, error_text);
            return Err(AiError::ProviderError(format!("OpenAI API error: {}", status)));
        }
        
        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|_| AiError::InvalidResponse)?;
        
        let content = chat_response.choices
            .into_iter()
            .next()
            .ok_or(AiError::InvalidResponse)?
            .message
            .content;
        
        Ok(AiResponse {
            content,
            tokens_used: chat_response.usage.map(|u| u.total_tokens).unwrap_or(0),
            model: self.chat_model.clone(),
        })
    }
}

