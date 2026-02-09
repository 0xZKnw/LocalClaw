# Configuration MCP Personnalisée

LocaLM supporte maintenant la configuration personnalisée de serveurs MCP via un fichier `mcp.json`, compatible avec Claude Desktop et Cursor.

## 📁 Emplacements

Le fichier `mcp.json` peut être placé à deux endroits :

| Type | Chemin | Description |
|------|--------|-------------|
| **Global** | `~/.localm/mcp.json` | Serveurs disponibles dans tous les projets |
| **Projet** | `./.localm/mcp.json` | Serveurs spécifiques au projet (commitable) |

### Chemins par OS

- **Windows** : `%APPDATA%\LocaLM\mcp.json`
- **macOS** : `~/Library/Application Support/LocaLM/mcp.json`
- **Linux** : `~/.local/share/LocaLM/mcp.json`

## 📝 Format mcp.json

Le format suit le standard de Claude Desktop :

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_votre_token_ici"
      }
    },
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    },
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "votre_clé_api"
      }
    }
  }
}
```

### Champs Supportés

| Champ | Type | Requis | Description |
|-------|------|--------|-------------|
| `command` | string | Pour stdio | Commande à exécuter (ex: `npx`, `uvx`, `node`) |
| `args` | array | Pour stdio | Arguments de la commande |
| `env` | object | Optionnel | Variables d'environnement |
| `url` | string | Pour HTTP | URL du serveur MCP (SSE) |

## 🎯 Exemples de Configuration

### 1. GitHub

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_votre_token"
      }
    }
  }
}
```

**Installation** : `npm install -g @modelcontextprotocol/server-github`

### 2. Filesystem

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  }
}
```

**Installation** : `npm install -g @modelcontextprotocol/server-filesystem`

### 3. PostgreSQL

```json
{
  "mcpServers": {
    "postgres": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres"],
      "env": {
        "POSTGRES_CONNECTION_STRING": "postgresql://user:pass@localhost/dbname"
      }
    }
  }
}
```

**Installation** : `npm install -g @modelcontextprotocol/server-postgres`

### 4. SQLite

```json
{
  "mcpServers": {
    "sqlite": {
      "command": "uvx",
      "args": ["mcp-server-sqlite"]
    }
  }
}
```

**Installation** : `pip install mcp-server-sqlite`

### 5. Brave Search

```json
{
  "mcpServers": {
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "votre_clé_api"
      }
    }
  }
}
```

**Installation** : `npm install -g @modelcontextprotocol/server-brave-search`

### 6. Playwright

```json
{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["-y", "@playwright/mcp@latest"]
    }
  }
}
```

**Installation** : `npm install -g @playwright/mcp`

### 7. Docker

```json
{
  "mcpServers": {
    "docker": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-docker"]
    }
  }
}
```

**Installation** : `npm install -g @modelcontextprotocol/server-docker`

### 8. Serveur HTTP (SSE)

```json
{
  "mcpServers": {
    "exa-search": {
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
```

## 🛠️ Outils de Gestion MCP

LocaLM fournit 3 outils pour gérer les serveurs MCP :

### 1. `mcp_add_server` - Ajouter un serveur

Ajoute un serveur MCP au fichier de configuration.

**Exemple d'utilisation :**
```json
{
  "id": "github",
  "name": "GitHub",
  "type": "stdio",
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-github"],
  "env": {
    "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_xxx"
  }
}
```

### 2. `mcp_list_servers` - Lister les serveurs

Affiche tous les serveurs MCP configurés (presets + personnalisés).

**Retourne :**
- Liste des serveurs avec leur ID, nom, statut (enabled/disabled)
- Configuration complète
- Source (preset, global, ou projet)

### 3. `mcp_remove_server` - Supprimer un serveur

Supprime un serveur MCP de la configuration.

**Paramètres :**
```json
{
  "id": "github"
}
```

## 🔄 Hiérarchie de Chargement

Les configurations sont chargées dans cet ordre (priorité croissante) :

1. **Presets intégrés** → Serveurs MCP prédéfinis dans LocaLM
2. **Configuration globale** → `~/.localm/mcp.json`
3. **Configuration projet** → `./.localm/mcp.json`

**Règle** : Les configurations de niveau supérieur écrasent celles de niveau inférieur avec le même ID.

Exemple : Si vous définissez un serveur "github" dans votre `mcp.json` projet, il remplacera le preset.

## 🚀 Démarrage Rapide

### Méthode 1 : Via l'IA (Recommandé)

Demandez simplement à l'IA d'ajouter un serveur :

```
Ajoute le serveur MCP GitHub avec mon token ghp_xxx
```

L'IA utilisera `mcp_add_server` pour configurer automatiquement.

### Méthode 2 : Manuellement

1. **Créez le répertoire** (si nécessaire) :
```bash
mkdir -p ~/.localm  # Global
# ou
mkdir -p .localm    # Projet
```

2. **Créez le fichier** `mcp.json` :
```bash
# Global
notepad ~/.localm/mcp.json

# Projet  
notepad .localm/mcp.json
```

3. **Ajoutez votre configuration** (voir exemples ci-dessus)

4. **Redémarrez LocaLM** pour charger les nouveaux serveurs

## 📚 Serveurs MCP Populaires

### Officiels (Model Context Protocol)

| Serveur | Description | Installation |
|---------|-------------|--------------|
| `@modelcontextprotocol/server-github` | Accès GitHub | `npm i -g @modelcontextprotocol/server-github` |
| `@modelcontextprotocol/server-filesystem` | Opérations fichiers | `npm i -g @modelcontextprotocol/server-filesystem` |
| `@modelcontextprotocol/server-postgres` | Base PostgreSQL | `npm i -g @modelcontextprotocol/server-postgres` |
| `@modelcontextprotocol/server-brave-search` | Recherche web | `npm i -g @modelcontextprotocol/server-brave-search` |
| `@modelcontextprotocol/server-puppeteer` | Automatisation navigateur | `npm i -g @modelcontextprotocol/server-puppeteer` |
| `@modelcontextprotocol/server-docker` | Gestion Docker | `npm i -g @modelcontextprotocol/server-docker` |
| `@modelcontextprotocol/server-slack` | Intégration Slack | `npm i -g @modelcontextprotocol/server-slack` |
| `@modelcontextprotocol/server-memory` | Mémoire persistante | `npm i -g @modelcontextprotocol/server-memory` |

### Communauté

| Serveur | Description | Installation |
|---------|-------------|--------------|
| `@playwright/mcp` | Tests E2E Playwright | `npm i -g @playwright/mcp` |
| `mcp-server-sqlite` | Base SQLite | `pip install mcp-server-sqlite` |
| `mcp-server-git` | Opérations Git | `pip install mcp-server-git` |
| `mcp-server-fetch` | Récupération web | `pip install mcp-server-fetch` |

## 🔧 Prérequis

- **Node.js 18+** pour les serveurs npm/npx
- **Python 3.10+** pour les serveurs Python (uvx)
- **UV** (optionnel) : `pip install uv` pour les serveurs uvx

## 🎓 Utilisation Avancée

### Variables d'Environnement

Vous pouvez référencer des variables d'environnement système :

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"
      }
    }
  }
}
```

LocaLM remacera `${VAR}` par la valeur de la variable d'environnement.

### Multiple Serveurs

Configurez autant de serveurs que nécessaire :

```json
{
  "mcpServers": {
    "github": { ... },
    "postgres": { ... },
    "slack": { ... },
    "docker": { ... }
  }
}
```

### Désactiver un Serveur

Pour désactiver temporairement un serveur sans le supprimer :

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { ... },
      "enabled": false
    }
  }
}
```

## 🐛 Dépannage

### Le serveur ne démarre pas

1. **Vérifiez l'installation** :
```bash
npx -y @modelcontextprotocol/server-github --version
```

2. **Vérifiez les logs** dans la console LocaLM

3. **Testez manuellement** :
```bash
npx -y @modelcontextprotocol/server-github
```

### Variables d'environnement manquantes

Assurez-vous que les variables sont définies avant de démarrer LocaLM :

```bash
export GITHUB_TOKEN=ghp_xxx
localm
```

Ou utilisez un fichier `.env` dans votre répertoire projet.

### Conflits d'ID

Si vous avez des conflits entre presets et configuration personnalisée :
- La configuration personnalisée (mcp.json) a toujours priorité
- Utilisez un ID différent si vous voulez garder les deux

## 📝 Notes de Compatibilité

- Le format `mcp.json` est **100% compatible** avec Claude Desktop
- Vous pouvez copier/coller votre configuration Claude Desktop directement
- Les transports supportés : **stdio** (processus) et **http/sse** (URL)

---

## 🎉 Exemple Complet

Voici un exemple de configuration complète pour un développeur full-stack :

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"
      }
    },
    "postgres-dev": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres"],
      "env": {
        "POSTGRES_CONNECTION_STRING": "postgresql://dev:dev@localhost:5432/myapp"
      }
    },
    "docker": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-docker"]
    },
    "playwright": {
      "command": "npx",
      "args": ["-y", "@playwright/mcp@latest"]
    },
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "${BRAVE_API_KEY}"
      }
    },
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  }
}
```

Avec cette configuration, vous pouvez :
- Gérer des issues GitHub
- Interroger votre base PostgreSQL
- Gérer des conteneurs Docker
- Créer des tests Playwright
- Rechercher sur le web
- Lire/écrire des fichiers

Tout ça directement depuis LocaLM ! 🚀
