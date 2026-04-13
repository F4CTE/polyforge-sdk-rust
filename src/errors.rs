/// Errors returned by the Polyforge SDK.
#[derive(Debug, thiserror::Error)]
pub enum PolyforgeError {
    /// An error response from the Polyforge API.
    #[error("API error {status}: {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
        request_id: Option<String>,
        suggestion: Option<String>,
    },

    /// An HTTP transport error from reqwest.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// A JSON serialization or deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A client-side input validation error.
    #[error("Validation error: {0}")]
    Validation(String),
}

/// A convenience alias for `std::result::Result<T, PolyforgeError>`.
pub type Result<T> = std::result::Result<T, PolyforgeError>;
