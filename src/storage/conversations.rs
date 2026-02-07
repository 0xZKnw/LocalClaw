//! Conversation storage
//!
//! Manages saving and loading of chat conversations.

use crate::storage::{get_data_dir, StorageError};
use crate::types::message::Message;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// A chat conversation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique identifier for the conversation
    pub id: String,
    /// Human-readable title (auto-generated from first message)
    pub title: String,
    /// List of messages in the conversation
    pub messages: Vec<Message>,
    /// When the conversation was created
    pub created_at: DateTime<Utc>,
    /// When the conversation was last updated
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    /// Create a new conversation with an optional first message
    pub fn new(first_message: Option<Message>) -> Self {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let (title, messages) = if let Some(msg) = first_message {
            let title = generate_title(&msg.content);
            (title, vec![msg])
        } else {
            ("New Conversation".to_string(), vec![])
        };

        Self {
            id,
            title,
            messages,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: Message) {
        // If this is the first message, update the title
        if self.messages.is_empty() {
            self.title = generate_title(&message.content);
        }

        self.messages.push(message);
        self.updated_at = Utc::now();
    }
}

/// Generate a conversation title from a message
///
/// Takes the first 50 characters of the message content
fn generate_title(content: &str) -> String {
    let title = content.chars().take(50).collect::<String>();
    if content.len() > 50 {
        format!("{}...", title)
    } else {
        title
    }
}

/// Get the conversations directory
fn get_conversations_dir() -> Result<PathBuf, StorageError> {
    Ok(get_data_dir()?.join("conversations"))
}

/// Get the file path for a conversation
fn get_conversation_path(id: &str) -> Result<PathBuf, StorageError> {
    Ok(get_conversations_dir()?.join(format!("{}.json", id)))
}

/// Save a conversation to disk
pub fn save_conversation(conversation: &Conversation) -> Result<(), StorageError> {
    let dir = get_conversations_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = get_conversation_path(&conversation.id)?;
    let json = serde_json::to_string_pretty(conversation)?;
    fs::write(path, json)?;
    tracing::info!("Saved conversation: {}", conversation.id);
    Ok(())
}

/// Load a conversation from disk
pub fn load_conversation(id: &str) -> Result<Conversation, StorageError> {
    let path = get_conversation_path(id)?;

    if !path.exists() {
        return Err(StorageError::ConversationNotFound(id.to_string()));
    }

    let json = fs::read_to_string(&path)?;
    let conversation: Conversation = serde_json::from_str(&json)?;
    tracing::debug!("Loaded conversation: {}", id);
    Ok(conversation)
}

/// List all conversations
///
/// Returns a list of conversations sorted by updated_at (most recent first)
pub fn list_conversations() -> Result<Vec<Conversation>, StorageError> {
    let conversations_dir = get_conversations_dir()?;

    if !conversations_dir.exists() {
        return Ok(vec![]);
    }

    let mut conversations = vec![];

    for entry in fs::read_dir(conversations_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            match fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<Conversation>(&json) {
                    Ok(conv) => conversations.push(conv),
                    Err(e) => {
                        tracing::warn!("Failed to parse conversation file {:?}: {}", path, e);
                        continue;
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read conversation file {:?}: {}", path, e);
                    continue;
                }
            }
        }
    }

    // Sort by updated_at, most recent first
    conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(conversations)
}

/// Delete a conversation
pub fn delete_conversation(id: &str) -> Result<(), StorageError> {
    let path = get_conversation_path(id)?;

    if !path.exists() {
        return Err(StorageError::ConversationNotFound(id.to_string()));
    }

    fs::remove_file(path)?;
    tracing::debug!("Deleted conversation: {}", id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::Role;

    #[test]
    fn test_conversation_creation() {
        let msg = Message::new(Role::User, "Hello, world!");
        let conv = Conversation::new(Some(msg));

        assert!(!conv.id.is_empty());
        assert_eq!(conv.title, "Hello, world!");
        assert_eq!(conv.messages.len(), 1);
        assert!(conv.created_at <= Utc::now());
    }

    #[test]
    fn test_title_generation() {
        let long_message = "a".repeat(100);
        let title = generate_title(&long_message);
        assert_eq!(title.len(), 53); // 50 chars + "..."
        assert!(title.ends_with("..."));

        let short_message = "Short";
        let title = generate_title(short_message);
        assert_eq!(title, "Short");
    }

    #[test]
    fn test_add_message() {
        let mut conv = Conversation::new(None);
        assert_eq!(conv.title, "New Conversation");

        let msg = Message::new(Role::User, "First message");
        conv.add_message(msg);

        assert_eq!(conv.title, "First message");
        assert_eq!(conv.messages.len(), 1);
    }

    #[test]
    fn test_conversation_round_trip() {
        // This test requires actual file system, so we use tempfile
        // However, since our functions use get_data_dir(), we need to test at a higher level
        // For now, test serialization/deserialization

        let msg = Message::new(Role::User, "Test message");
        let conv = Conversation::new(Some(msg));

        let json = serde_json::to_string(&conv).unwrap();
        let deserialized: Conversation = serde_json::from_str(&json).unwrap();

        assert_eq!(conv.id, deserialized.id);
        assert_eq!(conv.title, deserialized.title);
        assert_eq!(conv.messages.len(), deserialized.messages.len());
    }
}
