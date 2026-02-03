//! Message types
//!
//! Defines chat message structures and roles.

use serde::{Deserialize, Serialize};

/// Role of a message sender
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Message from the user
    User,
    /// Message from the AI assistant
    Assistant,
    /// System prompt
    System,
}

/// A single chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender
    pub role: Role,
    /// The content of the message
    pub content: String,
    /// Timestamp when the message was created
    pub timestamp: u64,
}

impl Message {
    /// Create a new message
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::new(Role::User, "Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello, world!");
        assert!(msg.timestamp > 0);
    }

    #[test]
    fn test_role_equality() {
        assert_eq!(Role::User, Role::User);
        assert_ne!(Role::User, Role::Assistant);
    }
}
