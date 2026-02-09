use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use crate::agent::tools::{Tool, ToolResult, ToolError};

pub mod loader;
pub mod registry;

pub use registry::SkillRegistry;

/// Represents a loaded skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub disable_auto_invoke: bool,
    pub allowed_tools: Vec<String>,
    pub path: PathBuf,
}

/// A tool that wraps a Skill
pub struct SkillTool {
    pub skill: Skill,
}

impl SkillTool {
    pub fn new(skill: Skill) -> Self {
        Self { skill }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        &self.skill.name
    }

    fn description(&self) -> &str {
        &self.skill.description
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "description": "This skill takes no parameters. Invoking it activates the skill's knowledge."
        })
    }

    async fn execute(&self, _params: Value) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "skill_name": self.skill.name,
                "content": self.skill.content
            }),
            message: format!("Skill '{}' active. Instructions:\n{}", self.skill.name, self.skill.content),
        })
    }
}

/// Error type for skill operations
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
    #[error("Missing frontmatter")]
    MissingFrontmatter,
}

/// Parse a skill file (SKILL.md)
pub fn parse_skill(content: &str, path: PathBuf) -> Result<Skill, SkillError> {
    // Simple manual frontmatter parser since we don't have serde_yaml
    if !content.starts_with("---") {
        return Err(SkillError::MissingFrontmatter);
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(SkillError::InvalidFrontmatter("End of frontmatter not found".to_string()));
    }

    let frontmatter_str = parts[1];
    let markdown_content = parts[2].trim().to_string();

    let mut name = String::new();
    let mut description = String::new();
    let mut disable_auto_invoke = false;
    let mut allowed_tools = Vec::new();

    for line in frontmatter_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "name" => name = value.to_string(),
                "description" => description = value.to_string(),
                "disable_auto_invoke" => disable_auto_invoke = value.parse().unwrap_or(false),
                "allowed_tools" => {
                    // Handle comma-separated list like "file_read, file_write"
                    // Also handle JSON-like array [file_read, file_write] if simple
                    let clean_value = value.trim_matches(|c| c == '[' || c == ']');
                    allowed_tools = clean_value.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {} // Ignore unknown keys
            }
        }
    }

    if name.is_empty() {
        // Fallback: use directory name or filename if name not provided
        if let Some(stem) = path.file_stem() {
            name = stem.to_string_lossy().to_string();
        } else {
            return Err(SkillError::InvalidFrontmatter("Missing 'name' field".to_string()));
        }
    }

    // Prefix with "skill_" if not present, to match requirement "skill_<name>"
    // The prompt says: 'Skill tool name format: "skill_<name>"'
    // But the YAML name might be "my-skill". 
    // We should probably sanitize the name to be a valid tool name.
    let tool_name = if name.starts_with("skill_") {
        name.clone()
    } else {
        format!("skill_{}", name.replace('-', "_"))
    };

    Ok(Skill {
        name: tool_name,
        description,
        content: markdown_content,
        disable_auto_invoke,
        allowed_tools,
        path,
    })
}
