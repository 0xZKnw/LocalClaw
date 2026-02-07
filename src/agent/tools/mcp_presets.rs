//! MCP Server Presets - Pre-configured MCP servers for popular services
//!
//! Provides ready-to-use configurations for the most popular MCP servers.
//! Users just need to provide API keys and the servers will be auto-configured.

use super::mcp_client::{McpServerConfig, McpTransport};
use std::collections::HashMap;

/// Get all available MCP server presets
pub fn get_all_presets() -> Vec<McpPreset> {
    vec![
        // ============================================================
        // Official MCP Servers (from modelcontextprotocol/servers)
        // ============================================================
        McpPreset {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            description: "Accès à l'API GitHub: repos, issues, PRs, fichiers, branches, commits. Nécessite GITHUB_PERSONAL_ACCESS_TOKEN.".to_string(),
            category: McpCategory::VersionControl,
            config: McpServerConfig {
                id: "github".to_string(),
                name: "GitHub".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["GITHUB_PERSONAL_ACCESS_TOKEN".to_string()],
            install_hint: "npm install -g @modelcontextprotocol/server-github".to_string(),
        },

        McpPreset {
            id: "filesystem".to_string(),
            name: "Filesystem (MCP)".to_string(),
            description: "Serveur MCP officiel pour les opérations de fichiers avancées.".to_string(),
            category: McpCategory::FileSystem,
            config: McpServerConfig {
                id: "filesystem".to_string(),
                name: "Filesystem".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-filesystem".to_string(),
                        ".".to_string(),
                    ],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @modelcontextprotocol/server-filesystem".to_string(),
        },

        McpPreset {
            id: "git".to_string(),
            name: "Git (MCP)".to_string(),
            description: "Serveur MCP officiel pour les opérations Git: status, diff, log, commit, branch, etc.".to_string(),
            category: McpCategory::VersionControl,
            config: McpServerConfig {
                id: "git".to_string(),
                name: "Git".to_string(),
                transport: McpTransport::Stdio {
                    command: "uvx".to_string(),
                    args: vec!["mcp-server-git".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "pip install mcp-server-git".to_string(),
        },

        McpPreset {
            id: "brave-search".to_string(),
            name: "Brave Search".to_string(),
            description: "Recherche web via l'API Brave Search. Alternative gratuite. Nécessite BRAVE_API_KEY.".to_string(),
            category: McpCategory::Search,
            config: McpServerConfig {
                id: "brave_search".to_string(),
                name: "Brave Search".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-brave-search".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["BRAVE_API_KEY".to_string()],
            install_hint: "npm install -g @modelcontextprotocol/server-brave-search".to_string(),
        },

        McpPreset {
            id: "fetch".to_string(),
            name: "Fetch (MCP)".to_string(),
            description: "Récupérer le contenu de pages web et d'APIs. Convertit HTML en Markdown.".to_string(),
            category: McpCategory::Web,
            config: McpServerConfig {
                id: "fetch".to_string(),
                name: "Fetch".to_string(),
                transport: McpTransport::Stdio {
                    command: "uvx".to_string(),
                    args: vec!["mcp-server-fetch".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "pip install mcp-server-fetch".to_string(),
        },

        McpPreset {
            id: "memory".to_string(),
            name: "Memory (Knowledge Graph)".to_string(),
            description: "Mémoire persistante sous forme de graphe de connaissances. Permet de stocker et rappeler des informations entre sessions.".to_string(),
            category: McpCategory::KnowledgeMemory,
            config: McpServerConfig {
                id: "memory".to_string(),
                name: "Memory".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-memory".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @modelcontextprotocol/server-memory".to_string(),
        },

        McpPreset {
            id: "sequential-thinking".to_string(),
            name: "Sequential Thinking".to_string(),
            description: "Raisonnement séquentiel avancé pour résoudre des problèmes complexes étape par étape.".to_string(),
            category: McpCategory::DeveloperTools,
            config: McpServerConfig {
                id: "thinking".to_string(),
                name: "Sequential Thinking".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-sequential-thinking".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @modelcontextprotocol/server-sequential-thinking".to_string(),
        },

        // ============================================================
        // Database Servers
        // ============================================================
        McpPreset {
            id: "sqlite".to_string(),
            name: "SQLite".to_string(),
            description: "Accès aux bases de données SQLite: requêtes SQL, création de tables, analyse de données.".to_string(),
            category: McpCategory::Database,
            config: McpServerConfig {
                id: "sqlite".to_string(),
                name: "SQLite".to_string(),
                transport: McpTransport::Stdio {
                    command: "uvx".to_string(),
                    args: vec!["mcp-server-sqlite".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "pip install mcp-server-sqlite".to_string(),
        },

        McpPreset {
            id: "postgres".to_string(),
            name: "PostgreSQL".to_string(),
            description: "Accès aux bases de données PostgreSQL. Nécessite POSTGRES_CONNECTION_STRING.".to_string(),
            category: McpCategory::Database,
            config: McpServerConfig {
                id: "postgres".to_string(),
                name: "PostgreSQL".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-postgres".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["POSTGRES_CONNECTION_STRING".to_string()],
            install_hint: "npm install -g @modelcontextprotocol/server-postgres".to_string(),
        },

        // ============================================================
        // Browser & Automation
        // ============================================================
        McpPreset {
            id: "puppeteer".to_string(),
            name: "Puppeteer (Browser)".to_string(),
            description: "Automatisation de navigateur web: navigation, screenshots, interaction avec les pages.".to_string(),
            category: McpCategory::BrowserAutomation,
            config: McpServerConfig {
                id: "puppeteer".to_string(),
                name: "Puppeteer".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-puppeteer".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @modelcontextprotocol/server-puppeteer".to_string(),
        },

        McpPreset {
            id: "playwright".to_string(),
            name: "Playwright (Browser)".to_string(),
            description: "Automatisation de navigateur avec Playwright: navigation, tests, screenshots.".to_string(),
            category: McpCategory::BrowserAutomation,
            config: McpServerConfig {
                id: "playwright".to_string(),
                name: "Playwright".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@playwright/mcp@latest".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @playwright/mcp".to_string(),
        },

        // ============================================================
        // Cloud & DevOps
        // ============================================================
        McpPreset {
            id: "docker".to_string(),
            name: "Docker".to_string(),
            description: "Gestion de conteneurs Docker: images, conteneurs, volumes, réseaux.".to_string(),
            category: McpCategory::CloudDevOps,
            config: McpServerConfig {
                id: "docker".to_string(),
                name: "Docker".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-docker".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @modelcontextprotocol/server-docker".to_string(),
        },

        McpPreset {
            id: "kubernetes".to_string(),
            name: "Kubernetes".to_string(),
            description: "Gestion de clusters Kubernetes: pods, services, deployments, logs.".to_string(),
            category: McpCategory::CloudDevOps,
            config: McpServerConfig {
                id: "kubernetes".to_string(),
                name: "Kubernetes".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "kubernetes-mcp-server".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g kubernetes-mcp-server".to_string(),
        },

        // ============================================================
        // Communication
        // ============================================================
        McpPreset {
            id: "slack".to_string(),
            name: "Slack".to_string(),
            description: "Accès à Slack: envoyer/lire des messages, canaux, recherche. Nécessite SLACK_BOT_TOKEN.".to_string(),
            category: McpCategory::Communication,
            config: McpServerConfig {
                id: "slack".to_string(),
                name: "Slack".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-slack".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["SLACK_BOT_TOKEN".to_string()],
            install_hint: "npm install -g @modelcontextprotocol/server-slack".to_string(),
        },

        // ============================================================
        // Search & Research
        // ============================================================
        McpPreset {
            id: "exa".to_string(),
            name: "Exa Search".to_string(),
            description: "Recherche web avancée avec Exa: recherche sémantique, code, entreprises, recherche approfondie.".to_string(),
            category: McpCategory::Search,
            config: McpServerConfig {
                id: "exa".to_string(),
                name: "Exa Search".to_string(),
                transport: McpTransport::Http {
                    url: "https://mcp.exa.ai/mcp".to_string(),
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["EXA_API_KEY".to_string()],
            install_hint: "Aucune installation requise - serveur HTTP distant.".to_string(),
        },

        // ============================================================
        // Misc
        // ============================================================
        McpPreset {
            id: "everything-search".to_string(),
            name: "Everything Search".to_string(),
            description: "Recherche de fichiers ultra-rapide sur Windows avec Everything SDK.".to_string(),
            category: McpCategory::FileSystem,
            config: McpServerConfig {
                id: "everything".to_string(),
                name: "Everything Search".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "mcp-everything-search".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g mcp-everything-search (Windows uniquement, nécessite Everything)".to_string(),
        },

        McpPreset {
            id: "notionapi".to_string(),
            name: "Notion".to_string(),
            description: "Accès à Notion: pages, bases de données, blocs. Nécessite NOTION_API_KEY.".to_string(),
            category: McpCategory::KnowledgeMemory,
            config: McpServerConfig {
                id: "notion".to_string(),
                name: "Notion".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@notionhq/notion-mcp-server".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["NOTION_API_KEY".to_string()],
            install_hint: "npm install -g @notionhq/notion-mcp-server".to_string(),
        },

        McpPreset {
            id: "google-drive".to_string(),
            name: "Google Drive".to_string(),
            description: "Accès à Google Drive: lister, lire et rechercher des fichiers.".to_string(),
            category: McpCategory::CloudStorage,
            config: McpServerConfig {
                id: "gdrive".to_string(),
                name: "Google Drive".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-gdrive".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec![],
            install_hint: "npm install -g @modelcontextprotocol/server-gdrive".to_string(),
        },

        McpPreset {
            id: "sentry".to_string(),
            name: "Sentry".to_string(),
            description: "Accès aux erreurs et issues Sentry. Nécessite SENTRY_AUTH_TOKEN.".to_string(),
            category: McpCategory::Monitoring,
            config: McpServerConfig {
                id: "sentry".to_string(),
                name: "Sentry".to_string(),
                transport: McpTransport::Stdio {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@modelcontextprotocol/server-sentry".to_string()],
                },
                env: HashMap::new(),
                enabled: false,
            },
            required_env: vec!["SENTRY_AUTH_TOKEN".to_string()],
            install_hint: "npm install -g @modelcontextprotocol/server-sentry".to_string(),
        },
    ]
}

/// Categories for MCP server presets
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum McpCategory {
    VersionControl,
    FileSystem,
    Search,
    Web,
    Database,
    BrowserAutomation,
    CloudDevOps,
    Communication,
    KnowledgeMemory,
    CloudStorage,
    Monitoring,
    DeveloperTools,
}

impl McpCategory {
    pub fn label(&self) -> &'static str {
        match self {
            McpCategory::VersionControl => "Contrôle de version",
            McpCategory::FileSystem => "Système de fichiers",
            McpCategory::Search => "Recherche",
            McpCategory::Web => "Web",
            McpCategory::Database => "Base de données",
            McpCategory::BrowserAutomation => "Automatisation navigateur",
            McpCategory::CloudDevOps => "Cloud & DevOps",
            McpCategory::Communication => "Communication",
            McpCategory::KnowledgeMemory => "Mémoire & Connaissances",
            McpCategory::CloudStorage => "Stockage cloud",
            McpCategory::Monitoring => "Monitoring",
            McpCategory::DeveloperTools => "Outils développeur",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            McpCategory::VersionControl => "🔀",
            McpCategory::FileSystem => "📁",
            McpCategory::Search => "🔍",
            McpCategory::Web => "🌐",
            McpCategory::Database => "🗄️",
            McpCategory::BrowserAutomation => "🌍",
            McpCategory::CloudDevOps => "☁️",
            McpCategory::Communication => "💬",
            McpCategory::KnowledgeMemory => "🧠",
            McpCategory::CloudStorage => "📦",
            McpCategory::Monitoring => "📊",
            McpCategory::DeveloperTools => "🛠️",
        }
    }
}

/// MCP server preset with metadata
#[derive(Clone, Debug)]
pub struct McpPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: McpCategory,
    pub config: McpServerConfig,
    pub required_env: Vec<String>,
    pub install_hint: String,
}
