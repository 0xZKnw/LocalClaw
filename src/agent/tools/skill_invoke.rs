use async_trait::async_trait;
use serde_json::Value;
use crate::agent::tools::{Tool, ToolResult, ToolError};
use crate::agent::skills::loader::SkillLoader;

pub struct SkillInvokeTool;

#[async_trait]
impl Tool for SkillInvokeTool {
    fn name(&self) -> &str {
        "skill_invoke"
    }

    fn description(&self) -> &str {
        "Invoke a specific skill by name. The skill's instructions will be added to the context."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name of the skill to invoke (e.g., 'playwright', 'git-master')"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let name = params["name"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("name is required".to_string()))?;

        // Normalize name: lowercase and trim
        let target_name = name.trim().to_lowercase();
        
        let skills = SkillLoader::load_all().await;
        
        // Find skill
        // We check for:
        // 1. Exact match (e.g. "skill_git_master" == "skill_git_master")
        // 2. Match without "skill_" prefix (e.g. "git_master" == "git_master")
        // 3. Match with hyphens/underscores handling if needed (the loader replaces - with _)
        
        let skill = skills.iter().find(|s| {
            let s_name = s.name.to_lowercase();
            let s_simple_name = s_name.trim_start_matches("skill_");
            
            s_name == target_name || 
            s_simple_name == target_name ||
            s_name == format!("skill_{}", target_name)
        });

        match skill {
            Some(s) => {
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({
                        "name": s.name,
                        "description": s.description,
                        "content": s.content,
                        "path": s.path,
                        "allowed_tools": s.allowed_tools
                    }),
                    message: format!("Skill '{}' invoked successfully.", s.name),
                })
            },
            None => {
                Err(ToolError::ExecutionFailed(format!("Skill '{}' not found. Use skill_list to see available skills.", name)))
            }
        }
    }
}
