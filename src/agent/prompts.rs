//! Dynamic prompt system for the agent
//!
//! Provides context injection, system reminders, and specialized prompts
//! for different agent states and tasks.

use crate::agent::loop_runner::AgentContext;
use crate::agent::planning::TaskPlan;
use crate::agent::tools::ToolInfo;

/// Build the complete system prompt with tool instructions and context
pub fn build_agent_system_prompt(
    base_prompt: &str,
    tools: &[ToolInfo],
    ctx: Option<&AgentContext>,
    plan: Option<&TaskPlan>,
) -> String {
    let mut prompt = String::new();
    
    // Base system prompt
    if !base_prompt.trim().is_empty() {
        prompt.push_str(base_prompt);
        prompt.push_str("\n\n");
    }
    
    // Agent identity and capabilities
    prompt.push_str(AGENT_IDENTITY);
    prompt.push('\n');
    
    // Thinking instructions
    prompt.push_str(THINKING_INSTRUCTIONS);
    prompt.push('\n');
    
    // Tool instructions
    if !tools.is_empty() {
        prompt.push_str(&build_tool_instructions_advanced(tools));
        prompt.push('\n');
    }
    
    // Planning instructions
    prompt.push_str(PLANNING_INSTRUCTIONS);
    prompt.push('\n');
    
    // Context injection if available
    if let Some(context) = ctx {
        prompt.push_str(&build_context_reminder(context));
        prompt.push('\n');
    }
    
    // Current plan status
    if let Some(plan) = plan {
        prompt.push_str(&build_plan_reminder(plan));
        prompt.push('\n');
    }
    
    prompt
}

/// Agent identity prompt
const AGENT_IDENTITY: &str = r#"## Identité
Tu es un assistant IA avancé avec des capacités d'agent autonome, similaire à Claude Code ou OpenCode. Tu peux:
- Réfléchir et planifier avant d'agir
- Lire, créer, éditer, supprimer et déplacer des fichiers
- Exécuter des commandes shell complètes (bash/powershell)
- Effectuer des opérations Git (status, diff, log, commit, branch, stash)
- Rechercher dans le code et sur le web
- Récupérer le contenu de pages web et d'APIs
- Comparer des fichiers, faire du find-and-replace multi-fichiers
- Inspecter le système (processus, environnement, info système)
- Se connecter à des serveurs MCP externes (GitHub, Brave Search, bases de données, etc.)
- Itérer et améliorer tes réponses

Tu travailles de manière autonome mais tu demandes confirmation pour les actions dangereuses.
Tu privilégies l'édition de fichiers existants (file_edit) plutôt que la réécriture complète (file_write).
"#;

/// Instructions for thinking/reasoning mode
const THINKING_INSTRUCTIONS: &str = r#"## Mode Réflexion
Avant chaque action importante, prends le temps de réfléchir:

<thinking>
- Quel est l'objectif principal ?
- Quelles informations ai-je besoin ?
- Quel outil est le plus approprié ?
- Quels sont les risques potentiels ?
</thinking>

Tu peux utiliser les balises <thinking></thinking> pour montrer ton raisonnement.
Ce contenu ne sera pas montré à l'utilisateur mais t'aide à mieux raisonner.

## Gestion des erreurs
Quand un outil échoue ou qu'une action ne marche pas:
- NE T'ARRÊTE JAMAIS après une seule erreur
- Réfléchis dans un bloc <thinking> à ce qui a mal tourné
- Essaie une approche alternative (autre outil, autres paramètres, reformulation)
- Si après 2-3 tentatives rien ne fonctionne, explique le problème à l'utilisateur et propose des solutions
- Tu es un assistant PERSISTANT et DÉBROUILLARD
"#;

/// Instructions for planning
const PLANNING_INSTRUCTIONS: &str = r#"## Planification
Pour les tâches complexes, crée un plan structuré:

1. Analyse la demande et identifie les étapes nécessaires
2. Crée une liste de tâches ordonnées
3. Exécute chaque tâche une par une
4. Vérifie les résultats et ajuste si nécessaire
5. Résume les résultats à la fin

Tu peux mettre à jour ton plan avec l'outil todo_write si disponible.
"#;

/// Build advanced tool instructions with examples
pub fn build_tool_instructions_advanced(tools: &[ToolInfo]) -> String {
    if tools.is_empty() {
        return String::new();
    }
    
    let mut out = String::from(
        r#"## Outils Disponibles

Pour utiliser un outil, réponds UNIQUEMENT avec un objet JSON dans ce format:
```json
{"tool": "nom_outil", "params": {...}}
```

⚠️ IMPORTANT:
- Utilise UN SEUL outil par message
- N'ajoute PAS de texte avant ou après le JSON
- Attends le résultat avant de continuer
- Si un outil échoue, essaie une approche différente
- N'utilise JAMAIS de placeholders comme "<the content>", "<contenu>", "<résultat>" dans les paramètres des outils. Mets TOUJOURS le vrai contenu, les vraies données. Si tu dois écrire dans un fichier, écris le CONTENU REEL et COMPLET, pas un placeholder.
- Quand tu utilises file_write après un web_search, tu DOIS utiliser les données réelles obtenues du web_search dans le champ "content"

"#,
    );

    out.push_str("### Liste des outils:\n\n");

    for tool in tools {
        out.push_str(&format!("**{}**\n", tool.name));
        out.push_str(&format!("  Description: {}\n", tool.description));
        
        // Add schema info
        if let Some(props) = tool.parameters_schema.get("properties") {
            out.push_str("  Paramètres:\n");
            if let Some(obj) = props.as_object() {
                for (name, schema) in obj {
                    let type_str = schema.get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("any");
                    let desc = schema.get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    out.push_str(&format!("    - {}: {} - {}\n", name, type_str, desc));
                }
            }
        }
        
        // Add example for common tools
        if let Some(example) = get_tool_example(&tool.name) {
            out.push_str(&format!("  Exemple: {}\n", example));
        }
        
        out.push('\n');
    }

    out
}

/// Get example usage for common tools
fn get_tool_example(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        // Search tools
        "web_search" => Some(r#"{"tool": "web_search", "params": {"query": "latest AI news 2024"}}"#),
        "code_search" => Some(r#"{"tool": "code_search", "params": {"query": "React hooks tutorial"}}"#),
        // File read tools
        "file_read" => Some(r#"{"tool": "file_read", "params": {"path": "src/main.rs", "start_line": 1, "end_line": 50}}"#),
        "file_list" => Some(r#"{"tool": "file_list", "params": {"path": ".", "recursive": true, "max_depth": 2}}"#),
        "file_info" => Some(r#"{"tool": "file_info", "params": {"path": "src/main.rs"}}"#),
        "file_search" => Some(r#"{"tool": "file_search", "params": {"query": "TODO", "path": "./src", "file_pattern": "rs"}}"#),
        // File write/edit tools
        "file_write" => Some(r#"{"tool": "file_write", "params": {"path": "output.txt", "content": "Hello World"}}"#),
        "file_edit" => Some(r#"{"tool": "file_edit", "params": {"path": "src/main.rs", "old_string": "fn old_name()", "new_string": "fn new_name()"}}"#),
        "file_create" => Some(r#"{"tool": "file_create", "params": {"path": "src/new_file.rs", "content": "//! New module\n"}}"#),
        "file_delete" => Some(r#"{"tool": "file_delete", "params": {"path": "temp_file.txt"}}"#),
        "file_move" => Some(r#"{"tool": "file_move", "params": {"source": "old.rs", "destination": "new.rs"}}"#),
        "file_copy" => Some(r#"{"tool": "file_copy", "params": {"source": "template.rs", "destination": "new_module.rs"}}"#),
        "directory_create" => Some(r#"{"tool": "directory_create", "params": {"path": "src/new_module"}}"#),
        // Search tools
        "grep" => Some(r#"{"tool": "grep", "params": {"pattern": "fn main", "path": "./src"}}"#),
        "glob" => Some(r#"{"tool": "glob", "params": {"pattern": "**/*.rs"}}"#),
        // Shell tools
        "bash" => Some(r#"{"tool": "bash", "params": {"command": "cargo build 2>&1", "timeout_secs": 120}}"#),
        "bash_background" => Some(r#"{"tool": "bash_background", "params": {"command": "cargo watch -x run"}}"#),
        // Git tools
        "git_status" => Some(r#"{"tool": "git_status", "params": {}}"#),
        "git_diff" => Some(r#"{"tool": "git_diff", "params": {"staged": false}}"#),
        "git_log" => Some(r#"{"tool": "git_log", "params": {"count": 10, "oneline": true}}"#),
        "git_commit" => Some(r#"{"tool": "git_commit", "params": {"message": "feat: add new feature", "files": ["src/main.rs"]}}"#),
        "git_branch" => Some(r#"{"tool": "git_branch", "params": {"action": "list"}}"#),
        "git_stash" => Some(r#"{"tool": "git_stash", "params": {"action": "save", "message": "WIP"}}"#),
        // Web tools
        "web_fetch" => Some(r#"{"tool": "web_fetch", "params": {"url": "https://api.example.com/data"}}"#),
        "web_download" => Some(r#"{"tool": "web_download", "params": {"url": "https://example.com/file.zip", "path": "downloads/file.zip"}}"#),
        // Dev tools
        "diff" => Some(r#"{"tool": "diff", "params": {"file_a": "old.rs", "file_b": "new.rs"}}"#),
        "find_replace" => Some(r#"{"tool": "find_replace", "params": {"search": "old_name", "replace": "new_name", "path": "./src", "file_pattern": "rs"}}"#),
        "patch" => Some(r#"{"tool": "patch", "params": {"path": "src/main.rs", "patch": "-old line\n+new line"}}"#),
        "wc" => Some(r#"{"tool": "wc", "params": {"path": "src/main.rs"}}"#),
        // System tools
        "tree" => Some(r#"{"tool": "tree", "params": {"path": ".", "max_depth": 3}}"#),
        "which" => Some(r#"{"tool": "which", "params": {"command": "cargo"}}"#),
        "system_info" => Some(r#"{"tool": "system_info", "params": {}}"#),
        "process_list" => Some(r#"{"tool": "process_list", "params": {"filter": "node"}}"#),
        "environment" => Some(r#"{"tool": "environment", "params": {"name": "PATH"}}"#),
        // Thinking/planning
        "think" => Some(r#"{"tool": "think", "params": {"thought": "Je dois d'abord analyser le code..."}}"#),
        "todo_write" => Some(r#"{"tool": "todo_write", "params": {"todos": [{"id": "1", "content": "Analyser le code", "status": "in_progress"}]}}"#),
        _ => None,
    }
}

/// Build context reminder based on agent state
fn build_context_reminder(ctx: &AgentContext) -> String {
    let mut reminder = String::from("\n## Rappel de Contexte\n");
    
    // Iteration info
    reminder.push_str(&format!(
        "- Itération actuelle: {}\n",
        ctx.iteration
    ));
    
    // Time elapsed
    let elapsed = ctx.elapsed().as_secs();
    if elapsed > 30 {
        reminder.push_str(&format!(
            "- Temps écoulé: {}s (attention au temps)\n",
            elapsed
        ));
    }
    
    // Recent tool usage
    if !ctx.tool_history.is_empty() {
        reminder.push_str("- Outils récemment utilisés:\n");
        for entry in ctx.tool_history.iter().rev().take(3) {
            let status = if entry.error.is_some() { "❌" } else { "✅" };
            reminder.push_str(&format!("  {} {}\n", status, entry.tool_name));
        }
    }
    
    // Warnings
    if ctx.consecutive_errors > 0 {
        reminder.push_str(&format!(
            "\n⚠️ {} erreur(s) consécutive(s). Essaie une approche différente.\n",
            ctx.consecutive_errors
        ));
    }
    
    if ctx.is_stuck() {
        reminder.push_str("\n⚠️ ATTENTION: Tu sembles répéter les mêmes actions. Change d'approche!\n");
    }
    
    reminder
}

/// Build plan reminder
fn build_plan_reminder(plan: &TaskPlan) -> String {
    let mut reminder = String::from("\n## Plan Actuel\n");
    reminder.push_str(&format!("Objectif: {}\n", plan.goal));
    reminder.push_str(&format!("Progression: {:.0}%\n\n", plan.progress()));
    
    // Show current and next tasks
    if let Some(current) = plan.tasks.iter().find(|t| t.status == crate::agent::planning::TaskStatus::InProgress) {
        reminder.push_str(&format!("🔄 En cours: {}\n", current.description));
    }
    
    let pending: Vec<_> = plan.pending_tasks();
    if !pending.is_empty() {
        reminder.push_str("⏳ À faire:\n");
        for task in pending.iter().take(3) {
            reminder.push_str(&format!("  - {}\n", task.description));
        }
        if pending.len() > 3 {
            reminder.push_str(&format!("  ... et {} autres\n", pending.len() - 3));
        }
    }
    
    reminder
}

/// Build a focused prompt for a specific task
pub fn build_task_prompt(task_description: &str, available_tools: &[&str]) -> String {
    let prompt = format!(
        r#"## Tâche Spécifique
{}

Outils disponibles pour cette tâche: {}

Instructions:
1. Analyse la tâche
2. Choisis l'outil le plus approprié
3. Exécute avec les bons paramètres
4. Analyse le résultat
5. Conclus ou continue si nécessaire
"#,
        task_description,
        available_tools.join(", ")
    );
    
    prompt
}

/// Build a reflection prompt after tool execution
pub fn build_reflection_prompt(tool_name: &str, result: &str, was_success: bool) -> String {
    if was_success {
        format!(
            r#"## Résultat de l'outil `{}`

Le résultat est:
{}

Analyse ce résultat et décide de la prochaine étape:
1. Si tu as TOUTES les informations nécessaires → rédige ta réponse finale complète à l'utilisateur (sans JSON, en langage naturel)
2. Si tu as besoin de plus de données → utilise un autre outil avec le bon format JSON
3. Si tu dois écrire/modifier un fichier → utilise les VRAIES données obtenues ci-dessus dans le contenu du fichier (JAMAIS de placeholder)

IMPORTANT: Quand tu réponds à l'utilisateur, utilise les données CONCRÈTES du résultat ci-dessus. Ne dis pas "voici le résultat" sans inclure les informations réelles.
"#,
            tool_name, result
        )
    } else {
        format!(
            r#"## L'outil `{}` a échoué

Erreur: {}

NE T'ARRÊTE PAS. Réfléchis et choisis une nouvelle stratégie:
1. Les paramètres étaient-ils corrects ? (vérifie le chemin, la syntaxe, les noms)
2. Peux-tu utiliser un autre outil pour atteindre le même objectif ?
3. Peux-tu reformuler ta requête ?
4. Si rien ne fonctionne après 2 tentatives, explique le problème à l'utilisateur et propose des alternatives.

Choisis une approche et agis MAINTENANT.
"#,
            tool_name, result
        )
    }
}

/// Build a summary request prompt
pub fn build_summary_prompt(context: &str) -> String {
    format!(
        r#"## Demande de Résumé

Basé sur les informations suivantes:
{}

Fournis un résumé clair et concis qui répond à la question initiale de l'utilisateur.
Inclus:
- Les points clés trouvés
- Les sources utilisées (si pertinent)
- Une conclusion
"#,
        context
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_build_tool_instructions() {
        let tools = vec![
            ToolInfo {
                name: "web_search".to_string(),
                description: "Search the web".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search query"}
                    }
                }),
            }
        ];
        
        let instructions = build_tool_instructions_advanced(&tools);
        assert!(instructions.contains("web_search"));
        assert!(instructions.contains("Search the web"));
    }
}
