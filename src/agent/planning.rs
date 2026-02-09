//! Planning and TODO management system
//!
//! Provides structured task planning and tracking for the agent.
//! Inspired by Claude Code's TODO system for managing complex multi-step tasks.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Status of a task in the plan
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Not yet started
    Pending,
    /// Currently being worked on
    InProgress,
    /// Successfully completed
    Completed,
    /// Failed to complete
    Failed,
    /// Skipped (not needed)
    Skipped,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Priority level for tasks
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Medium
    }
}

/// A single task in the plan
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    /// Unique task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Current status
    pub status: TaskStatus,
    /// Priority level
    pub priority: TaskPriority,
    /// Parent task ID (for subtasks)
    pub parent_id: Option<String>,
    /// Dependencies (task IDs that must complete first)
    pub dependencies: Vec<String>,
    /// Tool to use (if known)
    pub tool: Option<String>,
    /// Additional notes
    pub notes: Option<String>,
    /// Result/output when completed
    pub result: Option<String>,
}

impl Task {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            description: description.into(),
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            parent_id: None,
            dependencies: Vec::new(),
            tool: None,
            notes: None,
            result: None,
        }
    }
    
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
    
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }
    
    pub fn with_dependency(mut self, dep_id: impl Into<String>) -> Self {
        self.dependencies.push(dep_id.into());
        self
    }
    
    /// Check if task can be started (all dependencies completed)
    pub fn can_start(&self, completed_ids: &[String]) -> bool {
        self.dependencies.iter().all(|dep| completed_ids.contains(dep))
    }
}

/// A complete task plan
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Plan ID
    pub id: String,
    /// Plan title/goal
    pub goal: String,
    /// All tasks in the plan
    pub tasks: Vec<Task>,
    /// When the plan was created
    pub created_at: u64,
    /// Last update time
    pub updated_at: u64,
}

impl TaskPlan {
    pub fn new(goal: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
            
        Self {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            goal: goal.into(),
            tasks: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Add a task to the plan
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
        self.touch();
    }
    
    /// Get task by ID
    pub fn get_task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
    
    /// Get mutable task by ID
    pub fn get_task_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }
    
    /// Update task status
    pub fn update_status(&mut self, id: &str, status: TaskStatus) {
        if let Some(task) = self.get_task_mut(id) {
            task.status = status;
            self.touch();
        }
    }
    
    /// Set task result
    pub fn set_result(&mut self, id: &str, result: impl Into<String>) {
        if let Some(task) = self.get_task_mut(id) {
            task.result = Some(result.into());
            self.touch();
        }
    }
    
    /// Get next task to work on
    pub fn next_task(&self) -> Option<&Task> {
        let completed_ids: Vec<String> = self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();
        
        // First, check for in-progress tasks
        if let Some(task) = self.tasks.iter().find(|t| t.status == TaskStatus::InProgress) {
            return Some(task);
        }
        
        // Then find next pending task that can start
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending && t.can_start(&completed_ids))
            .min_by_key(|t| match t.priority {
                TaskPriority::Critical => 0,
                TaskPriority::High => 1,
                TaskPriority::Medium => 2,
                TaskPriority::Low => 3,
            })
    }
    
    /// Get all completed tasks
    pub fn completed_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Completed).collect()
    }
    
    /// Get all pending tasks
    pub fn pending_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Pending).collect()
    }
    
    /// Check if plan is complete
    pub fn is_complete(&self) -> bool {
        self.tasks.iter().all(|t| {
            matches!(t.status, TaskStatus::Completed | TaskStatus::Skipped)
        })
    }
    
    /// Get progress percentage
    pub fn progress(&self) -> f32 {
        if self.tasks.is_empty() {
            return 0.0;
        }
        let completed = self.tasks.iter()
            .filter(|t| matches!(t.status, TaskStatus::Completed | TaskStatus::Skipped))
            .count();
        (completed as f32 / self.tasks.len() as f32) * 100.0
    }
    
    /// Update timestamp
    fn touch(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
    
    /// Generate markdown summary
    pub fn to_markdown(&self) -> String {
        let mut md = format!("## Plan: {}\n\n", self.goal);
        md.push_str(&format!("Progression: {:.0}%\n\n", self.progress()));
        
        for task in &self.tasks {
            let icon = match task.status {
                TaskStatus::Pending => "⏳",
                TaskStatus::InProgress => "🔄",
                TaskStatus::Completed => "✅",
                TaskStatus::Failed => "❌",
                TaskStatus::Skipped => "⏭️",
            };
            
            let priority = match task.priority {
                TaskPriority::Critical => " [CRITICAL]",
                TaskPriority::High => " [HIGH]",
                TaskPriority::Medium => "",
                TaskPriority::Low => " [low]",
            };
            
            md.push_str(&format!("{} {}{}\n", icon, task.description, priority));
            
            if let Some(ref tool) = task.tool {
                md.push_str(&format!("   Outil: {}\n", tool));
            }
            if let Some(ref result) = task.result {
                let truncated = if result.len() > 100 {
                    format!("{}...", crate::truncate_str(result, 100))
                } else {
                    result.clone()
                };
                md.push_str(&format!("   Résultat: {}\n", truncated));
            }
        }
        
        md
    }
}

/// Manager for task plans
pub struct PlanManager {
    /// Current active plan
    current_plan: Option<TaskPlan>,
    /// History of completed plans
    history: Vec<TaskPlan>,
}

impl PlanManager {
    pub fn new() -> Self {
        Self {
            current_plan: None,
            history: Vec::new(),
        }
    }
    
    /// Create a new plan
    pub fn create_plan(&mut self, goal: impl Into<String>) -> &mut TaskPlan {
        // Archive current plan if exists
        if let Some(plan) = self.current_plan.take() {
            self.history.push(plan);
        }
        
        self.current_plan = Some(TaskPlan::new(goal));
        self.current_plan.as_mut().unwrap()
    }
    
    /// Get current plan
    pub fn current(&self) -> Option<&TaskPlan> {
        self.current_plan.as_ref()
    }
    
    /// Get mutable current plan
    pub fn current_mut(&mut self) -> Option<&mut TaskPlan> {
        self.current_plan.as_mut()
    }
    
    /// Parse plan from LLM response
    pub fn parse_plan_from_response(&mut self, response: &str) -> Option<&TaskPlan> {
        // Try to extract JSON plan
        if let Some(plan) = extract_json_plan(response) {
            self.current_plan = Some(plan);
            return self.current_plan.as_ref();
        }
        
        // Try to extract markdown plan
        if let Some(plan) = extract_markdown_plan(response) {
            self.current_plan = Some(plan);
            return self.current_plan.as_ref();
        }
        
        None
    }
    
    /// Update plan from TodoWrite-style JSON
    pub fn update_from_todos(&mut self, todos: &Value) -> bool {
        let todos = match todos.as_array() {
            Some(arr) => arr,
            None => return false,
        };
        
        let plan = self.current_plan.get_or_insert_with(|| TaskPlan::new("Plan"));
        
        for todo in todos {
            let id = todo.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let content = todo.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
            
            if id.is_empty() || content.is_empty() {
                continue;
            }
            
            let task_status = match status {
                "pending" => TaskStatus::Pending,
                "in_progress" => TaskStatus::InProgress,
                "completed" => TaskStatus::Completed,
                "cancelled" | "skipped" => TaskStatus::Skipped,
                _ => TaskStatus::Pending,
            };
            
            // Update existing or add new
            if let Some(task) = plan.get_task_mut(id) {
                task.status = task_status;
                task.description = content.to_string();
            } else {
                let mut task = Task::new(content);
                task.id = id.to_string();
                task.status = task_status;
                plan.add_task(task);
            }
        }
        
        true
    }
}

impl Default for PlanManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract JSON plan from response
fn extract_json_plan(response: &str) -> Option<TaskPlan> {
    // Find JSON object with plan/tasks
    let start = response.find('{').or_else(|| response.find('['))?;
    let json_str = &response[start..];
    
    // Try to parse as plan
    if let Ok(value) = serde_json::from_str::<Value>(json_str) {
        // Check for todos array
        if let Some(todos) = value.get("todos").or(value.as_array().map(|_| &value)) {
            if let Some(arr) = todos.as_array() {
                let mut plan = TaskPlan::new("Plan extrait");
                for item in arr {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        let mut task = Task::new(content);
                        if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                            task.id = id.to_string();
                        }
                        if let Some(status) = item.get("status").and_then(|v| v.as_str()) {
                            task.status = match status {
                                "completed" => TaskStatus::Completed,
                                "in_progress" => TaskStatus::InProgress,
                                _ => TaskStatus::Pending,
                            };
                        }
                        plan.add_task(task);
                    }
                }
                if !plan.tasks.is_empty() {
                    return Some(plan);
                }
            }
        }
    }
    
    None
}

/// Extract markdown plan from response
fn extract_markdown_plan(response: &str) -> Option<TaskPlan> {
    let mut plan = TaskPlan::new("Plan");
    let mut task_count = 0;
    
    for line in response.lines() {
        let trimmed = line.trim();
        
        // Check for numbered items: "1. Task description"
        if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            if let Some(rest) = rest.strip_prefix('.').or(rest.strip_prefix(')')) {
                let desc = rest.trim();
                if !desc.is_empty() {
                    task_count += 1;
                    let mut task = Task::new(desc);
                    task.id = task_count.to_string();
                    plan.add_task(task);
                }
            }
        }
        // Check for checkbox items: "- [ ] Task" or "- [x] Task"
        else if let Some(rest) = trimmed.strip_prefix("- [ ]").or(trimmed.strip_prefix("* [ ]")) {
            let desc = rest.trim();
            if !desc.is_empty() {
                task_count += 1;
                let mut task = Task::new(desc);
                task.id = task_count.to_string();
                plan.add_task(task);
            }
        }
        else if let Some(rest) = trimmed.strip_prefix("- [x]").or(trimmed.strip_prefix("* [x]")) {
            let desc = rest.trim();
            if !desc.is_empty() {
                task_count += 1;
                let mut task = Task::new(desc);
                task.id = task_count.to_string();
                task.status = TaskStatus::Completed;
                plan.add_task(task);
            }
        }
        // Check for bullet items: "- Task"
        else if let Some(rest) = trimmed.strip_prefix("- ").or(trimmed.strip_prefix("* ")) {
            let desc = rest.trim();
            // Skip if it looks like a result or note, not a task
            if !desc.is_empty() && !desc.contains(':') && desc.len() < 200 {
                task_count += 1;
                let mut task = Task::new(desc);
                task.id = task_count.to_string();
                plan.add_task(task);
            }
        }
    }
    
    if plan.tasks.is_empty() {
        None
    } else {
        Some(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_task_creation() {
        let task = Task::new("Test task")
            .with_priority(TaskPriority::High)
            .with_tool("web_search");
        
        assert_eq!(task.description, "Test task");
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.tool, Some("web_search".to_string()));
        assert_eq!(task.status, TaskStatus::Pending);
    }
    
    #[test]
    fn test_plan_progress() {
        let mut plan = TaskPlan::new("Test");
        
        let mut task1 = Task::new("Task 1");
        task1.status = TaskStatus::Completed;
        plan.add_task(task1);
        
        plan.add_task(Task::new("Task 2"));
        
        assert!((plan.progress() - 50.0).abs() < 0.1);
    }
    
    #[test]
    fn test_extract_markdown_plan() {
        let response = r#"
Here's the plan:
1. Search for information
2. Analyze results
3. Generate summary
        "#;
        
        let plan = extract_markdown_plan(response).unwrap();
        assert_eq!(plan.tasks.len(), 3);
        assert_eq!(plan.tasks[0].description, "Search for information");
    }
    
    #[test]
    fn test_task_dependencies() {
        let task = Task::new("Task")
            .with_dependency("1")
            .with_dependency("2");
        
        assert!(!task.can_start(&[]));
        assert!(!task.can_start(&["1".to_string()]));
        assert!(task.can_start(&["1".to_string(), "2".to_string()]));
    }
}
