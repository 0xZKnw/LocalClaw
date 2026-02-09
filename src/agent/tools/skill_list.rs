use async_trait::async_trait;
use serde_json::Value;
use crate::agent::tools::{Tool, ToolResult, ToolError};
use crate::agent::skills::loader::SkillLoader;

pub struct SkillListTool;

#[async_trait]
impl Tool for SkillListTool {
    fn name(&self) -> &str {
        "skill_list"
    }

    fn description(&self) -> &str {
        "List all available skills with their descriptions."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "description": "No parameters required"
        })
    }

    async fn execute(&self, _params: Value) -> Result<ToolResult, ToolError> {
        let skills = SkillLoader::load_all().await;
        
        let skill_infos: Vec<Value> = skills.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "description": s.description,
                "path": s.path,
                "auto_invoke": !s.disable_auto_invoke,
                "allowed_tools": s.allowed_tools
            })
        }).collect();
        
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "skills": skill_infos,
                "count": skills.len()
            }),
            message: format!("Found {} skills.", skills.len()),
        })
    }
}
