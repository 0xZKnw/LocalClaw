use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use regex::Regex;
use crate::agent::tools::{Tool, ToolResult, ToolError};
use crate::storage::get_data_dir;

pub struct SkillCreateTool;

#[async_trait]
impl Tool for SkillCreateTool {
    fn name(&self) -> &str {
        "skill_create"
    }

    fn description(&self) -> &str {
        "Create a new skill for the AI. Generates a SKILL.md file with the provided content and metadata."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Skill name (alphanumeric + hyphens only)"
                },
                "description": {
                    "type": "string",
                    "description": "Short description of what the skill does"
                },
                "content": {
                    "type": "string",
                    "description": "Markdown instructions for the skill"
                },
                "is_global": {
                    "type": "boolean",
                    "description": "If true, save to global skills directory. If false, save to project .localm/skills/",
                    "default": false
                },
                "disable_auto_invoke": {
                    "type": "boolean",
                    "description": "Whether to disable automatic invocation of this skill",
                    "default": false
                },
                "allowed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of tool names allowed for this skill (optional)"
                }
            },
            "required": ["name", "description", "content"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let name = params["name"].as_str()
            .ok_or_else(|| {
                tracing::error!("skill_create: name parameter is missing");
                ToolError::InvalidParameters("name is required".to_string())
            })?;
        tracing::info!("Creating skill '{}'...", name);
        let description = params["description"].as_str()
            .ok_or_else(|| {
                tracing::error!("skill_create: description parameter is missing");
                ToolError::InvalidParameters("description is required".to_string())
            })?;
        let content = params["content"].as_str()
            .ok_or_else(|| {
                tracing::error!("skill_create: content parameter is missing");
                ToolError::InvalidParameters("content is required".to_string())
            })?;
        let is_global = params["is_global"].as_bool().unwrap_or(false);
        let disable_auto_invoke = params["disable_auto_invoke"].as_bool().unwrap_or(false);
        let allowed_tools = params["allowed_tools"].as_array();

        // Validate name
        let name_regex = Regex::new(r"^[a-zA-Z0-9-]+$").map_err(|e| {
            tracing::error!("skill_create: regex creation error: {}", e);
            ToolError::ExecutionFailed(format!("Regex error: {}", e))
        })?;
        if !name_regex.is_match(name) {
            tracing::error!("skill_create: invalid skill name '{}' - must be alphanumeric with hyphens only", name);
            return Err(ToolError::InvalidParameters("Skill name must be alphanumeric with hyphens only".to_string()));
        }

        // Determine directory
        let base_dir = if is_global {
            get_data_dir()
                .map_err(|e| {
                    tracing::error!("skill_create: failed to get data dir: {}", e);
                    ToolError::ExecutionFailed(format!("Failed to get data dir: {}", e))
                })?
                .join("skills")
        } else {
            PathBuf::from(".localm").join("skills")
        };

        let skill_dir = base_dir.join(name);
        
        // Create directory
        if !skill_dir.exists() {
            tokio::fs::create_dir_all(&skill_dir).await
                .map_err(|e| {
                    tracing::error!("skill_create: failed to create directory {}: {}", skill_dir.display(), e);
                    ToolError::ExecutionFailed(format!("Failed to create directory {}: {}", skill_dir.display(), e))
                })?;
            tracing::info!("Created skill directory: {}", skill_dir.display());
        }

        // Generate SKILL.md content
        let mut frontmatter = format!(
            "---\nname: {}\ndescription: {}\ndisable_auto_invoke: {}\n",
            name, description, disable_auto_invoke
        );

        if let Some(tools) = allowed_tools {
            frontmatter.push_str("allowed_tools:\n");
            for tool in tools {
                if let Some(t) = tool.as_str() {
                    frontmatter.push_str(&format!("  - {}\n", t));
                }
            }
        }

        frontmatter.push_str("---\n\n");
        frontmatter.push_str(content);

        let file_path = skill_dir.join("SKILL.md");
        
        // Write file
        tokio::fs::write(&file_path, frontmatter).await
            .map_err(|e| {
                tracing::error!("skill_create: failed to write file {}: {}", file_path.display(), e);
                ToolError::ExecutionFailed(format!("Failed to write file {}: {}", file_path.display(), e))
            })?;

        tracing::info!("Skill '{}' created successfully at {}", name, file_path.display());

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": file_path.display().to_string(),
                "name": name,
                "is_global": is_global
            }),
            message: format!("Skill '{}' created successfully at {}", name, file_path.display()),
        })
    }
}
