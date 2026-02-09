use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use regex::Regex;
use crate::agent::tools::{Tool, ToolResult, ToolError, ToolRegistry};
use crate::agent::skills::SkillRegistry;
use crate::storage::get_data_dir;

pub struct SkillCreateTool {
    skill_registry: Arc<SkillRegistry>,
    tool_registry: Arc<ToolRegistry>,
}

impl SkillCreateTool {
    pub fn new(skill_registry: Arc<SkillRegistry>, tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            skill_registry,
            tool_registry,
        }
    }
    
    /// Inject UTF-8 encoding fix for Python scripts on Windows
    fn inject_utf8_fix(content: &str) -> String {
        // Check if the script already has UTF-8 handling
        if content.contains("reconfigure(encoding") || content.contains("# -*- coding: utf-8 -*-") {
            return content.to_string();
        }
        
        // UTF-8 fix to inject after imports
        let utf8_fix = r#"
# UTF-8 encoding fix for Windows console
import sys
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

"#;
        
        // Find where to inject: after all import statements
        let lines: Vec<&str> = content.lines().collect();
        let mut insert_pos = 0;
        let mut found_imports = false;
        
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                found_imports = true;
                insert_pos = i + 1;
            } else if found_imports && !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Found non-import, non-comment, non-empty line after imports
                break;
            }
        }
        
        // Reconstruct with UTF-8 fix injected
        let mut result = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == insert_pos && found_imports {
                result.push_str(utf8_fix);
            }
            result.push_str(line);
            result.push('\n');
        }
        
        // If no imports found, prepend the fix
        if !found_imports {
            let mut new_result = utf8_fix.to_string();
            new_result.push_str(content);
            return new_result;
        }
        
        result
    }
}

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
                    "description": "Markdown instructions for the AI. MUST contain actionable steps. If providing 'files' (e.g. python scripts), explain here how to run them (e.g. 'Run python script.py')."
                },
                "files": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Optional map of filenames to content (e.g. {'script.py': 'print(\"hello\")'}). Create Python scripts here to handle complex logic."
                },
                "is_global": {
                    "type": "boolean",
                    "description": "If true, save to global skills directory. If false, save to project .localclaw/skills/",
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
        let files = params["files"].as_object()
            .ok_or_else(|| {
                tracing::error!("skill_create: files parameter is missing");
                ToolError::InvalidParameters("You MUST provide a 'files' object containing an executable script (e.g., 'main.py')".to_string())
            })?;

        // Enforce executable presence
        let valid_extensions = [".py", ".js", ".ts", ".sh"];
        let has_executable = files.keys().any(|k| valid_extensions.iter().any(|ext| k.ends_with(ext)));
        
        if !has_executable {
             return Err(ToolError::InvalidParameters(
                format!("Skill must contain an executable file ending in: {:?}", valid_extensions)
            ));
        }

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
            PathBuf::from(".localclaw").join("skills")
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
        
        // Write SKILL.md
        tokio::fs::write(&file_path, frontmatter).await
            .map_err(|e| {
                tracing::error!("skill_create: failed to write file {}: {}", file_path.display(), e);
                ToolError::ExecutionFailed(format!("Failed to write file {}: {}", file_path.display(), e))
            })?;

        // Write additional files
        for (filename, content_val) in files {
            if let Some(content_str) = content_val.as_str() {
                // Validate filename
                if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
                    tracing::warn!("skill_create: invalid filename '{}' skipped for security (no paths allowed)", filename);
                    continue;
                }
                
                // Inject UTF-8 fix for Python files on Windows
                let final_content = if filename.ends_with(".py") {
                    Self::inject_utf8_fix(content_str)
                } else {
                    content_str.to_string()
                };
                
                let extra_file_path = skill_dir.join(filename);
                tokio::fs::write(&extra_file_path, final_content).await
                    .map_err(|e| {
                        tracing::error!("skill_create: failed to write extra file {}: {}", extra_file_path.display(), e);
                        ToolError::ExecutionFailed(format!("Failed to write extra file {}: {}", extra_file_path.display(), e))
                    })?;
                tracing::info!("Created extra file: {}", extra_file_path.display());
            }
        }

        tracing::info!("Skill '{}' created successfully at {}", name, file_path.display());

        // Refresh skills
        self.skill_registry.load_and_register_all(&self.tool_registry).await;
        tracing::info!("Skills reloaded");

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": file_path.display().to_string(),
                "name": name,
                "is_global": is_global
            }),
            message: format!("Skill '{}' created successfully at {}. Skills reloaded.", name, file_path.display()),
        })
    }
}
