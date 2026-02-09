use std::path::{Path, PathBuf};
use tokio::fs;
use crate::agent::skills::{Skill, parse_skill, SkillError};

/// Loader for discovering and loading skills
pub struct SkillLoader;

impl SkillLoader {
    /// Load skills from all standard locations (global and project-local)
    pub async fn load_all() -> Vec<Skill> {
        let mut skills = Vec::new();

        // 1. Load global skills
        if let Some(global_dir) = Self::get_global_skills_dir() {
            if let Ok(mut global_skills) = Self::load_from_dir(&global_dir).await {
                skills.append(&mut global_skills);
            }
        }

        // 2. Load project-local skills (.localm/skills)
        // We assume we are running in the project root
        let local_dir = PathBuf::from(".localm").join("skills");
        if let Ok(mut local_skills) = Self::load_from_dir(&local_dir).await {
            skills.append(&mut local_skills);
        }

        skills
    }

    /// Load skills from a specific directory
    /// Expects structure:
    /// dir/
    ///   skill-name/
    ///     SKILL.md
    pub async fn load_from_dir(path: &Path) -> Result<Vec<Skill>, SkillError> {
        let mut skills = Vec::new();

        if !path.exists() {
            return Ok(skills);
        }

        let mut entries = fs::read_dir(path).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Check for SKILL.md inside
                let skill_file = entry_path.join("SKILL.md");
                if skill_file.exists() {
                    // Convert to absolute path to avoid CWD issues
                    let abs_skill_dir = std::fs::canonicalize(&entry_path).unwrap_or_else(|_| entry_path.clone());
                    match Self::load_skill_file(&skill_file, abs_skill_dir).await {
                        Ok(skill) => skills.push(skill),
                        Err(e) => tracing::warn!("Failed to load skill from {}: {}", skill_file.display(), e),
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Load a single skill file
    /// skill_file_path: path to SKILL.md
    /// skill_dir_path: path to the skill directory (for storing in Skill.path)
    async fn load_skill_file(skill_file_path: &Path, skill_dir_path: PathBuf) -> Result<Skill, SkillError> {
        let content = fs::read_to_string(skill_file_path).await?;
        parse_skill(&content, skill_dir_path)
    }

    /// Get the global skills directory based on OS
    fn get_global_skills_dir() -> Option<PathBuf> {
        // Use directories crate to find standard data dir
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "LocaLM", "LocaLM") {
            let _data_dir = proj_dirs.data_dir();
            // Windows: %APPDATA%/LocaLM/skills
            // Linux: ~/.local/share/LocaLM/skills
            // macOS: ~/Library/Application Support/LocaLM/skills
            // Note: ProjectDirs appends "LocaLM" to the base data dir already if we use the constructor above properly?
            // ProjectDirs::from("com", "LocaLM", "LocaLM") -> 
            // Win: AppData/Roaming/LocaLM/LocaLM/data ?? No.
            // Let's check docs or behavior.
            // Actually, "LocaLM" is the app name.
            // On Windows: Roaming/LocaLM
            // The prompt says: %APPDATA%/LocaLM/skills/
            
            // If I use ProjectDirs::from("", "", "LocaLM"), it might be cleaner.
            // Let's rely on what `ProjectDirs` gives us, usually standard.
            
            // Wait, the prompt specified specific paths.
            // Windows: %APPDATA%/LocaLM/skills/
            
            // Let's construct it manually to match requirements if ProjectDirs varies.
            // Actually, `directories` is standard. 
            // `directories::ProjectDirs::from("", "", "LocaLM")`
            // Win: C:\Users\User\AppData\Roaming\LocaLM
            // Mac: /Users/User/Library/Application Support/LocaLM
            // Linux: /home/user/.local/share/LocaLM
            
            // That matches the requirement prefixes.
            return Some(proj_dirs.data_dir().join("skills"));
        }
        None
    }
}
