//! Streaming inference support
//!
//! Handles token-by-token streaming output from the model.

/// Represents a token emitted during streaming inference.
#[derive(Debug, Clone)]
pub enum StreamToken {
    /// A generated token string
    Token(String),
    /// Generation completed successfully (EOS token reached)
    Done,
    /// Generation hit max_tokens limit without EOS (response may be incomplete)
    Truncated { tokens_generated: u32, max_tokens: u32 },
    /// An error occurred during generation
    Error(String),
}

impl StreamToken {
    /// Returns true if this is a token variant
    pub fn is_token(&self) -> bool {
        matches!(self, StreamToken::Token(_))
    }

    /// Returns true if generation is complete (with EOS)
    pub fn is_done(&self) -> bool {
        matches!(self, StreamToken::Done)
    }

    /// Returns true if generation was truncated (hit max_tokens)
    pub fn is_truncated(&self) -> bool {
        matches!(self, StreamToken::Truncated { .. })
    }

    /// Returns true if an error occurred
    pub fn is_error(&self) -> bool {
        matches!(self, StreamToken::Error(_))
    }

    /// Extracts the token string if this is a Token variant
    pub fn as_token(&self) -> Option<&str> {
        match self {
            StreamToken::Token(s) => Some(s),
            _ => None,
        }
    }

    /// Extracts the error message if this is an Error variant
    pub fn as_error(&self) -> Option<&str> {
        match self {
            StreamToken::Error(s) => Some(s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_token_variants() {
        let token = StreamToken::Token("hello".to_string());
        assert!(token.is_token());
        assert!(!token.is_done());
        assert!(!token.is_error());
        assert_eq!(token.as_token(), Some("hello"));

        let done = StreamToken::Done;
        assert!(!done.is_token());
        assert!(done.is_done());
        assert!(!done.is_error());

        let error = StreamToken::Error("test error".to_string());
        assert!(!error.is_token());
        assert!(!error.is_done());
        assert!(error.is_error());
        assert_eq!(error.as_error(), Some("test error"));
    }
}
