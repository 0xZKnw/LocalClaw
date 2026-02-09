use std::sync::Arc;
use dashmap::DashMap;
use crate::agent::tools::ToolRegistry;
use crate::agent::skills::{Skill, SkillTool};

/// Registry for managing available skills
pub struct SkillRegistry {
    skills: DashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: DashMap::new(),
        }
    }

    /// Add a skill to the registry
    pub async fn register(&self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Get a skill by name
    pub fn get(&self, name: &str) -> Option<Skill> {
        self.skills.get(name).map(|r| r.value().clone())
    }

    /// List all skills
    pub fn list(&self) -> Vec<Skill> {
        self.skills.iter().map(|r| r.value().clone()).collect()
    }

    /// Register all skills as tools in the main ToolRegistry
    pub async fn register_as_tools(&self, tool_registry: &ToolRegistry) {
        for skill in self.skills.iter() {
            let tool = SkillTool::new(skill.value().clone());
            tool_registry.register(Arc::new(tool)).await;
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
