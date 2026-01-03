//! AI Error Types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("AI features are not enabled for your organization")]
    Disabled,
    
    #[error("No API key configured for AI provider")]
    NoApiKey,
    
    #[error("Your role does not have access to AI features")]
    Forbidden,
    
    #[error("Monthly usage limit reached. Contact your administrator.")]
    MonthlyLimitExceeded,
    
    #[error("Daily request limit reached. Try again tomorrow.")]
    DailyLimitExceeded,
    
    #[error("This AI provider is not approved for HIPAA compliance")]
    HipaaNotApproved,
    
    #[error("AI content generation is disabled under SOX compliance (read-only mode)")]
    SoxReadOnly,
    
    #[error("{0}")]
    MaintenanceMode(String),
    
    #[error("Provider error: {0}")]
    ProviderError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Invalid response from AI provider")]
    InvalidResponse,
    
    #[error("File not found or not accessible")]
    FileNotFound,
    
    #[error("File content could not be extracted")]
    ContentExtractionFailed,
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Unable to process request. Please try again later.")]
    InternalError,
}

impl AiError {
    /// Returns true if this error should be logged as a warning vs error
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            AiError::Disabled
                | AiError::NoApiKey
                | AiError::Forbidden
                | AiError::MonthlyLimitExceeded
                | AiError::DailyLimitExceeded
                | AiError::HipaaNotApproved
                | AiError::SoxReadOnly
                | AiError::MaintenanceMode(_)
        )
    }
    
    /// Get status code for HTTP response
    pub fn status_code(&self) -> u16 {
        match self {
            AiError::Disabled | AiError::NoApiKey => 503,
            AiError::Forbidden | AiError::HipaaNotApproved | AiError::SoxReadOnly => 403,
            AiError::MonthlyLimitExceeded | AiError::DailyLimitExceeded => 429,
            AiError::MaintenanceMode(_) => 503, // Service Unavailable
            AiError::FileNotFound => 404,
            _ => 500,
        }
    }
}

